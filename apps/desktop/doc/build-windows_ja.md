# Windows でのビルド

[English](build-windows.md) | [日本語](build-windows_ja.md)

Windows 11 で、何も入っていない状態から Modular Agent Desktop をビルドできるようにする手順です。
ツールはすべて `winget` で入れます。

## 1. ツールチェーンのインストール

PowerShell で実行します。Build Tools のインストーラーは管理者権限（UAC）を求め、終わるまでしばらくかかります。

```powershell
# MSVC C++ コンパイラ + Windows SDK（Windows 上の Rust と Tauri に必要）
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

# Rust (rustup)
winget install --id Rustlang.Rustup -e

# Node.js LTS（fnm でバージョンを管理したい場合は「オプション: fnm で Node.js を入れる」を参照）
winget install --id OpenJS.NodeJS.LTS -e

# CMake: aws-lc-sys などのネイティブ依存のビルドで必要になった場合の備え
winget install --id Kitware.CMake -e
```

Tauri が使う WebView2 ランタイムは Windows 11 に最初から入っています。Windows 10 で入っていない場合は
[Microsoft のサイト](https://developer.microsoft.com/microsoft-edge/webview2/)から入れてください。

インストールが終わったら、更新された `PATH` を読み込むために**ターミナルを開き直してください**。

## 2. Rust の設定と Node.js の確認

ワークスペースは edition 2024 を使っているので、Rust 1.85 以上が必要です。

```powershell
rustup default stable-x86_64-pc-windows-msvc
rustup component add rustfmt clippy
rustc --version
```

あわせて、`node --version` と `npm --version` が動くことも確認してください。

## 3. JavaScript の依存をインストールする

desktop アプリは、Tauri プラグインの JS バインディングをローカルの `file:` パスで参照しています。
そのビルド成果物（`dist-js/`）はリポジトリに含まれていないので、先にビルドしてください。
ビルドしないと、フロントエンドが `tauri-plugin-modular-agent-api` を解決できません。

リポジトリのルートから次を実行します。

```powershell
cd crates/tauri-plugin-modular-agent
npm install
npm run build

cd ../../apps/desktop
npm install
```

## 4. ビルドと起動

`apps/desktop` で次を実行します。

```powershell
npm run check        # svelte-check
npm run tauri dev    # 開発ビルド。アプリのウィンドウが開く
npm run tauri build  # リリースビルド + NSIS インストーラー
```

初回のビルドは依存をすべてコンパイルするので、数分かかります。成果物はワークスペース直下の `target/` に出力されます。

- `target/debug/modular-agent-desktop.exe`（開発ビルド）
- `target/release/modular-agent-desktop.exe` と `target/release/bundle/nsis/*-setup.exe`（リリースビルド）

Rust 側だけを確かめたいときは、ワークスペース内のどこからでも `cargo build -p modular-agent-desktop` を実行してください。

## オプション: fnm で Node.js を入れる

上の手順で入れる Node.js は、システム全体で 1 つのバージョンだけです。プロジェクトごとにバージョンを切り替えたい場合は、
代わりに [fnm](https://github.com/Schniz/fnm) を使います。手順 1 の `OpenJS.NodeJS.LTS` を飛ばし（入れてしまった場合はアンインストールし）、
次を実行してください。

```powershell
winget install --id Schniz.fnm -e
fnm install --lts
fnm default <インストールされたバージョン>   # 例: fnm default 24.21.0
```

fnm の Node は、fnm の初期化を実行したシェルでしか `PATH` に入りません。使うシェルごとに設定してください。

### PowerShell

プロファイルの末尾に次の 1 行を追加します（`notepad $PROFILE` で開きます。ファイルがなければ作成してください）。

```powershell
fnm env --use-on-cd --shell powershell | Out-String | Invoke-Expression
```

### Git Bash

`~/.bashrc` に次の行を書きます。

```bash
eval "$(fnm env --use-on-cd --shell bash)"
```

Git Bash はログインシェルとして起動するので、読むのは `~/.bashrc` ではなく `~/.bash_profile` です。
`~/.bash_profile` がまだなければ、`~/.bashrc` を読み込むものを作ってください。

```bash
test -f ~/.profile && . ~/.profile
test -f ~/.bashrc && . ~/.bashrc
```

### cmd（とシェルのプロファイルを読まないアプリ）

cmd にはプロファイルがないので、fnm の default バージョンのディレクトリをユーザーの `PATH` に追加します。
設定 →「アカウントの環境変数を編集」→ `Path` →「新規」で次を入力します。

```text
%APPDATA%\fnm\aliases\default
```

cmd では常に `fnm default` のバージョンが使われ、`cd` しても切り替わりません。`setx` や
`[Environment]::SetEnvironmentVariable` を使うと既存の `Path` が展開されたり切り詰められたりすることがあるので、
環境変数のダイアログで追加してください。

新しいターミナルを開き、設定したどのシェルでも `node --version` と `npm --version` が動けば完了です。

## トラブルシューティング

- **`node`、`cargo`、`cmake` が見つからない** — ターミナルを開き直してください。インストーラーが更新した `PATH` は
  新しく起動したプロセスにしか反映されません。すでに起動しているアプリ（IDE など）から開いたターミナルの場合は、
  そのアプリを再起動してください。
- **`linker 'link.exe' not found`、または Windows SDK のヘッダーが見つからない** — Build Tools に
  「C++ によるデスクトップ開発」ワークロードが入っていません。上の Build Tools のコマンドを再実行するか、
  Visual Studio Installer からこのワークロードを追加してください。
- **`tauri-plugin-modular-agent-api` を解決できない** — 手順 3 のプラグインのビルドを飛ばしています。
  `crates/tauri-plugin-modular-agent` で `npm run build` を実行してから、`apps/desktop` で `npm install` をやり直してください。
- **`d3-scale` などモジュール UI の依存を解決できない** — モジュールパッケージの `ui/` は固有の npm 依存を持ち、
  dev サーバーが初回起動時に `ui/node_modules` へ自動インストールします。そのインストールが失敗した場合は、
  該当の `ui/` ディレクトリ（例: `crates/modular-agent-std/ui`）で `npm install` を実行してから起動し直してください。

オプションのモジュールパッケージ（データベース、メッセージングなど）を入れる方法は
[カスタムビルド (ma-config)](../README_ja.md#カスタムビルド-ma-config) を参照してください。
