<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import {
  Braces,
  Clipboard,
  Play,
  RotateCcw,
  SendHorizontal,
  ShieldCheck,
} from "lucide-vue-next";

import SegmentedControl from "../components/SegmentedControl.vue";
import SelectControl from "../components/SelectControl.vue";
import { ensureSettings } from "../composables/useSettings";
import { useUi } from "../composables/useUi";
import { rpc } from "../rpc";
import { errorMessage } from "../utils";

interface ApiFieldDefinition {
  name: string;
  kind: "string" | "number" | "json";
  default: string;
  sign: boolean;
  encrypt: boolean;
  omit_empty: boolean;
}

interface ApiDefinition {
  id: string;
  name: string;
  function: string;
  request_type: string;
  variant: string;
  protocol: string;
  endpoint: string;
  content_type: "form" | "json";
  interface_name?: string;
  response_sign_fields?: string[];
  fields: ApiFieldDefinition[];
}

interface ApiProduct {
  id: string;
  name: string;
  interfaces: ApiDefinition[];
}

interface ApiCatalog {
  products: ApiProduct[];
}

interface VerificationResult {
  status: "passed" | "failed" | "skipped" | "error";
  message: string;
  sign_source?: string | null;
}

interface ApiExecutionResult {
  api_id: string;
  duration_ms: number;
  request: {
    method: string;
    endpoint: string;
    content_type: string;
    payload: unknown;
    envelope: unknown;
    sign_source: string;
  };
  response: {
    status: number;
    success: boolean;
    content_type: string;
    binary: boolean;
    body: string;
    parsed: unknown;
    processed: unknown;
    verification: VerificationResult;
  };
}

type PayloadMode = "fields" | "json";
type ResultMode = "response" | "raw" | "request";

const payloadModes = [
  { value: "fields", label: "字段表单" },
  { value: "json", label: "JSON" },
];
const resultModes = [
  { value: "request", label: "请求封装" },
  { value: "raw", label: "原样响应" },
  { value: "response", label: "解析响应" },
];

const catalog = ref<ApiCatalog>({ products: [] });
const selectedProductId = ref("");
const selectedApiId = ref("");
const endpoint = ref("");
const timeoutSeconds = ref(60);
const payloadMode = ref<PayloadMode>("fields");
const resultMode = ref<ResultMode>("response");
const payloadValues = reactive<Record<string, string>>({});
const payloadJson = ref("{}");
const loading = ref(true);
const busy = ref(false);
const result = ref<ApiExecutionResult | null>(null);
const { notify, confirm } = useUi();

const selectedProduct = computed(() => catalog.value.products.find((product) => product.id === selectedProductId.value));
const selectedDefinition = computed(() => selectedProduct.value?.interfaces.find((entry) => entry.id === selectedApiId.value));
const productOptions = computed(() => catalog.value.products.map((product) => ({
  value: product.id,
  label: product.name,
})));
const interfaceOptions = computed(() => (selectedProduct.value?.interfaces ?? []).map((entry) => ({
  value: entry.id,
  label: `${interfaceDisplayName(entry.name)} · ${variantLabel(entry.variant)}`,
})));
const protocolLabel = computed(() => selectedDefinition.value ? protocolName(selectedDefinition.value.protocol) : "");
const resultText = computed(() => {
  if (!result.value) return "";
  if (resultMode.value === "raw") return result.value.response.body;
  if (resultMode.value === "request") {
    return formatJson({
      method: result.value.request.method,
      endpoint: result.value.request.endpoint,
      content_type: result.value.request.content_type,
      payload: result.value.request.payload,
      envelope: result.value.request.envelope,
      sign_source: result.value.request.sign_source,
    });
  }
  const processed = result.value.response.processed;
  if (processed !== null && processed !== undefined) return formatJson(processed);
  const parsed = result.value.response.parsed;
  if (parsed !== null && parsed !== undefined) return formatJson(parsed);
  return result.value.response.body;
});

watch(selectedProductId, () => {
  selectedApiId.value = selectedProduct.value?.interfaces[0]?.id ?? "";
});

watch(selectedApiId, () => resetDefinition());

