<script setup lang="ts">
import { ref } from "vue";
import { Banknote, SendHorizontal } from "lucide-vue-next";

import ResultCard from "../components/ResultCard.vue";
import { runApiAction } from "../composables/useApiAction";
import type { ActionResult } from "../composables/useApiAction";
import { useUi } from "../composables/useUi";
import { errorMessage } from "../utils";

const merchantNumber = ref("");
const amount = ref("");
const busy = ref(false);
const result = ref<ActionResult | null>(null);
const { notify, confirm } = useUi();

async function submit() {
  const customer = merchantNumber.value.trim();
  const amountText = amount.value.trim();
  if (!customer) {
    notify("请填写商户编号", "error");
    return;
  }
  const amountValue = Number(amountText);
  if (!amountText || !Number.isFinite(amountValue) || amountValue <= 0) {
    notify("请填写正确的结算金额", "error");
    return;
  }

  const confirmed = await confirm(
    "确认发起结算",
    `商户编号：${customer}\n结算金额：${amountValue.toFixed(2)} 元\n\n该操作会发起真实资金请求，结果由你自行承担，请确认商户编号与金额无误。`,
    "确认结算",
  );
  if (!confirmed) return;

  busy.value = true;
  result.value = null;
  try {
    result.value = await runApiAction("public.md5.settlement_apply", customer, {
      P4_amount: amountValue.toFixed(2),
    });
    notify(result.value.ok ? "结算已提交" : "结算未成功", result.value.ok ? "success" : "warning");
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
        <Banknote :size="18" />
        <div>
          <h2>发起结算</h2>
          <span>向指定商户发起结算打款</span>
        </div>
      </div>
    </header>

    <div class="action-body">
      <label class="field">
        <span>商户编号</span>
        <input
          v-model="merchantNumber"
          autocomplete="off"
          spellcheck="false"
          placeholder="请输入商户编号"
          :disabled="busy"
        />
      </label>
      <label class="field">
        <span>结算金额（元）</span>
        <input
          v-model="amount"
          inputmode="decimal"
          autocomplete="off"
          spellcheck="false"
          placeholder="请输入结算金额"
          :disabled="busy"
          @keyup.enter="submit"
        />
      </label>
    </div>

    <footer class="surface-footer">
      <button type="button" class="btn primary" :disabled="busy" @click="submit">
        <SendHorizontal :size="16" />{{ busy ? "提交中" : "提交结算" }}
      </button>
    </footer>

    <ResultCard v-if="result" :result="result" />
  </div>
</template>

<style scoped>
.action-surface {
  max-width: 760px;
}
.action-body {
  display: grid;
  gap: 14px;
  padding: 20px;
}
</style>
