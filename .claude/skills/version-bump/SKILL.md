---
name: version-bump
description: >-
  Set the version shared by every crate and app in this workspace by editing every file
  that carries it — the root Cargo.toml (workspace.package and workspace.dependencies),
  the npm manifests that track it (plugin, desktop), tauri.conf.json, the core / llm
  README dependency snippets, and Cargo.lock. Use this whenever the user wants to change,
  raise, or set the version or cut a release: "バージョンを 0.33.0 に", "bump the
  version", "minor で上げて", "次のパッチを切る", "set the version to 1.0", "アプリの
  バージョンを上げて". Always reach for this instead of editing a single version field by
  hand, because the number is duplicated across many spots and ad hoc edits reliably miss
  one.
---

# Version bump (whole workspace)

Every in-tree crate (macros, core, std, llm, the Tauri plugin) and both apps (desktop,
cli) share one version. The crates take it through `version.workspace = true`, so their
own `Cargo.toml` files never change; everything else that carries the number is listed
below. `tools/ma-config` (outside the workspace), `apps/desktop/widget-kit` and
`crates/modular-agent-std/ui` keep their own versions — leave them alone.

## The fields to edit

Paths are relative to the workspace root.

| File | Field(s) |
|------|----------|
| `Cargo.toml` | `[workspace.package]` → `version`, and the `version` of the five in-tree entries in `[workspace.dependencies]` |
| `crates/tauri-plugin-modular-agent/package.json` | top-level `"version"` (npm `tauri-plugin-modular-agent-api`) |
| `apps/desktop/package.json` | top-level `"version"` |
| `apps/desktop/package-lock.json` | top-level `"version"`, `packages[""].version`, and `packages["../../crates/tauri-plugin-modular-agent"].version` |
| `apps/desktop/src-tauri/tauri.conf.json` | top-level `"version"` |
| `crates/modular-agent-core/README.md`, `README_ja.md` | `modular-agent-core = ...` dependency snippets (major.minor, e.g. `"0.32"`) |
| `crates/modular-agent-llm/README.md` | `modular-agent-llm = ...` dependency snippet |
| `Cargo.lock` | the seven workspace packages' `version` lines — let cargo rewrite them |

`crates/tauri-plugin-modular-agent/package-lock.json` is gitignored; update its two
version lines too (or run `npm install --package-lock-only` there) so a local
`npm publish` does not see a stale number.

## Workflow

### 1. Resolve the target version

- Explicit → use it as given.
- `patch` / `minor` / `major` → read `[workspace.package].version` from the root
  `Cargo.toml` and step it.
- State the resolved target back to the user so a misread is obvious.

### 2. Edit the fields, then let cargo update the lock

Edit everything in the table except `Cargo.lock`, then run
`cargo check --workspace --all-targets`; it rewrites the seven `[[package]]` versions in
`Cargo.lock` and nothing else.

### 3. Verify nothing was missed

```bash
cargo metadata --no-deps --format-version 1 \
  | python -c "import json,sys;[print(p['name'],p['version']) for p in json.load(sys.stdin)['packages']]"
grep -n 'version = "' Cargo.toml                                   # workspace.package + 5 deps -> NEW
grep -n '"version"' crates/tauri-plugin-modular-agent/package.json # -> NEW
grep -n '"version"' apps/desktop/package.json apps/desktop/src-tauri/tauri.conf.json
grep -n -A1 -e '"name": "modular-agent-desktop"' -e '"name": "tauri-plugin-modular-agent-api"' \
  apps/desktop/package-lock.json
grep -n 'modular-agent-\(core\|llm\) = ' crates/modular-agent-core/README*.md crates/modular-agent-llm/README.md
```

### 4. Show the diff and hand back

Show `git diff --stat` to the user. **Do not commit, tag or push** — that is the user's
call. A release is tagged with a single `v<NEW>` tag (tags are lightweight, so push it
explicitly: `git push origin main v<NEW>`).
