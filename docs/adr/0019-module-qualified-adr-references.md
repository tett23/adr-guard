# ADR 0019: モジュールの名前やディレクトリを前に付けたADRの参照を、そのディレクトリのADRへつなぐ

ステータス: 採択

## 文脈

`links` は、文書の中の `ADR NNNN` を参照として見つける（[ADR 0009](0009-require-links-for-adr-references.md)）。
参照先の候補は文書のパスだけから決め、文書に近い順に探す（[ADR 0009](0009-require-links-for-adr-references.md)、[0011](0011-nested-adr-directories.md)、[0014](0014-directory-name-variants.md)）。
モジュールの下の文書なら `docs/<モジュール>/adr`、次に `docs/adr` の順である。`docs/adr/` の文書の候補には、モジュールのディレクトリは入らない。

モジュールごとにADRのディレクトリを持つリポジトリ（[ADR 0002](0002-module-adr-directories.md)）では、ほかのモジュールのADRを指すときに、`golem-driver ADR 0015` のように、モジュールの名前を前に付けて書く。
横断のADRを、`docs/adr ADR 0187` のように、ディレクトリを前に付けて書くこともある。
いまの `links` は、前に付いた名前を見ないので、文書の近くのディレクトリに同じ番号のADRがあれば、そこへつないでしまう。

tett23/golem では、次の誤ったリンクが9個、main にコミットされている（2026-11-07 の時点、`6086e7c`）。

| 文書 | 書いてある参照 | できたリンク先 | 正しいリンク先 |
| --- | --- | --- | --- |
| `docs/adr/0172-continuous-double-dashes.md`（5個） | `golem-driver ADR 0015` | `docs/adr/0015-regex-styles.md` | `docs/golem-driver/adr/0015-vertical-dashes.md` |
| `docs/golem-html/adr/0003-cite-links-across-files.md`（4個） | `golem-doc ADR 0008` | `docs/adr/0008-first-release-verification.md` | `docs/golem-doc/adr/0008-multiple-index-and-bibliography.md` |

リンクの文字は `golem-driver [ADR 0015](0015-regex-styles.md)` となり、見た目は正しく、指す先だけが違う。
いまの `links` の検査は、リンクの無い参照だけを見るので、この誤りを見つけられない。
コミットしていない草稿でも、`links --fix` を流すたびに同じ誤りが起き、手で直す必要があった。

Golem の文書では、`ADR` の前に付く語は次のとおりである（main の `.md` を数えたもの）。

- モジュールの名前：`golem-driver`、`golem-doc` など、`docs/<名前>/adr/` があるもの。
- ディレクトリ：`docs/adr`（27個）、`docs/golem-driver`（1個）。
- ほかのリポジトリの名前：`peg-hs`、`shem`。すべて GitHub の URL へのリンクが付いている。

一方、英語の文書では `see ADR 0001` のように、名前ではない語が `ADR` の前に来ることが多い。
前の語をすべて名前とみなすことはできない。

## 実装すること

### 修飾子

`ADR NNNN` の直前に、ちょうど1つの半角の空白をはさんで置かれた語を「修飾子」の候補とする。

- 語は、`A-Z`、`a-z`、`0-9`、`.`、`_`、`/`、`-` の続きのうち、最も長いものとする。語の前は、行の始まりか、これらでない文字とする。語は英数字で始まる（箇条書きの `- ADR 0001` の `-` は語でない）。
- 語に `/` が含まれれば、「ディレクトリの修飾子」とする。
- `/` が含まれず、`-` が含まれれば、「名前の修飾子」とする。
- `/` も `-` も含まれない語は、その名前のモジュールのADRのディレクトリ（下の解決の規則で見つかるもの）があるときだけ、「名前の修飾子」とする。無ければ修飾子とせず、いまの参照として扱う（`see ADR 0001` は変わらない）。
- リンクのテキストの中に修飾子があるもの（`[golem-parser ADR 0007](...)`）も、同じ規則で修飾子とする。

