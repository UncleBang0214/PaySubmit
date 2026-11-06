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
  { value: "SETTLE_BANKCARD", label: "变更结算卡" },
  { value: "MERCHANT_INFO", label: "变更商户信息" },
  { value: "MERCHANT_CREDENTIAL", label: "变更资质" },
  { value: "MERCHANT_INFO_SYN", label: "商户信息变更查询" },
];
const periodOptions = [
  { value: "", label: "请选择" },
  { value: "T1", label: "次工作日（T1）" },
  { value: "D1", label: "次日（D1）" },
  { value: "D0", label: "当日（D0）" },
];
const bankTypeOptions = [
  { value: "", label: "请选择" },
  { value: "TOPUBLIC", label: "对公" },
  { value: "TOPRIVATE", label: "对私" },
];
const settleModeOptions = [
  { value: "", label: "请选择" },
  { value: "NOTOPEN", label: "不开通" },
  { value: "AUTO", label: "自动结算" },
  { value: "SELF", label: "自主结算" },
];

const form = reactive({
  merchantNo: "",
  merchantEntryAlterationType: "SETTLE_BANKCARD",
  settlementPeriod: "",
  accountName: "",
  updateAccountName: "",
  accountNo: "",
  updateAccountNo: "",
  settleBankType: "",
  updateSettleBankType: "",
  bankCode: "",
  updateBankCode: "",
  settlementMode: "",
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
    [form.accountName, "原账户名称"],
    [form.accountNo, "原结算卡号"],
    [form.settleBankType, "原结算卡类型"],
    [form.bankCode, "原联行号"],
  ] as [string, string][]) {
    if (!value.trim()) {
      notify(`请填写${label}`, "error");
      return;
    }
  }

  const confirmed = await confirm(
    "确认变更结算信息",
    `子商户编号：${form.merchantNo.trim()}\n原账户名：${form.accountName.trim()}\n原结算卡号：${form.accountNo.trim()}\n\n该操作会变更子商户的结算信息，请确认无误。`,
    "确认变更",
  );
  if (!confirmed) return;

  busy.value = true;
  result.value = null;
  try {
    result.value = await runModifySettle({
      merchant_no: form.merchantNo.trim(),
      merchant_entry_alteration_type: form.merchantEntryAlterationType,
      settlement_period: form.settlementPeriod,
      account_name: form.accountName.trim(),
      update_account_name: form.updateAccountName.trim(),
      account_no: form.accountNo.trim(),
      update_account_no: form.updateAccountNo.trim(),
      settle_bank_type: form.settleBankType,
      update_settle_bank_type: form.updateSettleBankType,
      bank_code: form.bankCode.trim(),
      update_bank_code: form.updateBankCode.trim(),
      settlement_mode: form.settlementMode,
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
        <span>变更类型<span class="req">*</span></span>
        <SelectControl
          v-model="form.merchantEntryAlterationType"
          :options="alterationOptions"
          aria-label="变更类型"
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
      <label class="field">
        <span>原账户名称<span class="req">*</span></span>
        <input
          v-model="form.accountName"
          autocomplete="off"
          spellcheck="false"
          placeholder="请输入原结算账户名"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>变更后账户名称</span>
        <input
          v-model="form.updateAccountName"
          autocomplete="off"
          spellcheck="false"
          placeholder="留空表示不修改"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>原结算卡号<span class="req">*</span></span>
        <input
          v-model="form.accountNo"
          autocomplete="off"
          spellcheck="false"
          placeholder="请输入原结算银行卡号"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>变更后结算卡号</span>
        <input
          v-model="form.updateAccountNo"
          autocomplete="off"
          spellcheck="false"
          placeholder="留空表示不修改"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>原结算卡类型<span class="req">*</span></span>
        <SelectControl
          v-model="form.settleBankType"
          :options="bankTypeOptions"
          aria-label="原结算卡类型"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>变更后结算卡类型</span>
        <SelectControl
          v-model="form.updateSettleBankType"
          :options="bankTypeOptions"
          aria-label="变更后结算卡类型"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>原联行号<span class="req">*</span></span>
        <input
          v-model="form.bankCode"
          autocomplete="off"
          spellcheck="false"
          placeholder="请输入原银行联行号"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>变更后联行号</span>
        <input
          v-model="form.updateBankCode"
          autocomplete="off"
          spellcheck="false"
          placeholder="留空表示不修改"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>结算方式</span>
        <SelectControl
          v-model="form.settlementMode"
          :options="settleModeOptions"
          aria-label="结算方式"
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
  max-width: 880px;
}
.action-body {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
  padding: 20px;
}
.req {
  margin-left: 2px;
  color: var(--danger);
}
@media (max-width: 720px) {
  .action-body {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
