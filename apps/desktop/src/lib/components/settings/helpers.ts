import { toast } from "svelte-sonner";

import { setCoreSettings } from "$lib/module";
import type { CoreSettings } from "$lib/types";

// Silent on success, toast on error. The backend error is shown as the
// description: it distinguishes an actual save failure from "saved, but
// applying failed" (e.g. the MCP server port is already in use).
export async function saveCoreSettings(partial: Partial<CoreSettings>) {
  try {
    await setCoreSettings(partial);
  } catch (e) {
    toast.error("Failed to apply settings", { description: String(e) });
    console.error("Failed to apply settings:", e);
  }
}

export function blurOnEnter(e: KeyboardEvent) {
  if (e.key === "Enter") (e.target as HTMLInputElement).blur();
}
