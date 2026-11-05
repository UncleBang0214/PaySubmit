<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { FolderOpen, KeyRound, LoaderCircle, Save } from "lucide-vue-next";

import FilePicker from "../components/FilePicker.vue";
import { choosePublicCertificatePath, chooseSmPrivateKeyPath } from "../dialogs";
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

// 面向 C 端只使用国密密钥：商户 SM2 私钥 + 平台 SM2 公钥（均为 Base64）。
const DOMAIN: ApiDomain = "domestic";
const COMBO: ApiCombo = "sm";
const SCOPE = "global";

const keyFields: { key: CredentialField; label: string; hint: string }[] = [
  {
    key: "sm2_private_key",
    label: "商户 SM2 私钥",
    hint: "Base64 编码的私钥标量，可用下方证书读取获得",
  },
  {
    key: "sm2_public_key",
    label: "平台 SM2 公钥",
    hint: "Base64 编码的 SEC1 公钥，可用下方证书读取获得",
  },
];

interface ExtractResponse {
  private_key_base64: string | null;
  public_key_base64: string | null;
  private_key_format: string | null;
  public_key_format: string | null;
}

const { snapshot, saving, ensureSettings } = useSettings();
const { notify } = useUi();
const drafts = reactive<Partial<Record<CredentialField, string>>>({});
const busy = ref(false);
const loading = ref(true);

const showCert = ref(false);
const privatePath = ref("");
const certPath = ref("");
const password = ref("");
const extracting = ref(false);

const summary = computed(() => snapshot.value?.sm_domains?.[DOMAIN]?.combos?.[COMBO]);
const canExtract = computed(() => Boolean(privatePath.value || certPath.value));

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
    if (value !== undefined && value.trim() !== "") changes[field.key] = value.trim();
  }
  if (Object.keys(changes).length === 0) {
    notify("没有需要保存的密钥内容", "neutral");
    return;
  }
  busy.value = true;
  try {
    await saveApiChanges(DOMAIN, COMBO, SCOPE, changes, snapshot.value?.revision ?? 0);
    for (const field of keyFields) delete drafts[field.key];
    notify("国密密钥已加密保存");
  } catch (error) {
    notify(errorMessage(error), "error");
  } finally {
    busy.value = false;
  }
}

async function pickPrivate() {
  const selected = await chooseSmPrivateKeyPath();
  if (selected) privatePath.value = selected;
}

async function pickCert() {
  const selected = await choosePublicCertificatePath();
  if (selected) certPath.value = selected;
}

async function extractKeys() {
  if (!canExtract.value) {
    notify("请选择商户私钥或平台公钥证书文件", "error");
    return;
  }
  if (privatePath.value && !password.value) {
    notify("读取商户私钥需要填写私钥密码", "error");
    return;
  }
  extracting.value = true;
  try {
    const result = await rpc<ExtractResponse>("sm_cert.extract", {
      private_key_path: privatePath.value,
      public_cert_path: certPath.value,
      password: password.value,
    });
    if (result.private_key_base64) drafts.sm2_private_key = result.private_key_base64;
    if (result.public_key_base64) drafts.sm2_public_key = result.public_key_base64;
    notify("证书已读取，请点击保存密钥");
    password.value = "";
    showCert.value = false;
  } catch (error) {
    notify(errorMessage(error), "error");
  } finally {
    extracting.value = false;
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
          <span>国密密钥仅保存在本机，加密存储，不回显明文</span>
        </div>
      </div>
    </header>

    <div class="credentials-body">
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
        <textarea
          v-model="drafts[field.key]"
          rows="3"
          spellcheck="false"
          autocomplete="off"
          :disabled="busy || loading"
          :placeholder="configured(field.key) ? '留空表示不修改' : `请粘贴${field.label} Base64`"
        ></textarea>
        <span class="key-hint">{{ field.hint }}</span>
      </div>

      <section class="cert-reader">
        <button type="button" class="btn" :disabled="extracting" @click="showCert = !showCert">
          <KeyRound :size="15" />{{ showCert ? "收起证书读取" : "读取国密证书转 Base64" }}
        </button>

        <div v-if="showCert" class="cert-panel">
          <label class="field-label">商户私钥文件（SM2 / PFX / P12）</label>
          <FilePicker
            :path="privatePath"
            label="选择商户私钥"
            :disabled="extracting"
            @select="pickPrivate"
          />
          <label class="field-label">私钥密码</label>
          <input
            v-model="password"
            type="password"
            autocomplete="off"
            spellcheck="false"
            placeholder="读取商户私钥需要填写密码"
            :disabled="extracting || !privatePath"
          />
          <label class="field-label">平台公钥证书（CER / CRT / PEM）</label>
          <FilePicker
            :path="certPath"
            label="选择平台公钥证书"
            :disabled="extracting"
            @select="pickCert"
          />
          <div class="cert-actions">
            <button
              type="button"
              class="btn primary"
              :disabled="extracting || !canExtract"
              @click="extractKeys"
            >
              <LoaderCircle v-if="extracting" :size="15" class="spin" />{{
                extracting ? "读取中" : "读取并填入"
              }}
            </button>
          </div>
        </div>
      </section>
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
.key-field textarea {
  width: 100%;
  resize: vertical;
  font-family: var(--font-code);
  font-size: 12px;
}
.key-hint {
  color: var(--text-faint);
  font-size: 11.5px;
}
.cert-reader {
  display: grid;
  gap: 12px;
}
.cert-reader > .btn {
  justify-self: start;
}
.cert-panel {
  display: grid;
  gap: 8px;
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 14px;
  background: var(--surface-subtle);
}
.field-label {
  color: var(--text-muted);
  font-size: 12px;
}
.cert-panel input {
  width: 100%;
}
.cert-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 4px;
}
</style>
