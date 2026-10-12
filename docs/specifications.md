# 仕様

## adr-guard

コミット済みのADRを、ステータスの行を除いて変更できないようにするツール（[ADR 0001](adr/0001-standalone-adr-guard.md)、[0002](adr/0002-module-adr-directories.md)）。
[tett23/send-to-kindle](https://github.com/tett23/send-to-kindle) の `tools/adr-guard` から、履歴ごと独立させた。

### 規則

- 対象は、リポジトリのルートからの相対パスが次のどちらかで始まるファイル（[ADR 0002](adr/0002-module-adr-directories.md)）。`adr/` より下の深さは問わない。
  - `docs/adr/`
  - `docs/<モジュール>/adr/`。`<モジュール>` は空でない1つのパス要素（`/` を含まない）。モノレポでモジュールごとにADRを分ける場合に使う。
- `docs/a/b/adr/` のような2段以上のモジュールや、`packages/<モジュール>/docs/adr/` のような `docs/` 以外の場所は対象外。
- ステータスの行は `ステータス:` で始まる行とする。比較の前に、この行を `ステータス:` に置きかえて正規化し、正規化後に差分があれば変更とみなす。
- コミット済みのADRの、ステータスの行以外の変更、削除、リネームを違反とする。新規追加は許可する。
- 意図してADRを書きかえる場合は、次の方法で違反を許可する（[ADR 0008](adr/0008-allow-intentional-adr-rewrites.md)）。
  - `check-range`：コミットメッセージのトレーラー（最後の段落）に `ADR-Guard-Allow: <理由>` があるコミットでは、ADRの変更・削除・リネームを違反としない。キーの大文字と小文字は区別しない。理由（前後の空白を除いた値）が空なら許可しない。許可したときは `check-range: <コミット>: ADR-Guard-Allow により許可しました: <理由>` を標準エラーに表示する。許可はトレーラーを付けたコミットだけに及ぶ。
  - `pre-commit`：環境変数 `ADR_GUARD_ALLOW` の値（前後の空白を除いたもの）が空でなければ、違反があっても成功し、`pre-commit: ADR_GUARD_ALLOW により許可しました: <値>` を標準エラーに表示する。
  - `claude-hook` には許可の手段がない。

### サブコマンド

- 引数が次のいずれでもなければ、使い方の1行（`usage: adr-guard <pre-commit|claude-hook|check-range <base> <head>|list [--status]|proposed [--status]|--version|--help>`）を標準エラーに表示して終了コード64で終わる。`--version`、`--help` に余分な引数が付いた場合や、`list`、`proposed` に `--status` 以外の引数が付いた場合も同じ。
- `adr-guard --version`、`adr-guard -V`（[ADR 0003](adr/0003-version-and-help-options.md)）
  - `adr-guard <Cargo.toml の version>` の1行を標準出力に表示し、終了コード0で終わる。
- `adr-guard --help`、`adr-guard -h`（[ADR 0003](adr/0003-version-and-help-options.md)）
  - 1行目に `--version` と同じ行、続けて概要と、各サブコマンドとオプションの説明を標準出力に表示し、終了コード0で終わる。
  - サブコマンドごとのヘルプ（`adr-guard pre-commit --help` など）はない。
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
- `adr-guard list [--status]`（[ADR 0005](adr/0005-list-and-proposed-subcommands.md)、[0007](adr/0007-list-status-option.md)）
  - 作業ツリーにあるすべてのADRのパスを、1行に1つずつ標準出力に出力し、終了コード0で終わる。
  - `--status` では、`<ステータス><タブ><パス>` の形で出力する。ステータスの行が無いADRはステータスを空にする（行はタブで始まる）。
- `adr-guard proposed [--status]`（[ADR 0005](adr/0005-list-and-proposed-subcommands.md)、[0006](adr/0006-proposed-status-option.md)）
  - 作業ツリーにあるADRのうち、ステータスが提案のもののパスを、1行に1つずつ標準出力に出力し、終了コード0で終わる。
  - `--status` では、`<ステータス><タブ><パス>` の形で出力する。
  - ステータスは、最初のステータスの行の `ステータス:` より後ろの、前後の空白を除いた値。これが `提案` で始まれば提案とする。ステータスの行が無いADRは対象外。
- `list` と `proposed` で共通
  - 対象は、`git ls-files --cached --others --exclude-standard` で列挙される（追跡されているか、追跡されていないが無視もされていない）ファイルのうち、ADRのディレクトリ以下にあって拡張子が `.md` で、作業ツリーに存在するもの。未コミットのADRも含む。
  - パスはリポジトリのルートからの相対パスで、昇順に並べる。どのディレクトリで実行しても同じ出力になる。
  - 該当が無ければ何も出力せずに終了コード0、gitのリポジトリの外などファイルを列挙できなければ終了コード1で終わる。

### 配布

- `v` で始まるタグのpushで、GitHubのリリースを作成する（`.github/workflows/release.yml`）。
  - タグが `v<Cargo.toml の version>` と一致しなければ失敗する。一致すれば `cargo test --locked` を実行してからビルドする。
  - ビルドしたバイナリの `--version` の出力が `adr-guard <タグからvを除いたもの>` と一致しなければ失敗する（[ADR 0003](adr/0003-version-and-help-options.md)）。
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
