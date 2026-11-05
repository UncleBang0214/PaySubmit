import { rpc } from "../rpc";

interface RawExecutionResult {
  duration_ms: number;
  response: {
    status: number;
    success: boolean;
    body: string;
    parsed: unknown;
    processed: unknown;
  };
}

export interface ResultItem {
  label: string;
  value: string;
}

export interface ActionResult {
  ok: boolean;
  status: number;
  retCode: string;
  message: string;
  items: ResultItem[];
  /** 服务器原始响应正文，仅在结果卡展开查看，便于排查。 */
  rawBody: string;
}

// 余额查询：只展示用户关心的关键字段，避免暴露技术细节。
const BALANCE_FIELD_LABELS: Record<string, string> = {
  rt4_customerNumber: "商户编号",
  rt5_accountStatus: "账户状态",
  rt11_createDate: "查询时间",
  rt15_amountToBeSettled: "可结算金额",
};

// 账户状态枚举的中文含义，便于普通用户理解。
const ACCOUNT_STATUS_LABELS: Record<string, string> = {
  AVAILABLE: "正常",
  FROZEN: "冻结",
  FREEZE_DEBIT: "冻结出账",
  FREEZE_CREDIT: "冻结入账",
  CANCELLED: "已注销",
};

// 发起结算：展示商户、订单与流水信息。
const SETTLEMENT_FIELD_LABELS: Record<string, string> = {
  rt4_customerNumber: "商户编号",
  rt5_orderId: "订单号",
  rt6_serialNumber: "流水号",
};

const SUCCESS_CODES = new Set(["0000", "000000", "SUCCESS", "0"]);

function flatten(value: unknown, out: Record<string, string>) {
  if (value === null || value === undefined) return;
  if (typeof value === "string") {
    const text = value.trim();
    if (text.startsWith("{") || text.startsWith("[")) {
      try {
        flatten(JSON.parse(text), out);
        return;
      } catch {
        // 非严格 JSON，继续按表单文本解析。
      }
    }
    flattenFormUrlEncoded(text, out);
    return;
  }
  if (Array.isArray(value)) {
    for (const item of value) flatten(item, out);
    return;
  }
  if (typeof value === "object") {
    for (const [key, item] of Object.entries(value as Record<string, unknown>)) {
      if (item !== null && typeof item === "object") flatten(item, out);
      else out[key] = String(item ?? "");
    }
    return;
  }
}

/** 兼容形如 rt1_bizType=xxx&rt2_retCode=0000 的表单文本响应。 */
function flattenFormUrlEncoded(text: string, out: Record<string, string>) {
  for (const part of text.split("&")) {
    const index = part.indexOf("=");
    if (index <= 0) continue;
    let key = part.slice(0, index).trim();
    let value = part.slice(index + 1).trim();
    try {
      key = decodeURIComponent(key);
      value = decodeURIComponent(value);
    } catch {
      // 保留原始片段。
    }
    if (key) out[key] = value;
  }
}

/** 调用后端执行一次业务请求，并归一化为适合展示的结果。 */
export async function runApiAction(
  apiId: string,
  customerNumber: string,
  payload: Record<string, unknown>,
): Promise<ActionResult> {
  const raw = await rpc<RawExecutionResult>("api.execute", {
    api_id: apiId,
    customer_number: customerNumber,
    payload,
    timeout_seconds: 60,
  });

  const flat: Record<string, string> = {};
  flatten(raw.response.processed, flat);
  if (Object.keys(flat).length === 0) flatten(raw.response.parsed, flat);
  if (Object.keys(flat).length === 0) flatten(raw.response.body, flat);

  const retCode = flat["rt2_retCode"] ?? "";
  const message = flat["rt3_retMsg"] ?? flat["rt12_desc"] ?? "";
  const ok = raw.response.success && (retCode ? SUCCESS_CODES.has(retCode) : true);

  const fieldLabels = apiId.includes("balance") ? BALANCE_FIELD_LABELS : SETTLEMENT_FIELD_LABELS;
  const items: ResultItem[] = [];
  for (const [key, label] of Object.entries(fieldLabels)) {
    const rawValue = flat[key];
    if (rawValue === undefined || rawValue === "") continue;
    const value =
      key === "rt5_accountStatus" ? (ACCOUNT_STATUS_LABELS[rawValue] ?? rawValue) : rawValue;
    items.push({ label, value });
  }

  return { ok, status: raw.response.status, retCode, message, items, rawBody: raw.response.body };
}
