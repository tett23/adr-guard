# ADR 0012: `Status:` で始まるステータスの行を認め、ステータスの行を1つに限り、ステータスの値を代数的データ型で表す

ステータス: 採択→状態の一覧は[ADR 0016](0016-accepted-not-implemented-status.md)

## 文脈

adr-guard は、行頭が `ステータス:`（半角のコロン）の行をステータスの行とする（[ADR 0001](0001-standalone-adr-guard.md)、[0005](0005-list-and-proposed-subcommands.md)、[0010](0010-allow-status-option.md)）。
ステータスの行は次の3か所で使う。

- `--allow-status`：コミット済みのADRで、ステータスの行の変更を許可する（[ADR 0010](0010-allow-status-option.md)）。`normalize()` が、`ステータス:` で始まるすべての行を `ステータス:` だけに置きかえてから比較する。
- `proposed`：最初のステータスの行の値が `提案` で始まるADRを一覧する（[ADR 0005](0005-list-and-proposed-subcommands.md)、[0006](0006-proposed-status-option.md)）。
- `list --status`：最初のステータスの行の値をそのまま出力する（[ADR 0007](0007-list-status-option.md)）。

英語でADRを書くリポジトリでは、ステータスの行を `Status: Accepted` のように書く。adr-guard はこの行をステータスの行と認めない。

また、ステータスの行が無いADRや、ステータスの行が2つ以上あるADRも、いまはそのまま通る。
ステータスの行が無ければ、そのADRの状態は分からない。2つ以上あれば、どれが正しいか分からず、`--allow-status` で2行目以降も書きかえられる。
ADRには、ステータスの行がちょうど1つあるべきである。

さらに、ステータスの値は文字列のまま扱われ、どの値がありうるかはコードに表れていない。
`提案` で始まるかどうかの判定が `proposed` の中に直接書かれている。
英語の値を加えると判定が散らばるため、ありうる状態を代数的データ型（Rustの `enum`）として列挙し、値の解釈を1か所にまとめる。

## 実装すること

### ステータスの行の判定

行（改行と、行末の `\r` を除いたもの）が、行頭から `ステータス:` または `Status:`（見出しの大文字と小文字は区別し、コロンは半角）で始まるとき、その行をステータスの行とする。

- 行頭に空白や記号がある行（`- Status:`、`* Status:`、`  Status:`）は認めない。
- 見出しの後ろに空白を挟んでコロンを置く形（`Status :`）や、全角のコロン（`ステータス：`）は認めない。
- ステータスの値は、ステータスの行のコロンより後ろの、前後の空白を除いた文字列とする（従来どおり）。この文字列を「元の値」と呼ぶ。

| 行 | ステータスの行か | 元の値 |
|---|---|---|
| `ステータス: 提案` | はい（従来どおり） | `提案` |
| `ステータス:提案` | はい（従来どおり） | `提案` |
| `Status: Accepted` | はい | `Accepted` |
| `Status:Accepted` | はい | `Accepted` |
| `- Status: Accepted`、`* Status: proposed`、`- ステータス: 採択` | いいえ | |
| `ステータス：提案` | いいえ（従来どおり） | |
| `前置き ステータス: 提案` | いいえ（従来どおり） | |
| `status: accepted`、`STATUS: Accepted` | いいえ | |
| `Status：Accepted`、`Status : Accepted` | いいえ | |
| `  Status: Accepted`、`**Status:** Accepted`、`## Status` | いいえ | |
| `Statuses: x` | いいえ | |

### ステータスの行の数の検査

- ADR（[ADR 0002](0002-module-adr-directories.md)、[0011](0011-nested-adr-directories.md) のADRのパスにある `.md` のファイル）は、ステータスの行をちょうど1つ持たなければならない。
  - ステータスの行が無ければ、`<パス>: ステータスの行がありません（ステータス: または Status: で始まる行が要ります）` を違反とする。
  - ステータスの行が2つ以上あれば、`<パス>:<行>: ステータスの行が2つ以上あります（最初は<行>行目）` を、2つ目以降の行ごとに違反とする。
