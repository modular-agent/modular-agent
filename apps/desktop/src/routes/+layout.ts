import { initGlobals } from "$lib/agent";

import type { LayoutLoad } from "./$types";

export const ssr = false;

export const load: LayoutLoad = async () => {
  await initGlobals();
};
