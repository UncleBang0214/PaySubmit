<script setup lang="ts">
import { ref } from "vue";
import { CircleCheck, CircleX } from "lucide-vue-next";

import type { ActionResult } from "../composables/useApiAction";

defineProps<{ result: ActionResult }>();

const showRaw = ref(false);
</script>

<template>
  <section class="result-card" :class="result.ok ? 'is-ok' : 'is-err'">
    <header class="result-head">
      <span class="result-badge">
        <CircleCheck v-if="result.ok" :size="18" />
        <CircleX v-else :size="18" />
      </span>
      <div class="result-title">
        <strong>{{ result.ok ? "操作成功" : "操作未成功" }}</strong>
        <span v-if="result.retCode">返回码 {{ result.retCode }}</span>
      </div>
    </header>
    <p v-if="result.message" class="result-message">{{ result.message }}</p>
    <dl v-if="result.items.length" class="result-grid">
      <template v-for="item in result.items" :key="item.label">
        <dt>{{ item.label }}</dt>
        <dd>{{ item.value }}</dd>
      </template>
    </dl>
    <p v-else-if="!result.message" class="result-empty">
      未获取到可展示的返回信息（HTTP {{ result.status }}）
    </p>

    <div v-if="result.rawBody" class="raw-toggle">
      <button type="button" class="raw-toggle-btn" @click="showRaw = !showRaw">
        {{ showRaw ? "收起原始返回" : "查看合利宝服务端原始返回" }}
      </button>
      <pre v-if="showRaw" class="raw-body">{{ result.rawBody }}</pre>
    </div>
  </section>
</template>

<style scoped>
.result-card {
  display: grid;
  gap: 12px;
  margin: 18px 20px 20px;
  border: 1px solid var(--border);
  border-left: 3px solid var(--border-strong);
  border-radius: 8px;
  padding: 16px 18px;
  background: var(--surface-subtle);
}
.result-card.is-ok {
  border-left-color: var(--success);
}
.result-card.is-err {
  border-left-color: var(--danger);
}
.result-head {
  display: flex;
  align-items: center;
  gap: 10px;
}
.result-badge {
  display: inline-grid;
  width: 30px;
  height: 30px;
  place-items: center;
  border-radius: 8px;
  background: var(--danger-soft);
  color: var(--danger);
}
.is-ok .result-badge {
  background: var(--success-soft);
  color: var(--success);
}
.result-title {
  display: flex;
  align-items: baseline;
  gap: 8px;
}
.result-title strong {
  color: var(--text);
  font-size: 14px;
}
.result-title span {
  color: var(--text-faint);
  font-size: 12px;
}
.result-message {
  margin: 0;
  color: var(--text-muted);
  font-size: 13px;
  line-height: 1.6;
}
.result-grid {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: 8px 18px;
  margin: 0;
  font-size: 13px;
}
.result-grid dt {
  color: var(--text-muted);
}
.result-grid dd {
  margin: 0;
  color: var(--text);
  font-weight: 600;
  word-break: break-all;
}
.result-empty {
  margin: 0;
  color: var(--text-faint);
  font-size: 13px;
}
.raw-toggle {
  border-top: 1px dashed var(--border);
  padding-top: 10px;
}
.raw-toggle-btn {
  border: 0;
  padding: 0;
  background: transparent;
  color: var(--text-muted);
  font-size: 12px;
  cursor: pointer;
}
.raw-toggle-btn:hover {
  color: var(--accent);
}
.raw-body {
  max-height: 240px;
  overflow: auto;
  margin: 10px 0 0;
  border-radius: 6px;
  padding: 10px;
  background: var(--surface);
  color: var(--text-muted);
  font-family: var(--font-code);
  font-size: 11.5px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
