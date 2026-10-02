/** Same limit as the Core (`attachment::MAX_BYTES`). */
export const MAX_ATTACHMENT_BYTES = 20 * 1024 * 1024;

/** Reads a picked file for upload; rejects with an Error whose message is a `rule.*` key. */
export function readAttachmentFile(file: File): Promise<{ base64: string; name: string }> {
  if (file.size === 0 || file.size > MAX_ATTACHMENT_BYTES) return Promise.reject(new Error("rule.attachment_size"));
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = reader.result as string;
      resolve({ base64: dataUrl.split(",")[1], name: file.name });
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}
