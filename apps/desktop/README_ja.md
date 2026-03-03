<div align="center">

<img alt="logo" width="150" height="150" src="src-tauri/icons/Square150x150Logo.png">

<br>

<img alt="Modular Agent" width="353" height="50" src="doc/images/modular_agent_title.png">

<br>

![Developer Preview](https://img.shields.io/badge/Status-Developer_Preview-orange)
[![Give it a Star](https://img.shields.io/badge/⭐_Give_it_a_Star-F5C518)](https://github.com/modular-agent/modular-agent-desktop)
<!-- [![GitHub release](https://img.shields.io/github/v/release/modular-agent/modular-agent-desktop?style=flat)](https://github.com/modular-agent/modular-agent-desktop/releases) -->
<!-- [![GitHub downloads](https://img.shields.io/github/downloads/modular-agent/modular-agent-desktop/total?style=flat)](https://github.com/modular-agent/modular-agent-desktop/releases) -->

![Tauri 2](https://img.shields.io/badge/Tauri_2-24C8D8?logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-DEA584?logo=rust&logoColor=white)
![Svelte 5](https://img.shields.io/badge/Svelte_5-FF3E00?logo=svelte&logoColor=white)
![Windows](https://img.shields.io/badge/-Windows-0078D4?logo=windows&logoColor=white)
![macOS](https://img.shields.io/badge/-macOS-000000?logo=apple&logoColor=white)
![Linux](https://img.shields.io/badge/-Linux-FCC624?logo=linux&logoColor=black)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE_APACHE-2.0)

</div>

Modular Agentは、AIワークフローをビジュアルに構築するローカルファーストなデスクトップアプリケーションです。80以上のビルド済みエージェント（LLM、データベース、Webスクレイピング、メッセージングなど）を接続してパイプラインを設計し、ワンクリックでローカル実行できます。クラウド不要。

[English](README.md) | [日本語](README_ja.md)

<div align="center">
<img alt="Workflow Editor" width="800" src="doc/images/screenshot_editor.jpg">
</div>

## 特徴

### エディタ

- **ビジュアルワークフローエディタ** — ノードベースのドラッグ＆ドロップでエージェントパイプラインを設計
- **マルチタブ編集** — 複数のプリセットを同時に編集
- **Undo/Redo** — すべてのエディタ操作に対応するコマンドパターン履歴
- **カスタムショートカット** — キー割り当ての変更、Quick Add（Ctrl+1〜5）で素早くエージェント配置
- **ダークモード** — システム連動のカラースキーム

### エージェント

- **80以上のビルトインエージェント** — LLM、Web/HTTP、Slack、SQLデータベース、スクリーンキャプチャなど
- **拡張可能** — Rust crateでエージェントプラグインを追加
- **ストリームベースのデータフロー** — エージェント間のリアルタイムデータストリーミング

### ランタイム

- **ローカル実行** — すべての処理はローカルマシン上で完結。クラウド不要
- **システムトレイ** — バックグラウンドでワークフローを実行
- **クロスプラットフォーム** — Windows、macOS、Linux

### プリセット

- **プリセット管理** — ワークフロー設定の保存、読み込み、インポート/エクスポート
- **自動起動** — アプリ起動時に実行するプリセットを設定可能

## はじめに

> **開発者リリース** - ビルド済みバイナリはまだ提供されていません。

### 前提条件

- [Rust](https://www.rust-lang.org/tools/install) 1.85+
- [Node.js](https://nodejs.org/) 20+
- [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)（プラットフォーム固有の依存関係）

### ビルド・実行

```bash
npm install              # 依存パッケージのインストール
npm run tauri build      # プロダクションビルド
```

ビルド後、実行ファイルをコピーするか、インストーラーを実行してください:

- **実行ファイル** - `src-tauri/target/release/modular-agent-desktop.exe`（Windows）/ `modular-agent-desktop`（macOS/Linux）
- **インストーラー** - `src-tauri/target/release/bundle/msi/*.msi`（Windows）/ `dmg/*.dmg`（macOS）/ `deb/*.deb`（Linux）

### 使い方

1. Modular Agentを起動
2. プリセットを開くか、新規作成
3. キャンバス上で右クリック→「Add agent」、またはダブルクリックでエージェント一覧を表示
4. ポート間をドラッグして接続
5. 右クリック→「Play」（または `Ctrl+.` / `Cmd+.`）でワークフローを開始

### 仕組み

1. **プリセット** はワークフロー設定 — 接続されたエージェントのグラフ
2. **エージェント** は処理ユニット（例: 「Chat Completion」「HTTP Request」「Text Template」）
3. 各エージェントには **入力/出力ポート** があり、ポート間をドラッグして接続
4. **Play** を押すとパイプラインが開始 — データがエージェント間をリアルタイムで流れる

### アーキテクチャ

- **フロントエンド** - [SvelteKit](https://svelte.dev/docs/kit/)（静的アダプタ）+ [Svelte 5](https://svelte.dev/)、[TypeScript](https://www.typescriptlang.org/)、[Tailwind CSS](https://tailwindcss.com/)、[Svelte Flow](https://svelteflow.dev/)、[shadcn-svelte](https://www.shadcn-svelte.com/)
- **バックエンド** - [Rust](https://www.rust-lang.org/) + [Tauri 2](https://v2.tauri.app/)
- **コア** - [`modular-agent-core`](https://github.com/modular-agent/modular-agent-core) エージェントランタイム

```text
src/                    # Svelteフロントエンド
  routes/               # SvelteKitページ
    preset_editor/      # ビジュアルワークフローエディタ
    settings/           # アプリ設定
  lib/
    components/         # UIコンポーネント
src-tauri/src/          # Rustバックエンド
```

## エージェントプラグイン

以下のエージェントcrateが標準で組み込まれています:

| Crate | 説明 |
| ----- | ---- |
| [modular-agent-std](https://github.com/modular-agent/modular-agent-std) | 標準ユーティリティエージェント |
| [modular-agent-llm](https://github.com/modular-agent/modular-agent-llm) | LLM連携（OpenAI、Ollama） |
| [modular-agent-web](https://github.com/modular-agent/modular-agent-web) | HTTP、スクレイピング、YouTube |
| [modular-agent-slack](https://github.com/modular-agent/modular-agent-slack) | Slackメッセージング |
| [modular-agent-sqlx](https://github.com/modular-agent/modular-agent-sqlx) | SQLite、MySQL、PostgreSQL |
| [modular-agent-lifelog](https://github.com/modular-agent/modular-agent-lifelog) | スクリーンキャプチャ、ウィンドウ追跡 |

### 新しいエージェントプラグインの追加

1. `src-tauri/Cargo.toml` に依存関係を追加:

    ```toml
    [dependencies]
    modular-agent-xxx = { git = "https://github.com/modular-agent/modular-agent-xxx.git", tag = "v0.1.0" }
    ```

2. `src-tauri/src/lib.rs` でcrateをインポート:

    ```rust
    #[allow(unused_imports)]
    use modular_agent_xxx;
    ```

3. アプリケーションを再ビルド。

## 関連プロジェクト

- [modular-agent-core](https://github.com/modular-agent/modular-agent-core) - Modular Agentコアランタイム
- [tauri-plugin-modular-agent](https://github.com/modular-agent/tauri-plugin-modular-agent) - Tauriプラグイン

## コントリビューション

- [GitHub Issues](https://github.com/modular-agent/modular-agent-desktop/issues) — バグ報告と機能リクエスト
- PR歓迎 — [CONTRIBUTING.md](CONTRIBUTING.md) を参照（CLAへの署名が必要）

## ライセンス

このプロジェクトは Apache License, Version 2.0 の下でライセンスされています。
