#!/bin/sh
# Modular Agent one-command installer for macOS / Linux.
#
# Clones the repository and builds the desktop app or the `ma` CLI from
# source, with the recommended agent packages unless a minimal build is
# chosen. What to build is asked interactively; passing any option skips
# the questions. Assumes the Tauri prerequisites (git, Rust, Node.js, the
# platform toolchain) are installed: https://v2.tauri.app/start/prerequisites/
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/modular-agent/modular-agent/main/scripts/install.sh | sh
#
# Options (any option skips the questions):
#   --cli        Install the `ma` command-line runner instead of the desktop app
#   --minimal    Build with only the in-tree agent packages (std, llm); skip
#                cloning the recommended agent packages
#   --dir <dir>  Clone destination (default: ./modular-agent)
#   --help, -h   Show this help

set -eu

REPO_URL="https://github.com/modular-agent/modular-agent.git"
DOCS_URL="https://modular-agent.github.io/docs/getting-started"
OS=$(uname -s 2>/dev/null || echo unknown)

# The recommended starting set from custom_agents/README.md.
RECOMMENDED_AGENTS="modular-agent-lifelog modular-agent-mattermost modular-agent-monty
modular-agent-slack modular-agent-sqlx modular-agent-web modular-agent-zapcode"

TARGET=desktop
DIR=modular-agent
MINIMAL=0
WIZARD=1

info() { printf '\033[1;34m==>\033[0m %s\n' "$1"; }
die() {
    printf '\033[1;31merror:\033[0m %s\n' "$1" >&2
    exit 1
}

usage() {
    cat <<'EOF'
Usage: install.sh [options]

Run without options to be asked what to build. Any option skips the questions:

  --cli        Install the `ma` command-line runner instead of the desktop app
  --minimal    Build with only the in-tree agent packages (std, llm)
  --dir <dir>  Clone destination (default: ./modular-agent)
  --help, -h   Show this help
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
    --cli)
        TARGET=cli
        WIZARD=0
        ;;
    --minimal)
        MINIMAL=1
        WIZARD=0
        ;;
    --dir)
        [ $# -ge 2 ] || die "--dir requires an argument"
        DIR=$2
        WIZARD=0
        shift
        ;;
    --help | -h)
        usage
        exit 0
        ;;
    *) die "unknown option: $1 (try --help)" ;;
    esac
    shift
done

# --- What to build -----------------------------------------------------------

if [ "$WIZARD" = 1 ] && [ -r /dev/tty ]; then
    {
        printf 'Modular Agent is built from source. A first desktop build typically takes\n'
        printf '20-40 minutes and ~10 GB of disk; the minimal CLI build is the fastest.\n\n'
        printf 'Build the desktop app or the ma CLI? [desktop/cli] (desktop): '
    } >/dev/tty
    read -r ans </dev/tty || ans=
    case "$ans" in c* | C*) TARGET=cli ;; esac

    printf 'Include the recommended agent packages (web, scripting, messaging,\ndatabases)? [Y/n] (Y): ' >/dev/tty
    read -r ans </dev/tty || ans=
    case "$ans" in n* | N*) MINIMAL=1 ;; esac
fi

# --- Clone -------------------------------------------------------------------

if [ -d "$DIR/.git" ]; then
    info "Updating existing clone at $DIR"
    git -C "$DIR" pull --ff-only
else
    info "Cloning $REPO_URL into $DIR"
    git clone "$REPO_URL" "$DIR"
fi

# --- Agent packages ----------------------------------------------------------

if [ "$MINIMAL" = 0 ]; then
    info "Cloning the recommended agent packages"
    for name in $RECOMMENDED_AGENTS; do
        if [ -d "$DIR/custom_agents/$name/.git" ]; then
            printf '  %s: updating existing clone\n' "$name"
            git -C "$DIR/custom_agents/$name" pull --ff-only
        else
            git clone "https://github.com/modular-agent/$name.git" "$DIR/custom_agents/$name"
        fi
    done

    if [ -f "$DIR/apps/$TARGET/ma-config.toml" ]; then
        info "Building the configurator and applying the existing agent selection"
        MA_CONFIG_FLAG=--apply
    else
        info "Building the configurator and selecting the default agent set (first compile, a few minutes)"
        MA_CONFIG_FLAG=--defaults
    fi
    (cd "$DIR" && cargo run --manifest-path tools/ma-config/Cargo.toml -- "$TARGET" "$MA_CONFIG_FLAG")
fi

# --- Build -------------------------------------------------------------------

if [ "$TARGET" = cli ]; then
    info "Building and installing the ma CLI (typically 10-20 minutes on a first build)"
    cargo install --path "$DIR/apps/cli" --locked
    info "Done. The ma binary is on your PATH (via ~/.cargo/bin)."
else
    info "Building the desktop app (typically 20-40 minutes and ~10 GB of disk on a first build)"
    (cd "$DIR/apps/desktop" && npm install && npm run tauri build)
    info "Done. Launch the app, or use the installer to install it:"
    case "$OS" in
    Darwin)
        printf '  App:       %s/target/release/bundle/macos/*.app\n' "$DIR"
        printf '  Installer: %s/target/release/bundle/dmg/*.dmg\n' "$DIR"
        ;;
    *)
        printf '  Executable: %s/target/release/modular-agent-desktop\n' "$DIR"
        printf '  Packages:   %s/target/release/bundle/ (deb, rpm, AppImage)\n' "$DIR"
        ;;
    esac
fi

printf '\nNext: build your first patch: %s/first-patch/\n' "$DOCS_URL"
if [ "$MINIMAL" = 0 ]; then
    printf 'To change the agent package selection: %s/installation/#adding-agent-packages\n' "$DOCS_URL"
fi