- この検査は次の場所で行う。
  - `pre-commit`：ステージされた追加・変更のADRの、インデックスの内容を検査する。違反があれば表示して終了コード1で終わる。`ADR_GUARD_ALLOW`（[ADR 0008](0008-allow-intentional-adr-rewrites.md)）はこの検査を許可しない。
  - 新しいサブコマンド `adr-guard check-status`：作業ツリーのすべてのADRを検査し、違反を標準エラーに表示する。違反があれば終了コード1、無ければ終了コード0で終わる。CIで使う。
  - composite action：`check-range` と `links` の後に `adr-guard check-status` を実行する。入力 `check-status`（既定は `true`）を加え、`false` のときは実行しない。既存のADRの移行が済むまで、使う側のリポジトリが検査を止められるようにする。
- 状態が `Unknown`（下記）であることは、どこでもエラーにしない。

### データ構造

ステータスの行を次の型で表す。`status(content)` は、ステータスの行がちょうど1つあれば `Some(StatusLine)` を、無いか2つ以上あれば `None` を返す。数の検査は、別に行の一覧を返す関数で行う。

```rust
/// ステータスの行（ADR 0012）。
struct StatusLine<'a> {
    /// 1から始まる行番号。
    line: usize,
    /// コロンより後ろの、前後の空白を除いた元の値。`list --status` などではこれを出力する。
    raw: &'a str,
    /// 元の値を解釈した状態。
    status: Status,
}

/// ADRの状態。
enum Status {
    /// `提案…`、`Proposed`。レビュー待ち。
    Proposed,
    /// `採択…`（`採択→` で始まるものを除く）、`Accepted`。
    Accepted,
    /// `採択→…`、`Superseded by …`。一部または全部が後のADRで変更された。
    Superseded(Vec<Change>),
    /// `却下…`、`Rejected`。
    Rejected,
    /// `破棄…`、`Discarded`。採択されずに取りやめた。
    Discarded,
    /// `廃止…`、`Deprecated`。かつて採択され、いまは効力が無い。
    Deprecated,
    /// 上のどれにも当たらない値（空の値を含む）。元の値は `StatusLine::raw` にある。
    Unknown,
}

/// 後のADRによる変更の1つ。
struct Change {
    /// 変わった部分（`ADRのディレクトリ` など）。全体が変わった場合や書かれていない場合は None。
    part: Option<String>,
    /// 変更したADRの番号（4桁の数字の文字列）。書かれていなければ空。
    by: Vec<String>,
}
```

### 元の値から状態への変換

元の値を、上から順に調べて最初に当たったものとする。

日本語（先頭の一致で判定する。`提案` の判定は従来どおり）：

| 元の値 | 状態 |
|---|---|
| `提案` で始まる | `Proposed` |
| `採択` に、0個以上の空白と `→`（U+2192）が続く | `Superseded`（変更の解析は下記） |
| `採択` で始まる | `Accepted` |
| `却下` で始まる | `Rejected` |
| `破棄` で始まる | `Discarded` |
| `廃止` で始まる | `Deprecated` |

英語（英字の大文字と小文字を区別せず、キーワードの直後が値の終わりか英数字以外の文字であるときに当たる）：

| 元の値 | 状態 |
|---|---|
| `Proposed` | `Proposed` |
| `Accepted` | `Accepted` |
| `Superseded` | `Superseded`（変更の解析は下記） |
| `Rejected` | `Rejected` |
| `Discarded` | `Discarded` |
| `Deprecated` | `Deprecated` |

どれにも当たらなければ `Unknown` とする。例えば `Draft`、`採用`、`Acceptedly`、空の値は `Unknown` である。

#### 変更（`Change`）の解析

状態の分類は先頭のキーワードだけで決まり、変更の解析はその後に行う。解析の結果によって状態が `Unknown` に変わることはない。

- 前処理：残りの文字列（`採択→` または `Superseded` より後ろ）のインラインのリンク `[テキスト](リンク先)` を、テキストだけに置きかえる。リンク先は使わない。
- ADRの番号の参照は、`ADR` に半角の空白1つまたは `-` と4桁の数字が続くもの（`ADR 0002`、`ADR-0005`）と、それに `、` か `,`（後ろに空白があってもよい）で区切って続く4桁の数字とする。前後が英数字に続く場合は参照としない。これは [ADR 0009](0009-require-links-for-adr-references.md) の参照の規則に、`ADR-0005` の形を加えたものである。`links` の検査の規則は変えない。
- 日本語（`採択→…`）：
  - 残りの文字列が、4桁の数字を `、` か `,` で区切って並べただけのもの（`採択→0004`）であれば、`part` が None で、`by` がそれらの番号の `Change` 1つとする。
  - そうでなければ、参照の並び（`ADR 0005、0006` のように区切りで続くものを1つの並びとする）ごとに `Change` を1つ作る。`part` は、前の並びの終わり（最初の並びでは文字列の先頭）からその並びの始まりまでの文字列から、前後の空白、先頭の `、` と `,`、末尾の `は` を除いたものとし、空なら None とする。`by` はその並びの番号とする。
  - 最後の並びより後ろに、空白と `、` を除いて文字が残れば、それを `part` とし、`by` が空の `Change` を加える。参照が1つも無い場合は、残りの文字列全体がこれに当たる。
  - 残りの文字列が空なら、変更は空とする。
