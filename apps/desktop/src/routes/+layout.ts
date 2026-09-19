import { initGlobals } from "$lib/module";

import type { LayoutLoad } from "./$types";

export const ssr = false;

export const load: LayoutLoad = async () => {
  await initGlobals();
};
