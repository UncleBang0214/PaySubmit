<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import { CircleAlert } from "lucide-vue-next";

const props = defineProps<{ title: string; message: string; confirmLabel: string }>();
const emit = defineEmits<{ resolve: [confirmed: boolean] }>();

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") emit("resolve", false);
}

onMounted(() => document.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => document.removeEventListener("keydown", onKeydown));
</script>

<template>
  <div class="dialog-backdrop" @click.self="emit('resolve', false)">
    <section class="confirm-dialog" role="dialog" aria-modal="true" :aria-label="props.title">
      <span class="dialog-icon"><CircleAlert :size="21" /></span>
      <div class="dialog-copy">
        <h2>{{ title }}</h2>
        <p>{{ message }}</p>
      </div>
      <footer>
        <button type="button" class="btn ghost" @click="emit('resolve', false)">取消</button
        ><button type="button" class="btn primary" autofocus @click="emit('resolve', true)">
          {{ confirmLabel }}
        </button>
      </footer>
    </section>
  </div>
</template>
