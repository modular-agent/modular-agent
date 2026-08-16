<div align="center">

<img alt="logo" width="150" height="150" src="src-tauri/icons/Square150x150Logo.png">

<br>

<img alt="Modular Agent" width="343" height="60" src="doc/images/modular_agent_title.svg">

<br>

![Developer Preview](https://img.shields.io/badge/Status-Developer_Preview-orange)
<!-- [![GitHub release](https://img.shields.io/github/v/release/modular-agent/modular-agent?style=flat)](https://github.com/modular-agent/modular-agent/releases) -->
<!-- [![GitHub downloads](https://img.shields.io/github/downloads/modular-agent/modular-agent/total?style=flat)](https://github.com/modular-agent/modular-agent/releases) -->

![Tauri 2](https://img.shields.io/badge/Tauri_2-24C8D8?logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-DEA584?logo=rust&logoColor=white)
![Svelte 5](https://img.shields.io/badge/Svelte_5-FF3E00?logo=svelte&logoColor=white)
![Windows](https://img.shields.io/badge/-Windows-0078D4?logo=windows&logoColor=white)
![macOS](https://img.shields.io/badge/-macOS-000000?logo=apple&logoColor=white)
![Linux](https://img.shields.io/badge/-Linux-FCC624?logo=linux&logoColor=black)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE_APACHE-2.0)

</div>

モジュラーシンセのようにAIワークフローを組み上げる — 拡張可能なエージェントをビジュアルにパッチングし、リアルタイムに動き続けるパイプラインを構築。LLM、データベース、Webスクレイピング、メッセージングなど。プライバシーファースト、クラウド不要。

[English](README.md) | [日本語](README_ja.md)

<div align="center">
<img alt="Workflow Editor" width="800" src="doc/images/screenshot_editor.jpg">
</div>

## 特徴

### エージェント

- ⚡ **ストリームベースのデータフロー** — エージェント間のリアルタイムデータストリーミング
- 🤖 **ビルトインエージェント** — LLM、Web/HTTP、Slack、SQLデータベース、スクリーンキャプチャなど
- 🧩 **拡張可能** — Rust crateでエージェントプラグインを追加

### ランタイム

- 🏠 **ローカル実行** — すべての処理はローカルマシン上で完結。クラウド不要
- 💻 **クロスプラットフォーム** — Windows、macOS、Linux
- 📦 **組み込み可能** — コアランタイム（[modular-agent-core](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-core)）は依存関係を最小限に抑えており、さまざまなアプリに組み込んでパッチを実行できる

### エディタ

- 🎨 **ビジュアルワークフローエディタ** — ノードベースのドラッグ＆ドロップでエージェントパイプラインを設計
- 💾 **パッチ管理** — ワークフロー設定の保存、読み込み、インポート/エクスポート
- 🚀 **自動起動** — アプリ起動時に実行するパッチを設定可能
- 🔲 **システムトレイ** — バックグラウンドでワークフローを実行

## はじめに

> **開発者リリース** - ビルド済みバイナリはまだ提供されていません。

### 前提条件

- [Rust](https://www.rust-lang.org/tools/install) 1.85+
- [Node.js](https://nodejs.org/) 20+
- [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)（プラットフォーム固有の依存関係）

### ビルド

monorepo のチェックアウト内の `apps/desktop` で:

```bash
npm install              # 依存パッケージのインストール
npm run tauri build      # プロダクションビルド
```

Cargo の成果物はリポジトリルートの workspace 共通 `target/` に出力されます:

- **実行ファイル** - `target/release/modular-agent-desktop.exe`（Windows）/ `modular-agent-desktop`（macOS/Linux）
- **インストーラー** - `target/release/bundle/nsis/*-setup.exe`（Windows）/ `dmg/*.dmg`（macOS）/ `deb/*.deb`（Linux）

### 実行

1. Modular Agentを起動
2. パッチを開くか、新規作成
3. キャンバス上で右クリック→「Add agent」、またはダブルクリックでエージェント一覧を表示
4. ポート間をドラッグして接続
5. 右クリック→「Play」（または `Ctrl+.` / `Cmd+.`）でワークフローを開始

## 技術概要

### 仕組み

1. **パッチ** はワークフロー設定 — 接続されたエージェントのグラフ
2. **エージェント** は処理ユニット（例: 「Chat Completion」「HTTP Request」「Text Template」）
3. 各エージェントには **入力/出力ポート** があり、ポート間をドラッグして接続
4. **Play** を押すとパイプラインが開始 — データがエージェント間をリアルタイムで流れる

### アーキテクチャ

- **フロントエンド** - [SvelteKit](https://svelte.dev/docs/kit/)（静的アダプタ）+ [Svelte 5](https://svelte.dev/)、[TypeScript](https://www.typescriptlang.org/)、[Tailwind CSS](https://tailwindcss.com/)、[Svelte Flow](https://svelteflow.dev/)、[shadcn-svelte](https://www.shadcn-svelte.com/)
- **バックエンド** - [Rust](https://www.rust-lang.org/) + [Tauri 2](https://v2.tauri.app/)
- **コア** - [`modular-agent-core`](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-core) エージェントランタイム

```text
src/                    # Svelteフロントエンド
  routes/               # SvelteKitページ
    patch_editor/      # ビジュアルワークフローエディタ
    settings/           # アプリ設定
  lib/
    components/         # UIコンポーネント
src-tauri/src/          # Rustバックエンド
```

### 関連プロジェクト

- [modular-agent-core](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-core) - Modular Agentコアランタイム
- [tauri-plugin-modular-agent](https://github.com/modular-agent/modular-agent/tree/main/crates/tauri-plugin-modular-agent) - Tauriプラグイン

## エージェントプラグイン

標準ビルドには、このリポジトリ内にあるエージェントcrateが組み込まれています:

| Crate | 説明 |
| ----- | ---- |
| [modular-agent-std](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-std) | 標準ユーティリティエージェント |
| [modular-agent-llm](https://github.com/modular-agent/modular-agent/tree/main/crates/modular-agent-llm) | LLM連携（OpenAI、Anthropic、Google、Ollama） |

### カスタムビルド

さらに多くのエージェントパッケージ — Webスクレイピング、Slack / Mattermost メッセージング、SQLデータベース、スクリーンキャプチャ、スクリプトエージェントなど — は各自のリポジトリにあり、ビルド時に追加できます:

1. 使いたいエージェントリポジトリをリポジトリルートの `custom_agents/` に clone します。リポジトリ一覧と詳細は [custom_agents/README.md](../../custom_agents/README.md) を参照してください。
2. **ma-config** TUIウィザードを実行し、エージェントとクレートごとの feature を選択します（clone 済みのエージェントだけが表示されます）:

   ```bash
   cargo run --manifest-path ../../tools/ma-config/Cargo.toml -- desktop
   ```

   選択内容は `apps/desktop/ma-config.toml` に保存され、次回以降に再利用されます。

3. `npm run tauri dev` または `npm run tauri build` でリビルドします。

## コントリビューション

- ⭐ **スターで応援する** — プロジェクトを広めるのに役立ちます
- 🤝 PR歓迎 — [CONTRIBUTING.md](../../CONTRIBUTING.md) を参照

## ライセンス

このプロジェクトは Apache License, Version 2.0 の下でライセンスされています。
