# Modular Agent one-command installer for Windows.
#
# Clones the repository and builds the desktop app or the `ma` CLI from
# source, with the recommended agent packages unless a minimal build is
# chosen. What to build is asked interactively; passing any option skips
# the questions. Assumes the Tauri prerequisites (git, Rust, Node.js,
# Visual Studio Build Tools) are installed:
# https://v2.tauri.app/start/prerequisites/
#
# Usage (PowerShell):
#   irm https://raw.githubusercontent.com/modular-agent/modular-agent/main/scripts/install.ps1 | iex
#
# Options (any option skips the questions; download the script to pass them):
#   -Cli        Install the `ma` command-line runner instead of the desktop app
#   -Minimal    Build with only the in-tree agent packages (std, llm); skip
#               cloning the recommended agent packages
#   -Dir <dir>  Clone destination (default: .\modular-agent)

param(
    [switch]$Cli,
    [switch]$Minimal,
    [string]$Dir = "modular-agent"
)

$ErrorActionPreference = "Stop"
$RepoUrl = "https://github.com/modular-agent/modular-agent.git"
$DocsUrl = "https://modular-agent.github.io/docs/getting-started"

# The recommended starting set from custom_agents/README.md.
$RecommendedAgents = @(
    "modular-agent-lifelog", "modular-agent-mattermost", "modular-agent-monty",
    "modular-agent-slack", "modular-agent-sqlx", "modular-agent-web", "modular-agent-zapcode"
)

function Info($msg) { Write-Host "==> $msg" -ForegroundColor Cyan }
function Fail($msg) { Write-Host "error: $msg" -ForegroundColor Red; exit 1 }

# --- What to build -----------------------------------------------------------

$BuildCli = [bool]$Cli
$BuildMinimal = [bool]$Minimal

if ($PSBoundParameters.Count -eq 0) {
    Write-Host "Modular Agent is built from source. A first desktop build typically takes"
    Write-Host "20-40 minutes and ~10 GB of disk; the minimal CLI build is the fastest."
    Write-Host ""
    $ans = Read-Host "Build the desktop app or the ma CLI? [desktop/cli] (desktop)"
    if ($ans -match '^[cC]') { $BuildCli = $true }
    $ans = Read-Host "Include the recommended agent packages (web, scripting, messaging, databases)? [Y/n] (Y)"
    if ($ans -match '^[nN]') { $BuildMinimal = $true }
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

# --- Agent packages ----------------------------------------------------------

if (-not $BuildMinimal) {
    Info "Cloning the recommended agent packages"
    foreach ($name in $RecommendedAgents) {
        $cloneDir = Join-Path $Dir "custom_agents\$name"
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
        Info "Building the configurator and applying the existing agent selection"
        $maConfigFlag = "--apply"
    }
    else {
        Info "Building the configurator and selecting the default agent set (first compile, a few minutes)"
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
    Write-Host "To change the agent package selection: $DocsUrl/installation/#adding-agent-packages"
}
