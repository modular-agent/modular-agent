# Contributing to Modular Agent

Thank you for your interest in contributing to **Modular Agent**.

Modular Agent is a multi-agent workflow system focused on
agent orchestration and reusable agents.

This repository is a monorepo. It holds the engine, the agent libraries every build
needs, both applications, and the build configurator — see the table in
[README.md](README.md) for the full layout. Agent libraries that only some builds need
live in their own repositories under [github.com/modular-agent](https://github.com/modular-agent);
please file issues about those on the repository that owns them.

## How to Contribute

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Open a pull request

Questions, ideas, and anything open-ended go to
[Discussions](https://github.com/orgs/modular-agent/discussions), which are organization
wide and cover every modular-agent repository at once. Issues are for bug reports and
feature requests.

When you open an issue, pick the component the report is about from the dropdown in the
template. Because several release lines share this repository, tags and versions carry a
component prefix — `core-v0.26.0`, `std-v0.16.0`, `desktop-v0.18.0`, `cli-v0.4.0`, and so
on. Quote the one you are running.

## Development

The whole repository is one Cargo workspace, so it builds and tests as a unit:

```sh
cargo check --workspace --all-targets
cargo test --workspace --all-features
```

Building a single package needs `-p`, not `--workspace`. The v2 resolver unifies features
across packages built together, so a `--workspace` release build can enable features an
app does not want:

```sh
cargo build -p modular-agent-cli --release
```

The desktop app runs through its own dev server:

```sh
cd apps/desktop && npm install && npm run tauri dev
```

Rust edition 2024, minimum Rust 1.92.0.

## Formatting

The repository is fully formatted, and pull requests are expected to keep it that way.
Rust uses `cargo fmt`; the JavaScript, TypeScript, Svelte, and CSS under `apps/desktop`
and `crates/tauri-plugin-modular-agent` uses prettier, configured per package. Format only
the files you changed — a repository-wide reformat buries the real diff.

A `pre-commit` hook that formats staged files is available; install it in your clone with:

```sh
cp ~/.git-hooks/pre-commit .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
```

Formatting-only commits are listed in `.git-blame-ignore-revs`. Enable it once per clone
with `git config blame.ignoreRevsFile .git-blame-ignore-revs`.

## License

Contributions are accepted under the Apache License 2.0, the license this project ships
under. See [LICENSE_APACHE-2.0](LICENSE_APACHE-2.0).

Thank you for helping improve this project.
