//! Attachment file storage (roadmap 3.5, ADR-12): bytes live under the app
//! data folder, named by content hash (SHA-256) — same file written twice
//! (even for different patients) is stored once — and are encrypted with a
//! key derived from the database's own data key (same HKDF/XChaCha20-Poly1305
//! construction as the Recovery Key wrap, `keys::recovery`), so a stolen
//! laptop does not expose patient images any more than it exposes the database.

use std::fs;
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use crate::error::{CoreError, Result};
use crate::keys::DataKey;

const MAGIC: &[u8; 4] = b"AAT1";
const HKDF_INFO: &[u8] = b"artaveo-dental/attachment/v1";
/// 20 MiB: generous for dental X-rays, intra-oral photos and scanned documents.
pub const MAX_BYTES: usize = 20 * 1024 * 1024;

fn file_key(data_key: &DataKey) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, data_key.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(HKDF_INFO, &mut okm).expect("32 bytes is a valid HKDF length");
    okm
}

fn path_for(dir: &Path, sha256: &str) -> PathBuf {
    // Two-level fan-out so the folder never holds tens of thousands of files directly.
    dir.join(&sha256[0..2]).join(format!("{sha256}.bin"))
}

/// Encrypts and writes `bytes` if not already stored; returns the hex SHA-256
/// of the plaintext (the attachment's content address) and its size.
pub fn store(dir: &Path, data_key: &DataKey, bytes: &[u8]) -> Result<(String, i64)> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(CoreError::validation("attachment must be 1 byte - 20 MiB"));
    }
    let sha256 = hex::encode(Sha256::digest(bytes));
    let path = path_for(dir, &sha256);
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut key = file_key(data_key);
        let mut nonce = [0u8; 24];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let cipher = XChaCha20Poly1305::new((&key).into());
        key.zeroize();
        let ct = cipher
            .encrypt(XNonce::from_slice(&nonce), bytes)
            .map_err(|_| CoreError::validation("attachment encryption failed"))?;
        let mut out = Vec::with_capacity(4 + 24 + ct.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        // Write-then-rename: a crash never leaves a half-written content file.
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, out)?;
        fs::rename(&tmp, &path)?;
    }
    Ok((sha256, bytes.len() as i64))
}

/// Reads and decrypts a previously stored attachment.
pub fn read(dir: &Path, data_key: &DataKey, sha256: &str) -> Result<Vec<u8>> {
    // BR-1: the record can exist while the file does not (data restored without the images).
    let raw = fs::read(path_for(dir, sha256)).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            CoreError::api(
                artaveo_shared::ErrorCode::NotFound,
                "the image file is missing on this computer; restore it from a backup that includes images",
            )
        } else {
            CoreError::from(e)
        }
    })?;
    if raw.len() < 4 + 24 || &raw[..4] != MAGIC {
        return Err(CoreError::validation("attachment file is corrupt"));
    }
    let nonce = &raw[4..28];
    let mut key = file_key(data_key);
    let cipher = XChaCha20Poly1305::new((&key).into());
    key.zeroize();
    cipher
        .decrypt(XNonce::from_slice(nonce), &raw[28..])
        .map_err(|_| CoreError::validation("attachment is corrupt or the key is wrong"))
}

