<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import AppHeader from "./components/AppHeader.vue";
import AppSidebar from "./components/AppSidebar.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import ConsentGate from "./components/ConsentGate.vue";
import ToastRegion from "./components/ToastRegion.vue";
import { useUi } from "./composables/useUi";
import { MODULES } from "./modules";
import { rpc } from "./rpc";
import type { ModuleDefinition, ModuleKey } from "./types";
import ApiSubmitView from "./views/ApiSubmitView.vue";
import CredentialsView from "./views/CredentialsView.vue";

const active = ref<ModuleKey>("submit");
const dark = ref(localStorage.getItem("paysubmit-theme") === "dark");
const { confirmation, resolveConfirmation } = useUi();

// 启动同意书：未同意前不渲染主界面，拒绝则退出应用。
const consented = ref(false);

function onConsentAgree() {
  consented.value = true;
}

async function onConsentRefuse() {
  try {
    await rpc("app.exit");
  } catch {
    window.close();
  }
}

const views = {
  submit: ApiSubmitView,
  credentials: CredentialsView,
};

const currentModule = computed<ModuleDefinition>(
  () => MODULES.find((module) => module.key === active.value) ?? MODULES[0],
);
const currentView = computed(() => views[active.value]);

watch(
  dark,
  (value) => {
    const theme = value ? "dark" : "light";
    document.documentElement.dataset.theme = theme;
    document.documentElement.style.colorScheme = theme;
    localStorage.setItem("paysubmit-theme", theme);
    // 同步原生窗口标题栏配色（Windows 深色标题栏），否则仅网页内容变色、系统边框不变。
    if (isTauri()) {
      getCurrentWindow()
        .setTheme(theme)
        .catch(() => undefined);
    }
  },
  { immediate: true },
);
</script>

<template>
  <ConsentGate v-if="!consented" @agree="onConsentAgree" @refuse="onConsentRefuse" />
  <div v-else class="app-shell">
    <AppSidebar
      :active="active"
      :dark="dark"
      @navigate="active = $event"
      @toggle-theme="dark = !dark"
    />
    <main class="workspace">
      <AppHeader :module="currentModule" />
      <div class="page-viewport">
        <KeepAlive>
          <component :is="currentView" :key="active" />
        </KeepAlive>
      </div>
    </main>
    <ToastRegion />
    <ConfirmDialog
      v-if="confirmation"
      :title="confirmation.title"
      :message="confirmation.message"
      :confirm-label="confirmation.confirmLabel"
      @resolve="resolveConfirmation"
    />
  </div>
</template>
