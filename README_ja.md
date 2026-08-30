<div align="center">

<img alt="Modular Agent" width="150" height="150" src="https://raw.githubusercontent.com/modular-agent/modular-agent/main/crates/modular-agent-core/doc/images/Square150x150Logo.png">
<br/>
<br/>

<img alt="Modular Agent" width="343" height="60" src="https://raw.githubusercontent.com/modular-agent/modular-agent/main/apps/desktop/doc/images/modular_agent_title.svg">
<br>
<br>

![Developer Preview](https://img.shields.io/badge/Status-Developer_Preview-orange)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE_APACHE-2.0)
[![Crates.io](https://img.shields.io/crates/v/modular-agent-core.svg)](https://crates.io/crates/modular-agent-core)
[![Documentation](https://docs.rs/modular-agent-core/badge.svg)](https://docs.rs/modular-agent-core)

![Tauri 2](https://img.shields.io/badge/Tauri_2-24C8D8?logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-DEA584?logo=rust&logoColor=white)
![Svelte 5](https://img.shields.io/badge/Svelte_5-FF3E00?logo=svelte&logoColor=white)
![Windows](https://img.shields.io/badge/-Windows-0078D4?logo=windows&logoColor=white)
![macOS](https://img.shields.io/badge/-macOS-000000?logo=apple&logoColor=white)
![Linux](https://img.shields.io/badge/-Linux-FCC624?logo=linux&logoColor=black)

[English](README.md) | [日本語](README_ja.md)

</div>

モジュラーシンセのように AI ワークフローを組み上げる — 拡張可能なエージェントをビジュアルにパッチングし、リアルタイムに動き続けるパイプラインを構築。LLM、データベース、Web スクレイピング、メッセージングなど。プライバシーファースト、クラウド不要。

<div align="center">
<img alt="Workflow Editor" width="800" src="https://raw.githubusercontent.com/modular-agent/modular-agent/main/apps/desktop/doc/images/screenshot_editor.jpg">
</div>

Modular Agent は、シンセサイザーのモジュールを配線するように AI エージェントを組み合わせてワークフローを作る、デスクトップアプリ + Rust フレームワークです。キャンバスにエージェントを置き、ポート同士を接続して Run スイッチを入れると、値がグラフをリアルタイムにストリームします。パッチは一度きりのスクリプトではなく、動き続けるパイプラインです。すべての処理はローカルマシン上で実行され、LLM エンドポイントやデータベース、メッセージングサービスへは、パッチに組み込んだときだけアクセスします。

## 仕組み

- **Patch** — JSON として保存されるワークフロー。エージェントの集合と、それらをつなぐ接続からなる
- **Agent** — エージェント定義から作られる処理ユニット。名前付きの入出力ポートと config を持つ
- **Connection** — 出力ポートと入力ポートをつなぐ（docs では「ワイヤー」と表記）。特別なハンドル `config:<key>` を指定すると、値をエージェントの config に流し込める
- 値（`AgentValue`）はグラフを非同期にストリームし、外部トリガごとに生成される `AgentContext` が、エージェントをまたいで同一フローを識別する

実行中のパッチは、内蔵の [MCP サーバー](crates/modular-agent-core/README_ja.md#外部エージェントによる編集mcp-サーバー)を通じて、外部の AI エージェント（Claude Code など）からライブに参照・編集することもできます。

概念の詳細は [core README](crates/modular-agent-core/README_ja.md)、最初のパッチの作り方は[ドキュメントサイト](https://modular-agent.github.io/docs/ja/)を参照してください。

## クイックインストール

[Tauri の前提条件](https://v2.tauri.app/start/prerequisites/)（Rust、Node.js、プラットフォームのツールチェーン）が揃っていれば、1 コマンドでリポジトリのクローンからソースビルドまで行えます。デスクトップアプリと `ma` CLI のどちらをビルドするか、[推奨エージェントパッケージ](custom_agents/README.md#recommended-agent-repositories)を含めるかは対話で選べます。デスクトップアプリの初回ビルドは 20〜40 分・約 10 GB のディスクを使います:

```bash
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/modular-agent/modular-agent/main/scripts/install.sh | sh
```

```powershell
# Windows (PowerShell)
irm https://raw.githubusercontent.com/modular-agent/modular-agent/main/scripts/install.ps1 | iex
```

アップデートするには、同じディレクトリで同じコマンドをもう一度実行してください。既存のクローンを pull し、同じ構成でリビルドします。

手動でのビルド手順とエージェント選択の変更は[インストールガイド](https://modular-agent.github.io/docs/ja/getting-started/installation/)を参照してください。

## ドキュメント

> **Developer Preview** — ビルド済みバイナリはまだ提供されていません。docs ではソースからのビルド手順を案内しています。

- **[ドキュメントサイト](https://modular-agent.github.io/docs/ja/)** — [インストール](https://modular-agent.github.io/docs/ja/getting-started/installation/)、[はじめてのパッチ](https://modular-agent.github.io/docs/ja/getting-started/first-patch/)、[Chat エージェントを使う](https://modular-agent.github.io/docs/ja/getting-started/chat-patch/)
- [デスクトップアプリ](apps/desktop) — ビジュアルパッチエディタ
- [`ma` CLI](apps/cli) — コマンドラインでパッチを実行
- [modular-agent-core](crates/modular-agent-core) — 組み込み可能なライブラリとしてのエンジン（[crates.io](https://crates.io/crates/modular-agent-core) / [docs.rs](https://docs.rs/modular-agent-core)）
- [tauri-plugin-modular-agent](crates/tauri-plugin-modular-agent) — 自作の Tauri アプリにエンジンを組み込む
- [エージェントライブラリ](#エージェントライブラリ) — パッチに組み込めるものの一覧。ビルド方法は [custom_agents/README.md](custom_agents/README.md)
- [CONTRIBUTING.md](CONTRIBUTING.md) / [GitHub Discussions](https://github.com/orgs/modular-agent/discussions)

## 構成

| Path | App / Crate | 説明 |
|---|---|---|
| [`apps/desktop`](apps/desktop) | `modular-agent-desktop` | ビジュアルワークフローエディタ（Tauri 2 + Svelte 5） |
| [`apps/cli`](apps/cli) | `modular-agent-cli` | `ma` コマンドラインパッチランナー |
| [`crates/modular-agent-core`](crates/modular-agent-core) | `modular-agent-core` | オーケストレーションエンジン、エージェントランタイム、パッチローダ |
| [`crates/modular-agent-macros`](crates/modular-agent-macros) | `modular-agent-macros` | `#[modular_agent]` 手続きマクロ |
| [`crates/modular-agent-std`](crates/modular-agent-std) | `modular-agent-std` | 標準ユーティリティエージェント |
| [`crates/modular-agent-llm`](crates/modular-agent-llm) | `modular-agent-llm` | OpenAI / Claude / Ollama エージェント |
| [`crates/tauri-plugin-modular-agent`](crates/tauri-plugin-modular-agent) | `tauri-plugin-modular-agent` | Tauri プラグインブリッジ（Rust + guest-js） |
| [`tools/ma-config`](tools/ma-config) | `ma-config` | エージェント選択 / ビルド設定 TUI |

## エージェントライブラリ

エージェントはパッケージ単位で提供されます。`std` と `llm` はこのリポジトリにあり、すべてのビルドに含まれます。それ以外は [github.com/modular-agent](https://github.com/modular-agent) 配下の各リポジトリにあり、使いたいものを `custom_agents/` に clone して ma-config ウィザードで選択します。詳細は [custom_agents/README.md](custom_agents/README.md) を参照してください。

| カテゴリ | パッケージ | エージェント |
|---|---|---|
| In-tree | [modular-agent-std](crates/modular-agent-std) | 標準ユーティリティ: 配列、文字列、テンプレート、ファイル、タイマー、フィルタ（50+） |
| In-tree | [modular-agent-llm](crates/modular-agent-llm) | LLM 連携: OpenAI、Claude、Ollama |
| 汎用 | [modular-agent-web](https://github.com/modular-agent/modular-agent-web) | Web/HTTP、スクレイピング、検索、YouTube |
| 汎用 | [modular-agent-monty](https://github.com/modular-agent/modular-agent-monty) | Monty スクリプトエージェント |
| 汎用 | [modular-agent-zapcode](https://github.com/modular-agent/modular-agent-zapcode) | ZapCode TypeScript スクリプトエージェント |
| メッセージング | [modular-agent-slack](https://github.com/modular-agent/modular-agent-slack) | Slack メッセージング |
| メッセージング | [modular-agent-mattermost](https://github.com/modular-agent/modular-agent-mattermost) | Mattermost メッセージング |
| データ / メディア | [modular-agent-lifelog](https://github.com/modular-agent/modular-agent-lifelog) | スクリーンキャプチャ、ウィンドウトラッキング |
| データベース | [modular-agent-sqlx](https://github.com/modular-agent/modular-agent-sqlx) | SQL データベース（PostgreSQL、MySQL、SQLite） |

この organization のものに限らず、エージェント crate を持つリポジトリなら何でも `custom_agents/` に clone すればウィザードが認識します。

## コントリビューション

- ⭐ **スターで応援する** — プロジェクトを広めるのに役立ちます
- 🤝 PR 歓迎 — [CONTRIBUTING.md](CONTRIBUTING.md) を参照。質問やアイデアは [GitHub Discussions](https://github.com/orgs/modular-agent/discussions) へ

## License

このプロジェクトは [Apache License, Version 2.0](LICENSE_APACHE-2.0) の下でライセンスされています。
