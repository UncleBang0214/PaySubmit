<script setup lang="ts">
import { CheckCircle2, FileKey2, FolderOpen } from "lucide-vue-next";

const props = withDefaults(
  defineProps<{
    path: string;
    label: string;
    disabled?: boolean;
    kind?: "file" | "folder";
  }>(),
  { disabled: false, kind: "file" },
);

defineEmits<{ select: [] }>();

function basename(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}
</script>

<template>
  <button
    type="button"
    class="file-picker"
    :class="{ selected: props.path }"
    :disabled="props.disabled"
    @click="$emit('select')"
  >
    <span class="file-icon">
      <CheckCircle2 v-if="props.path" :size="20" />
      <FolderOpen v-else-if="props.kind === 'folder'" :size="20" />
      <FileKey2 v-else :size="20" />
    </span>
    <span class="file-copy">
      <strong>{{ props.path ? basename(props.path) : props.label }}</strong>
      <small>{{ props.path || "尚未选择" }}</small>
    </span>
    <span class="picker-action">选择</span>
  </button>
</template>

<style scoped>
.file-picker {
  display: flex;
  width: 100%;
  align-items: center;
  gap: 10px;
  border: 1px dashed var(--border-strong);
  border-radius: 8px;
  padding: 9px 12px;
  background: var(--surface);
  text-align: left;
  transition:
    border-color 0.16s ease,
    background-color 0.16s ease;
}
.file-picker:hover:not(:disabled) {
  border-color: var(--accent);
  background: var(--surface-hover);
}
.file-picker.selected {
  border-style: solid;
  border-color: color-mix(in srgb, var(--success) 40%, var(--border));
}
.file-icon {
  display: grid;
  width: 30px;
  height: 30px;
  place-items: center;
  flex: 0 0 30px;
  border-radius: 7px;
  background: var(--accent-soft);
  color: var(--accent);
}
.file-picker.selected .file-icon {
  background: var(--success-soft);
  color: var(--success);
}
.file-copy {
  display: grid;
  min-width: 0;
  flex: 1;
}
.file-copy strong {
  overflow: hidden;
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.file-copy small {
  overflow: hidden;
  color: var(--text-faint);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.picker-action {
  flex: 0 0 auto;
  color: var(--accent);
  font-size: 12px;
  font-weight: 620;
}
</style>