### 修飾子の解決

修飾子から、参照先を探すADRのディレクトリを1つ決める。ADRのディレクトリは、作業ツリーにADR（`<4桁の数字>-` で始まる `.md`）が1つ以上あるディレクトリとする。

- 名前の修飾子 `<名前>`：文書のディレクトリから上にたどった各祖先（`candidate_dirs` と同じもの。近い祖先が先）について、`<祖先>/docs/<名前>/<ADRのディレクトリの名前>` を `ADR_DIR_NAMES` の順に調べ、最初にあるものとする。
- ディレクトリの修飾子 `<パス>`：先頭の `./` と末尾の `/` を除き、リポジトリのルートからのパスとする。`<パス>` 自身がADRのディレクトリならそれを、そうでなければ `<パス>/<ADRのディレクトリの名前>` を `ADR_DIR_NAMES` の順に調べ、最初にあるものとする。
- 解決したディレクトリの直下に `NNNN-*.md` がちょうど1つあれば、それを参照先とする。文書のパスによる候補の順は使わない。
- 参照先が文書自身なら、自身への参照として扱い、リンクにしない。

### リンクの無い参照（`links`、`links --fix`、pre-commit hook）

- 修飾子の付いた参照の参照先が見つかれば、修飾子を含めた `<修飾子> ADR NNNN` をリンクのテキストにする（`golem-driver ADR 0015` は `[golem-driver ADR 0015](../golem-driver/adr/0015-vertical-dashes.md)`）。報告の文は `<パス>:<行>: <修飾子> ADR NNNN にリンクがありません` とする。
- 修飾子の付いた参照の参照先が見つからなければ、リンクにせず、次のどれかを誤りとして報告する。ほかのディレクトリへはつながない。
  - `<パス>:<行>: <修飾子> ADR NNNN: <修飾子> のADRのディレクトリがありません`
  - `<パス>:<行>: <修飾子> ADR NNNN: <ディレクトリ> に ADR NNNN がありません`
  - `<パス>:<行>: <修飾子> ADR NNNN: <ディレクトリ> に ADR NNNN が2つ以上あります`
- 列挙（`golem-driver ADR 0015、0016`）の続く番号は、最初の番号の修飾子で解決する。続く番号のリンクのテキストは、いまと同じく番号だけとする（`[golem-driver ADR 0015](...)、[0016](...)`）。
- 修飾子の無い参照の振る舞いは変えない。

### リンクの付いた参照の検査

修飾子の付いた参照に、すでにインラインのリンクがあるもの（次の3つの形）を検査する。

- `[<修飾子> ADR NNNN](<リンク先>)`（正しい形）
- `<修飾子> [ADR NNNN](<リンク先>)`（修飾子がリンクの外にある形）
- 上の2つに区切り（`、` か `,` と空白）で続く `[NNNN](<リンク先>)`

リンク先が URL（`://` を含むもの）なら検査しない。
そうでなければ、リンク先を文書のディレクトリからの相対パスとして解決し（`#` 以降を除く）、修飾子から決まる参照先と比べる。

- リンク先が違えば、`<パス>:<行>: <修飾子> ADR NNNN のリンク先 <リンク先> は、<正しい参照先> ではありません` を誤りとして報告する。参照先が見つからなければ、上のリンクの無い参照と同じ文で報告する。
- リンク先が同じでも、修飾子がリンクの外にある形なら、`<パス>:<行>: <修飾子> ADR NNNN の修飾子がリンクの外にあります` を誤りとして報告する。
- 正しい形でリンク先も同じなら、何もしない。
- `--fix` では、参照先が見つかれば、正しい形 `[<修飾子> ADR NNNN](<正しい参照先への相対パス>)` に書きかえる（修飾子の外の形は、修飾子をリンクの中に入れる）。続く番号は、リンク先だけを直す。コミット済みのADRと記録は、いまと同じく `--include-committed` を付けたときだけ書きかえる。
- 修飾子の無いリンクは検査しない。

