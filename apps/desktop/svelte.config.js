// Tauri doesn't have a Node.js server to do proper SSR
// so we will use adapter-static to prerender the app (SSG)
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      fallback: "index.html",
    }),
    alias: {
      "@": "./src",
      // Link the in-tree plugin's TypeScript source instead of its gitignored
      // rollup output (dist-js/), so a fresh clone needs no plugin build and
      // guest-js/ edits apply without a rebuild. Applies to Vite and to the
      // generated tsconfig paths alike.
      "tauri-plugin-modular-agent-api": "../../crates/tauri-plugin-modular-agent/guest-js/index.ts",
    },
    typescript: {
      // guest-js/ has no node_modules of its own; type-check its
      // @tauri-apps/api imports against the desktop's copy. (Vite resolves
      // them through resolve.dedupe in vite.config.js.) Paths in the
      // generated tsconfig are relative to .svelte-kit/.
      config(tsconfig) {
        tsconfig.compilerOptions.paths["@tauri-apps/api/*"] = ["../node_modules/@tauri-apps/api/*"];
      },
    },
  },
};

export default config;
