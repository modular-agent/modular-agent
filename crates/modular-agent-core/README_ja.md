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

[English](https://github.com/modular-agent/modular-agent/blob/main/crates/modular-agent-core/README.md) | [日本語](https://github.com/modular-agent/modular-agent/blob/main/crates/modular-agent-core/README_ja.md)

</div>

ストリームベースのメッセージオーケストレーションによるモジュラーマルチエージェントシステムを構築するための Rust フレームワークです。

## 概要

modular-agent-core は [Modular Agent](https://github.com/modular-agent/modular-agent) プロジェクトのオーケストレーションエンジンです。エージェントは **Patch** — JSON で定義される、接続されたエージェントのグラフ — として配線され、値はその中を非同期にストリームします。このクレートはランタイム（エージェントのライフサイクル、メッセージルーティング、パッチのロード）とエージェント定義用の `#[modular_agent]` マクロを提供します。依存関係を意図的に最小限に抑えており、CLI ツール、デスクトップアプリ、サーバーに組み込めます。

エージェントの実装は別クレートにあります: 同一リポジトリ内の [modular-agent-std](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-std)（ユーティリティ）と [modular-agent-llm](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-llm)（LLM 連携）、および各自のリポジトリで増え続ける[エージェントライブラリ](https://github.com/modular-agent/modular-agent/blob/main/README_ja.md#エージェントライブラリ)群です。[Modular Agent デスクトップアプリ](https://github.com/modular-agent/modular-agent/tree/main/apps/desktop)は、このクレートの上に構築されたビジュアルエディタです。

## インストール

```toml
[dependencies]
modular-agent-core = "0.29"
```

デフォルト Feature を無効にする場合:

```toml
[dependencies]
modular-agent-core = { version = "0.29", default-features = false, features = ["llm"] }
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

    // 3. Patch を読み込み・開始
    let patch_id = ma.open_patch_from_file("patch.json", None).await?;
    ma.start_patch(&patch_id).await?;

    // 4. 入力を送信・出力を受信
    ma.write_external_input("input".into(), AgentValue::string("hello")).await?;
    if let Some(value) = rx.recv().await {
        println!("Output: {:?}", value);
    }

    // 5. クリーンアップ
    ma.stop_patch(&patch_id).await?;
    ma.quit();
    Ok(())
}
```

## 概念

### ModularAgent

`ModularAgent` はオーケストレータです。`init()` がバイナリに登録されたすべてのエージェント定義を収集し、`ready().await` でランタイムが起動します。パッチは `open_patch_from_file` で読み込み（プログラムからの構築も可能）、`start_patch` / `stop_patch` で制御します。`write_external_input` で値を投入し、`subscribe_to_event` でエンジンが emit するすべてのイベント（外部出力、エージェントエラー、構造変更）を観測し、`quit()` でランタイムを終了します。

### 定義（Definition）とスペック（Spec）

**`AgentDefinition`** は `#[modular_agent]` マクロが生成・登録する設計図です: kind、title、description、category、UI ヒント、ポート一覧、config spec（各 config キーの型とデフォルト値）。**`AgentSpec`** はパッチ内の定義のインスタンス 1 つです: id、定義を参照する `def_name`、そのインスタンスのポートと config 値、位置などのエディタメタデータ。

古い定義に対して書かれたパッチを開くと、`reconcile_spec()` が各 spec を移行します: 欠けている config キーは現在のデフォルト値で埋められ、定義に存在しなくなったキーは `_` 接頭辞にリネームされ（エージェントは `new()` の中で一度だけ読み取れるため lazy migration が可能。その後は取り除かれる）、ポートと config spec は現在の定義で上書きされます。

### Patch の JSON 形式

パッチは `agents` と `connections` を持つ JSON ファイルです。以下は外部入力を外部出力へそのまま流す [`examples/patches/echo.json`](https://github.com/modular-agent/modular-agent/blob/main/crates/modular-agent-core/examples/patches/echo.json) です:

```jsonc
{
  "id": "echo",
  "name": "Echo",
  "agents": [
    {
      "id": "in", // パッチ内ローカルなエージェント id。connections から参照される
      "def_name": "modular_agent_core::external_agent::ExternalInputAgent",
      "outputs": ["value"], // このエージェントの出力ポート
      "configs": { "name": "input" } // このインスタンスの config 値
    },
    {
      "id": "out",
      "def_name": "modular_agent_core::external_agent::ExternalOutputAgent",
      "inputs": ["value"], // このエージェントの入力ポート
      "configs": { "name": "output" }
    }
  ],
  "connections": [
    {
      "source": "in", // 送信元エージェント id
      "source_handle": "value", // 送信元の出力ポート
      "target": "out", // 送信先エージェント id
      "target_handle": "value" // 送信先の入力ポート
    }
  ]
}
```

### ポートと `config:` ハンドル

接続は通常入力ポートを指しますが、`target_handle` を `config:<key>` の形にすると、値は送信先の **config** に流し込まれます — config 値を静的に設定する代わりに、グラフが実行時に計算できるということです。たとえば String Join エージェントに区切り文字を与える場合:

```json
{
  "source": "sep_input",
  "source_handle": "value",
  "target": "join",
  "target_handle": "config:sep"
}
```

### AgentValue

`AgentValue` は接続を流れる値の型です。クローンは軽量です — 大きなペイロードは `Arc` 越しに持ち、コレクションはイミュータブル（`im`）な構造です。

| 変種 | 内容 |
| --- | --- |
| `Unit` | 空の値。トリガ信号として使う |
| `Boolean` | `bool` |
| `Integer` | `i64` |
| `Number` | `f64` |
| `String` | UTF-8 文字列 |
| `Image` | 画像データ（`image` feature） |
| `Array` | 値の順序付き配列 |
| `Object` | 文字列キーの値マップ |
| `Tensor` | `f32` テンソル。埋め込みなどに |
| `Message` | LLM チャットメッセージ（`llm` feature） |
| `Error` | 値として運ばれる `AgentError` |

### AgentContext

外部トリガごとに `AgentContext` が 1 つ生成され、そこから生じた値とともに移動して、エージェントをまたぐ 1 つのフローをエンドツーエンドで識別します。パッチスコープの変数、ネストした map 操作の分岐系譜を追跡するフレームスタック（フレームごとに index と length）、長時間処理をキャンセルするための任意の `CancellationToken` を運びます。

### 組み込みの外部 I/O エージェント

4 つの組み込みエージェントが、エージェントネットワークと外の世界を橋渡しします:

| エージェント | タイトル | 役割 |
| --- | --- | --- |
| `ExternalInputAgent` | `ExtIn->` | 入口: 設定した `name` 宛の `write_external_input()` の値を転送する |
| `ExternalOutputAgent` | `->ExtOut` | 出口: `ModularAgentEvent::ExternalOutput` を emit する |
| `LocalInputAgent` | `LocalIn->` | パッチスコープのローカル入力 |
| `LocalOutputAgent` | `->LocalOut` | パッチスコープのローカル出力 |

### 登録の仕組み

`#[modular_agent]` マクロは各定義をリンク時に [inventory](https://crates.io/crates/inventory) クレートへ登録し、`ModularAgent::init()` がそれらをすべて収集します。エージェント crate をリンクするだけで（`use` 1 つで十分）そのエージェントが使えるようになります。ここから 1 つの制約が生じます: **依存グラフの中に modular-agent-core はちょうど 1 コピーだけ**存在しなければなりません。2 コピーあると inventory のレジストリが 2 つに分裂し、片方に登録されたエージェントはもう片方から見えなくなります — エージェント crate は同じ core に依存する必要があります（Modular Agent workspace ではパス依存で実現）。

## エージェントを書く

`#[modular_agent]` マクロを付けた struct を定義し、`AsAgent` を実装します:

```rust
use modular_agent_core::{
    AgentContext, AgentData, AgentError, AgentSpec, AgentValue, AsAgent, ModularAgent,
    async_trait, modular_agent,
};

/// Repeats the input string.
///
/// # Ports
/// - Input `input`: String to repeat
/// - Output `output`: The repeated string
///
/// # Configuration
/// - `count`: Number of repetitions
#[modular_agent(
    title = "Repeat",
    category = "Example",
    inputs = ["input"],
    outputs = ["output"],
    integer_config(name = "count", default = 2),
)]
struct RepeatAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for RepeatAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self { data: AgentData::new(ma, id, spec) })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let count = self.configs()?.get_integer_or("count", 2);
        let out = value.as_str().unwrap_or_default().repeat(count as usize);
        self.output(ctx, "output".into(), AgentValue::string(out)).await
    }
}
```

- struct の `///` doc コメントが定義の `description` になり、デスクトップアプリで markdown としてレンダリングされます — ワークフローを配線する人に向けて書いてください。
- config マクロ: `string_config`、`integer_config`、`number_config`、`boolean_config`、`text_config`、`object_config`、`array_config`。
- 任意のライフサイクルメソッド: `start()`、`stop()`、および実行時の config 変更に反応する `configs_changed()`。

参照実装: in-tree の [modular-agent-std](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-std) と [modular-agent-llm](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-llm)、独立したエージェント crate の例として [SlackPostAgent](https://github.com/modular-agent/modular-agent-slack/blob/main/src/agents.rs)。

## パッチを実行する

同梱の CLI example は、stdin/stdout を指定した外部チャンネルにつないでパッチを実行します（デフォルトの `file` feature が必要）:

```bash
cargo run --example cli -- examples/patches/echo.json -i input -o output
```

実用には、このクレートの上に構築されたパッチランナー [`ma` CLI](https://github.com/modular-agent/modular-agent/tree/main/apps/cli) があります:

```bash
echo "Hello" | ma ./patch.json
```

## Feature Flags

| Feature           | デフォルト | 説明                                                  |
| ----------------- | ---------- | ----------------------------------------------------- |
| `file`            | 有効       | Patch のファイル読み込みサポート                      |
| `image`           | 有効       | photon-rs による画像処理                              |
| `llm`             | 有効       | Message / ToolCall 型による LLM 連携                  |
| `mcp`             | 有効       | Model Context Protocol 連携                           |
| `mcp-http-client` | 無効       | リモート MCP サーバー用 streamable HTTP クライアント  |
| `mcp-server`      | 無効       | 内蔵 MCP サーバー（`file` を含む）                    |
| `test-utils`      | 無効       | テストユーティリティ                                  |

## 外部エージェントによる編集（MCP サーバー）

`mcp-server` feature を有効にすると、ホストアプリケーションは実行中の `ModularAgent` を localhost の MCP エンドポイントとして公開でき、Claude Code などの外部 AI エージェントが自然言語からエージェント定義の参照、パッチの構築・編集、実行中フローの動作確認を行えるようになります。

```toml
modular-agent-core = { version = "0.29", features = ["mcp-server"] }
```

```rust
use modular_agent_core::mcp_server::{McpServerConfig, start_mcp_server};

// http://127.0.0.1:8765/mcp で streamable HTTP を提供（localhost のみ）。
let handle = start_mcp_server(
    ma.clone(),
    McpServerConfig {
        port: 8765,
        // save_patch ツールの保存先ルート。None なら保存不可。
        patches_dir: Some("/path/to/patches".into()),
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

サーバーは 17 のツールを公開します:

- **定義参照** — `list_agent_definitions`、`get_agent_definition`
- **パッチ CRUD** — `list_patches`、`create_patch`、`get_patch_spec`、`save_patch`
- **エージェント / 接続編集** — `add_agent`、`update_agent_spec`、`set_agent_configs`、`remove_agent`、`add_connection`、`remove_connection`
- **実行・検証** — `start_patch`、`stop_patch`、`write_external_input`、`get_agent_errors`、`get_external_outputs`

典型的なセッション: `list_agent_definitions` でカタログを取得し、`create_patch` → `add_agent` ×4（Slack Listener / Slack To Message / Chat / Slack Post）→ `add_connection` ×3 → `save_patch`。さらに `start_patch` で実行し、`write_external_input` でテスト値を投入して `get_external_outputs` / `get_agent_errors` をポーリングすれば、フローをエンドツーエンドで動作確認できます。両ポーリングツールは `latest_seq`（そのレスポンスで返した最後のレコードの seq）を返し、次の呼び出しで `since_seq` として渡すと新しいレコードだけを受け取れます。`dropped > 0` はイベントコレクタが broadcast ストリームに追いつけず、一部のイベントをキャプチャできなかったことを示します。なお、キャプチャバッファ自体は種別ごとに最新 200 レコードのみ保持するため、ポーリングが間に合わなかったレコードは `dropped` に反映されずに押し出されることがあります。構造変更は `ModularAgentEvent::PatchStructureChanged` を emit するため、ホスト（modular-agent-desktop など）は UI をライブ更新できます。

サーバーは `127.0.0.1` のみにバインドします。`token` を設定した場合、すべてのリクエストに `Authorization: Bearer <token>` ヘッダーが必須で、ない場合は 401 で拒否されます。トークンなしでは認証がないため、有効化は明示的に行ってください。`modular-agent-desktop` では Settings → Core から（トークンは自動生成）、`modular-agent-cli` では `--mcp-port <PORT>` と `--mcp-token <TOKEN>` フラグで有効化します。

## ドキュメント

- API ドキュメント: [docs.rs/modular-agent-core](https://docs.rs/modular-agent-core)
- プロジェクトドキュメント: [modular-agent.github.io/docs](https://modular-agent.github.io/docs/ja/)

## 関連リポジトリ

### アプリケーション

- [modular-agent-desktop](https://github.com/modular-agent/modular-agent/tree/main/apps/desktop) - ビジュアル Patch エディタ (Tauri 2 + Svelte 5)
- [modular-agent-cli](https://github.com/modular-agent/modular-agent/tree/main/apps/cli) - `ma` コマンドラインパッチランナー

### In-tree エージェントライブラリ

- [modular-agent-std](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-std) - 標準ユーティリティエージェント (50+)
- [modular-agent-llm](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-llm) - LLM 連携（OpenAI、Claude、Ollama）

さらに多くのエージェントライブラリ — Web、メッセージング、メディア、データベース — が各自のリポジトリにあります。[全一覧](https://github.com/modular-agent/modular-agent/blob/main/README_ja.md#エージェントライブラリ)を参照してください。

### プラグイン

- [tauri-plugin-modular-agent](https://github.com/modular-agent/modular-agent/tree/main/crates/tauri-plugin-modular-agent) - Tauri プラグインブリッジ

## License

[Apache License, Version 2.0](https://github.com/modular-agent/modular-agent/blob/main/LICENSE_APACHE-2.0) の下でライセンスされています。
