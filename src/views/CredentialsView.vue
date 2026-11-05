<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { FolderOpen, KeyRound, LoaderCircle, Save } from "lucide-vue-next";

import { saveApiChanges, useSettings } from "../composables/useSettings";
import type {
  ApiCombo,
  ApiDomain,
  CredentialChanges,
  CredentialField,
} from "../composables/useSettings";
import { useUi } from "../composables/useUi";
import { rpc } from "../rpc";
import { errorMessage } from "../utils";

// 面向 C 端仅保留「公共产品」一套 3DES + MD5 密钥。
const PRODUCT_SCOPE = "PublicProduct";
const DOMAIN: ApiDomain = "domestic";
const COMBO: ApiCombo = "des_md5";

const keyFields: { key: CredentialField; label: string; hint: string }[] = [
  { key: "triple_des_key", label: "3DES 密钥", hint: "用于请求字段的三重DES加密" },
  { key: "md5_key", label: "MD5 密钥", hint: "用于请求参数签名" },
];

const { snapshot, saving, ensureSettings } = useSettings();
const { notify } = useUi();
const drafts = reactive<Partial<Record<CredentialField, string>>>({});
const busy = ref(false);
const loading = ref(true);

const summary = computed(() => snapshot.value?.products?.[PRODUCT_SCOPE]?.combos?.[COMBO]);

function configured(key: CredentialField): boolean {
  const combo = summary.value;
  if (!combo) return false;
  return (combo as Record<string, unknown>)[key] === true;
}

onMounted(async () => {
  try {
    await ensureSettings();
  } catch (error) {
    notify(errorMessage(error), "error");
  } finally {
    loading.value = false;
  }
});

async function save() {
  const changes: CredentialChanges = {};
  for (const field of keyFields) {
    const value = drafts[field.key];
    if (value !== undefined && value !== "") changes[field.key] = value;
  }
  if (Object.keys(changes).length === 0) {
    notify("没有需要保存的密钥内容", "neutral");
    return;
  }
  busy.value = true;
  try {
    await saveApiChanges(DOMAIN, COMBO, PRODUCT_SCOPE, changes, snapshot.value?.revision ?? 0);
    for (const field of keyFields) delete drafts[field.key];
    notify("本地密钥已加密保存");
  } catch (error) {
    notify(errorMessage(error), "error");
  } finally {
    busy.value = false;
  }
}

async function openConfigDir() {
  try {
    const result = await rpc<{ path: string }>("app.open_config_dir");
    notify(`已打开配置目录：${result.path}`, "neutral");
  } catch (error) {
    notify(errorMessage(error), "error");
  }
}
</script>

<template>
  <div class="tool-surface credentials-surface">
    <header class="surface-header">
      <div class="section-title">
        <KeyRound :size="18" />
        <div>
          <h2>本地密钥</h2>
          <span>仅保存在本机，加密存储，不回显明文</span>
        </div>
      </div>
    </header>

    <div class="credentials-body">
      <div class="product-row">
        <span class="product-label">产品</span>
        <span class="product-value">公共产品</span>
      </div>

      <div
        v-for="field in keyFields"
        :key="field.key"
        class="key-field"
        :class="{ configured: configured(field.key) }"
      >
        <div class="key-head">
          <span class="key-label">{{ field.label }}</span>
          <span class="key-status">{{ configured(field.key) ? "已配置" : "未配置" }}</span>
        </div>
        <input
          v-model="drafts[field.key]"
          type="password"
          autocomplete="off"
          spellcheck="false"
          :disabled="busy || loading"
          :placeholder="configured(field.key) ? '留空表示不修改' : `请输入${field.label}`"
        />
        <span class="key-hint">{{ field.hint }}</span>
      </div>
    </div>

    <footer class="surface-footer">
      <span class="settings-status">{{ saving ? "正在保存" : "密钥留空表示不修改" }}</span>
      <button type="button" class="btn ghost" :disabled="busy" @click="openConfigDir">
        <FolderOpen :size="15" />打开配置目录
      </button>
      <button type="button" class="btn primary" :disabled="busy || loading" @click="save">
        <LoaderCircle v-if="busy" :size="15" class="spin" /><Save v-else :size="15" />保存密钥
      </button>
    </footer>
  </div>
</template>

<style scoped>
.credentials-surface {
  max-width: 760px;
}
.credentials-body {
  display: grid;
  gap: 16px;
  padding: 20px;
}
.product-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 12px 14px;
  background: var(--surface-subtle);
}
.product-label {
  color: var(--text-muted);
  font-size: 13px;
}
.product-value {
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
}
.key-field {
  display: grid;
  gap: 7px;
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 14px;
  background: var(--surface);
}
.key-field.configured {
  border-color: color-mix(in srgb, var(--success) 40%, var(--border));
}
.key-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.key-label {
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
}
.key-status {
  border-radius: 999px;
  padding: 1px 9px;
  background: var(--surface-hover);
  color: var(--text-faint);
  font-size: 11px;
}
.key-field.configured .key-status {
  background: var(--success-soft);
  color: var(--success);
}
.key-field input {
  width: 100%;
}
.key-hint {
  color: var(--text-faint);
  font-size: 11.5px;
}
</style>