watch(payloadMode, (mode, previous) => {
  if (mode === "json") {
    payloadJson.value = formatJson({ ...payloadValues });
  } else if (previous === "json") {
    try {
      const parsed = JSON.parse(payloadJson.value) as Record<string, unknown>;
      for (const key of Object.keys(payloadValues)) delete payloadValues[key];
      for (const [key, value] of Object.entries(parsed)) {
        payloadValues[key] = typeof value === "string" ? value : JSON.stringify(value);
      }
    } catch {
      payloadMode.value = "json";
      notify("请求 JSON 格式无效，无法切换到字段表单", "error");
    }
  }
});

onMounted(loadCatalog);

async function loadCatalog() {
  loading.value = true;
  try {
    await ensureSettings();
    catalog.value = await rpc<ApiCatalog>("api.catalog");
    selectedProductId.value = catalog.value.products[0]?.id ?? "";
  } catch (error) {
    notify(errorMessage(error), "error");
  } finally {
    loading.value = false;
  }
}

function resetDefinition() {
  const definition = selectedDefinition.value;
  for (const key of Object.keys(payloadValues)) delete payloadValues[key];
  if (!definition) {
    endpoint.value = "";
    payloadJson.value = "{}";
    result.value = null;
    return;
  }
  for (const field of definition.fields) payloadValues[field.name] = field.default;
  endpoint.value = definition.endpoint;
  payloadJson.value = formatJson({ ...payloadValues });
  result.value = null;
  resultMode.value = "response";
}

function restoreDefaults() {
  resetDefinition();
  notify("已恢复当前接口默认参数", "neutral");
}

function setPayloadMode(value: string) {
  payloadMode.value = value as PayloadMode;
}

function setResultMode(value: string) {
  resultMode.value = value as ResultMode;
}

async function execute() {
  const definition = selectedDefinition.value;
  if (!definition) {
    notify("请选择接口", "error");
    return;
  }
  if (!endpoint.value.trim()) {
    notify("请填写接口地址", "error");
    return;
  }
  let payload: Record<string, unknown>;
  try {
    payload = payloadMode.value === "json"
      ? JSON.parse(payloadJson.value) as Record<string, unknown>
      : { ...payloadValues };
    if (!payload || Array.isArray(payload) || typeof payload !== "object") throw new Error("请求参数必须是 JSON 对象");
  } catch (error) {
    notify(errorMessage(error), "error");
    return;
  }

  const confirmed = await confirm(
    "确认提交真实请求",
    `该请求会直接发送到：${endpoint.value.trim()}\n\n如为交易类接口，将产生真实的资金请求。软件仅原样提交你填写的参数，不担保结果，相关后果由操作者本人承担。`,
    "确认提交",
  );
  if (!confirmed) return;

  busy.value = true;
  result.value = null;
  try {
    result.value = await rpc<ApiExecutionResult>("api.execute", {
      api_id: definition.id,
      endpoint: endpoint.value.trim(),
      payload,
      timeout_seconds: timeoutSeconds.value,
    });
    resultMode.value = "response";
    notify(result.value.response.success ? "接口请求完成" : `接口返回 HTTP ${result.value.response.status}`, result.value.response.success ? "success" : "warning");
  } catch (error) {
    notify(errorMessage(error), "error");
  } finally {
    busy.value = false;
  }
}

async function copyResult() {
  if (!resultText.value) {
    notify("暂无可复制的调试结果", "neutral");
    return;
  }
  try {
    await navigator.clipboard.writeText(resultText.value);
    notify("调试结果已复制");
  } catch (error) {
    notify(errorMessage(error), "error");
  }
}

function protocolName(protocol: string) {
  return ({
    form_md5_fields: "表单 · MD5 / 3DES",
    form_sm_fields: "表单 · SM2 / SM4",
    form_sm_sign: "表单 · SM2",
    form_sm_envelope: "表单信封 · SM2 / SM4",
    json_sm_envelope: "JSON 信封 · SM2 / SM4",
    json_rsa_aes: "JSON 信封 · RSA / AES",
    json_3des_sha256: "JSON · 3DES / SHA-256",
    json_aes_sha256: "JSON · AES / SHA-256",
  } as Record<string, string>)[protocol] ?? protocol;
}

function variantLabel(variant: string) {
  return ({ md5: "MD5", sm: "国密", rsa: "RSA", sha256: "SHA-256" } as Record<string, string>)[variant] ?? variant;
}

function interfaceDisplayName(name: string) {
  const separator = name.indexOf("-");
  return separator >= 0 ? name.slice(separator + 1).trim() : name;
}

