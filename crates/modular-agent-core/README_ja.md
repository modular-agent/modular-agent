<div align="center">

<img alt="Modular Agent" width="150" height="150" src="https://raw.githubusercontent.com/modular-agent/modular-agent-core/main/doc/images/Square150x150Logo.png">
<br/>

<img alt="modular-agent-core" height="40" src="https://raw.githubusercontent.com/modular-agent/modular-agent-core/main/doc/images/modular_agent_core_title.svg">
<br/>
<br/>

![Language](https://img.shields.io/github/languages/top/modular-agent/modular-agent-core)
[![Crates.io](https://img.shields.io/crates/v/modular-agent-core.svg)](https://crates.io/crates/modular-agent-core)
[![Documentation](https://docs.rs/modular-agent-core/badge.svg)](https://docs.rs/modular-agent-core)
[![License](https://img.shields.io/crates/l/modular-agent-core.svg)](https://github.com/modular-agent/modular-agent-core#license)

[English](README.md) | [日本語](README_ja.md)

</div>

ストリームベースのメッセージオーケストレーションによるモジュラーマルチエージェントシステムを構築するための Rust フレームワークです。

## 特徴

### エージェント

- **ストリームベースのデータフロー** — エージェント間のリアルタイムデータストリーミング
- **豊富なビルトインエージェント** — LLM、Web/HTTP、Slack、SQL データベース、スクリーンキャプチャなど（[エージェントライブラリ](#関連リポジトリ)経由）
- **拡張可能** — Rust クレートでエージェントプラグインを追加

### ランタイム

- **ローカル実行** — すべての処理はローカルマシン上で実行。クラウド依存なし
- **クロスプラットフォーム** — Windows, macOS, Linux
- **組み込み可能** — 最小限の依存関係。CLI ツール、デスクトップアプリ、サーバーなど任意の Rust アプリケーションに組み込み可能

## 概要

modular-agent-core は、複数のエージェントをオーケストレーションするための非同期・ストリームベースのアーキテクチャを提供します。エージェントはメッセージパッシングで通信し、Preset を使ってネットワークとして構成できます。このクレートは最小限の依存関係を持つコアライブラリであり、個々のエージェント実装は別パッケージとして提供されています。

## インストール

```toml
[dependencies]
modular-agent-core = "0.23"
```

デフォルト Feature を無効にする場合:

```toml
[dependencies]
modular-agent-core = { version = "0.23", default-features = false, features = ["llm"] }
```

## クイックスタート

```rust
use modular_agent_core::{AgentError, AgentValue, ModularAgent, ModularAgentEvent};

#[tokio::main]
async fn main() -> Result<(), AgentError> {
    // 1. 初期化
    let ma = ModularAgent::init()?;
    ma.ready().await?;

    // 2. 出力をサブスクライブ（レースコンディション回避のため開始前に行う）
    let mut rx = ma.subscribe_to_event(|event| {
        if let ModularAgentEvent::ExternalOutput(name, value) = event {
            if name == "output" { return Some(value); }
        }
        None
    });

    // 3. Preset を読み込み・開始
    let preset_id = ma.open_preset_from_file("preset.json", None).await?;
    ma.start_preset(&preset_id).await?;

    // 4. 入力を送信・出力を受信
    ma.write_external_input("input".into(), AgentValue::string("hello")).await?;
    if let Some(value) = rx.recv().await {
        println!("Output: {:?}", value);
    }

    // 5. クリーンアップ
    ma.stop_preset(&preset_id).await?;
    ma.quit();
    Ok(())
}
```

## Feature Flags

| Feature      | デフォルト | 説明                                            |
| ------------ | ---------- | ----------------------------------------------- |
| `file`       | 有効       | Preset のファイル読み込みサポート               |
| `image`      | 有効       | photon-rs による画像処理                        |
| `llm`        | 有効       | Message / ToolCall 型による LLM 連携            |
| `mcp`        | 有効       | Model Context Protocol 連携                     |
| `test-utils` | 無効       | テストユーティリティ                            |

## ドキュメント

API ドキュメントは [docs.rs/modular-agent-core](https://docs.rs/modular-agent-core) で公開されています。

## 関連リポジトリ

### アプリケーション

- [modular-agent-desktop](https://github.com/modular-agent/modular-agent-desktop) - ビジュアル Preset エディタ (Tauri 2 + Svelte 5)

### エージェントライブラリ — 汎用

- [modular-agent-std](https://github.com/modular-agent/modular-agent-std) - 標準ユーティリティエージェント (50+)
- [modular-agent-llm](https://github.com/modular-agent/modular-agent-llm) - OpenAI, Ollama 連携

### エージェントライブラリ — データソース

- [modular-agent-lifelog](https://github.com/modular-agent/modular-agent-lifelog) - スクリーンキャプチャ、ウィンドウトラッキング
- [modular-agent-slack](https://github.com/modular-agent/modular-agent-slack) - Slack メッセージング
- [modular-agent-web](https://github.com/modular-agent/modular-agent-web) - HTTP、スクレイピング、YouTube

### エージェントライブラリ — データベース

- [modular-agent-duckdb](https://github.com/modular-agent/modular-agent-duckdb) - DuckDB 分析
- [modular-agent-lancedb](https://github.com/modular-agent/modular-agent-lancedb) - ベクトルデータベース
- [modular-agent-mongodb](https://github.com/modular-agent/modular-agent-mongodb) - MongoDB CRUD
- [modular-agent-sqlx](https://github.com/modular-agent/modular-agent-sqlx) - SQLite, MySQL, PostgreSQL
- [modular-agent-surrealdb](https://github.com/modular-agent/modular-agent-surrealdb) - SurrealDB グラフDB

### プラグイン

- [tauri-plugin-modular-agent](https://github.com/modular-agent/tauri-plugin-modular-agent) - Tauri プラグインブリッジ

## ライセンス

[Apache 2.0](LICENSE-APACHE) または [MIT](LICENSE-MIT) のデュアルライセンスです。
