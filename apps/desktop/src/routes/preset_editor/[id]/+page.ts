import { error } from "@sveltejs/kit";

import { getPresetInfo, getPresetSpec } from "tauri-plugin-mak-api";

import { presetToFlow } from "$lib/agent";

import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ params }) => {
  const info = await getPresetInfo(params.id);
  if (!info) {
    error(404, {
      message: "Not preset info found",
    });
  }

  const spec = await getPresetSpec(params.id);
  if (!spec) {
    error(404, {
      message: "Not preset spec found",
    });
  }

  const flow = presetToFlow(info, spec);

  return {
    preset_id: params.id,
    info,
    spec,
    flow,
  };
};
