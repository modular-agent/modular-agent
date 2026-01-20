import { invoke } from "@tauri-apps/api/core";

import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import type { AgentConfigs } from "tauri-plugin-mak-api";

import type { CoreSettings } from "./types";

// Tailwind CSS

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithoutChild<T> = T extends { child?: any } ? Omit<T, "child"> : T;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithoutChildren<T> = T extends { children?: any } ? Omit<T, "children"> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & { ref?: U | null };

// Tauri

const isEdge = typeof navigator !== "undefined" && navigator.userAgent?.includes("Edg");

// app

export async function exitApp(): Promise<void> {
  await invoke("exit_app_cmd");
}

// settings

export async function getCoreSettings(): Promise<CoreSettings> {
  return await invoke("get_core_settings_cmd");
}

export async function setCoreSettings(newSettings: Partial<CoreSettings>): Promise<void> {
  await invoke("set_core_settings_cmd", { newSettings });
}

export async function setGlobalConfigs(defName: string, configs: AgentConfigs): Promise<void> {
  await invoke("set_global_configs_cmd", { defName, configs });
}

// utilities

export function truncate(str: string, maxLength: number, suffix: string = "..."): string {
  if (str.length <= maxLength) {
    return str;
  }
  return str.slice(0, maxLength) + suffix;
}