- 英語（`Superseded …`）：残りの文字列の先頭の空白と `by`（大文字と小文字を区別しない）を除き、そこに含まれるすべての参照の番号を `by` とした、`part` が None の `Change` を1つ作る。

| 元の値 | 状態 |
|---|---|
| `採択→0004` | `Superseded([{ part: None, by: ["0004"] }])` |
| `採択→ADRのディレクトリは[ADR 0002](0002-module-adr-directories.md)` | `Superseded([{ part: Some("ADRのディレクトリ"), by: ["0002"] }])` |
| `採択→proposed の出力は[ADR 0006](...)、list の出力は[ADR 0007](...)` | `Superseded([{ Some("proposed の出力"), ["0006"] }, { Some("list の出力"), ["0007"] }])` |
| `採択→A、BはADR 0005、0006` | `Superseded([{ Some("A、B"), ["0005", "0006"] }])` |
| `採択→一部は別の方法に変更` | `Superseded([{ Some("一部は別の方法に変更"), [] }])` |
| `Superseded by [ADR-0005](0005-example.md)` | `Superseded([{ None, ["0005"] }])` |
| `superseded by ADR 0003, 0004` | `Superseded([{ None, ["0003", "0004"] }])` |
| `Superseded` | `Superseded([{ None, [] }])` |

### 状態を使う箇所

- `proposed`：ステータスの行がちょうど1つあり、状態が `Proposed` のADRを対象とする。日本語の判定（`提案` で始まる）は従来と同じで、英語の `Proposed` と `Status:` の行が加わる。
- `list --status`、`proposed --status`：`<ステータス>` には元の値をそのまま出力する（従来どおり）。状態の名前や日本語への置きかえはしない。ステータスの行が無いか2つ以上あるADRは、ステータスを空にする（`list` の出力の対象であることは変えない）。
- `--allow-status`（`normalize()`）：変更の前後の内容から、それぞれのステータスの行（`ステータス:` と `Status:` で始まるすべての行）を取り除いてから比較する（従来は `ステータス:` だけに置きかえていた）。
  - ステータスの値の変更に加え、見出しの変更（`Status: Accepted` から `ステータス: 採択`）と、ステータスの行が無かったADRへのステータスの行の追加を許可する。追加を許可するのは、ステータスの行が無いADRを、トレーラーを使わずに直せるようにするためである。
  - ステータスの行の位置を変える変更も許可される。ステータスの行を除く・2つ目を加える変更は、比較では許可されるが、上の数の検査で違反になる。

### `--help`、README、仕様

- `--help` の「ADRとステータス」に、ステータスの行の判定（`ステータス:`、`Status:`）、ステータスの行がちょうど1つ要ること、状態の一覧（日本語と英語の値）を加える。`proposed` の説明を「ステータスが提案（`提案…` または `Proposed`）のADR」に変える。`check-status` の説明を加える。
- `README.md` と `docs/specifications.md` の、ステータスの行の判定、数の検査、`check-status`、actionの入力 `check-status`、`--allow-status` の正規化、`proposed` の判定の記述を更新する。READMEに、ステータスの行が無いADRがあるリポジトリの移行の手順（`check-status` で一覧し、`--allow-status` を付けた検査のもとでステータスの行を加えてコミットする。移行が済むまでactionを `check-status: false` にする）を記載する。
- 使い方の1行に `check-status` を加える。
- `src/main.rs` の先頭のコメントの、ステータスの行についての記述を更新する。

### このリポジトリ

- CIの `adr` ジョブで、`cargo run -- links` の後に `cargo run -- check-status` を実行する。
- このリポジトリのADRは、すべて `ステータス:` の行をちょうど1つ持つため、移行は要らない（実装の前に `check-status` で確かめる）。

### リリース

- ステータスの行の判定、数の検査（pre-commitとCIの結果が変わる）、`--allow-status` の正規化、`proposed` の対象が変わるため、マイナーバージョンを上げて `v0.7.0` としてリリースする（`v0.6.0` は [ADR 0011](0011-nested-adr-directories.md) で使う）。

