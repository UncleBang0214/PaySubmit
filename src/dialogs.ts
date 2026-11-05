import { open } from "@tauri-apps/plugin-dialog";

export async function chooseSmPrivateKeyPath(): Promise<string | null> {
  const selected = await open({
    directory: false,
    multiple: false,
    filters: [{ name: "商户私钥", extensions: ["sm2", "pfx", "p12"] }],
  });
  return typeof selected === "string" ? selected : null;
}

export async function choosePublicCertificatePath(): Promise<string | null> {
  const selected = await open({
    directory: false,
    multiple: false,
    filters: [{ name: "公钥证书", extensions: ["cer", "crt", "pem"] }],
  });
  return typeof selected === "string" ? selected : null;
}
