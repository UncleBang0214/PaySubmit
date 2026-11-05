export type ModuleKey = "balance" | "settlement" | "credentials";
export type StatusTone = "neutral" | "success" | "error" | "warning";

export interface ModuleDefinition {
  key: ModuleKey;
  label: string;
  description: string;
  group: "workspace";
  icon: "wallet" | "banknote" | "certificate";
}
