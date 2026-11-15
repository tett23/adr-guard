# ADR 0027: 旧形式の `置換（…）` を、修正を受けた状態として読む

Status: accepted

## 文脈

[ADR 0012](0012-status-line-and-status-adt.md) は、旧形式のステータスの値を、先頭の語で状態に分けると定めた。
どの語にも当たらない値は、状態を Unknown とする。

[ADR 0022](0022-status-transitions.md) は、旧形式の行を新形式に書きかえるコミットの状態遷移を検査する。
旧形式の値が Unknown なら、遷移元を判定できないので、書きかえを違反とする。
この違反は、`--allow-status`、`ADR_GUARD_ALLOW`、トレーラーのどれでも許可されない。

そのため、旧形式の値が Unknown のADRは、移行スクリプト（`scripts/migrate-adr-status`）でも、人や AI が手で書いても、新形式に移せない。
この ADR は、tett23 のリポジトリで Unknown と読まれる旧形式の値を、修正を受けた状態として読めるようにする。

## 調査した事項

`main`（`3574a65`）の adr-guard で、次を確かめた。

tett23 のリポジトリ（dotfiles を含む）のADRのステータスの行を、`git ls-files` で列挙したADRから集めた。
行は1227件あった。
そのうち、[ADR 0012](0012-status-line-and-status-adt.md) のどの語（`提案`、`採択`、`却下`、`破棄`、`廃止`、英語の `Proposed`、`Accepted`、`Superseded`、`Rejected`、`Discarded`、`Deprecated`）でも始まらない行は、次の2件だけだった。

| ADR | ステータスの行 |
|---|---|
| novel-crawler `docs/adr/0002-sqlite-persistence.md` | `ステータス: 置換（[ADR 0034](0034-remove-sqlite.md)）` |
| novel-crawler `docs/adr/0004-exclude-hameln.md` | `ステータス: 置換（[ADR 0027](0027-restore-hameln.md)）` |

どちらも、ADR全体が後のADRに置きかえられたことを表す。
novel-crawler は、部分の置きかえを `採択（<部分>は置換（[ADR NNNN](…)））` と書き、全体の置きかえを `置換（[ADR NNNN](…)）` と書いている。

一時リポジトリで、`ステータス: 置換（[ADR 0002](0002-x2.md)）` をコミットしてから、`Status: superseded [{"part":null,"by":["docs/adr/0002-x2.md"]}]` に書きかえた。
`pre-commit --allow-status` と `check-range --allow-status` は、どちらも `許可されていない状態遷移: 置換（[ADR 0002](0002-x2.md)） -> superseded [...]` で終了コード1で終わった。

## 用語

- 変更、部分名、注記：[ADR 0026](0026-read-more-legacy-references.md) の用語と同じ。変更は部分名と修正先の列を持ち、注記は最後の参照の後ろの文字列から得る。

## 実装すること

### 旧形式の値の読み方

[ADR 0012](0012-status-line-and-status-adt.md) の旧形式の値の読み方に、次の1つを加える。

- `置換` で始まる値は、状態を Superseded とする。

判定の順は、`採択…` の後、`却下…` の前とする。
`置換` で始まる値は、ほかのどの語でも始まらないので、順は結果に影響しない。

### 変更の読み方

`置換` の後ろを、[ADR 0026](0026-read-more-legacy-references.md) の `採択→` の後ろと同じ規則で変更に分ける。
ただし、先に次を行う。

- `置換` の後ろの前後の空白を除く。
- 先頭が `→` なら、それを除き、続く空白も除く。
- 先頭が開き括弧（`（` か `(`）で、末尾が閉じ括弧（`）` か `)`）なら、その2つを除く。

`置換（[ADR 0034](0034-remove-sqlite.md)）` は、部分名なしの変更1件で、修正先は `docs/adr/0034-remove-sqlite.md` になる。

### 移行スクリプトと検査

移行スクリプトは、`置換…` の行を、ほかの Superseded の旧形式の行と同じく `Status: superseded [...]` に移す。
最後の参照の後ろの注記は、[ADR 0026](0026-read-more-legacy-references.md) のとおり、最後の変更の `note` に入れる。
`置換` の行でも注記を落とさない。
pre-commit と `check-range` は、`置換…` の行から新形式への書きかえを、旧形式の修正先と新形式の修正先の比較（[ADR 0022](0022-status-transitions.md)）で検査する。

