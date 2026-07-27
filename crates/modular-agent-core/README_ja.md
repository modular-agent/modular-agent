<div align="center">

<img alt="Modular Agent" width="150" height="150" src="https://raw.githubusercontent.com/modular-agent/modular-agent/main/crates/modular-agent-core/doc/images/Square150x150Logo.png">
<br/>

<img alt="modular-agent-core" height="40" src="https://raw.githubusercontent.com/modular-agent/modular-agent/main/crates/modular-agent-core/doc/images/modular_agent_core_title.svg">
<br/>
<br/>

![Language](https://img.shields.io/github/languages/top/modular-agent/modular-agent)
[![Crates.io](https://img.shields.io/crates/v/modular-agent-core.svg)](https://crates.io/crates/modular-agent-core)
[![Documentation](https://docs.rs/modular-agent-core/badge.svg)](https://docs.rs/modular-agent-core)
[![License](https://img.shields.io/crates/l/modular-agent-core.svg)](https://github.com/modular-agent/modular-agent#license)

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
modular-agent-core = "0.26"
```

デフォルト Feature を無効にする場合:

```toml
[dependencies]
modular-agent-core = { version = "0.26", default-features = false, features = ["llm"] }
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
| `mcp-server` | 無効       | 内蔵 MCP サーバー（`file` を含む）              |
| `test-utils` | 無効       | テストユーティリティ                            |

## 外部エージェントによる編集（MCP サーバー）

`mcp-server` feature を有効にすると、ホストアプリケーションは実行中の `ModularAgent` を localhost の MCP エンドポイントとして公開でき、Claude Code などの外部 AI エージェントが自然言語からエージェント定義の参照、プリセットの構築・編集、実行中フローの動作確認を行えるようになります。

```toml
modular-agent-core = { version = "0.26", features = ["mcp-server"] }
```

```rust
use modular_agent_core::mcp_server::{McpServerConfig, start_mcp_server};

// http://127.0.0.1:8765/mcp で streamable HTTP を提供（localhost のみ）。
let handle = start_mcp_server(
    ma.clone(),
    McpServerConfig {
        port: 8765,
        // save_preset ツールの保存先ルート。None なら保存不可。
        presets_dir: Some("/path/to/presets".into()),
        // 必須の Bearer トークン。None なら認証なし。
        token: Some("secret".into()),
    },
)
.await?;
// ...
handle.stop().await;
```

Claude Code からの接続:

```bash
claude mcp add --transport http modular-agent http://127.0.0.1:8765/mcp \
    --header "Authorization: Bearer secret"
```

たとえば次のように依頼します:

> Slack チャンネルを listen して、メッセージを Chat エージェントに送り、返答をチャンネルに投稿するフローを作って

外部エージェントは通常、`list_agent_definitions` でカタログを取得したあと、`create_preset` → `add_agent` ×4（Slack Listener / Slack To Message / Chat / Slack Post）→ `add_connection` ×3 → `save_preset` の順にツールを呼びます。さらに `start_preset` で実行し、`write_external_input` でテスト値を投入して `get_external_outputs` / `get_agent_errors` をポーリングすれば、フローをエンドツーエンドで動作確認できます。両ポーリングツールは `latest_seq`（そのレスポンスで返した最後のレコードの seq）を返し、次の呼び出しで `since_seq` として渡すと新しいレコードだけを受け取れます。`dropped > 0` はイベントコレクタが broadcast ストリームに追いつけず、一部のイベントをキャプチャできなかったことを示します。なお、キャプチャバッファ自体は種別ごとに最新 200 レコードのみ保持するため、ポーリングが間に合わなかったレコードは `dropped` に反映されずに押し出されることがあります。構造変更は `ModularAgentEvent::PresetStructureChanged` を emit するため、ホスト（modular-agent-desktop など）は UI をライブ更新できます。ツールは全 17 種で、定義参照・プリセット CRUD・エージェント/接続編集・設定更新・start/stop・実行時検証をカバーします。

サーバーは `127.0.0.1` のみにバインドします。`token` を設定した場合、すべてのリクエストに `Authorization: Bearer <token>` ヘッダーが必須で、ない場合は 401 で拒否されます。トークンなしでは認証がないため、有効化は明示的に行ってください。`modular-agent-desktop` では Settings → Core から（トークンは自動生成）、`modular-agent-cli` では `--mcp-port <PORT>` と `--mcp-token <TOKEN>` フラグで有効化します。

## ドキュメント

API ドキュメントは [docs.rs/modular-agent-core](https://docs.rs/modular-agent-core) で公開されています。

## 関連リポジトリ

### アプリケーション

- [modular-agent-desktop](https://github.com/modular-agent/modular-agent/tree/main/apps/desktop) - ビジュアル Preset エディタ (Tauri 2 + Svelte 5)

### エージェントライブラリ — 汎用

- [modular-agent-std](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-std) - 標準ユーティリティエージェント (50+)
- [modular-agent-llm](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-llm) - OpenAI, Ollama 連携

### エージェントライブラリ — データソース

- [modular-agent-lifelog](https://github.com/modular-agent/modular-agent-lifelog) - スクリーンキャプチャ、ウィンドウトラッキング
- [modular-agent-slack](https://github.com/modular-agent/modular-agent-slack) - Slack メッセージング
- [modular-agent-web](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-web) - HTTP、スクレイピング、YouTube

### エージェントライブラリ — データベース

- [modular-agent-duckdb](https://github.com/modular-agent/modular-agent-duckdb) - DuckDB 分析
- [modular-agent-lancedb](https://github.com/modular-agent/modular-agent-lancedb) - ベクトルデータベース
- [modular-agent-mongodb](https://github.com/modular-agent/modular-agent-mongodb) - MongoDB CRUD
- [modular-agent-sqlx](https://github.com/modular-agent/modular-agent-sqlx) - SQLite, MySQL, PostgreSQL
- [modular-agent-surrealdb](https://github.com/modular-agent/modular-agent-surrealdb) - SurrealDB グラフDB

### プラグイン

- [tauri-plugin-modular-agent](https://github.com/modular-agent/modular-agent/tree/main/crates/tauri-plugin-modular-agent) - Tauri プラグインブリッジ

## ライセンス

[Apache License, Version 2.0](../../LICENSE_APACHE-2.0) の下でライセンスされています。
