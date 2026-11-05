<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { KeyRound, LoaderCircle, Save } from "lucide-vue-next";

import SelectControl from "../components/SelectControl.vue";
import { credentialFields, loadApiCatalog, ensureSettings, saveApiChanges, useSettings } from "../composables/useSettings";
import type { ApiCatalog, ApiCombo, ApiDomain, CredentialChanges, CredentialField } from "../composables/useSettings";
import { useUi } from "../composables/useUi";
import { errorMessage } from "../utils";

const fieldLabels: Record<CredentialField, string> = {
  customer_number: "商户编号",
  md5_key: "MD5签名密钥",
  sha256_key: "SHA256签名密钥",
  triple_des_key: "3DES加密密钥",
  aes_key: "AES加密密钥",
  rsa_private_key: "商户RSA私钥",
  rsa_public_key: "平台RSA公钥",
  sm2_private_key: "商户SM2私钥",
  sm2_public_key: "平台SM2公钥",
};

const { snapshot, saving, ensureSettings: ensure } = useSettings();
const { notify } = useUi();
const catalog = ref<ApiCatalog>({ domains: [] });
const domain = ref<ApiDomain>("domestic");
const combo = ref<ApiCombo>("des_md5");
const product = ref("");
const drafts = reactive<Partial<Record<CredentialField, string>>>({});
const busy = ref(false);
const loadingCatalog = ref(true);

const domainOptions = computed(() => catalog.value.domains.map((item) => ({ value: item.id, label: item.name })));
const currentDomain = computed(() => catalog.value.domains.find((item) => item.id === domain.value));
const comboOptions = computed(() => (currentDomain.value?.combos ?? []).map((item) => ({ value: item.id, label: item.name })));
const currentCombo = computed(() => currentDomain.value?.combos.find((item) => item.id === combo.value));
const productOptions = computed(() => (currentCombo.value?.products ?? []).map((item) => ({ value: item.id, label: item.name })));
const fields = computed<CredentialField[]>(() => ["customer_number", ...(currentCombo.value?.fields ?? [])]);
const scope = computed(() => (combo.value === "sm" ? domain.value : product.value));

onMounted(async () => {
  try {
    await ensure();
    catalog.value = await loadApiCatalog();
    domain.value = catalog.value.domains[0]?.id ?? "domestic";
    combo.value = catalog.value.domains[0]?.combos[0]?.id ?? "des_md5";
    product.value = catalog.value.domains[0]?.combos[0]?.products[0]?.id ?? "";
  } catch (error) {
    notify(errorMessage(error), "error");
  } finally {
    loadingCatalog.value = false;
  }
});

function syncProduct() {
  const first = comboOptions.value[0];
  if (!comboOptions.value.some((option) => option.value === combo.value)) {
    combo.value = (first?.value as ApiCombo) ?? combo.value;
  }
  product.value = productOptions.value[0]?.value ?? "";
}

async function save() {
  const changes: CredentialChanges = {};
  for (const field of fields.value) {
    const value = drafts[field];
    if (value !== undefined && value !== "") changes[field] = value;
  }
  if (Object.keys(changes).length === 0) {
    notify("没有需要保存的密钥内容", "neutral");
    return;
  }
  if (!scope.value) {
    notify("请先选择产品", "error");
    return;
  }
  busy.value = true;
  try {
    await saveApiChanges(domain.value, combo.value, scope.value, changes, snapshot.value?.revision ?? 0);
    for (const field of fields.value) delete drafts[field];
    notify("本地密钥已加密保存");
  } catch (error) {
    notify(errorMessage(error), "error");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="tool-surface credentials-pane">
    <header class="surface-header">
      <div class="section-title"><KeyRound :size="18" /><div><h2>本地密钥</h2><span>仅保存在本机，加密存储，不回显明文</span></div></div>
    </header>

    <div class="credentials-body">
      <div class="field"><span>业务域</span><SelectControl v-model="domain" :options="domainOptions" aria-label="业务域" :disabled="busy || loadingCatalog" /></div>
      <div class="field"><span>签名组合</span><SelectControl v-model="combo" :options="comboOptions" aria-label="签名组合" :disabled="busy || loadingCatalog" /></div>
      <div class="field"><span>产品</span><SelectControl v-model="product" :options="productOptions" aria-label="产品" :disabled="busy || loadingCatalog" /></div>

      <label v-for="field in fields" :key="field" class="field">
        <span>{{ fieldLabels[field] }}</span>
        <input v-model="drafts[field]" :type="field === 'customer_number' ? 'text' : 'password'" autocomplete="off" spellcheck="false" :disabled="busy" :placeholder="field === 'customer_number' ? '留空表示不修改' : '留空表示不修改'" />
      </label>
    </div>

    <footer class="surface-footer">
      <span class="settings-status">{{ saving ? "正在保存" : "密钥留空表示不修改" }}</span>
      <button type="button" class="btn primary" :disabled="busy || loadingCatalog" @click="save">
        <LoaderCircle v-if="busy" :size="15" class="spin" /><Save v-else :size="15" />保存密钥
      </button>
    </footer>
  </div>
</template>
