<script setup lang="ts">
import { ref } from "vue";
import { Search, Wallet } from "lucide-vue-next";

import ResultCard from "../components/ResultCard.vue";
import { runApiAction } from "../composables/useApiAction";
import type { ActionResult } from "../composables/useApiAction";
import { useUi } from "../composables/useUi";
import { errorMessage } from "../utils";

const merchantNumber = ref("");
const busy = ref(false);
const result = ref<ActionResult | null>(null);
const { notify } = useUi();

async function submit() {
  const customer = merchantNumber.value.trim();
  if (!customer) {
    notify("请填写商户编号", "error");
    return;
  }
  busy.value = true;
  result.value = null;
  try {
    result.value = await runApiAction("public.md5.balance_query", customer, {});
    notify(result.value.ok ? "查询成功" : "查询未成功", result.value.ok ? "success" : "warning");
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
        <Wallet :size="18" />
        <div>
          <h2>余额查询</h2>
          <span>输入商户编号，查询账户余额</span>
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
          @keyup.enter="submit"
        />
      </label>
    </div>

    <footer class="surface-footer">
      <button type="button" class="btn primary" :disabled="busy" @click="submit">
        <Search :size="16" />{{ busy ? "查询中" : "查询余额" }}
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