/// CT-25: checks an upload before it is stored. Only images and PDFs, and the bytes must really be
/// what the extension says (an `.exe` or a text file named `b.png` is refused). Returns the clean
/// file name (no folders, no `..`) and the mime type.
pub fn check_upload(file_name: &str, bytes: &[u8]) -> Result<(String, &'static str)> {
    use artaveo_shared::ValidationRule;
    let bad = |m: &str| CoreError::invalid("file_name", ValidationRule::AttachmentType, m.to_string());
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or("").trim().trim_start_matches('.');
    if base.is_empty() || base.chars().count() > 150 || base.chars().any(|c| c.is_control()) {
        return Err(bad("the file name is not valid"));
    }
    let ext = base.rsplit('.').next().map(|e| e.to_ascii_lowercase()).unwrap_or_default();
    let (mime, ok): (&'static str, bool) = match ext.as_str() {
        "png" => ("image/png", bytes.starts_with(&[0x89, b'P', b'N', b'G'])),
        "jpg" | "jpeg" => ("image/jpeg", bytes.starts_with(&[0xFF, 0xD8, 0xFF])),
        "gif" => ("image/gif", bytes.starts_with(b"GIF8")),
        "webp" => ("image/webp", bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP"),
        "pdf" => ("application/pdf", bytes.starts_with(b"%PDF")),
        _ => return Err(bad("only images (png, jpg, webp, gif) and PDF files can be attached")),
    };
    if !ok {
        return Err(bad("the file content does not match its type"));
    }
    Ok((base.to_string(), mime))
}

/// Longest side of a thumbnail, in pixels.
pub const THUMBNAIL_SIDE: u32 = 256;
/// Refuse to decode images with more pixels than this (a decompression bomb inside a 20 MiB file).
const MAX_PIXELS: u64 = 120_000_000;

/// A small JPEG of an image attachment (OF-018), or `None` when the bytes are not an image
/// the Core can read (PDF, scan format, damaged file) — the UI then shows an icon instead.
pub fn make_thumbnail(bytes: &[u8]) -> Option<Vec<u8>> {
    use image::{ImageFormat, ImageReader};
    let reader = ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?;
    if !matches!(
        reader.format()?,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::WebP
    ) {
        return None;
    }
    let (w, h) = reader.into_dimensions().ok()?;
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return None;
    }
    let img = ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?.decode().ok()?;
    let small = img.thumbnail(THUMBNAIL_SIDE, THUMBNAIL_SIDE).to_rgb8();
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80);
    small.write_with_encoder(enc).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([200, 30, 30, 255]));
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    #[test]
    fn thumbnails_shrink_images_and_skip_everything_else() {
        let big = png(2000, 1000);
        let thumb = make_thumbnail(&big).expect("a PNG gets a thumbnail");
        let t = image::load_from_memory(&thumb).unwrap();
        assert_eq!((t.width(), t.height()), (256, 128), "longest side 256, aspect kept");
        assert!(thumb.len() < big.len() / 4);
        assert!(make_thumbnail(b"%PDF-1.4 not an image").is_none());
        assert!(make_thumbnail(b"").is_none());
        assert!(make_thumbnail(&big[..100]).is_none(), "truncated file");
    }

    #[test]
    fn uploads_must_be_what_they_claim_to_be() {
        let png = png(4, 4);
        assert_eq!(check_upload("a.PNG", &png).unwrap(), ("a.PNG".to_string(), "image/png"));
        // CT-25: folders are stripped from the name; type and content must agree.
        assert_eq!(check_upload("../../x.png", &png).unwrap().0, "x.png");
        assert_eq!(check_upload("C:\\scans\\x.png", &png).unwrap().0, "x.png");
        assert!(check_upload("b.png", b"hello").is_err(), "text named .png");
        assert!(check_upload("run.exe", b"MZ\x90\x00").is_err(), "executables are refused");
        assert!(check_upload("noext", &png).is_err());
        assert!(check_upload("..", &png).is_err());
        assert_eq!(check_upload("r.pdf", b"%PDF-1.4 x").unwrap().1, "application/pdf");
        assert!(check_upload("r.pdf", &png).is_err());
    }

    #[test]
    fn stores_dedupes_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let key = DataKey::generate();
        let (hash1, size1) = store(dir.path(), &key, b"xray bytes").unwrap();
        let (hash2, _) = store(dir.path(), &key, b"xray bytes").unwrap();
        assert_eq!(hash1, hash2, "identical content is stored once");
        assert_eq!(size1, 10);
        let back = read(dir.path(), &key, &hash1).unwrap();
        assert_eq!(back, b"xray bytes");
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let dir = tempfile::tempdir().unwrap();
        let (hash, _) = store(dir.path(), &DataKey::generate(), b"secret").unwrap();
        assert!(read(dir.path(), &DataKey::generate(), &hash).is_err());
    }

    #[test]
    fn rejects_empty_and_oversized() {
        let dir = tempfile::tempdir().unwrap();
        let key = DataKey::generate();
        assert!(store(dir.path(), &key, b"").is_err());
        assert!(store(dir.path(), &key, &vec![0u8; MAX_BYTES + 1]).is_err());
    }
}
