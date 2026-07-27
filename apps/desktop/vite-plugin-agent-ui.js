import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parse as parseToml } from "smol-toml";
import { searchForWorkspaceRoot } from "vite";

// Composes per-module agent UI packages at build time, mirroring how
// ma-config resolves Rust crates. Reads ma-config.toml (the single source of
// truth for which agent packages are in the build) and, for every agent with
// a Path source, statically imports <path>/ui/src/index.ts when it exists —
// exposed as the virtual module "virtual:agent-ui" which registers all
// NodeViews / ConfigWidgets into the desktop registry.

const VIRTUAL_ID = "virtual:agent-ui";
const RESOLVED_ID = "\0" + VIRTUAL_ID;

const desktopRoot = path.dirname(fileURLToPath(import.meta.url));
const maConfigPath = path.join(desktopRoot, "ma-config.toml");
const widgetKitDir = path.join(desktopRoot, "widget-kit");

/** @typedef {{ name: string, dir: string, dirPosix: string }} UiPackage */

/** @param {string} p */
function toPosix(p) {
  return p.replace(/\\/g, "/");
}

// ma-config.toml is gitignored and may be absent (fresh clone) — degrade to
// an empty package list so the build still succeeds.
/** @returns {UiPackage[]} */
function discoverUiPackages() {
  if (!existsSync(maConfigPath)) return [];
  let config;
  try {
    config = parseToml(readFileSync(maConfigPath, "utf-8"));
  } catch (e) {
    console.warn(`[agent-ui] failed to parse ma-config.toml: ${e}`);
    return [];
  }
  const agents = /** @type {any[]} */ (Array.isArray(config.agents) ? config.agents : []);
  /** @type {UiPackage[]} */
  const packages = [];
  for (const agent of agents) {
    const source = agent?.source;
    if (source?.type !== "Path" || typeof source.path !== "string") continue;
    // Paths in ma-config.toml are cargo path-dep paths, relative to src-tauri.
    const dir = path.resolve(desktopRoot, "src-tauri", source.path, "ui");
    if (!existsSync(path.join(dir, "package.json"))) continue;
    packages.push({ name: agent.name, dir, dirPosix: toPosix(dir) });
  }
  return packages;
}

/** @param {UiPackage[]} packages */
function generateVirtualModule(packages) {
  const lines = [
    `import { registerNodeView, registerConfigWidget } from "$lib/components/preset-editor/custom-ui/registry";`,
  ];
  packages.forEach((pkg, i) => {
    // Absolute path with forward slashes — Vite resolves it directly, so UI
    // packages outside the project root need no workspace/npm wiring.
    const entry = toPosix(path.join(pkg.dir, "src", "index.ts"));
    lines.push(`import { ui as ui${i} } from ${JSON.stringify(entry)};`);
  });
  packages.forEach((pkg, i) => {
    lines.push(
      `for (const [k, c] of Object.entries(ui${i}.nodeViews ?? {})) registerNodeView(k, c);`,
      `for (const [k, c] of Object.entries(ui${i}.configWidgets ?? {})) registerConfigWidget(k, c);`,
    );
  });
  return lines.join("\n") + "\n";
}

/** @returns {import("vite").Plugin} */
export default function agentUi() {
  // Refreshed on each virtual-module load; the config-hook values (fs.allow)
  // are fixed at server start, so adding a brand-new UI package dir to
  // ma-config.toml requires a dev-server restart.
  let packages = discoverUiPackages();

  return {
    name: "agent-ui",

    config() {
      return {
        server: {
          fs: {
            // Setting fs.allow replaces Vite's defaults, so re-include the
            // workspace root explicitly alongside the out-of-root UI dirs.
            allow: [
              searchForWorkspaceRoot(process.cwd()),
              desktopRoot,
              widgetKitDir,
              ...packages.map((pkg) => pkg.dir),
            ],
          },
        },
        resolve: {
          // A second svelte/xyflow copy from a UI package's node_modules
          // would break the shared runtime — force a single instance.
          dedupe: [
            "svelte",
            "@xyflow/svelte",
            "@modular-agent/widget-kit",
            "tauri-plugin-modular-agent-api",
          ],
          // UI packages import the widget-kit without npm registry access;
          // resolve it straight to the desktop's copy.
          alias: {
            "@modular-agent/widget-kit": toPosix(path.join(widgetKitDir, "src", "index.ts")),
          },
        },
      };
    },

    resolveId(id) {
      if (id === VIRTUAL_ID) return RESOLVED_ID;
    },

    load(id) {
      if (id !== RESOLVED_ID) return;
      packages = discoverUiPackages();
      return generateVirtualModule(packages);
    },

    configureServer(server) {
      // Regenerate the virtual module when ma-config.toml changes. The
      // registry is a non-reactive Map, so a full reload is required either
      // way (already-rendered nodes would keep stale components).
      server.watcher.add(maConfigPath);
      const onFsEvent = (/** @type {string} */ file) => {
        if (path.normalize(file) !== path.normalize(maConfigPath)) return;
        const mod = server.moduleGraph.getModuleById(RESOLVED_ID);
        if (mod) server.moduleGraph.invalidateModule(mod);
        server.ws.send({ type: "full-reload" });
      };
      server.watcher.on("change", onFsEvent);
      server.watcher.on("add", onFsEvent);
      server.watcher.on("unlink", onFsEvent);
    },

    handleHotUpdate({ file, server }) {
      // Edits inside a UI package: partial HMR would leave rendered nodes on
      // stale components (non-reactive registry Map) — force a full reload.
      if (packages.some((pkg) => file.startsWith(pkg.dirPosix + "/"))) {
        server.ws.send({ type: "full-reload" });
        return [];
      }
    },
  };
}