function formatJson(value: unknown) {
  if (typeof value === "string") {
    try {
      return JSON.stringify(JSON.parse(value), null, 2);
    } catch {
      return value;
    }
  }
  return JSON.stringify(value, null, 2);
}
</script>

<template>
  <div class="api-debug-workspace">
    <section class="tool-surface api-config-pane">
      <header class="surface-header">
        <div class="section-title">
          <SendHorizontal :size="18" />
          <div><h2>接口入口</h2><span>{{ selectedProduct?.name || "加载中" }}</span></div>
        </div>
      </header>

      <div class="api-config-scroll">
        <div class="api-config-section">
          <div class="field">
            <span>产品</span>
            <SelectControl v-model="selectedProductId" :options="productOptions" aria-label="接口产品" :disabled="loading || busy" />
          </div>
          <div class="field">
            <span>接口</span>
            <SelectControl v-model="selectedApiId" :options="interfaceOptions" aria-label="接口" :disabled="loading || busy" />
          </div>
          <div v-if="selectedDefinition" class="api-protocol-meta">
            <span>{{ protocolLabel }}</span>
            <span>{{ selectedDefinition.content_type.toUpperCase() }}</span>
            <span>{{ selectedDefinition.fields.length }} 字段</span>
          </div>
          <label class="field">
            <span>接口地址</span>
            <input v-model="endpoint" autocomplete="off" spellcheck="false" :disabled="busy" />
          </label>
          <label class="field api-timeout-field">
            <span>超时（秒）</span>
            <input v-model.number="timeoutSeconds" type="number" min="1" max="300" :disabled="busy" />
          </label>
        </div>
      </div>

      <footer class="surface-footer">
        <button type="button" class="btn" :disabled="busy || !selectedDefinition" @click="restoreDefaults"><RotateCcw :size="16" />重置参数</button>
        <button type="button" class="btn primary" :disabled="busy || !selectedDefinition" @click="execute"><Play :size="16" />{{ busy ? "请求中" : "发送请求" }}</button>
      </footer>
    </section>

    <section class="tool-surface api-payload-pane">
      <header class="surface-header">
        <div class="section-title"><Braces :size="18" /><div><h2>请求参数</h2><span>{{ selectedDefinition?.request_type || "等待选择" }}</span></div></div>
        <SegmentedControl :model-value="payloadMode" :options="payloadModes" label="参数编辑模式" @update:model-value="setPayloadMode" />
      </header>
      <div v-if="payloadMode === 'fields'" class="api-fields-grid">
        <label v-for="field in selectedDefinition?.fields" :key="field.name" class="field api-payload-field">
          <span>
            <code>{{ field.name }}</code>
            <i v-if="field.sign">签名</i>
            <i v-if="field.encrypt" class="encrypted">加密</i>
          </span>
          <textarea v-if="field.kind === 'json'" v-model="payloadValues[field.name]" rows="3" spellcheck="false" :disabled="busy"></textarea>
          <input v-else v-model="payloadValues[field.name]" autocomplete="off" spellcheck="false" :disabled="busy" />
        </label>
        <div v-if="!selectedDefinition" class="api-empty">请选择接口</div>
      </div>
      <textarea v-else v-model="payloadJson" class="code-editor api-json-editor" spellcheck="false" :disabled="busy"></textarea>
    </section>

    <section class="tool-surface api-result-pane">
      <header class="surface-header">
        <div class="section-title">
          <ShieldCheck :size="18" />
          <div>
            <h2>提交结果</h2>
            <span v-if="result">HTTP {{ result.response.status }} · {{ result.duration_ms }} ms<template v-if="resultMode === 'raw'"> · {{ result.response.binary ? "二进制响应（BASE64 原文）" : (result.response.content_type || "未知类型") }} · {{ result.response.body.length }} 字符</template></span>
            <span v-else>{{ busy ? "等待响应" : "尚未请求" }}</span>
          </div>
        </div>
        <div class="header-actions">
          <SegmentedControl :model-value="resultMode" :options="resultModes" label="结果视图" @update:model-value="setResultMode" />
          <button type="button" class="icon-btn" title="复制当前结果" :disabled="!resultText" @click="copyResult"><Clipboard :size="17" /></button>
        </div>
      </header>
      <textarea class="code-editor api-result-output" :value="resultText" readonly spellcheck="false" placeholder="提交结果会显示在这里"></textarea>
    </section>
  </div>
</template>
