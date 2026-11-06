<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { FilePen, SendHorizontal } from "lucide-vue-next";

import ResultCard from "../components/ResultCard.vue";
import SelectControl from "../components/SelectControl.vue";
import { runModifySettle } from "../composables/useApiAction";
import type { ActionResult } from "../composables/useApiAction";
import { useSettings } from "../composables/useSettings";
import { useUi } from "../composables/useUi";
import { errorMessage } from "../utils";

const alterationOptions = [
  { value: "SETTLE_BANKCARD", label: "结算卡变更" },
  { value: "MERCHANT_INFO", label: "商户信息变更" },
];
const bankTypeOptions = [
  { value: "TOPRIVATE", label: "对私" },
  { value: "TOPUBLIC", label: "对公" },
];
const settleModeOptions = [
  { value: "MERCHANT", label: "商户结算" },
  { value: "PLATFORM", label: "平台结算" },
];
const settleTypeOptions = [
  { value: "SELF", label: "自主结算" },
  { value: "AUTO", label: "自动结算" },
];
const periodOptions = [
  { value: "T1", label: "T1" },
  { value: "D0", label: "D0" },
  { value: "S0", label: "S0" },
];

const form = reactive({
  merchantNo: "",
  merchantEntryAlterationType: "SETTLE_BANKCARD",
  accountName: "",
  accountNo: "",
  settleBankType: "TOPRIVATE",
  bankCode: "",
  settleMode: "MERCHANT",
  settlementMode: "SELF",
  settlementPeriod: "T1",
});

const busy = ref(false);
const result = ref<ActionResult | null>(null);
const { snapshot, ensureSettings } = useSettings();
const { notify, confirm } = useUi();

const parentNo = computed(
  () => snapshot.value?.sm_domains?.domestic?.customer_number?.trim() ?? "",
);

onMounted(() => {
  ensureSettings().catch((error) => notify(errorMessage(error), "error"));
});

async function submit() {
  if (!parentNo.value) {
    notify("请先在「设置」中配置平台商商户编号", "error");
    return;
  }
  if (!form.merchantNo.trim()) {
    notify("请填写子商户编号", "error");
    return;
  }
  for (const [value, label] of [
    [form.accountName, "结算账户名"],
    [form.accountNo, "结算账号"],
    [form.bankCode, "银行编码"],
  ] as [string, string][]) {
    if (!value.trim()) {
      notify(`请填写${label}`, "error");
      return;
    }
  }

  const confirmed = await confirm(
    "确认变更结算信息",
    `子商户编号：${form.merchantNo.trim()}\n账户名：${form.accountName.trim()}\n结算账号：${form.accountNo.trim()}\n\n该操作会变更子商户的结算信息，请确认无误。`,
    "确认变更",
  );
  if (!confirmed) return;

  busy.value = true;
  result.value = null;
  try {
    result.value = await runModifySettle({
      merchant_no: form.merchantNo.trim(),
      account_name: form.accountName.trim(),
      account_no: form.accountNo.trim(),
      settle_bank_type: form.settleBankType,
      bank_code: form.bankCode.trim(),
      merchant_entry_alteration_type: form.merchantEntryAlterationType,
      settle_mode: form.settleMode,
      settlement_mode: form.settlementMode,
      settlement_period: form.settlementPeriod,
    });
    notify(result.value.ok ? "变更已提交" : "变更未成功", result.value.ok ? "success" : "warning");
  } catch (error) {
    notify(errorMessage(error), "error");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="tool-surface action-surface">
    <header class="surface-header">
      <div class="section-title">
        <FilePen :size="18" />
        <div>
          <h2>变更结算信息</h2>
          <span>修改子商户的结算卡与结算配置</span>
        </div>
      </div>
    </header>

    <div class="action-body">
      <label class="field">
        <span>子商户编号</span>
        <input
          v-model="form.merchantNo"
          autocomplete="off"
          spellcheck="false"
          placeholder="请输入要变更的子商户编号（E 开头）"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>变更类型</span>
        <SelectControl
          v-model="form.merchantEntryAlterationType"
          :options="alterationOptions"
          aria-label="变更类型"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>结算账户名</span>
        <input
          v-model="form.accountName"
          autocomplete="off"
          spellcheck="false"
          placeholder="请输入结算账户名"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>结算账号</span>
        <input
          v-model="form.accountNo"
          autocomplete="off"
          spellcheck="false"
          placeholder="请输入结算银行卡号"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>账户类型</span>
        <SelectControl
          v-model="form.settleBankType"
          :options="bankTypeOptions"
          aria-label="账户类型"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>银行编码</span>
        <input
          v-model="form.bankCode"
          autocomplete="off"
          spellcheck="false"
          placeholder="请输入银行编码"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>结算模式</span>
        <SelectControl
          v-model="form.settleMode"
          :options="settleModeOptions"
          aria-label="结算模式"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>结算方式</span>
        <SelectControl
          v-model="form.settlementMode"
          :options="settleTypeOptions"
          aria-label="结算方式"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>结算周期</span>
        <SelectControl
          v-model="form.settlementPeriod"
          :options="periodOptions"
          aria-label="结算周期"
          :disabled="busy"
        />
      </label>
    </div>

    <footer class="surface-footer">
      <span class="settings-status">平台商编号：{{ parentNo || "未配置" }}</span>
      <button type="button" class="btn primary" :disabled="busy" @click="submit">
        <SendHorizontal :size="16" />{{ busy ? "提交中" : "提交变更" }}
      </button>
    </footer>

    <ResultCard v-if="result" :result="result" />
  </div>
</template>

<style scoped>
.action-surface {
  max-width: 820px;
}
.action-body {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
  padding: 20px;
}
@media (max-width: 720px) {
  .action-body {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
