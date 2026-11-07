# ADR 0020: シンボリックリンクの文書を、ADRの参照の検査と書きかえの対象から除く

ステータス: 採択

## 文脈

`links`、pre-commit hook のADRの参照の検査（[ADR 0009](0009-require-links-for-adr-references.md)）と、`renumber` のリンクの書きかえ（[ADR 0018](0018-repair-duplicate-adr-numbers.md)）は、作業ツリーの `.md` を `git ls-files` で列挙して読む。
列挙には、シンボリックリンクも含まれる。シンボリックリンクを読むと、実体のファイルの内容が返る。

ADRの参照のリンク先は、文書のディレクトリからの相対パスである。
実体のファイルの中のリンクは、実体の場所から正しく解決できるように書かれている。
これをシンボリックリンクの場所で調べると、同じ内容を別のディレクトリから解決することになり、次の問題が起きる。

- リンク先の検査（[ADR 0019](0019-module-qualified-adr-references.md)）で、実体の場所では正しいリンクを、すべて誤りとして報告する。
- `links --fix` と `renumber` が、シンボリックリンクの場所から計算した相対パスを、シンボリックリンクを通じて実体のファイルに書きこむ。実体の場所から見ると、そのリンクは壊れる。
- 同じ内容を、実体とシンボリックリンクの2つのパスで二重に報告する。

tett23/golem の main（2026-11-07 の時点、`6086e7c`）では、`modules/golem-driver/diagnostics.md` が `docs/diagnostics.md` へのシンボリックリンクになっている。パッケージに同梱するためである。
[ADR 0019](0019-module-qualified-adr-references.md) を実装した adr-guard で `links` を流すと、この文書だけで、実体の場所では正しいリンク329個を、リンク先の誤りとして報告した。
`links --fix` を流せば、`docs/diagnostics.md` の329個のリンクを壊す。

実体のファイルが作業ツリーにあれば、そのファイルは実体のパスで検査される。シンボリックリンクの側を調べる必要はない。

## 実装すること

### シンボリックリンクの判定

- 作業ツリーでは、パスそのものがシンボリックリンクである（`std::fs::symlink_metadata` がシンボリックリンクを返す）ものを、シンボリックリンクの文書とする。シンボリックリンクのディレクトリの下のファイルは、`git ls-files` が列挙しないため、考えない。
- インデックスでは、モードが `120000` のものを、シンボリックリンクの文書とする。

### 対象から除くもの

- `links`（`--fix` を含む）：シンボリックリンクの `.md` を、検査せず、書きかえない。
- pre-commit hook のADRの参照の検査：ステージされた `.md` のうち、インデックスでシンボリックリンクのものを検査しない。
- `renumber`：リンクの書きかえと、リンクの無い参照の警告の対象から、シンボリックリンクの `.md` を除く。

### 参照先の解決

- ADRの参照の参照先を探すADRの一覧（`adr_paths`）は、変えない。シンボリックリンクのADRがあれば、いまと同じく参照先の候補になる。

### `--help`、README、仕様

- `links` の `--help` と README と `docs/specifications.md` に、シンボリックリンクの `.md` を検査しないことを記載する。

### リリース

[ADR 0019](0019-module-qualified-adr-references.md) と合わせて、`v0.10.0` としてリリースする。

## 実装しないこと

- シンボリックリンクのADR（`docs/adr/` の中のシンボリックリンク）の扱いの変更。コミット済みのADRの変更の検査、`check-status`、`list` などは、いまのまま扱う。
- 実体のファイルが作業ツリーの外にあるシンボリックリンクを、代わりに検査すること。作業ツリーの外の文書は、このリポジトリの検査の対象ではない。
- シンボリックリンクの場所から見て正しいリンクかどうかの検査。

## テスト設計

### 既存のテスト

既存のテストは変えずにすべて通す。

### 結合テスト（`tests/hooks.rs`）

`docs/golem-cli/adr/0005-x.md` と、`docs/diagnostics.md`（`[golem-cli ADR 0005](golem-cli/adr/0005-x.md)` を含む）と、`modules/m/diagnostics.md`（`../../docs/diagnostics.md` へのシンボリックリンク）をコミットした一時リポジトリで、次を確かめる。

- `links` が、何も表示せずに終了コード0で終わる。
- `docs/diagnostics.md` にリンクの無い `golem-cli ADR 0005` を足すと、`links` は `docs/diagnostics.md` の行だけを報告し、`modules/m/diagnostics.md` は報告しない。`links --fix` の後、`docs/diagnostics.md` のリンク先は `golem-cli/adr/0005-x.md`（実体の場所からの相対パス）になり、その後の `links` は終了コード0で終わる。
- シンボリックリンクだけを新たに作ってステージすると、pre-commit hook（`--allow-status --allow-links`）が通る。
- `docs/golem-cli/adr/0005-a.md` を先にコミットしてから `0005-x.md` を足した重複を `renumber` で直すと、`docs/diagnostics.md` のリンクは実体の場所からの相対パスで直り、`modules/m/diagnostics.md` を通じた書きかえは起きない（直した参照の件数が1件である）。

### リリース

`v0.10.0` のリリース後、ダウンロードしたバイナリで、tett23/golem の main（`6086e7c`）の作業ツリーの複製に `links` を流し、`modules/golem-driver/diagnostics.md` が報告されないことを確かめる。

## トレードオフ

- シンボリックリンクの `.md` を一律に除く。シンボリックリンクの場所で読まれることを意図した文書（実体の側では壊れたリンクを持つもの）は検査されなくなるが、そうした文書は実体の場所でリンクが壊れており、リポジトリの上で正しく読めない。実体の場所での検査を優先する。
- 実体が作業ツリーの外にあるシンボリックリンクも除くため、その内容は検査されない。外の文書を書きかえる危険を避けることを優先する。
- 参照先の候補からはシンボリックリンクのADRを除かない。参照先の解決は読むだけであり、書きかえの危険がないため、いまの振る舞いを保つ。

## 参考文献

- tett23/golem の main（`6086e7c`）の [modules/golem-driver/diagnostics.md](https://github.com/tett23/golem/blob/6086e7c509e3ca73a23f975899c05ada066282d6/modules/golem-driver/diagnostics.md)（`docs/diagnostics.md` へのシンボリックリンク）
