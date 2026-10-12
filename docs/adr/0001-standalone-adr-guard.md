# ADR 0001: adr-guard を独立したリポジトリとして提供する

ステータス: 採択→ADRのディレクトリは[ADR 0002](0002-module-adr-directories.md)

## 文脈

adr-guard は、コミット済みのADR（`docs/adr/`）をステータスの行を除いて変更できないようにするツールである。
[tett23/send-to-kindle](https://github.com/tett23/send-to-kindle) の `tools/adr-guard` として作られ、次の3つのサブコマンドを持つ（send-to-kindle の [ADR 0001](https://github.com/tett23/send-to-kindle/blob/main/docs/adr/0001-adr-immutability-hooks.md)、[0004](https://github.com/tett23/send-to-kindle/blob/main/docs/adr/0004-continuous-integration.md)）。

- `adr-guard pre-commit`：gitのpre-commit hookとして、ステージされた変更を検査する。
- `adr-guard claude-hook`：Claude CodeのPreToolUse hookとして、Write・Edit・MultiEditを検査する。
- `adr-guard check-range <base> <head>`：CIで、範囲内の各コミットを検査する。

ADRを使う運用はリポジトリを問わず同じであり、リポジトリごとに `tools/adr-guard` を複製すると、修正をすべての複製に反映する手間がかかる。
1つのリポジトリで開発・配布し、各リポジトリはそれを使うだけにしたい。

## 実装すること

### リポジトリ

- GitHubに `tett23/adr-guard` を作成する。ライセンスはMIT。
- send-to-kindle の `tools/adr-guard` の履歴を引き継ぐ。`git subtree split --prefix tools/adr-guard` で取り出した履歴を、このリポジトリの最初の履歴とする。
- リポジトリ直下をCargoパッケージ `adr-guard`（バイナリ1つ）とする。
- 挙動（3つのサブコマンド、ADRのディレクトリ `docs/adr/`、ステータスの行の判定 `ステータス:`、メッセージ、終了コード）は send-to-kindle の `tools/adr-guard` から変えない。

### このリポジトリ自身の運用

- このリポジトリでもADRを `docs/adr/` で管理し、adr-guard 自身で保護する。
  - `.githooks/pre-commit` と `.claude/settings.json` から、`cargo run --quiet -- <サブコマンド>` で自分自身を起動する。
- `docs/specifications.md` と `README.md` を置き、ADRによる変更のたびに更新する。

### 配布

- `v` で始まるタグのpushで、GitHubのリリースを作成する（send-to-kindle の [ADR 0006](https://github.com/tett23/send-to-kindle/blob/main/docs/adr/0006-release-binaries.md) と同じ方式）。
  - タグが `Cargo.toml` の `version` と一致することを確かめ、テストを実行してからビルドする。
  - ターゲットは `aarch64-apple-darwin` と `x86_64-unknown-linux-gnu` とする。
  - アーカイブ `adr-guard-<タグ>-<ターゲット>.tar.gz`（中身はバイナリ、`LICENSE`、`README.md`）と `SHA256SUMS` を添付する。
  - `-` を含むタグ（例：`v0.1.0-rc`）はプレリリースとして作成する。
- ソースからは `cargo install --git https://github.com/tett23/adr-guard --tag <タグ>` でインストールできる。

### 使う側のリポジトリ向けの提供物

- リポジトリ直下に composite action（`action.yml`）を置く。CIでの検査を `uses: tett23/adr-guard@<タグ>` の1ステップで行えるようにする。
  - 入力は `base` と `head`（必須）、`version`（省略時はactionのタグと同じバージョン）とする。
  - リリースから `x86_64-unknown-linux-gnu` のバイナリをダウンロードし、`SHA256SUMS` で検証してから `adr-guard check-range <base> <head>` を実行する。
  - Linux（x86_64）のランナーだけに対応する。
  - JavaScriptのactionではなくcomposite actionとし、Nodeに依存しない。
- READMEに、使う側のリポジトリでの設定手順を記載する。
  - adr-guard のインストール（リリースまたは `cargo install`）
  - `.githooks/pre-commit`（`exec adr-guard pre-commit`）と `git config core.hooksPath .githooks`
  - `.claude/settings.json` のPreToolUse hook（`adr-guard claude-hook`）
  - CIのワークフローの例（`fetch-depth: 0` でのチェックアウトと、composite actionの呼び出し）

### CI

- send-to-kindle の [ADR 0004](https://github.com/tett23/send-to-kindle/blob/main/docs/adr/0004-continuous-integration.md) と同じく、`main` へのpushとプルリクエストで次を実行する。
  - `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`
  - このリポジトリ自身のADRの検査（`cargo run -- check-range`）
- composite actionは、このリポジトリのCIでは検査しない（リリース前のバイナリが存在しないため）。リリース後に、使う側のリポジトリ（send-to-kindle）で確かめる。

## 実装しないこと

- ADRのディレクトリやステータスの行の判定の設定（設定ファイルやオプション）。必要になったらADRで追加する。
- 使う側のリポジトリにhookやワークフローを書き出す `init` などのサブコマンド。READMEの手順で設定する。
- crates.ioやHomebrewでの配布。
- Windows向けと、`x86_64-apple-darwin` などその他のターゲットのバイナリ。
- send-to-kindle の `tools/adr-guard` の削除と、send-to-kindle での切り替え。send-to-kindle側のADRで扱う。
- 関数型のスタイルへの書きかえなど、挙動を変えないリファクタリング。必要になったら別に行う。

## テスト設計

- 既存のテスト（単体テスト、pre-commit・claude-hook・check-rangeの結合テスト）を、変えずにすべて引き継いで通す。
- 履歴の引き継ぎ：このリポジトリで `git log` を実行し、send-to-kindle の `tools/adr-guard` を変更したコミット（`6d680c4`、`844bc71` に相当するもの）が含まれることを確かめる。
- このリポジトリ自身の運用：コミット済みのADRの本文を変えたコミットがpre-commit hookで拒否されることを、一度手元で確かめる。
- リリース：最初のタグ（`v0.1.0`）をpushし、2つのアーカイブと `SHA256SUMS` が添付されることを確かめる。プレリリースの扱いは、最初の確認で `v0.1.0-rc` を使って確かめる。
- composite action：send-to-kindle で切り替えたプルリクエストのCIで、ADRの検査が実行され成功することを確かめる。

## トレードオフ

- 使う側のリポジトリは、手元に adr-guard をインストールする必要がある。インストールしていないと、pre-commit hookはコマンドが見つからずにコミットが拒否され、Claude Codeのhookは終了コード2以外を返すため編集を止めない（フェイルオープン）。READMEに手順を記載して補う。
- 使う側の手元の adr-guard のバージョンはそろわない。CIではactionのタグでバージョンを固定するため、最終的な検査はCIで担保される。
- `serde_json` への依存は残す。Claude Codeから受け取るJSONの解析には、文字列のエスケープやサロゲートペアなどJSONの仕様全体への対応が要り、小さく再実装が容易とはいえない。
- composite actionをLinux（x86_64）に限るため、macOSのランナーでは使えない。CIのADR検査はOSに依存しないため、Linuxのランナーで十分である。
- 履歴を引き継ぐと、最初の履歴に send-to-kindle の文脈のコミットメッセージが残る。経緯を追えることを優先する。
