import { reactive, ref } from "vue";

import type { StatusTone } from "../types";

interface ToastMessage {
  id: number;
  message: string;
  tone: StatusTone;
}

interface ConfirmationState {
  title: string;
  message: string;
  confirmLabel: string;
}

const toasts = reactive<ToastMessage[]>([]);
const confirmation = ref<ConfirmationState | null>(null);
let toastId = 0;
let confirmationResolver: ((confirmed: boolean) => void) | null = null;

function notify(message: string, tone: StatusTone = "success") {
  const id = ++toastId;
  toasts.push({ id, message, tone });
  window.setTimeout(() => {
    const index = toasts.findIndex((toast) => toast.id === id);
    if (index >= 0) toasts.splice(index, 1);
  }, 3200);
}

function confirm(title: string, message: string, confirmLabel = "确认"): Promise<boolean> {
  if (confirmationResolver) confirmationResolver(false);
  confirmation.value = { title, message, confirmLabel };
  return new Promise<boolean>((resolve) => {
    confirmationResolver = resolve;
  });
}

function resolveConfirmation(confirmed: boolean) {
  const resolver = confirmationResolver;
  confirmationResolver = null;
  confirmation.value = null;
  resolver?.(confirmed);
}

export function useUi() {
  return { toasts, confirmation, notify, confirm, resolveConfirmation };
}
