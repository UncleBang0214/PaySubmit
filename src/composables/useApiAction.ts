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
}

// 只挑选面向普通用户的字段，避免暴露技术细节。
const FIELD_LABELS: Record<string, string> = {
  rt2_retCode: "返回码",
  rt3_retMsg: "返回说明",
  rt5_accountStatus: "账户状态",
  rt6_balance: "账户余额",
  rt7_frozenBalance: "冻结余额",
  rt8_d0Balance: "D0 余额",
  rt9_T1Balance: "T1 余额",
  rt10_currency: "币种",
  rt11_createDate: "创建日期",
  rt12_desc: "说明",
  rt5_orderId: "订单号",
  rt6_serialNumber: "流水号",
};

const SUCCESS_CODES = new Set(["0000", "000000", "SUCCESS", "0"]);

function flatten(value: unknown, out: Record<string, string>) {
  if (value === null || value === undefined) return;
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
  if (Object.keys(flat).length === 0) {
    try {
      flatten(JSON.parse(raw.response.body), flat);
    } catch {
      // 响应非 JSON，忽略；下方回退到原文提示。
    }
  }

  const retCode = flat["rt2_retCode"] ?? "";
  const message = flat["rt3_retMsg"] ?? flat["rt12_desc"] ?? "";
  const ok = raw.response.success && (retCode ? SUCCESS_CODES.has(retCode) : true);

  const items: ResultItem[] = [];
  for (const [key, label] of Object.entries(FIELD_LABELS)) {
    const value = flat[key];
    if (value !== undefined && value !== "") items.push({ label, value });
  }

  return { ok, status: raw.response.status, retCode, message, items };
}
