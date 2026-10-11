# 仕様

## adr-guard

コミット済みのADRを、ステータスの行を除いて変更できないようにするツール（ADR 0001、0002）。
[tett23/send-to-kindle](https://github.com/tett23/send-to-kindle) の `tools/adr-guard` から、履歴ごと独立させた。

### 規則

- 対象は、リポジトリのルートからの相対パスが次のどちらかで始まるファイル（ADR 0002）。`adr/` より下の深さは問わない。
  - `docs/adr/`
  - `docs/<モジュール>/adr/`。`<モジュール>` は空でない1つのパス要素（`/` を含まない）。モノレポでモジュールごとにADRを分ける場合に使う。
- `docs/a/b/adr/` のような2段以上のモジュールや、`packages/<モジュール>/docs/adr/` のような `docs/` 以外の場所は対象外。
- ステータスの行は `ステータス:` で始まる行とする。比較の前に、この行を `ステータス:` に置きかえて正規化し、正規化後に差分があれば変更とみなす。
- コミット済みのADRの、ステータスの行以外の変更、削除、リネームを違反とする。新規追加は許可する。

### サブコマンド

- 引数が次のいずれでもなければ、使い方を表示して終了コード64で終わる。
- `adr-guard pre-commit`
  - ステージされたADRの変更のうち、HEADに存在するファイルについて、HEADとインデックスの内容を比較する。
  - 違反があれば、`pre-commit: ` で始まるメッセージを表示して終了コード1で終わる。
  - HEADがない（初回コミット）なら検査しない。
- `adr-guard claude-hook`
  - Claude CodeのPreToolUse hookとして、標準入力のJSONを読む。Write・Edit・MultiEditを対象とする。
  - 対象ファイルがADRで、かつHEADに存在する場合、ツール実行後の内容を計算してHEADと比較する。違反があれば終了コード2で終わる。
  - それ以外（JSONを読めない、gitリポジトリでない、対象外のファイルなど）は終了コード0で終わる。
- `adr-guard check-range <base> <head>`
  - `<base>..<head>` のマージ以外の各コミットを、親コミットとの差分で検査する。
  - `<base>` がコミットとして存在しなければ、`<head>` から辿れるすべてのコミットを検査する。
  - 違反したコミットとファイルをすべて表示し、1件でもあれば終了コード1で終わる。

### 配布

- `v` で始まるタグのpushで、GitHubのリリースを作成する（`.github/workflows/release.yml`）。
  - タグが `v<Cargo.toml の version>` と一致しなければ失敗する。一致すれば `cargo test --locked` を実行してからビルドする。
  - ターゲットは `aarch64-apple-darwin` と `x86_64-unknown-linux-gnu`。
  - 成果物はターゲットごとの `adr-guard-<タグ>-<ターゲット>.tar.gz`（中身はバイナリ、`LICENSE`、`README.md`）と `SHA256SUMS`。アーカイブを作ったあと、中身がそろっていることを確かめる。
  - `-` を含むタグはプレリリースとして作成する。
  - macOS向けのバイナリは、Apple Developer IDによる署名と公証を行わない。
- ソースからは `cargo install --git https://github.com/tett23/adr-guard --tag <タグ>` でインストールできる。

### composite action（`action.yml`）

- 入力は `base`、`head`（必須）と `version`（任意）。
- `version` を省略すると、action自身の `Cargo.toml` のバージョン（`v<version>`）を使う。
- リリースから `x86_64-unknown-linux-gnu` のアーカイブと `SHA256SUMS` をダウンロードし、チェックサムを検証してから、ワークスペースで `adr-guard check-range <base> <head>` を実行する。
- Linux（x86_64）以外のランナーではエラーにする。

## 開発プロセス

### ADR

- 実装の前に `docs/adr/` にADRを作成し、人間のレビューを経てからコミットする。
- コミット済みのADRはステータスの行を除いて変更できない。仕様を変更する場合は新しいADRを作成する。
- このリポジトリでは、開発中の adr-guard 自身（`cargo run`）で、pre-commit hook、Claude Codeのhook、CIの三か所で強制する。

### CI

- `main` へのpushとプルリクエストで、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` と、ADRの検査（`cargo run -- check-range`）を実行する。

## 破棄したADR

なし