### pre-commit hook

ステージされた `.md` について、いまのリンクの無い参照の検査（[ADR 0009](0009-require-links-for-adr-references.md)）に、修飾子の付いた参照の誤りと、リンク先の食い違いを加える。

### `--help`、README、仕様

- `links` の `--help` と README に、修飾子の書き方と解決の規則、リンク先の検査を記載する。
- `docs/specifications.md` に、修飾子、解決の規則、報告の文を記載する。

### リリース

既存のリンクの検査が新たに失敗しうるため、マイナーバージョンを上げて `v0.10.0` としてリリースする。

## 実装しないこと

- 修飾子の無いリンクの、リンク先の検査（`[ADR 0015](...)` がどのディレクトリを指していても誤りとしない）。
- ほかのリポジトリの名前（`peg-hs`、`shem`）を、URL に解決すること。修飾子の付いた参照の参照先が見つからない場合は、手でリンクを付ける。
- 名前とディレクトリの対応の設定。名前は `docs/<名前>/<ADRのディレクトリの名前>` に固定する。
- `golem-driver の ADR 0015` のように、修飾子と `ADR` の間に `の` などがある書き方。
- 参照形式のリンク（`[ADR 0015][1]`）のリンク先の検査。
- `renumber`（[ADR 0018](0018-repair-duplicate-adr-numbers.md)）の、修飾子の付いたリンクの無い参照の扱いの変更。

## テスト設計

### 既存のテスト

既存のテストは変えずにすべて通す。

### 単体テスト（`src/links.rs`）

修飾子の検出：

- `golem-driver ADR 0015`、`（golem-driver ADR 0015）`、`docs/adr ADR 0187`、`[golem-parser ADR 0007](x.md)` は修飾子を持つ。
- `- ADR 0001`、`see ADR 0001`（`see` のモジュールが無い場合）、`golem-driverのADR 0015`、`golem-driver  ADR 0015`（空白が2つ）は修飾子を持たない。
- `golem-driver ADR 0015、0016` の `0016` は `golem-driver` の修飾子を持つ。

解決とリンク（Golem の9個と同じ形の文字列で）：

- `docs/adr/x.md` の中の `golem-driver ADR 0015` が `[golem-driver ADR 0015](../golem-driver/adr/0015-vertical-dashes.md)` になる（`docs/adr/0015-regex-styles.md` があっても）。
- `docs/golem-html/adr/x.md` の中の `golem-doc ADR 0008` が `../../golem-doc/adr/0008-multiple-index-and-bibliography.md` へのリンクになる（`docs/adr/0008-first-release-verification.md` があっても）。
- `docs/specifications.md` の中の `docs/adr ADR 0187` が `adr/0187-*.md` へのリンクになる。`docs/golem-driver ADR 0015` が `golem-driver/adr/0015-vertical-dashes.md` へのリンクになる。
- `golem-nope ADR 0001`（ディレクトリが無い）、`golem-driver ADR 0099`（番号が無い）は、リンクにならず、誤りとして報告される。
- 修飾子の無い `ADR 0015` は、いまと同じく文書の近くの候補から解決される。

リンク先の検査：

