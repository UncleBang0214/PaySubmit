export type ModuleKey = "submit" | "credentials";
export type StatusTone = "neutral" | "success" | "error" | "warning";

export interface ModuleDefinition {
  key: ModuleKey;
  label: string;
  description: string;
  group: "workspace";
  icon: "api" | "certificate";
}
