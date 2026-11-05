<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { FolderOpen, KeyRound, LoaderCircle, Save } from "lucide-vue-next";

import FilePicker from "../components/FilePicker.vue";
import { choosePublicCertificatePath, chooseSmPrivateKeyPath } from "../dialogs";
import { saveApiChanges, useSettings } from "../composables/useSettings";
import type { CredentialChanges, CredentialField } from "../composables/useSettings";
import { useUi } from "../composables/useUi";
import { rpc } from "../rpc";
import { errorMessage } from "../utils";

// 余额查询走 MD5 签名，发起结算走国密 SM2 签名；两种密钥分别保存。
const MD5_SCOPE = "PublicProduct";

interface KeyFieldDef {
  key: CredentialField;
  label: string;
  hint: string;
  kind: "md5" | "sm";
}

const keyFields: KeyFieldDef[] = [
  { key: "md5_key", label: "MD5 签名密钥", hint: "余额查询接口签名使用", kind: "md5" },
  {
    key: "sm2_private_key",
    label: "商户 SM2 私钥",
    hint: "结算接口签名使用，Base64 编码的私钥标量",
    kind: "sm",
  },
  {
    key: "sm2_public_key",
    label: "平台 SM2 公钥",
    hint: "结算接口验签使用，Base64 编码的 SEC1 公钥",
    kind: "sm",
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

const summary = computed(() => snapshot.value?.sm_domains?.domestic?.combos?.sm);
const canExtract = computed(() => Boolean(privatePath.value || certPath.value));

function configured(field: KeyFieldDef): boolean {
  if (field.kind === "md5") {
    return snapshot.value?.products?.[MD5_SCOPE]?.combos?.des_md5?.[field.key] === true;
  }
  return (summary.value as Record<string, unknown> | undefined)?.[field.key] === true;
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
  const md5Changes: CredentialChanges = {};
  const smChanges: CredentialChanges = {};
  const md5Value = drafts.md5_key?.trim();
  const smPrivate = drafts.sm2_private_key?.trim();
  const smPublic = drafts.sm2_public_key?.trim();
  if (md5Value) md5Changes.md5_key = md5Value;
  if (smPrivate) smChanges.sm2_private_key = smPrivate;
  if (smPublic) smChanges.sm2_public_key = smPublic;
  if (Object.keys(md5Changes).length === 0 && Object.keys(smChanges).length === 0) {
    notify("没有需要保存的密钥内容", "neutral");
    return;
  }
  busy.value = true;
  try {
    let revision = snapshot.value?.revision ?? 0;
    if (Object.keys(md5Changes).length > 0) {
      const result = await saveApiChanges("domestic", "des_md5", MD5_SCOPE, md5Changes, revision);
      revision = result.revision;
    }
    if (Object.keys(smChanges).length > 0) {
      await saveApiChanges("domestic", "sm", "global", smChanges, revision);
    }
    delete drafts.md5_key;
    delete drafts.sm2_private_key;
    delete drafts.sm2_public_key;
    notify("本地密钥已加密保存");
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
          <span>密钥仅保存在本机，加密存储，不回显明文</span>
        </div>
      </div>
    </header>

    <div class="credentials-body">
      <div
        v-for="field in keyFields"
        :key="field.key"
        class="key-field"
        :class="{ configured: configured(field) }"
      >
        <div class="key-head">
          <span class="key-label">{{ field.label }}</span>
          <span class="key-status">{{ configured(field) ? "已配置" : "未配置" }}</span>
        </div>
        <input
          v-if="field.kind === 'md5'"
          v-model="drafts[field.key]"
          type="password"
          autocomplete="off"
          spellcheck="false"
          :disabled="busy || loading"
          :placeholder="configured(field) ? '留空表示不修改' : `请输入${field.label}`"
        />
        <textarea
          v-else
          v-model="drafts[field.key]"
          rows="3"
          spellcheck="false"
          autocomplete="off"
          :disabled="busy || loading"
          :placeholder="configured(field) ? '留空表示不修改' : `请粘贴${field.label} Base64`"
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
