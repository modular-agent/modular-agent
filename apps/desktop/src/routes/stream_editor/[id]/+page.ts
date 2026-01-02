import { error } from "@sveltejs/kit";

import { getAgentStreamInfo, getAgentStreamSpec } from "tauri-plugin-askit-api";

import { streamToFlow } from "$lib/agent";

import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ params }) => {
  const info = await getAgentStreamInfo(params.id);
  if (!info) {
    error(404, {
      message: "Not agent stream info found",
    });
  }

  const spec = await getAgentStreamSpec(params.id);
  if (!spec) {
    error(404, {
      message: "Not agent stream spec found",
    });
  }

  const flow = streamToFlow(info, spec);

  return {
    stream_id: params.id,
    info,
    spec,
    flow,
  };
};