### 採択後のステータスの変更

採択後、ステータスの行の判定を定めていた次のADRのステータスを変更する。

- [ADR 0001](0001-standalone-adr-guard.md)：`採択→ADRのディレクトリは[ADR 0002](0002-module-adr-directories.md)、ステータスの行の判定は[ADR 0012](0012-status-line-and-status-adt.md)`
- [ADR 0005](0005-list-and-proposed-subcommands.md)：`採択→proposed の出力は[ADR 0006](0006-proposed-status-option.md)、list の出力は[ADR 0007](0007-list-status-option.md)、ステータスの判定は[ADR 0012](0012-status-line-and-status-adt.md)`
- [ADR 0007](0007-list-status-option.md)：`採択→ステータスの値の取り出しは[ADR 0012](0012-status-line-and-status-adt.md)`
- [ADR 0010](0010-allow-status-option.md)：`採択→ステータスの行の判定と正規化は[ADR 0012](0012-status-line-and-status-adt.md)`

## 実装しないこと

- 状態が `Unknown` のADRをエラーにする検査。必要になったら別のADRで、オプションで有効にする形で追加する。
- `list --status` で、元の値の代わりに状態の名前を出力すること。状態での絞り込み。
- ステータスの値の遷移の検査（`提案` から `採択` への変更だけを許可する、など。[ADR 0010](0010-allow-status-option.md) から引き続き実装しない）。
- `ステータス` と `Status` 以外の見出し（`状態:`、`status:`、`STATUS:`）、全角のコロン、リストの記号（`- `、`* `、`+ `）、太字（`**Status:**`）、行頭の空白。
- 見出し（`## Status`）の次の段落にステータスを書く Nygard の形式と、YAMLのfront matter（`status: accepted`）。
- `Superseded` 以外の英語の値（`Amended by …` など）からの変更の解析。
- フェンスで囲んだコードブロックの中の行を、ステータスの行の判定から除くこと（従来どおり、Markdownの構文は解析しない）。
- `links` の参照の規則への `ADR-0005` の形の追加。
- `claude-hook` でのステータスの行の数の検査（コミットの時点とCIで検査する）。

## テスト設計

既存の単体テスト（`status_values`、`non_proposed_statuses`、`status_line_only_change_is_allowed` など）と結合テストは、関数の戻り値の型に合わせて `status(...)` を `status(...).map(|s| s.raw)` のように読みかえる以外は変えず、すべて通す。確かめる内容は変えない。ただし、既存の `status_values` のうち、ステータスの行が2つある入力（`ステータス: 採択\nステータス: 提案\n` → `採択`）は、2つ以上あれば `None` になるため、期待する値を `None` に変える。

### 単体テスト

ステータスの行の判定（入力 → 元の値）：

- `ステータス: 提案` → `提案`、`Status: Accepted` → `Accepted`、`Status:Accepted` → `Accepted`、`Status: Accepted\r\n` → `Accepted`、`Status:` → 空の文字列。
- 次はステータスの行が無いとみなす（`None`）：`- Status: Accepted`、`* Status: proposed`、`- ステータス: 採択`、`status: accepted`、`STATUS: Accepted`、`Status：Accepted`、`Status : Accepted`、`  Status: Accepted`、`**Status:** Accepted`、`## Status\n\nAccepted\n`、`前置き Status: Accepted`、`Statuses: x`。
- 2つ以上あれば `None`、行の一覧はすべての行番号：`Status: Accepted\nステータス: 提案\n` → `None`、行は `[1, 2]`。

状態への変換（元の値 → 状態）：

- `提案`、`提案（レビュー中）`、`Proposed`、`proposed`、`PROPOSED` → `Proposed`
- `採択`、`採択（2026-10-01）`、`Accepted`、`accepted` → `Accepted`
- `却下`、`Rejected` → `Rejected`、`破棄`、`Discarded` → `Discarded`、`廃止`、`廃止（v2 で削除）`、`Deprecated` → `Deprecated`
- `Draft`、`採用`、`Acceptedly`、`Proposal`、空の文字列 → `Unknown`
- 「変更の解析」の表のすべての値と、`採択 → 0004`（`→` の前の空白）→ `Superseded([{ None, ["0004"] }])`、`採択→` → `Superseded([])`
- `Superseded by ADR 0003 on 2026-10-01` → `by` は `["0003"]`（日付の数字を番号としない）

