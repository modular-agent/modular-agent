import { getGlobalConfigs } from "$lib/modular_agent";

import type { PageLoad } from "./$types";

export const load: PageLoad = async () => {
  return {
    globalConfigs: await getGlobalConfigs(),
  };
};
