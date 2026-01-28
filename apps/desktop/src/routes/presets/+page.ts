import { getDirEntries } from "$lib/modular_agent";

import type { PageLoad } from "./$types";

export const load: PageLoad = async () => {
  const entries = await getDirEntries("");
  return { entries };
};
