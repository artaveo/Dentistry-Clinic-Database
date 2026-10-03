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
    let raw = fs::read(path_for(dir, sha256))?;
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
