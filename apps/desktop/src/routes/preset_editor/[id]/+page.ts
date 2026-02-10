import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ params }) => {
  return { preset_id: params.id };
};
