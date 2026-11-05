<script setup lang="ts">
import { CircleAlert, CircleCheck, Info } from "lucide-vue-next";

import { useUi } from "../composables/useUi";

const { toasts } = useUi();
</script>

<template>
  <div class="toast-region" aria-live="polite">
    <TransitionGroup name="toast">
      <div v-for="toast in toasts" :key="toast.id" class="toast" :class="toast.tone">
        <CircleCheck v-if="toast.tone === 'success'" :size="17" />
        <CircleAlert v-else-if="toast.tone === 'error' || toast.tone === 'warning'" :size="17" />
        <Info v-else :size="17" />
        <span>{{ toast.message }}</span>
      </div>
    </TransitionGroup>
  </div>
</template>
