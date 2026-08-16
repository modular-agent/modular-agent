import { loadPatchInfos } from "$lib/agent";

import type { PageLoad } from "./$types";

export const load: PageLoad = async () => {
  return {
    patchInfos: await loadPatchInfos(),
  };
};
