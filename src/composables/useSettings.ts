import { isTauri } from "@tauri-apps/api/core";
import { computed, readonly, ref, shallowRef } from "vue";

import { rpc } from "../rpc";
import { errorMessage } from "../utils";

export const credentialFields = [
  "customer_number", "md5_key", "sha256_key", "triple_des_key", "aes_key",
  "rsa_private_key", "rsa_public_key", "sm2_private_key", "sm2_public_key",
] as const;

export type CredentialField = typeof credentialFields[number];
export type CredentialChanges = Partial<Record<CredentialField, string | null>>;
export type CredentialSummary = { customer_number: string | null } & Record<Exclude<CredentialField, "customer_number">, boolean>;
export type ApiDomain = "domestic" | "cross_border";
export type ApiCombo = "des_md5" | "des_sha256" | "sm" | "rsa_aes" | "aes_sha256";
export type ApiSecretField = Exclude<CredentialField, "customer_number">;

export interface DomainCredentialSummary {
  customer_number: string | null;
  combos: Partial<Record<ApiCombo, CredentialSummary>>;
}

export interface ApiCatalogCombo {
  id: ApiCombo;
  name: string;
  fields: ApiSecretField[];
  products: { id: string; name: string }[];
}

export interface ApiCatalog {
  domains: { id: ApiDomain; name: string; combos: ApiCatalogCombo[] }[];
}

export interface SettingsSnapshot {
  revision: number;
  products: Record<string, DomainCredentialSummary>;
  sm_domains: Record<ApiDomain, DomainCredentialSummary>;
}

export const DESKTOP_SETTINGS_MESSAGE = "请在Windows桌面客户端中管理本地密钥";

const snapshot = shallowRef<SettingsSnapshot | null>(null);
const initialized = ref(false);
const loading = ref(false);
const saving = ref(false);
const loadError = ref("");
let initialization: Promise<SettingsSnapshot> | null = null;

function requireDesktop() {
  if (!isTauri()) throw new Error(DESKTOP_SETTINGS_MESSAGE);
}

async function initializeSettings(): Promise<SettingsSnapshot> {
  loading.value = true;
  loadError.value = "";
  try {
    requireDesktop();
    const result = await rpc<SettingsSnapshot>("settings.get");
    snapshot.value = result;
    initialized.value = true;
    return result;
  } catch (error) {
    loadError.value = `本地密钥加载失败：${errorMessage(error)}`;
    throw error;
  } finally {
    loading.value = false;
  }
}

export function ensureSettings(): Promise<SettingsSnapshot> {
  if (initialized.value && snapshot.value && !loading.value) return Promise.resolve(snapshot.value);
  initialization ??= initializeSettings();
  return initialization;
}

export async function loadApiCatalog(): Promise<ApiCatalog> {
  requireDesktop();
  return rpc<ApiCatalog>("settings.api.catalog");
}

async function writeSettings(action: string, params: Record<string, unknown>): Promise<SettingsSnapshot> {
  requireDesktop();
  await ensureSettings();
  if (saving.value) throw new Error("另一项密钥正在保存，请等待完成");
  saving.value = true;
  try {
    const result = await rpc<SettingsSnapshot>(action, params);
    snapshot.value = result;
    return result;
  } finally {
    saving.value = false;
  }
}

export async function saveApiChanges(domain: ApiDomain, combo: ApiCombo, scope: string, changes: CredentialChanges, revision: number): Promise<SettingsSnapshot> {
  return writeSettings("settings.api.save", { revision, domain, combo, scope, changes });
}

export function useSettings() {
  return {
    snapshot: readonly(snapshot),
    initialized: readonly(initialized),
    loading: readonly(loading),
    saving: readonly(saving),
    loadError: readonly(loadError),
    ensureSettings,
    loadApiCatalog,
    saveApiChanges,
  };
}

export const savingState = computed(() => saving.value);
