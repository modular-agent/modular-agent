# Modular Agent one-command installer for Windows.
#
# Clones the repository and builds the desktop app or the `ma` CLI from
# source, with the recommended module packages unless a minimal build is
# chosen. What to build is asked interactively; passing any option skips
# the questions, and without a console the defaults are used. Assumes the
# Tauri prerequisites (git, Rust, Node.js, Visual Studio Build Tools) are
# installed: https://v2.tauri.app/start/prerequisites/
#
# Usage (PowerShell):
#   irm https://raw.githubusercontent.com/modular-agent/modular-agent/main/scripts/install.ps1 | iex
#
# `| iex` cannot pass options. To pass them, run the downloaded text as a
# script block instead:
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/modular-agent/modular-agent/main/scripts/install.ps1))) -Cli
#
# Options (any option skips the questions):
#   -Cli        Install the `ma` command-line runner instead of the desktop app
#   -Minimal    Build with only the in-tree module packages (std, llm); skip
#               cloning the recommended module packages
#   -Dir <dir>  Clone destination (default: .\modular-agent)

param(
    [switch]$Cli,
    [switch]$Minimal,
    [string]$Dir = "modular-agent"
)

# Everything below runs in its own scope. Under `irm | iex` the script runs
# in the user's session, so without this the `Stop` error preference, the
# helper functions and every variable would stay behind after the install.
# $PSBoundParameters is read here because inside the block it would be the
# block's own (empty) set.
& {
    param([bool]$AskWhatToBuild)

    $ErrorActionPreference = "Stop"
    $RepoUrl = "https://github.com/modular-agent/modular-agent.git"
    $DocsUrl = "https://modular-agent.github.io/docs/getting-started"

    # The recommended starting set from custom_modules/README.md.
    $RecommendedModules = @(
        "modular-agent-lifelog", "modular-agent-mattermost", "modular-agent-monty",
        "modular-agent-slack", "modular-agent-sqlx", "modular-agent-web", "modular-agent-zapcode"
    )

    function Info($msg) { Write-Host "==> $msg" -ForegroundColor Cyan }
    # Stop with `throw`, not `exit`: under `irm | iex` the script runs inside the
    # user's session, so `exit` would close their PowerShell window. A throw ends
    # only the script, runs the `finally` blocks (Pop-Location), and still exits
    # with code 1 when the script is run with -File.
    function Fail($msg) {
        Write-Host "error: $msg" -ForegroundColor Red
        throw "Modular Agent installation stopped: $msg"
    }

    # --- What to build -----------------------------------------------------------

    $BuildCli = [bool]$Cli
    $BuildMinimal = [bool]$Minimal

    # Read-Host needs someone at a console: under `-NonInteractive` (CI,
    # scheduled tasks, an agent's shell) it throws, and with stdin redirected it
    # reads the pipe instead of the user. Matches -noni, the shortest
    # abbreviation of -NonInteractive that PowerShell accepts.
    function Test-CanPrompt {
        if (-not [Environment]::UserInteractive) { return $false }
        if ([Environment]::GetCommandLineArgs() -match '^[-/]noni') { return $false }
        if ([Console]::IsInputRedirected) { return $false }
        return $true
    }

    $DefaultsNote = "building the desktop app with the recommended module packages (pass -Cli or -Minimal to choose)"

    if ($AskWhatToBuild) {
        if (Test-CanPrompt) {
            try {
                Write-Host "Modular Agent is built from source. A first desktop build typically takes"
                Write-Host "20-40 minutes and ~10 GB of disk; the minimal CLI build is the fastest."
                Write-Host ""
                $ans = Read-Host "Build the desktop app or the ma CLI? [desktop/cli] (desktop)"
                if ($ans -match '^[cC]') { $BuildCli = $true }
                $ans = Read-Host "Include the recommended module packages (web, scripting, messaging, databases)? [Y/n] (Y)"
                if ($ans -match '^[nN]') { $BuildMinimal = $true }
            }
            catch {
                # A host the checks above did not catch: fall back to the defaults.
                $BuildCli = $false
                $BuildMinimal = $false
                Info "Could not ask ($($_.Exception.Message)); $DefaultsNote"
            }
        }
        else {
            Info "No console to ask on; $DefaultsNote"
        }
    }

    # --- Clone -------------------------------------------------------------------

    if (Test-Path (Join-Path $Dir ".git")) {
        Info "Updating existing clone at $Dir"
        & git -C $Dir pull --ff-only
        if ($LASTEXITCODE -ne 0) { Fail "git pull failed" }
    }
    else {
        Info "Cloning $RepoUrl into $Dir"
        & git clone $RepoUrl $Dir
        if ($LASTEXITCODE -ne 0) { Fail "git clone failed" }
    }

    # --- Module packages ----------------------------------------------------------

    if (-not $BuildMinimal) {
        Info "Cloning the recommended module packages"
        foreach ($name in $RecommendedModules) {
            $cloneDir = Join-Path $Dir "custom_modules\$name"
            if (Test-Path (Join-Path $cloneDir ".git")) {
                Write-Host "  ${name}: updating existing clone"
                & git -C $cloneDir pull --ff-only
                if ($LASTEXITCODE -ne 0) { Fail "git pull $name failed" }
            }
            else {
                & git clone "https://github.com/modular-agent/$name.git" $cloneDir
                if ($LASTEXITCODE -ne 0) { Fail "git clone $name failed" }
            }
        }

        $app = if ($BuildCli) { "cli" } else { "desktop" }
        if (Test-Path (Join-Path $Dir "apps\$app\ma-config.toml")) {
            Info "Building the configurator and applying the existing module selection"
            $maConfigFlag = "--apply"
        }
        else {
            Info "Building the configurator and selecting the default module set (first compile, a few minutes)"
            $maConfigFlag = "--defaults"
        }
        Push-Location $Dir
        try {
            & cargo run --manifest-path tools/ma-config/Cargo.toml -- $app $maConfigFlag
            if ($LASTEXITCODE -ne 0) { Fail "ma-config failed" }
        }
        finally { Pop-Location }
    }

    # --- Build -------------------------------------------------------------------

    if ($BuildCli) {
        Info "Building and installing the ma CLI (typically 10-20 minutes on a first build)"
        & cargo install --path (Join-Path $Dir "apps\cli") --locked
        if ($LASTEXITCODE -ne 0) { Fail "cargo install failed" }
        Info "Done. The ma binary is on your PATH (via %USERPROFILE%\.cargo\bin)."
    }
    else {
        Info "Building the desktop app (typically 20-40 minutes and ~10 GB of disk on a first build)"
        Push-Location (Join-Path $Dir "apps\desktop")
        try {
            & npm install
            if ($LASTEXITCODE -ne 0) { Fail "npm install failed" }
            & npm run tauri build
            if ($LASTEXITCODE -ne 0) { Fail "tauri build failed" }
        }
        finally { Pop-Location }
        Info "Done. Launch the executable, or use the installer to install it:"
        Write-Host "  Executable: $Dir\target\release\modular-agent-desktop.exe"
        Write-Host "  Installer:  $Dir\target\release\bundle\nsis\*-setup.exe"
    }

    Write-Host ""
    Write-Host "Next: build your first patch: $DocsUrl/first-patch/"
    if (-not $BuildMinimal) {
        Write-Host "To change the module package selection: $DocsUrl/installation/#adding-module-packages"
    }
} ($PSBoundParameters.Count -eq 0)
