# ADR 0007: `list` にも `--status` オプションを加える

ステータス: 採択→ステータスの値の取り出しは[ADR 0012](0012-status-line-and-status-adt.md)

## 文脈

[ADR 0005](0005-list-and-proposed-subcommands.md) で `adr-guard list` はADRのパスだけを出力するとし、ステータスの出力を「実装しないこと」とした。
[ADR 0006](0006-proposed-status-option.md) で `adr-guard proposed --status` を加えたが、`list` の `--status` は同じく「実装しないこと」とした（`v0.2.3`）。

すべてのADRを、採択・却下・破棄などのステータスとともに一覧したいことがある。
`proposed --status` と同じ形で `list --status` を使えるようにし、2つのサブコマンドのオプションをそろえる。

## 実装すること

### `adr-guard list --status`

- `adr-guard list --status` では、すべてのADRを `<ステータス><タブ><パス>` の形で、1行に1つずつ標準出力に出力する。
  - 例：`採択→ADRのディレクトリはADR 0002	docs/adr/0001-standalone-adr-guard.md`
  - `<ステータス>` の値は `proposed --status` と同じ（最初のステータスの行の `ステータス:` より後ろの、前後の空白を除いた値）。
  - ステータスの行が無いADRも出力し、`<ステータス>` を空にする（行はタブで始まる）。どの行も「ステータス、タブ、パス」の2列になるようにする。
- 対象のADR、並び順、終了コードは `list`（[ADR 0005](0005-list-and-proposed-subcommands.md)）と同じ。
- `adr-guard list`（`--status` なし）の出力は変えない。
- `list` に `--status` 以外の引数が付いた場合（`--status` を2回付けた場合を含む）は、ほかの誤った引数と同じく使い方の1行を標準エラーに表示し、終了コード64で終わる。`--status` を置けるのは `list` の後ろだけとする。

### 引数の誤りとヘルプ

- 使い方の1行を `usage: adr-guard <pre-commit|claude-hook|check-range <base> <head>|list [--status]|proposed [--status]|--version|--help>` に変える。
- ヘルプの `list` の説明に、`--status` ではステータスをパスの前に付けることを加える。

### ドキュメントとリリース

- `docs/specifications.md` と `README.md` の `list` の記述を更新する。
- [ADR 0004](0004-release-version-options-as-patch.md) にならってパッチバージョンを上げ、`v0.2.4` としてリリースする。
- 採択後、[ADR 0005](0005-list-and-proposed-subcommands.md) のステータスを `採択→proposed の出力はADR 0006、list の出力はADR 0007` に、[ADR 0006](0006-proposed-status-option.md) のステータスを `採択→list の --status はADR 0007` に変更する。

## 実装しないこと

- ステータスでの絞り込み（`list --status 採択` など）。`grep` などで絞り込む。
- ステータスの行が無いADRに、空の代わりに `-` などの値を出力すること。
- `proposed` の挙動の変更。

## テスト設計

### 結合テスト

既存のテストは変えずにすべて通す（`list --status` を終了コード64とする [ADR 0006](0006-proposed-status-option.md) の確認だけは、下のとおり置きかえる）。一時リポジトリのファイルは [ADR 0005](0005-list-and-proposed-subcommands.md) と同じとする。

- `list --status`：標準出力が次の8行で、この順に並び、終了コードが0である。
  - `採択	docs/adr/0001-a.md`
  - `提案	docs/adr/0002-b.md`
  - `採択→0004	docs/adr/0003-c.md`
  - `却下	docs/adr/0004-d.md`
  - `破棄	docs/adr/0005-e.md`
  - `	docs/adr/0006-f.md`（ステータスが空）
  - `提案	docs/adr/0007-g.md`
  - `提案（レビュー中）	docs/server/adr/0001-a.md`
- サブディレクトリ（`docs/`）で実行しても同じ出力になる。
- ADRが無いリポジトリでは標準出力が空で終了コードが0、gitのリポジトリの外では終了コードが1である。
- `list --status x`、`list --status --status`、`--status list` では終了コードが64である。`list --no-status` と `list x` も従来どおり64である。
- `list`（`--status` なし）の出力が [ADR 0005](0005-list-and-proposed-subcommands.md) のテストと同じである。

### リリース

`v0.2.4` のリリース後、ダウンロードしたバイナリで、このリポジトリの `adr-guard list --status` が、すべてのADRをステータスとともに出力することを確かめる。

## トレードオフ

- ステータスの行が無いADRのステータスを空にするため、行がタブで始まり、目では見落としやすい。一方、どの行も2列になるため、`cut -f2` でパスを、`cut -f1` でステータスを取り出せる。
- `list` と `proposed` で同じ `--status` を持つため、片方の出力の形を変えるときは、もう片方も合わせて見直す必要がある。
