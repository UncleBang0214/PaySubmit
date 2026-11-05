<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { Check, ChevronDown } from "lucide-vue-next";

export interface SelectOption {
  value: string;
  label: string;
}

const props = withDefaults(
  defineProps<{
    modelValue: string;
    options: SelectOption[];
    placeholder?: string;
    ariaLabel?: string;
    disabled?: boolean;
  }>(),
  {
    placeholder: "请选择",
    ariaLabel: "选择选项",
    disabled: false,
  },
);

const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const root = ref<HTMLElement | null>(null);
const trigger = ref<HTMLButtonElement | null>(null);
const open = ref(false);
const openUpward = ref(false);
const activeIndex = ref(0);
const selectedOption = computed(() =>
  props.options.find((option) => option.value === props.modelValue),
);

function optionButtons() {
  return Array.from(
    root.value?.querySelectorAll<HTMLButtonElement>(".select-control-option") ?? [],
  );
}

async function focusOption(index: number) {
  activeIndex.value = Math.min(Math.max(index, 0), Math.max(props.options.length - 1, 0));
  await nextTick();
  optionButtons()[activeIndex.value]?.focus();
}

async function openMenu(focusSelected = false) {
  if (props.disabled || !props.options.length) return;
  const selectedIndex = props.options.findIndex((option) => option.value === props.modelValue);
  activeIndex.value = selectedIndex >= 0 ? selectedIndex : 0;
  const rect = root.value?.getBoundingClientRect();
  const menuHeight = Math.min(props.options.length * 38 + 10, 270);
  openUpward.value = Boolean(
    rect && rect.bottom + menuHeight > window.innerHeight - 16 && rect.top > menuHeight,
  );
  open.value = true;
  if (focusSelected) {
    await focusOption(activeIndex.value);
  } else {
    await nextTick();
    optionButtons()[activeIndex.value]?.scrollIntoView({ block: "nearest" });
  }
}

function close(focusTrigger = false) {
  open.value = false;
  if (focusTrigger) void nextTick(() => trigger.value?.focus());
}

function toggle() {
  if (open.value) close();
  else void openMenu();
}

function choose(value: string) {
  emit("update:modelValue", value);
  close(true);
}

function handleTriggerKeydown(event: KeyboardEvent) {
  if (!["ArrowDown", "ArrowUp"].includes(event.key)) return;
  event.preventDefault();
  void openMenu(true);
}

function handleOptionKeydown(event: KeyboardEvent, index: number) {
  if (event.key === "ArrowDown") {
    event.preventDefault();
    void focusOption((index + 1) % props.options.length);
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    void focusOption((index - 1 + props.options.length) % props.options.length);
  } else if (event.key === "Home") {
    event.preventDefault();
    void focusOption(0);
  } else if (event.key === "End") {
    event.preventDefault();
    void focusOption(props.options.length - 1);
  } else if (event.key === "Escape") {
    event.preventDefault();
    close(true);
  }
}

function handlePointerDown(event: PointerEvent) {
  if (root.value && !root.value.contains(event.target as Node)) close();
}

function handleDocumentKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && open.value) close(true);
}

onMounted(() => {
  document.addEventListener("pointerdown", handlePointerDown);
  document.addEventListener("keydown", handleDocumentKeydown);
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", handlePointerDown);
  document.removeEventListener("keydown", handleDocumentKeydown);
});
</script>

<template>
  <div ref="root" class="select-control" :class="{ open, disabled, 'open-upward': openUpward }">
    <button
      ref="trigger"
      type="button"
      class="select-control-trigger"
      :disabled="disabled"
      :aria-label="ariaLabel"
      aria-haspopup="listbox"
      :aria-expanded="open"
      @click="toggle"
      @keydown="handleTriggerKeydown"
    >
      <span :class="{ placeholder: !selectedOption }">{{
        selectedOption?.label ?? placeholder
      }}</span>
      <ChevronDown :size="16" class="select-control-chevron" />
    </button>
    <div v-if="open" class="select-control-menu" role="listbox">
      <button
        v-for="(option, index) in options"
        :key="option.value"
        type="button"
        class="select-control-option"
        :class="{ selected: option.value === modelValue }"
        role="option"
        :aria-selected="option.value === modelValue"
        @click="choose(option.value)"
        @keydown="handleOptionKeydown($event, index)"
      >
        <span>{{ option.label }}</span>
        <Check v-if="option.value === modelValue" :size="15" />
      </button>
    </div>
  </div>
</template>
