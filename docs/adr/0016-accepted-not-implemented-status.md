# ADR 0016: ステータス `採択（未実装）` を加え、未実装のADRを一覧する `unimplemented` サブコマンド

ステータス: 採択（未実装）

## 文脈

[ADR 0012](0012-status-line-and-status-adt.md) で、ステータスの値を `enum Status`（Proposed、Accepted、Superseded、Rejected、Discarded、Deprecated、Unknown）として解釈するようにした。

ADRは、人間のレビューで採択されてから実装される。
採択から実装までの間があくことがあり、その間のADRは「採択されたが、まだ実装されていない」状態にある。
しかし、いまのステータスの値には、この状態を表すものがない。
`採択` のADRが実装済みかどうかは、ADRの本文やコードを読まないと分からない。

ADRを使うリポジトリでは、未実装のADRをまとめて実装する運用がある（採択されて実装可能なものを、エージェントがすべて実装する、など）。
その前提として、未実装のADRを機械的に一覧できる必要がある。

そこで、ステータス `採択（未実装）` を加え、その状態のADRを一覧するサブコマンドを加える。
括弧は、日本語の文章で使う全角（`（）`）と、半角（`()`）のどちらで書いても同じに扱う。

## 実装すること

### ステータスの値

- 元の値（[ADR 0012](0012-status-line-and-status-adt.md)）が、`採択` の直後に、開き括弧、`未実装`、閉じ括弧の順で始まるとき、状態を `AcceptedNotImplemented` とする。
  - 開き括弧は `（`（U+FF08）か `(`（U+0028）、閉じ括弧は `）`（U+FF09）か `)`（U+0029）とする。開き括弧と閉じ括弧の種類はそろっていなくてもよい（`採択（未実装)` も認める）。
  - `採択` と開き括弧の間、括弧と `未実装` の間に空白は入れない。
  - 閉じ括弧の後ろに続く文字は問わない（`採択（未実装）。…` など。ほかの日本語の値と同じく先頭の一致）。
- 英語の値として、`Accepted (not implemented)` を同じ状態とする。
  - 大文字と小文字は区別しない（[ADR 0012](0012-status-line-and-status-adt.md) の英語の値と同じ）。
  - `Accepted` と開き括弧の間には半角の空白を1つ置く。括弧の種類の揺れは日本語と同じく認める。
- この判定は、`採択→`（Superseded）と `採択`（Accepted）の判定より前に行う。

| 元の値 | 状態 |
|---|---|
| `採択（未実装）`、`採択(未実装)`、`採択（未実装)`、`採択(未実装）` | `AcceptedNotImplemented` |
| `採択（未実装）。実装は別のリポジトリ` | `AcceptedNotImplemented` |
| `Accepted (not implemented)`、`accepted (Not Implemented)`、`Accepted（not implemented）` | `AcceptedNotImplemented` |
| `採択` | `Accepted`（従来どおり） |
| `採択（2026-10-01）` | `Accepted`（従来どおり） |
| `採択 （未実装）`、`採択（ 未実装 ）`、`採択【未実装】` | `Accepted`（`採択` で始まるため） |
| `採択→[ADR 0002](...)` | `Superseded`（従来どおり） |
| `未実装` | `Unknown` |

### データ構造

`enum Status`（[ADR 0012](0012-status-line-and-status-adt.md)）に、次の値を加える。

```rust
enum Status {
    // ...（従来の値）
    /// `採択（未実装）…`、`Accepted (not implemented)`。採択されたが、まだ実装されていない
    AcceptedNotImplemented,
}
```

`Accepted` の意味は「採択済み」のまま変えない。実装済みかどうかを明示しない `採択` は、従来どおり `Accepted` とする。

### `adr-guard unimplemented [--status]`

- ステータスの行がちょうど1つあり、状態が `AcceptedNotImplemented` のADRのパスを、1行に1つずつ標準出力に出力する。
- `--status` では、`<ステータス><タブ><パス>` の形で出力する（元の値のまま）。
- 対象のADR、並び順、終了コードは `proposed`（[ADR 0005](0005-list-and-proposed-subcommands.md)、[0006](0006-proposed-status-option.md)）と同じ。該当が無くても終了コード0。
- 使い方の1行に `unimplemented [--status]` を加える。