- `docs/adr/0172-x.md` の中の `golem-driver [ADR 0015](0015-regex-styles.md)` と、`docs/golem-html/adr/0003-x.md` の中の `golem-doc [ADR 0008](../../adr/0008-first-release-verification.md)` を、誤りとして報告し、`--fix` で `[golem-driver ADR 0015](../golem-driver/adr/0015-vertical-dashes.md)` と `[golem-doc ADR 0008](../../golem-doc/adr/0008-multiple-index-and-bibliography.md)` にする。
- リンク先が正しく、修飾子がリンクの外にある `golem-driver [ADR 0015](../golem-driver/adr/0015-vertical-dashes.md)` を誤りとして報告し、`--fix` で修飾子をリンクの中に入れる。
- `golem-driver ADR 0015、0016` が `[golem-driver ADR 0015](...)、[0016](...)` になる。
- `golem-driver [ADR 0015](0015-a.md)、[0016](0016-b.md)` の `0016` のリンク先も検査する。
- 正しい形で正しいリンク先、URL のリンク先（`[shem ADR 0001](https://...)`）、修飾子の無いリンクは報告しない。

### 結合テスト（`tests/hooks.rs`）

上の Golem と同じ配置のADR（`docs/adr/0008-*`、`0015-*`、`0187-*`、`docs/golem-driver/adr/0015-*`、`docs/golem-doc/adr/0008-*`）を持つ一時リポジトリで、次を確かめる。

- `links`：誤ったリンク先と、リンクの無い修飾子の付いた参照と、`golem-nope ADR 0001` を、パスと行と上の文で報告し、終了コード1。
- `links --fix --include-committed`：リンクの無い参照をリンクにし、誤ったリンク先を直す。`golem-nope ADR 0001` は直さずに報告し、終了コード1。それを消した後の `links` は終了コード0。
- `--fix` で直したコミット済みのADRの変更は、`pre-commit --allow-links` で許可される。
- pre-commit hook は、ステージした文書の誤ったリンク先を報告して拒否する。

### リリース

`v0.10.0` のリリース後、ダウンロードしたバイナリで、tett23/golem の main（`6086e7c`）の作業ツリーの複製に `links` を流し、上の9個が報告されることを確かめる。

## トレードオフ

- `-` も `/` も含まない語は、その名前のモジュールがあるときだけ修飾子とする。英語の文書の `see ADR 0001` を誤りにしないためである。そのため、`-` の無いほかのリポジトリの名前（`shem ADR 0009`）にリンクが無ければ、いまと同じく文書の近くのADRへつながる。Golem では、そうした参照にはすべて URL のリンクが付いており、`links` は触らない。
- `-` か `/` を含む語は、モジュールが無くても修飾子とし、参照先が無ければ誤りにする。`follow-up ADR 0003` のような英語も誤りになるが、ほかのディレクトリへ誤ってつなぐことよりも、報告して人に直させることを優先する。言いかえで避けられる。
- 名前の修飾子を `docs/<名前>/` に固定し、設定を持たない。[ADR 0002](0002-module-adr-directories.md) の配置の規則と同じであり、設定のファイルを増やさない。
- 修飾子の無いリンクのリンク先は検査しない。どのディレクトリを指すのが正しいかを、文書から決められないためである。
- 修飾子をリンクのテキストに含める。リンクの文字だけを見て、どのモジュールのADRへのリンクかが分かるようにするためである。Golem の main でも、修飾子をリンクの中に入れた形が1840個、外に置いた形が14個で、中に入れる形がふつうである。外に置いた形はリンク先が正しくても誤りとするため、既存の文書の書きかえが要るが、`links --fix` で直せる。
- リンク先の検査が加わるため、既存のリポジトリの `links` と CI が失敗しうる。`links --fix --include-committed` で直せ、その変更は `--allow-links`（[ADR 0015](0015-allow-link-changes.md)）で許可される。

## 参考文献

- tett23/golem の main（`6086e7c`）の [docs/adr/0172-continuous-double-dashes.md](https://github.com/tett23/golem/blob/6086e7c509e3ca73a23f975899c05ada066282d6/docs/adr/0172-continuous-double-dashes.md) と [docs/golem-html/adr/0003-cite-links-across-files.md](https://github.com/tett23/golem/blob/6086e7c509e3ca73a23f975899c05ada066282d6/docs/golem-html/adr/0003-cite-links-across-files.md)（誤ったリンクの実例）