正規化（`allowed_change` に `--allow-status` を与える）：

- `Status: Proposed` → `Status: Accepted`、`Status: Accepted` → `ステータス: 採択` を許可する。
- ステータスの行が無い内容に、ステータスの行を1つ加える変更を許可する。
- ステータスの行以外を変える変更を拒否する。
- `--allow-status` が無ければ、`Status:` の行の変更とステータスの行の追加を拒否する。

### 結合テスト

一時リポジトリで次を確かめる。

- `check-status`：ステータスの行がちょうど1つのADRだけなら終了コード0。ステータスの行が無いADR（`- Status: Accepted` だけのものを含む）と、2つあるADRがあれば、パスと行を表示して終了コード1。ADRでない `.md` は検査しない。`check-status x` は終了コード64。
- `pre-commit`：ステータスの行が無いADRと、2つあるADRをステージすると失敗する。ステージしていないADRは検査しない。`ADR_GUARD_ALLOW` を付けても失敗する。
- `pre-commit --allow-status`：コミット済みの `Status: Proposed` のADRの値の変更を許可する。ステータスの行が無いコミット済みのADRに `ステータス: 採択` を加える変更を許可する（加えた後は数の検査も通る）。
- `check-range --allow-status` と `claude-hook --allow-status`：`Status:` の行の値だけの変更を許可する。
- `proposed`：[ADR 0005](0005-list-and-proposed-subcommands.md) の一時リポジトリのファイルに、`docs/adr/0011-k.md`（`Status: Proposed`）、`docs/adr/0012-l.md`（`Status: Accepted`）、`docs/adr/0013-m.md`（`- Status: Proposed`）を加え、標準出力が従来の3行に `docs/adr/0011-k.md` を加えたものになる。
- `list --status`：加えた3件の行が `Proposed	…0011-k.md`、`Accepted	…0012-l.md`、`	…0013-m.md`（ステータスが空）である。
- `--help` の出力に `Status`、`Proposed`、`廃止`、`check-status` が含まれる。

### リリース

`v0.7.0` のリリース後、ダウンロードしたバイナリで、このリポジトリの `adr-guard check-status` が終了コード0で終わることを確かめる。

## トレードオフ

- ステータスの行が無いADRをエラーにするため、`- Status: Accepted` などで書いた古いADRがあるリポジトリは、v0.7.0 に上げると `check-status` とpre-commit hookで止まる。`--allow-status` のもとでステータスの行を加えれば直せるようにし、actionの `check-status: false` で移行まで止められるようにして補う。古い `- Status:` の行は本文として残る。
- `--allow-status` の正規化をステータスの行の取り除きに変えるため、ステータスの行の位置を変える変更も許可される。ステータスの行の位置はADRの内容ではないため許容する。
- `Status:` を認めるため、本文の行頭にある `Status: …`（`Status: 200 を返す` など）もステータスの行とみなされ、数の検査で違反になる。本文では行頭に置かない（箇条書きにする、など）ことで避ける。
- 却下した案：`- Status:` と `* Status:` も認める。古いADRやMADR 2のテンプレートを扱えるが、本文の箇条書きと一致しやすい。ステータスの行を1つに限る検査と組み合わせると、本文の `- Status: …` で誤って違反になるため認めない。
- 却下した案：解釈できない値をエラーにする。値の書き方はリポジトリごとに異なり（`提案（レビュー中）` など）、`Unknown` として受け入れる。
- `Unknown` は文字列を持たず、元の値は `StatusLine::raw` に置く。すべての状態で元の値を出力に使うため、`Unknown(String)` にすると同じ値を2か所に持つことになる。
- 状態を先頭のキーワードだけで分類し、変更の解析は分類に影響させない。`採択→` の後ろの書き方は自由な日本語であり、解析を厳密にすると、正しく書かれたステータスが `Unknown` になりうるためである。その代わり、`Change` の `part` と `by` は手がかりにとどまり、現時点ではどの出力にも使わない。
- `Deprecated`（`廃止`）を `Discarded`（`破棄`）と別の状態にする。`廃止` は「かつて採択され、いまは効力が無い」を意味し、採択されずに取りやめた `破棄` と意味が重ならないためである。
- `list --status` は元の値を出力するため、日本語と英語のADRが混ざるリポジトリでは、同じ状態が別の文字列で出力される。出力の互換性と、`採択→…` の補足を失わないことを優先する。
