//! The two applications ma-config can configure.
//!
//! Everything that used to differ between the desktop and CLI forks of this
//! tool is expressed here as data hanging off `AppKind`.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum AppKind {
    Desktop,
    Cli,
}

pub const ALL_APPS: [AppKind; 2] = [AppKind::Desktop, AppKind::Cli];

impl AppKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "desktop" => Some(AppKind::Desktop),
            "cli" => Some(AppKind::Cli),
            _ => None,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            AppKind::Desktop => "desktop",
            AppKind::Cli => "cli",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            AppKind::Desktop => "Modular Agent Desktop",
            AppKind::Cli => "Modular Agent CLI",
        }
    }

    /// Directory holding the app's `ma-config.toml`, relative to the workspace root.
    fn app_dir(self) -> &'static str {
        match self {
            AppKind::Desktop => "apps/desktop",
            AppKind::Cli => "apps/cli",
        }
    }

    /// Directory of the app's Cargo package, relative to the workspace root.
    fn crate_dir(self) -> &'static str {
        match self {
            AppKind::Desktop => "apps/desktop/src-tauri",
            AppKind::Cli => "apps/cli",
        }
    }

    /// Relative prefix that leads from the app's package back to the workspace root.
    pub fn crate_dir_to_root(self) -> &'static str {
        match self {
            AppKind::Desktop => "../../..",
            AppKind::Cli => "../..",
        }
    }

    pub fn config_path(self, root: &Path) -> PathBuf {
        root.join(self.app_dir()).join("ma-config.toml")
    }

    pub fn manifest_path(self, root: &Path) -> PathBuf {
        root.join(self.crate_dir()).join("Cargo.toml")
    }

    pub fn agents_rs_path(self, root: &Path) -> PathBuf {
        root.join(self.crate_dir()).join("src/agents.rs")
    }

    pub fn main_rs_path(self, root: &Path) -> PathBuf {
        root.join(self.crate_dir()).join("src/main.rs")
    }

    /// Features both apps need from modular-agent-core. The flow-editing MCP
    /// server is toggled at runtime, so it has to be compiled in either way.
    pub fn core_features(self) -> &'static [&'static str] {
        &["mcp-server"]
    }

    /// Only the desktop app links the Tauri plugin.
    pub fn needs_plugin(self) -> bool {
        matches!(self, AppKind::Desktop)
    }

    /// The CLI declares its generated agent module in main.rs; the desktop app
    /// declares it in lib.rs, which is not generated.
    pub fn needs_mod_agents(self) -> bool {
        matches!(self, AppKind::Cli)
    }

    pub fn build_hint(self) -> &'static str {
        match self {
            AppKind::Desktop => "Run `npm run tauri dev` or `npm run tauri build` in apps/desktop.",
            AppKind::Cli => "Run `cargo build -p modular-agent-cli` to build the CLI.",
        }
    }
}
