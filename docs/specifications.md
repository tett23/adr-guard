# 仕様

## adr-guard

コミット済みのADRを変更できないようにするツール（[ADR 0001](adr/0001-standalone-adr-guard.md)、[0002](adr/0002-module-adr-directories.md)）。ステータスの行の変更とADRの参照のリンクだけの変更は、オプションで許可する（[ADR 0009](adr/0009-require-links-for-adr-references.md)、[0010](adr/0010-allow-status-option.md)）。
[tett23/send-to-kindle](https://github.com/tett23/send-to-kindle) の `tools/adr-guard` から、履歴ごと独立させた。

### 規則

- 対象は、リポジトリのルートからの相対パスに次のどちらかを含むファイル（[ADR 0002](adr/0002-module-adr-directories.md)、[0011](adr/0011-nested-adr-directories.md)）。`adr/` より下の深さは問わない。
  - `docs/adr/`
  - `docs/<モジュール>/adr/`。`<モジュール>` は空でない1つのパス要素（`/` を含まない）。モノレポでモジュールごとにADRを分ける場合に使う。
- 上の `docs/adr/` と `docs/<モジュール>/adr/` は、どの深さにあってもよい。`docs` より前の要素（`packages/server/` など）は空でなければ名前を問わない（[ADR 0011](adr/0011-nested-adr-directories.md)）。例：`packages/server/docs/adr/`、`apps/web/docs/api/adr/`、`src/docs/server/adr/`。
- `docs/a/b/adr/` のような2段以上のモジュールと、`packages/server/adr/` のように `docs` を挟まない場所は対象外。
- ステータスの行は、行頭が `ステータス:` か `Status:`（見出しの大文字・小文字は区別し、コロンは半角）の行とする（[ADR 0012](adr/0012-status-line-and-status-adt.md)）。`- Status:`、`* Status:`、`status:`、全角のコロン、行頭の空白は認めない。
- ADRはステータスの行をちょうど1つ持つ。ステータスの行が無いADRは `<パス>: ステータスの行がありません（ステータス: または Status: で始まる行が要ります）`、2つ以上あるADRは2つ目以降の行ごとに `<パス>:<行>: ステータスの行が2つ以上あります（最初は<行>行目）` を違反とする。`pre-commit`（ステージされた追加・変更のADR。初回のコミットでは検査しない）と `check-status` で検査する。`ADR_GUARD_ALLOW` はこの違反を許可しない。
- ステータスの値（コロンより後ろの、前後の空白を除いた元の値）は、次の状態として解釈する。日本語は先頭の一致、英語は大文字と小文字を区別せずキーワードの直後が英数字でないときに当たる。どれにも当たらない値は `Unknown` とし、違反にはしない。
  - `提案…`、`Proposed`：Proposed
  - `採択` に0個以上の空白と `→` が続くもの、`Superseded…`：Superseded（後ろの「変わった部分」と変更したADRの番号を解析する。分類には影響しない）
  - `採択…`、`Accepted`：Accepted
  - `却下…`、`Rejected`：Rejected
  - `破棄…`、`Discarded`：Discarded
  - `廃止…`、`Deprecated`：Deprecated
- コミット済みのADRの変更、削除、リネームを違反とする。新規追加は許可する。
- 変更は、`pre-commit`、`check-range`、`claude-hook` のオプションで許可できる。変更の前後の内容に次の処理を施してから比較し、一致すれば許可する。どちらも付けなければ、前後がまったく同じときだけ許可する。
  - `--allow-status`（[ADR 0010](adr/0010-allow-status-option.md)、[0012](adr/0012-status-line-and-status-adt.md)）：ステータスの行（`ステータス:` と `Status:` で始まるすべての行）を取り除く。値と見出しの変更、ステータスの行が無かったADRへの追加、ステータスの行の位置の変更が許可される。`claude-hook` も受け付ける。
  - `--allow-links`（[ADR 0009](adr/0009-require-links-for-adr-references.md)）：下記のとおり、ADRの参照のリンクをテキストに戻す。`claude-hook` は受け付けない。
- 違反のメッセージは、`--allow-status` があれば `コミット済みのADRはステータスの行以外を変更できません: <パス>`、なければ `コミット済みのADRは変更できません（ステータスの行の変更は --allow-status で許可できます）: <パス>`。
- 監査や計測の記録も保護する（[ADR 0013](adr/0013-protect-audit-and-benchmark-records.md)）。
  - 記録は、ADRのパスと同じ形で `<名前>` が `audit` か `benchmarks` のディレクトリ（どの深さの `docs/audit/`、`docs/benchmarks/`、`docs/<モジュール>/audit/`、`docs/<モジュール>/benchmarks/`）以下の、すべてのファイル。`docs/audits/`、`docs/benchmark/` は対象外。
  - コミット済みの記録の変更、削除、リネームを違反とする。新規追加は許可する。`--allow-status` と `--allow-links` は記録には効かない。
  - 違反のメッセージは `コミット済みの記録（docs/audit/、docs/benchmarks/）は変更できません: <パス>` と `コミット済みの記録（docs/audit/、docs/benchmarks/）は削除・移動できません: <パス>`。記録の違反があれば、案内に `記録を直す場合は、新しい記録を作成してください。` を加える。
  - `pre-commit`、`check-range`、`claude-hook` で検査する。`claude-hook` では、コミット済みの記録の内容を変える編集を終了コード2で拒否する。
  - `links --fix` は、コミット済みの記録をコミット済みのADRと同じく既定では書きかえず、`--include-committed` で書きかえたときは `<パス>: コミット済みの記録です。コミットには ADR-Guard-Allow のトレーラーが要ります` を表示する。
- 意図してADRや記録を書きかえる場合は、次の方法で違反を許可する（[ADR 0008](adr/0008-allow-intentional-adr-rewrites.md)、[0013](adr/0013-protect-audit-and-benchmark-records.md)）。
  - `check-range`：コミットメッセージのトレーラー（最後の段落）に `ADR-Guard-Allow: <理由>` があるコミットでは、ADRと記録の変更・削除・リネームを違反としない。キーの大文字と小文字は区別しない。理由（前後の空白を除いた値）が空なら許可しない。許可したときは `check-range: <コミット>: ADR-Guard-Allow により許可しました: <理由>` を標準エラーに表示する。許可はトレーラーを付けたコミットだけに及ぶ。
  - `pre-commit`：環境変数 `ADR_GUARD_ALLOW` の値（前後の空白を除いたもの）が空でなければ、違反があっても成功し、`pre-commit: ADR_GUARD_ALLOW により許可しました: <値>` を標準エラーに表示する。
  - `claude-hook` には、環境変数やトレーラーによる許可の手段がない。
  - トレーラーは緊急用であり、ADRの参照にリンクを付ける変更には `--allow-links` を使う。
- `pre-commit` と `check-range` に `--allow-links` を付けると、コミット済みのADRの変更のうち、ADRの参照のリンクだけが異なる変更を許可する（[ADR 0009](adr/0009-require-links-for-adr-references.md)）。変更の前後で、テキストが `ADR <4桁>` か `<4桁>` のインラインのリンクをテキストだけに置きかえ、ステータスの行を正規化してから比較する。
- 文書（`.md`）の中のADRの参照にはリンクが要る（[ADR 0009](adr/0009-require-links-for-adr-references.md)）。
  - 参照は `ADR` と半角の空白と4桁の数字（前後が英数字でないもの）。`、` か `,`（後ろに空白があってもよい）で区切って続く4桁の数字も参照とする。
  - フェンスで囲んだコードブロック、インラインのコード（同じ数の `` ` `` の並びで囲んだ部分）、リンク（インラインと参照形式）の中と、ADR自身の番号は対象外。
  - 参照先は、文書のディレクトリから上にたどった各祖先 `<祖先>`（ルートを含み、`docs` の要素を含むものを除く）について、近い順に、文書が `<祖先>/docs/<モジュール>/` 以下（`<モジュール>` は `adr` 以外）なら `<祖先>/docs/<モジュール>/adr/`、続けて `<祖先>/docs/adr/` から、ファイル名が `<番号>-` で始まる `.md` がちょうど1つあるものを探す（[ADR 0011](adr/0011-nested-adr-directories.md)）。見つからない番号は検査しない。
  - リンクは `[ADR <番号>](<文書のディレクトリからの相対パス>)`、続く番号は `[<番号>](<相対パス>)`。

### サブコマンド

- オプションは、サブコマンドの後ろならどの位置に置いてもよい。同じオプションを2回付けた場合や、受け付けないオプションを付けた場合は引数の誤りとする。
- 引数が次のいずれでもなければ、使い方の1行（`usage: adr-guard <pre-commit [--allow-status] [--allow-links]|claude-hook [--allow-status]|check-range [--allow-status] [--allow-links] <base> <head>|list [--status]|proposed [--status]|links [--fix [--include-committed]]|check-status|--version|--help>`）を標準エラーに表示して終了コード64で終わる。`--version`、`--help` に余分な引数が付いた場合や、`list`、`proposed` に `--status` 以外の引数が付いた場合も同じ。
- `adr-guard --version`、`adr-guard -V`（[ADR 0003](adr/0003-version-and-help-options.md)）
  - `adr-guard <Cargo.toml の version>` の1行を標準出力に表示し、終了コード0で終わる。
- `adr-guard --help`、`adr-guard -h`（[ADR 0003](adr/0003-version-and-help-options.md)）
  - 1行目に `--version` と同じ行、続けて概要と、各サブコマンドとオプションの説明を標準出力に表示し、終了コード0で終わる。
  - サブコマンドごとのヘルプ（`adr-guard pre-commit --help` など）はない。
- `adr-guard pre-commit [--allow-status] [--allow-links]`
  - ステージされたADRの変更のうち、HEADに存在するファイルについて、HEADとインデックスの内容を比較する。HEADがない（初回コミット）ならこの比較はしない。
  - ステージされた追加・変更の `.md` のファイルの、インデックスの内容にリンクの無いADRの参照があれば違反とする。参照先の解決には作業ツリーのADRを使う。`ADR_GUARD_ALLOW` はこの違反を許可しない（[ADR 0009](adr/0009-require-links-for-adr-references.md)）。
  - 違反があれば、`pre-commit: ` で始まるメッセージを表示して終了コード1で終わる。
- `adr-guard claude-hook [--allow-status]`
  - Claude CodeのPreToolUse hookとして、標準入力のJSONを読む。Write・Edit・MultiEditを対象とする。
  - 対象ファイルがADRで、かつHEADに存在する場合、ツール実行後の内容を計算してHEADと比較する。違反があれば終了コード2で終わる。
  - それ以外（JSONを読めない、gitリポジトリでない、対象外のファイルなど）は終了コード0で終わる。
- `adr-guard check-range [--allow-status] [--allow-links] <base> <head>`
  - `<base>..<head>` のマージ以外の各コミットを、親コミットとの差分で検査する。
  - `<base>` がコミットとして存在しなければ、`<head>` から辿れるすべてのコミットを検査する。
  - 違反したコミットとファイルをすべて表示し、1件でもあれば終了コード1で終わる。
- `adr-guard list [--status]`（[ADR 0005](adr/0005-list-and-proposed-subcommands.md)、[0007](adr/0007-list-status-option.md)）
  - 作業ツリーにあるすべてのADRのパスを、1行に1つずつ標準出力に出力し、終了コード0で終わる。
  - `--status` では、`<ステータス><タブ><パス>` の形で出力する。ステータスは元の値のまま。ステータスの行が無いか2つ以上あるADRはステータスを空にする（行はタブで始まる）。
- `adr-guard proposed [--status]`（[ADR 0005](adr/0005-list-and-proposed-subcommands.md)、[0006](adr/0006-proposed-status-option.md)）
  - 作業ツリーにあるADRのうち、ステータスが提案のもののパスを、1行に1つずつ標準出力に出力し、終了コード0で終わる。
  - `--status` では、`<ステータス><タブ><パス>` の形で出力する。
  - ステータスの行がちょうど1つあり、その状態が Proposed のADRを対象とする（[ADR 0012](adr/0012-status-line-and-status-adt.md)）。
- `adr-guard links [--fix [--include-committed]]`（[ADR 0009](adr/0009-require-links-for-adr-references.md)）
  - 作業ツリーの `.md`（`list` と同じ列挙）にある、リンクの無いADRの参照を `<パス>:<行>: ADR <番号> にリンクがありません` の形で標準エラーに表示する。1件でもあれば修正の方法を表示して終了コード1、無ければ終了コード0で終わる。
  - `--fix` では、コミット済みのADR（ADRのパスにあって `HEAD` に存在するもの）を除いて、参照をリンクに置きかえる。コミット済みのADRに参照が残れば終了コード1で終わる。
  - `--include-committed`（`--fix` と一緒にだけ使える）では、コミット済みのADRも書きかえる。
- `adr-guard check-status`（[ADR 0012](adr/0012-status-line-and-status-adt.md)）
  - 作業ツリーのすべてのADR（`.md`）のステータスの行の数を検査し、違反を `check-status: ` に続けて標準エラーに表示する。違反があれば終了コード1、無ければ終了コード0で終わる。
- `list` と `proposed` と `links` と `check-status` で共通
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

- 入力は `base`、`head`（必須）と `version`、`allow-status`（既定は `false`）、`allow-links`（既定は `false`）、`links`（既定は `true`）、`check-status`（既定は `true`）（任意）。
- `version` を省略すると、action自身の `Cargo.toml` のバージョン（`v<version>`）を使う。
- リリースから `x86_64-unknown-linux-gnu` のアーカイブと `SHA256SUMS` をダウンロードし、チェックサムを検証してから、ワークスペースで `adr-guard check-range <base> <head>` を実行する。`allow-status`、`allow-links` が `true` なら、それぞれ `--allow-status`、`--allow-links` を付ける。`links` が `true` なら、続けて `adr-guard links` を実行する（[ADR 0009](adr/0009-require-links-for-adr-references.md)）。`check-status` が `true` なら、続けて `adr-guard check-status` を実行する（[ADR 0012](adr/0012-status-line-and-status-adt.md)）。
- Linux（x86_64）以外のランナーではエラーにする。

## 開発プロセス

### ADR

- 実装の前に `docs/adr/` にADRを作成し、人間のレビューを経てからコミットする。
- コミット済みのADRはステータスの行を除いて変更できない（このリポジトリのhookとCIは `--allow-status --allow-links` で検査する）。仕様を変更する場合は新しいADRを作成する。
- このリポジトリでは、開発中の adr-guard 自身（`cargo run`）で、pre-commit hook、Claude Codeのhook、CIの三か所で強制する。

### CI

- `main` へのpushとプルリクエストで、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` と、ADRの検査（`cargo run -- check-range --allow-status --allow-links`）、リンクの検査（`cargo run -- links`）、ステータスの行の検査（`cargo run -- check-status`）を実行する。

## 破棄したADR

なし