### 文書

README と `docs/specifications.md` の旧形式の値の一覧に、`置換…` を Superseded として加える。

## 実装しないこと

- `補足（…）` で始まる値を読むこと。調べたリポジトリに、この形の行はなかった。
- 英語の `Replaced` などを読むこと。調べたリポジトリに、この形の行はなかった。
- Unknown の旧形式の値からの書きかえを、検査せずに通すこと。

## 不変条件

1. `置換` で始まる旧形式の値は、Superseded と読まれる。
2. `置換（<参照>）` の変更の列は、`採択→<参照>` の変更の列と等しい。
3. `置換` で始まらない旧形式の値の状態は、この ADR の前と変わらない。
4. `置換（<参照>）` から、同じ修正先を持つ新形式の superseded への書きかえは、pre-commit と `check-range` を通る。
5. `置換…` の行の最後の参照の後ろの注記は、移行スクリプトの出力の最後の変更の `note` に入る。

| 旧形式の値 | 期待 |
|---|---|
| `置換（[ADR 0002](0002-b.md)）` | Superseded。部分名なし、修正先 `docs/adr/0002-b.md` |
| `置換([ADR 0002](0002-b.md))` | 同上 |
| `置換（[ADR 0002](0002-b.md)、[ADR 0003](0003-c.md)）` | Superseded。部分名なしの変更1件、修正先2件 |
| `置換→[ADR 0002](0002-b.md)` | Superseded。部分名なし、修正先 `docs/adr/0002-b.md` |
| `置換（[ADR 0002](0002-b.md)）で削った` | Superseded。部分名なし、修正先1件。移行スクリプトの出力の `note` は「で削った」 |
| `置換` | Superseded。変更なし（修正先の欠落として、移行と比較はエラー） |
| `採択（X は置換（[ADR 0002](0002-b.md)））` | Accepted（いまと同じ） |

## テスト設計

テストの名前には、`replaced_inv` に続けて不変条件の番号を入れる。

- `replaced_inv1_status`：上の表の各値を `parse` に通し、状態を確かめる。
- `replaced_inv2_changes`：上の表の `置換（…）` の値の変更の列が、対応する `採択→…` の変更の列と等しいことを確かめる。
- `replaced_inv3_others`：[ADR 0012](0012-status-line-and-status-adt.md) の既存の値の表を通し、状態が変わらないことを確かめる。既存のテストは変えずに通す。
- `replaced_inv4_pre_commit`：一時リポジトリで `置換（[ADR 0002](0002-b.md)）` をコミットし、新形式の superseded への書きかえのコミットが pre-commit と `check-range` を通ることを確かめる。移行スクリプトの `--write` で同じ行が移り、二度目の `--dry-run` が差分を出さないことも確かめる。
- `replaced_inv5_migrate_note`：`置換（[ADR 0002](0002-b.md)）で削った` を移行スクリプトで移し、`note` が「で削った」になることを確かめる。

### 実機での確認

実装の後、novel-crawler で `--dry-run` を実行し、`docs/adr/0002-sqlite-persistence.md` と `docs/adr/0004-exclude-hameln.md` が移行できないADRとして報告されないことを確かめる。

## トレードオフ

- `置換` は novel-crawler だけの書き方である。adr-guard の旧形式の読み方に、一つのリポジトリの語を加えることになる。しかし、旧形式の値が Unknown のままだと、[ADR 0022](0022-status-transitions.md) の検査により、そのADRは新形式に移せない。移せないADRを残さないことを優先する。

## 参考文献

- [ADR 0012](0012-status-line-and-status-adt.md)
- [ADR 0022](0022-status-transitions.md)
- [ADR 0024](0024-migration-dry-run-and-error-count.md)
- [ADR 0026](0026-read-more-legacy-references.md)
- novel-crawler の `docs/adr/0002-sqlite-persistence.md`、`docs/adr/0004-exclude-hameln.md`
