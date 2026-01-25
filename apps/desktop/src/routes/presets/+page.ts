import { getDirEntries } from "$lib/mak";

import type { PageLoad } from "./$types";

export const load: PageLoad = async () => {
  const entries = await getDirEntries("");
  return { entries };
};
