# ADR 0003: `--version` と `--help` オプションを加える

ステータス: 採択→リリースのバージョンはADR 0004

## 文脈

adr-guard は、各リポジトリのpre-commit hookと、Claude Codeの全体の設定のhookから、手元にインストールされたバイナリとして呼ばれる。
[ADR 0002](0002-module-adr-directories.md) で保護の対象が広がり、`v0.1.0` と `v0.2.0` では挙動が異なる。
しかし現状では、手元の adr-guard がどのバージョンかを確かめる方法がない。
また、サブコマンドの説明はREADMEにしかなく、引数が合わないときに1行の使い方を表示するだけである（[ADR 0001](0001-standalone-adr-guard.md)）。

更新が必要かを判断できるようにバージョンを表示するオプションと、手元で使い方を確かめられるヘルプのオプションを加える。

## 実装すること

### `--version` オプション

- `adr-guard --version` と `adr-guard -V` で、`adr-guard <バージョン>` の1行（例：`adr-guard 0.3.0`）を標準出力に表示し、終了コード0で終わる。
  - 書式はclapなど一般的なCLIの `--version` にそろえる。
  - バージョンは `Cargo.toml` の `version`（`env!("CARGO_PKG_VERSION")`）を使う。依存は増やさない。

### `--help` オプション

- `adr-guard --help` と `adr-guard -h` で、ヘルプを標準出力に表示し、終了コード0で終わる。
- ヘルプには次を含める。メッセージと同じく日本語で書く。
  - 1行目に `adr-guard <バージョン>`（`--version` と同じ）
  - adr-guard の概要（コミット済みのADRを、ステータスの行を除いて変更できないようにすること。対象のディレクトリ `docs/adr/` と `docs/<モジュール>/adr/`）
  - 3つのサブコマンド（`pre-commit`、`claude-hook`、`check-range <base> <head>`）と、オプション（`--version`・`-V`、`--help`・`-h`）の、それぞれの1行の説明

例：

```
adr-guard 0.3.0
コミット済みのADR（docs/adr/、docs/<モジュール>/adr/）を、ステータスの行を除いて変更できないようにする。

使い方:
  adr-guard pre-commit                 gitのpre-commit hookとして、ステージされた変更を検査する
  adr-guard claude-hook                Claude CodeのPreToolUse hookとして、Write・Edit・MultiEditを検査する
  adr-guard check-range <base> <head>  CIで、<base>..<head> の各コミットを検査する
  adr-guard --version, -V              バージョンを表示する
  adr-guard --help, -h                 このヘルプを表示する
```

### 引数の誤り

- `--version` や `--help` に余分な引数が付いた場合（例：`adr-guard --help x`）は、ほかの誤った引数と同じく、使い方の1行を標準エラーに表示して終了コード64で終わる。
- 使い方の1行を `usage: adr-guard <pre-commit|claude-hook|check-range <base> <head>|--version|--help>` に変える。
- 3つのサブコマンドの挙動は変えない。

### リリースのワークフロー

- ビルドしたバイナリを `--version` で実行し、出力が `adr-guard <タグからvを除いたもの>` と一致しなければ失敗する。2つのターゲットはどちらもランナーと同じアーキテクチャのため、そのまま実行できる。

### ドキュメントとリリース

- `docs/specifications.md` と `README.md` に `--version` と `--help` を記載する。READMEのインストール手順に、`adr-guard --version` で確かめる手順を加える。
- 機能の追加のため、マイナーバージョンを上げて `v0.3.0` としてリリースする。

## 実装しないこと

- `version` や `help` のサブコマンド。オプションだけにする。
- サブコマンドごとのヘルプ（`adr-guard pre-commit --help` など）。従来どおり引数の誤りとして扱う。
- 英語のヘルプや、言語の切り替え。
- コミットのSHAやビルド日時の表示。
- composite actionでのバージョンの表示。リリースのワークフローでバイナリのバージョンを確かめるため、actionでは確かめない。
- 手元の adr-guard が古い場合の警告や、更新の確認。

## テスト設計

### 結合テスト

既存のテストは変えずにすべて通す。そのうえで次を追加する。

- `--version` と `-V`：終了コード0で、標準出力が `adr-guard {CARGO_PKG_VERSION}\n` と一致し、標準エラーが空である。
- `--help` と `-h`：終了コード0で、標準エラーが空である。標準出力の1行目が `adr-guard {CARGO_PKG_VERSION}` で、`pre-commit`、`claude-hook`、`check-range <base> <head>`、`--version`、`-V`、`--help`、`-h` をすべて含む。`--help` と `-h` の出力は一致する。
- `--version x` と `--help x`：終了コード64で、標準出力が空である。
- 引数なし：終了コード64で、標準エラーの使い方に `--version` と `--help` が含まれる。

### リリース

`v0.3.0` のリリースで、両方のターゲットのビルドがバージョンの確認を通ることを確かめる。ダウンロードしたバイナリで、`adr-guard --version` が `adr-guard 0.3.0` を表示すること、`adr-guard --help` がヘルプを表示することを確かめる。

## トレードオフ

- 引数の解析ライブラリ（clapなど）は入れない。引数が少なく、パターンマッチで足りるため、依存を増やさない方針を優先する。そのため、`--version=x` などの細かな書式や、サブコマンドごとのヘルプには対応しない。
- ヘルプの文面は手で書くため、サブコマンドを増やしたときに更新を忘れうる。テストで各サブコマンドとオプションが含まれることを確かめて補う。
- 使い方の1行が変わるため、その文字列に依存しているものがあれば影響を受ける。終了コードは変わらない。
- リリースのワークフローでバージョンを確かめるため、ターゲットがランナーと異なるアーキテクチャになった場合（クロスコンパイル）は、この確認を見直す必要がある。
