import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parse as parseToml } from "smol-toml";
import { searchForWorkspaceRoot } from "vite";

// Composes per-module module UI packages at build time, mirroring how
// ma-config resolves Rust crates. Reads ma-config.toml (the single source of
// truth for which module packages are in the build) and, for every module —
// Workspace, Path, or a registry module with no source at all — statically
// imports <path>/ui/src/index.ts when it exists, exposed as the virtual module
// "virtual:module-ui" which registers all NodeViews / ConfigWidgets /
// NodeStyles into the desktop registry. A UI package's own npm dependencies
// (d3, three, ...) resolve from its ui/node_modules, so the plugin installs
// them on discovery when they are missing — no manual npm install per package.

const VIRTUAL_ID = "virtual:module-ui";
const RESOLVED_ID = "\0" + VIRTUAL_ID;

const desktopRoot = path.dirname(fileURLToPath(import.meta.url));
const maConfigPath = path.join(desktopRoot, "ma-config.toml");
const widgetKitDir = path.join(desktopRoot, "widget-kit");

/** @typedef {{ name: string, dir: string, dirPosix: string }} UiPackage */

/** @param {string} p */
function toPosix(p) {
  return p.replace(/\\/g, "/");
}

// Bare imports inside a UI package resolve from the package's own directory,
// not from the desktop's node_modules, so a package with dependencies needs
// its own ui/node_modules. Installed once, when absent; a failure is thrown
// rather than skipped, since a silently missing dependency surfaces later as
// an opaque "failed to resolve import" in the webview.
/** @param {string} name @param {string} dir */
function ensureInstalled(name, dir) {
  if (existsSync(path.join(dir, "node_modules"))) return;
  let pkg;
  try {
    pkg = JSON.parse(readFileSync(path.join(dir, "package.json"), "utf-8"));
  } catch (e) {
    throw new Error(`[module-ui] failed to read ${path.join(dir, "package.json")}: ${e}`);
  }
  if (Object.keys(pkg.dependencies ?? {}).length === 0) return;

  console.log(`[module-ui] installing dependencies for ${name} (${dir})`);
  // `npm ci` never rewrites a committed lockfile (`npm install` re-annotates
  // it whenever the npm version differs, leaving a dirty tracked file).
  const hasLockfile = existsSync(path.join(dir, "package-lock.json"));
  const args = [hasLockfile ? "ci" : "install", "--no-audit", "--no-fund"];
  // Under `npm run`, reuse the npm that launched us (matters with fnm/nvm,
  // where a bare "npm" may be a different version or not on PATH at all).
  // Otherwise go through the shell so Windows finds npm.cmd.
  const npmCli = process.env.npm_execpath;
  const result = npmCli
    ? spawnSync(process.execPath, [npmCli, ...args], { cwd: dir, stdio: "inherit" })
    : spawnSync(`npm ${args.join(" ")}`, { cwd: dir, stdio: "inherit", shell: true });
  if (result.error || result.status !== 0) {
    const reason = result.error ? String(result.error) : `exit code ${result.status}`;
    throw new Error(
      `[module-ui] npm ${args[0]} failed for ${name} (${reason}). ` +
        `Run \`npm install\` manually in ${dir} and restart.`,
    );
  }
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
    console.warn(`[module-ui] failed to parse ma-config.toml: ${e}`);
    return [];
  }
  const modules = /** @type {any[]} */ (Array.isArray(config.modules) ? config.modules : []);
  /** @type {UiPackage[]} */
  const packages = [];
  for (const module of modules) {
    const source = module?.source;
    /** @type {string} */
    let dir;
    if (source?.type === "Path" && typeof source.path === "string") {
      // Paths in ma-config.toml are relative to the workspace root.
      dir = path.resolve(desktopRoot, "../..", source.path, "ui");
    } else if (source?.type === "Workspace") {
      // In-tree modules live under crates/<name> at the workspace root.
      dir = path.resolve(desktopRoot, "../../crates", module.name, "ui");
    } else if (!source && typeof module?.name === "string") {
      // A registry module carries no source: it is the clone under custom_modules/.
      dir = path.resolve(desktopRoot, "../..", "custom_modules", module.name, "ui");
    } else {
      continue;
    }
    if (!existsSync(path.join(dir, "package.json"))) continue;
    ensureInstalled(module.name, dir);
    packages.push({ name: module.name, dir, dirPosix: toPosix(dir) });
  }
  return packages;
}

/** @param {UiPackage[]} packages */
function generateVirtualModule(packages) {
  const lines = [
    `import { registerNodeView, registerConfigWidget, registerNodeStyle } from "$lib/components/patch-editor/custom-ui/registry";`,
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
      `for (const [k, s] of Object.entries(ui${i}.nodeStyles ?? {})) registerNodeStyle(k, s);`,
    );
  });
  return lines.join("\n") + "\n";
}

/** @returns {import("vite").Plugin} */
export default function moduleUi() {
  // Refreshed on each virtual-module load; the config-hook values (fs.allow)
  // are fixed at server start, so adding a brand-new UI package dir to
  // ma-config.toml requires a dev-server restart.
  let packages = discoverUiPackages();

  return {
    name: "module-ui",

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
