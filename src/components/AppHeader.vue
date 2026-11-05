<script setup lang="ts">
import type { ModuleDefinition } from "../types";

defineProps<{
  module: ModuleDefinition;
}>();
</script>

<template>
  <header class="topbar">
    <div class="page-title">
      <h1>{{ module.label }}</h1>
      <span>{{ module.description }}</span>
    </div>
  </header>
</template>
