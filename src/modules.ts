import type { ModuleDefinition } from "./types";

export const MODULES: ModuleDefinition[] = [
  { key: "submit", label: "接口提交", description: "填写参数并提交到你自己的支付接口", group: "workspace", icon: "api" },
  { key: "credentials", label: "本地密钥", description: "本地加密保存商编与签名密钥", group: "workspace", icon: "certificate" },
];

export const MODULE_GROUPS = [{ key: "workspace" as const, label: "工作台" }];
