import { loadPatchInfos } from "$lib/module";

import type { PageLoad } from "./$types";

export const load: PageLoad = async () => {
  return {
    patchInfos: await loadPatchInfos(),
  };
};
