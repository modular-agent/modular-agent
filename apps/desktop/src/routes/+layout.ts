import type { LayoutLoad } from "./$types";

import { initGlobals } from "$lib/agent";

export const ssr = false;

export const load: LayoutLoad = async () => {
    await initGlobals();
};
