/** Same limits as the Core (`api::decode_logo`): png/jpg/webp, at most 2 MiB. */
export const MAX_LOGO_BYTES = 2 * 1024 * 1024;

/** Reads a picked logo; rejects with an Error whose message is a `rule.*` key. */
export function readLogo(file: File): Promise<{ base64: string; name: string; dataUrl: string }> {
  if (!/\.(png|jpe?g|webp)$/i.test(file.name)) return Promise.reject(new Error("rule.logo_type"));
  if (file.size === 0 || file.size > MAX_LOGO_BYTES) return Promise.reject(new Error("rule.logo_size"));
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = reader.result as string;
      resolve({ dataUrl, base64: dataUrl.split(",")[1], name: file.name });
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}
