import type { ModuleDefinition } from "./types";

export const MODULES: ModuleDefinition[] = [
  {
    key: "modify",
    label: "变更结算信息",
    description: "修改子商户的结算卡与结算配置",
    group: "workspace",
    icon: "edit",
  },
  {
    key: "balance",
    label: "余额查询",
    description: "输入商户编号，查询账户余额",
    group: "workspace",
    icon: "wallet",
  },
  {
    key: "settlement",
    label: "发起结算",
    description: "向指定商户发起结算打款",
    group: "workspace",
    icon: "banknote",
  },
  {
    key: "credentials",
    label: "设置",
    description: "平台商编号与国密密钥，仅存本机",
    group: "workspace",
    icon: "certificate",
  },
];

export const MODULE_GROUPS = [{ key: "workspace" as const, label: "功能" }];
