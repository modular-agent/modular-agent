---
name: version-bump
description: >-
  Set the modular-agent-desktop app version by editing every file that carries it —
  package.json, package-lock.json, src-tauri/Cargo.toml, src-tauri/Cargo.lock, and
  src-tauri/tauri.conf.json. Use this whenever the user wants to change, raise, or set
  the app version or cut a release: "バージョンを 0.17.0に", "bump the version", "minor
  で上げて", "次のパッチを切る", "set the version to 1.0", "アプリのバージョンを上げて". Always
  reach for this instead of editing a single version field by hand, because the version
  is duplicated across six spots (package-lock.json holds it twice and Cargo.lock hides
  it inside the crate's `[[package]]` entry) and editing them ad hoc reliably misses one.
---

# Version bump (modular-agent-desktop)

The desktop app version is duplicated across **five files / six fields**. There is no
tooling that syncs them — you edit each one, then verify none were missed. The whole point
of this skill is that the list below is complete, so nothing gets left behind.

## The six fields to edit

| File | Field(s) |
|------|----------|
| `package.json` | top-level `"version"` |
| `package-lock.json` | **two** fields: top-level `"version"` and `packages[""].version` |
| `src-tauri/tauri.conf.json` | top-level `"version"` |
| `src-tauri/Cargo.toml` | `[package]` → `version = "..."` |
| `src-tauri/Cargo.lock` | the `[[package]]` block whose `name = "modular-agent-desktop"` → its `version = "..."` |

`package-lock.json` and `Cargo.lock` are the two that get forgotten — the lockfile carries
the app version twice near the top, and Cargo.lock buries it in one block among hundreds.

## Workflow

Work from the repo root (`modular-agent-desktop/`).

### 1. Resolve the target version

- Explicit → use it as given (`0.17.0`).
- `patch` / `minor` / `major` → read the current version from `package.json` and step it
  (0.16.0 + minor → 0.17.0, + patch → 0.16.1, + major → 1.0.0).
- State the resolved target back to the user in your summary so a misread is obvious.

The current version is whatever is in `package.json`.

### 2. Edit all six fields

Edit each file to the new version. Some notes that save round-trips:

- In `package-lock.json`, both version lines sit near the top (the top-level key and the
  `""` root-package entry) — change both.
- In `Cargo.lock`, find `name = "modular-agent-desktop"` and change the `version` on the
  next line — do not touch any other crate's version.
- Editing `Cargo.lock` by hand is correct and deterministic. (Running a cargo command or
  letting rust-analyzer reconcile the lockfile also works, but a direct edit needs no
  build and no network.)

### 3. Verify nothing was missed

Check each of the six fields directly — don't just count occurrences of the version
string, because `Cargo.lock` and `package-lock.json` also list dependency versions and
another crate may coincidentally share your number, inflating the count:

```bash
grep -n '"version"' package.json                                  # -> NEW
grep -n '"version"' package-lock.json | head -2                   # top-level + root pkg -> NEW
grep -n '"version"' src-tauri/tauri.conf.json                     # -> NEW
grep -n '^version' src-tauri/Cargo.toml                           # [package] -> NEW
grep -n -A1 'name = "modular-agent-desktop"' src-tauri/Cargo.lock # entry -> NEW
```

All six should read the new version. As a final sweep, confirm the crate's own old
version is gone (a stray hit here means a field was missed):

```bash
grep -n -A1 'name = "modular-agent-desktop"' src-tauri/Cargo.lock   # not OLD
```

### 4. Show the diff and hand back

```bash
git diff --stat -- package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
```

Show it to the user. **Do not commit or push** — that is the user's call. When they're ready
they typically commit and tag with something like:

```bash
git commit -am "chore(release): <NEW>"
git tag v<NEW>
git push --follow-tags origin main
```

Offer that as a next step; don't run it unprompted.