### `--help`、README、仕様

- `--help` の「ADRとステータス」の状態の一覧に `採択（未実装）…`、`Accepted (not implemented)` を加え、括弧の全角と半角の揺れを認めることを書く。サブコマンドに `unimplemented` を加え、未実装のADRを探すのに使うことを書く。
- README と `docs/specifications.md` に、ステータスの値と `unimplemented` を記載する。

### リリース

- 新しい状態とサブコマンドの追加であり、これまで通っていたコミットが拒否されることも、既存のサブコマンドの出力が変わることもない。そのため、パッチバージョンを上げて `v0.8.3` としてリリースする（[ADR 0004](0004-release-version-options-as-patch.md) の判断）。
- 採択後、[ADR 0012](0012-status-line-and-status-adt.md) のステータスを `採択→状態の一覧は[ADR 0016](0016-accepted-not-implemented-status.md)` にする。

## 実装しないこと

- `採択（実装済み）` などの、実装済みを明示する値。`採択` を実装済みとみなすかは、リポジトリの運用に任せる。
- `採択（一部実装）` など、`未実装` 以外の括弧の中身の解釈。
- 未実装のADRがあるときに失敗する検査や終了コード。
- ADRの本文やコードから、実装済みかどうかを推定すること。
- `採択（未実装）` から `採択` へのステータスの変更の自動化。実装したときに人やエージェントがステータスの行を書きかえる（`--allow-status` で許可される）。
- `【】` などのほかの括弧、括弧の内側や外側の空白の揺れ。

## テスト設計

### 既存のテスト

既存のテストは変えずにすべて通す。`採択（2026-10-01）` が `Accepted` になるテストも、そのまま通る。

### 単体テスト

- 状態への変換：上の表のすべての値について、期待する状態になることを確かめる。
- `採択（未実装）` は `Accepted` にも `Superseded` にもならない。

### 結合テスト

- `docs/adr/0001-a.md`（`ステータス: 採択（未実装）`）、`0002-b.md`（`ステータス: 採択(未実装)`）、`0003-c.md`（`Status: Accepted (not implemented)`）、`0004-d.md`（`ステータス: 採択`）、`0005-e.md`（`ステータス: 提案`）を置いた一時リポジトリで、次を確かめる。
  - `unimplemented`：標準出力が `0001-a.md`、`0002-b.md`、`0003-c.md` のパスの3行で、この順に並び、終了コードが0である。
  - `unimplemented --status`：各行が元の値とパスのタブ区切りである。
  - `proposed`：`0005-e.md` だけを出力する（`採択（未実装）` は提案ではない）。
- 該当するADRが無いリポジトリでは、標準出力が空で終了コードが0である。gitのリポジトリの外では終了コードが1である。
- `unimplemented x`、`unimplemented --fix` は終了コード64である。
- `--help` の出力に `unimplemented` と `採択（未実装）` が含まれる。
- `pre-commit --allow-status`：コミット済みのADRのステータスを `採択（未実装）` から `採択` に変える変更を許可する。

### リリース

`v0.8.3` のリリース後、ダウンロードしたバイナリで、上の一時リポジトリの `adr-guard unimplemented` が3件を出力することを確かめる。

## トレードオフ

- 括弧の全角と半角の揺れを認めるが、空白やほかの括弧の揺れは認めない。揺れを広く認めると、`採択（未実装の部分は ADR NNNN）` のような補足と区別しにくくなるためである。認めない書き方は `Accepted` として扱われ、`unimplemented` に現れない。
- `採択` を実装済みとも未実装とも明示しないため、既存の `採択` のADRには、実装済みのものと未実装のものが混ざりうる。未実装を明示する運用に移るときは、未実装のADRのステータスを `採択（未実装）` に変える（`--allow-status` で許可される）。
- `AcceptedNotImplemented` を `Accepted` と別の状態にするため、`Accepted` を前提にした判定（いまは無い）を加えるときは、両方を考える必要がある。`Accepted { implemented: bool }` のように1つの値にまとめる案は、`採択` が実装済みかどうかを表さないことと合わないため採らない。
- 英語の値は `Accepted (not implemented)` だけを認める。`Accepted (unimplemented)` などの言いかえは、必要になったらADRで追加する。

## 参考文献

なし
