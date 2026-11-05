<script setup lang="ts">
import type { Component } from "vue";
import { Banknote, KeyRound, Moon, Sun, Wallet } from "lucide-vue-next";

import { MODULE_GROUPS, MODULES } from "../modules";
import type { ModuleDefinition, ModuleKey } from "../types";

defineProps<{ active: ModuleKey; dark: boolean }>();
const emit = defineEmits<{ navigate: [key: ModuleKey]; toggleTheme: [] }>();

const icons: Record<ModuleDefinition["icon"], Component> = {
  wallet: Wallet,
  banknote: Banknote,
  certificate: KeyRound,
};
</script>

<template>
  <aside class="sidebar">
    <div class="brand" aria-label="支付提交工具">
      <span class="brand-mark">P</span>
      <span class="brand-name"><strong>支付提交工具</strong><small>PAYSUBMIT</small></span>
    </div>

    <nav class="navigation" aria-label="功能导航">
      <section v-for="group in MODULE_GROUPS" :key="group.key" class="nav-group">
        <h2>{{ group.label }}</h2>
        <button
          v-for="item in MODULES.filter((module) => module.group === group.key)"
          :key="item.key"
          type="button"
          class="nav-item"
          :class="{ active: active === item.key }"
          :title="item.label"
          @click="emit('navigate', item.key)"
        >
          <component :is="icons[item.icon]" :size="17" :stroke-width="1.8" />
          <span class="nav-label">{{ item.label }}</span>
        </button>
      </section>
    </nav>

    <div class="sidebar-footer">
      <button
        type="button"
        class="theme-control"
        :title="dark ? '切换至浅色模式' : '切换至深色模式'"
        @click="emit('toggleTheme')"
      >
        <Sun v-if="dark" :size="16" />
        <Moon v-else :size="16" />
        <span>{{ dark ? "浅色模式" : "深色模式" }}</span>
      </button>
    </div>
  </aside>
</template>
