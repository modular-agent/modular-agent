# Building on Windows

[English](build-windows.md) | [日本語](build-windows_ja.md)

A from-scratch setup for building Modular Agent Desktop on Windows 11, using `winget` for
every tool.

## 1. Install the toolchain

Run these in PowerShell. The Build Tools installer asks for elevation (UAC) and takes a
while.

```powershell
# MSVC C++ compiler + Windows SDK (required by Rust on Windows and by Tauri)
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

# Rust (rustup)
winget install --id Rustlang.Rustup -e

# Node.js LTS (to manage versions with fnm instead, see "Optional: Node.js with fnm")
winget install --id OpenJS.NodeJS.LTS -e

# CMake: a fallback for native dependencies such as aws-lc-sys
winget install --id Kitware.CMake -e
```

The WebView2 runtime Tauri needs ships with Windows 11. On Windows 10, install it from
[Microsoft](https://developer.microsoft.com/microsoft-edge/webview2/) if it is missing.

**Open a new terminal** after the installs so the updated `PATH` is picked up.

## 2. Set up Rust and check Node.js

The workspace uses edition 2024, which needs Rust 1.85 or later.

```powershell
rustup default stable-x86_64-pc-windows-msvc
rustup component add rustfmt clippy
rustc --version
```

Also check that `node --version` and `npm --version` work.

## 3. Install the JavaScript dependencies

The desktop app depends on the Tauri plugin's JS bindings through a local `file:` path.
Their build output (`dist-js/`) is not committed, so build it first; otherwise the
frontend fails to resolve `tauri-plugin-modular-agent-api`.

From the repository root:

```powershell
cd crates/tauri-plugin-modular-agent
npm install
npm run build

cd ../../apps/desktop
npm install
```

## 4. Build and run

From `apps/desktop`:

```powershell
npm run check        # svelte-check
npm run tauri dev    # development build; opens the app window
npm run tauri build  # release build + NSIS installer
```

The first build compiles the whole dependency tree and takes several minutes. Artifacts
land in the workspace-level `target/`:

- `target/debug/modular-agent-desktop.exe` (dev)
- `target/release/modular-agent-desktop.exe` and
  `target/release/bundle/nsis/*-setup.exe` (release)

To check only the Rust side, run `cargo build -p modular-agent-desktop` from anywhere in
the workspace.

## Optional: Node.js with fnm

The Node.js installer above puts one version on the system. To switch versions per
project, use [fnm](https://github.com/Schniz/fnm) instead: skip `OpenJS.NodeJS.LTS` in
step 1 (or uninstall it) and run

```powershell
winget install --id Schniz.fnm -e
fnm install --lts
fnm default <installed version>   # e.g. fnm default 24.21.0
```

fnm puts Node on `PATH` only in shells that run its setup, so configure each shell you
use.

### PowerShell

Append to your profile (`notepad $PROFILE`; create the file if it does not exist):

```powershell
fnm env --use-on-cd --shell powershell | Out-String | Invoke-Expression
```

### Git Bash

`~/.bashrc`:

```bash
eval "$(fnm env --use-on-cd --shell bash)"
```

Git Bash starts as a login shell and reads `~/.bash_profile`, not `~/.bashrc`. If you do
not have a `~/.bash_profile` yet, create one that loads `~/.bashrc`:

```bash
test -f ~/.profile && . ~/.profile
test -f ~/.bashrc && . ~/.bashrc
```

### cmd (and apps that do not load a shell profile)

cmd has no profile, so add fnm's default-version directory to your user `PATH`:
Settings → "Edit environment variables for your account" → `Path` → New →

```text
%APPDATA%\fnm\aliases\default
```

cmd then always uses `fnm default`'s version; it does not switch on `cd`. Use the
Environment Variables dialog rather than `setx` or `[Environment]::SetEnvironmentVariable`,
which can expand or truncate the existing `Path`.

In a new terminal, `node --version` and `npm --version` should work in every shell you
configured.

## Troubleshooting

- **`node`, `cargo` or `cmake` not found** — open a new terminal. Installers update
  `PATH` for new processes only, and terminals started from an already-running app
  (e.g. an IDE) need that app restarted.
- **`linker 'link.exe' not found` or missing Windows SDK headers** — the Build Tools
  install is missing the "Desktop development with C++" workload. Re-run the Build Tools
  command above, or add the workload from the Visual Studio Installer.
- **Cannot resolve `tauri-plugin-modular-agent-api`** — step 3's plugin build was
  skipped; run `npm run build` in `crates/tauri-plugin-modular-agent`, then `npm install`
  again in `apps/desktop`.
- **Cannot resolve a module UI dependency such as `d3-scale`** — a module package's
  `ui/` has npm dependencies of its own, which the dev server installs into
  `ui/node_modules` automatically on first start. If that install failed, run
  `npm install` inside that `ui/` directory (e.g. `crates/modular-agent-std/ui`) and start
  again.

For optional module packages (databases, messaging, …) see
[Custom Build (ma-config)](../README.md#custom-build-ma-config).
