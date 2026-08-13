# custom_agents

Out-of-tree agent crates live here as plain git clones. Everything in this
directory except this README is ignored by git.

The wizard offers exactly the crates that are cloned here, so cloning is the
first step. Clone into this directory, keeping the repository name as the
directory name:

```sh
cd custom_agents
git clone https://github.com/modular-agent/modular-agent-lifelog.git
```

Then run the wizard for the app that should link it, and the clone becomes a
path dependency of that app — and, through it, a member of this workspace:

```sh
cargo run --manifest-path tools/ma-config/Cargo.toml -- desktop   # or: cli
```

A clone that no app selects is inert; cargo never looks at it. Each agent repo
depends on `modular-agent-core` by path, so there is exactly one linked copy of
core and one `inventory` agent registry.

## Recommended agent repositories

These first-party repositories are a good starting set; each clones from
`https://github.com/modular-agent/<repository>.git`:

| Repository | Agents |
| --- | --- |
| `modular-agent-lifelog` | Screen capture, window tracking agents |
| `modular-agent-mattermost` | Mattermost messaging agents |
| `modular-agent-monty` | Monty script agents |
| `modular-agent-slack` | Slack messaging agents |
| `modular-agent-sqlx` | SQL database agents (PostgreSQL, MySQL, SQLite) |
| `modular-agent-zapcode` | ZapCode TypeScript script agents |

Nothing here is limited to that organization: any repository holding an agent
crate can be cloned into this directory, and the wizard offers every subdirectory
that has a `Cargo.toml`.

## ma-registry.yaml

An agent repository describes itself to the wizard with an `ma-registry.yaml`
at its root — the catalog entry lives with the crate, not in this repository:

```yaml
name: modular-agent-lifelog
description: Screen capture, window tracking agents
available_features:
  - application
  - screen
default_features:
  - application
  - screen
default_for: [desktop]
```

- `name` — must match the directory name of the clone.
- `description` — the one line shown next to the entry in the wizard.
- `available_features` — Cargo features the wizard offers to select.
- `default_features` — the selection a fresh configuration starts from.
- `default_for` — apps (`desktop`, `cli`) that pre-select the crate.
- `conflicts` — crates that cannot be linked together, as
  `- with: <crate>` plus a `reason` and an optional `platform`.

Every field except `name` and `description` is optional. A clone without an
`ma-registry.yaml` still shows up: the wizard falls back to `name` and
`description` from its `Cargo.toml` `[package]`, with no feature selection and
no conflict checks.
