import { invoke } from "@tauri-apps/api/core";

export async function rpc<T>(action: string, params: Record<string, unknown> = {}): Promise<T> {
  return invoke<T>("invoke_action", { action, params });
}
