# 仕様

## adr-guard

コミット済みのADRを変更できないようにするツール（[ADR 0001](adr/0001-standalone-adr-guard.md)、[0002](adr/0002-module-adr-directories.md)）。ステータスの行の変更とADRの参照のリンクだけの変更は、オプションで許可する（[ADR 0009](adr/0009-require-links-for-adr-references.md)、[0010](adr/0010-allow-status-option.md)）。
[tett23/send-to-kindle](https://github.com/tett23/send-to-kindle) の `tools/adr-guard` から、履歴ごと独立させた。

### 規則

- 対象は、リポジトリのルートからの相対パスに次のどちらかを含むファイル（[ADR 0002](adr/0002-module-adr-directories.md)、[0011](adr/0011-nested-adr-directories.md)）。`adr/` より下の深さは問わない。
  - `docs/adr/`
  - `docs/<モジュール>/adr/`。`<モジュール>` は空でない1つのパス要素（`/` を含まない）。モノレポでモジュールごとにADRを分ける場合に使う。
- 上の `adr` の代わりに、`adrs`、`decisions`、`decision` の名前も使える（[ADR 0014](adr/0014-directory-name-variants.md)）。例：`docs/adrs/`、`docs/decisions/`、`docs/server/adrs/`、`packages/server/docs/decisions/`。名前は完全に一致するものだけで、`docs/adr-old/`、`docs/architecture/`、`docs/decided/`、大文字の `docs/Decisions/`、`docs/ADR/` は対象外。
- 上の `docs/adr/` と `docs/<モジュール>/adr/` は、どの深さにあってもよい。`docs` より前の要素（`packages/server/` など）は空でなければ名前を問わない（[ADR 0011](adr/0011-nested-adr-directories.md)）。例：`packages/server/docs/adr/`、`apps/web/docs/api/adr/`、`src/docs/server/adr/`。
- `docs/a/b/adr/` のような2段以上のモジュールと、`packages/server/adr/` のように `docs` を挟まない場所は対象外。
- ステータスの行は、行頭が `ステータス:` か `Status:`（見出しの大文字・小文字は区別し、コロンは半角）の行とする（[ADR 0012](adr/0012-status-line-and-status-adt.md)）。`- Status:`、`* Status:`、`status:`、全角のコロン、行頭の空白は認めない。
- ADRはステータスの行をちょうど1つ持つ。ステータスの行が無いADRは `<パス>: ステータスの行がありません（ステータス: または Status: で始まる行が要ります）`、2つ以上あるADRは2つ目以降の行ごとに `<パス>:<行>: ステータスの行が2つ以上あります（最初は<行>行目）` を違反とする。`pre-commit`（初回を含むステージされた追加・変更のADR）と `check-status` で検査する。`ADR_GUARD_ALLOW` はこの違反を許可しない。
- 新規ADRは `Status: proposed` で一度コミットする。採択して実装を待つ場合は `accepted-pending`、採択または実装完了は `accepted`、却下は `rejected`、取り下げは `discarded`、廃止は `deprecated` を使う。修正時は `Status: superseded [{"part":null,"by":["docs/adr/0002-example.md"]}]` のように部分名と修正先パスを書く。初回コミットを含め、pre-commit と check-range は状態遷移を検査する。違反は書き換えの許可オプションやトレーラーでも終了コード1になる（[ADR 0022](adr/0022-status-transitions.md)）。
- 旧形式はそのまま読み取れる。ステータス行を書き換える際は新形式を使う。移行するには対象リポジトリのルートで adr-guard のソースにある `scripts/migrate-adr-status` を実行して差分を確認し、`--write` で適用する。
- 旧形式のステータスの値（コロンより後ろの、前後の空白を除いた元の値）は、次の状態として解釈する。日本語は先頭の一致、英語は大文字と小文字を区別せずキーワードの直後が英数字でないときに当たる。どれにも当たらない値は `Unknown` とし、違反にはしない。
  - `提案…`、`Proposed`：Proposed
  - `採択` に0個以上の空白と `→` が続くもの、`Superseded…`：Superseded（後ろの「変わった部分」と変更したADRの番号を解析する。分類には影響しない）
  - `採択` の直後に開き括弧（`（` か `(`）、`未実装`、閉じ括弧（`）` か `)`）が続くもの、`Accepted (not implemented)`（`Accepted` の後の半角の空白は1つあってもなくてもよく、括弧の揺れも同じ）：AcceptedNotImplemented（[ADR 0016](adr/0016-accepted-not-implemented-status.md)）。Superseded と Accepted より先に判定する。
  - `採択…`、`Accepted`：Accepted
  - `却下…`、`Rejected`：Rejected
  - `破棄…`、`Discarded`：Discarded
  - `廃止…`、`Deprecated`：Deprecated
- コミット済みのADRの変更、削除、リネームを違反とする。新規追加は許可する。
- 変更は、`pre-commit`、`check-range`、`claude-hook`、`codex-hook` のオプションで許可できる。変更の前後の内容に次の処理を施してから比較し、一致すれば許可する。どちらも付けなければ、前後がまったく同じときだけ許可する。
  - `--allow-status`（[ADR 0010](adr/0010-allow-status-option.md)、[0012](adr/0012-status-line-and-status-adt.md)）：ステータスの行（`ステータス:` と `Status:` で始まるすべての行）を取り除く。値と見出しの変更、ステータスの行が無かったADRへの追加、ステータスの行の位置の変更が許可される。`claude-hook` と `codex-hook` も受け付ける。
  - `--allow-links`（[ADR 0009](adr/0009-require-links-for-adr-references.md)、[0015](adr/0015-allow-link-changes.md)）：下記のとおり、リンクに関する違いを取り除く。`claude-hook` と `codex-hook` は受け付けない。
- 違反のメッセージは、`--allow-status` があれば `コミット済みのADRはステータスの行以外を変更できません: <パス>`、なければ `コミット済みのADRは変更できません（ステータスの行の変更は --allow-status で許可できます）: <パス>`。
- 監査や計測の記録も保護する（[ADR 0013](adr/0013-protect-audit-and-benchmark-records.md)）。
  - 記録は、ADRのパスと同じ形で `<名前>` が `audit`、`audits`、`benchmarks`、`benchmark` のいずれかのディレクトリ（どの深さの `docs/<名前>/`、`docs/<モジュール>/<名前>/`）以下の、すべてのファイル（[ADR 0014](adr/0014-directory-name-variants.md)）。大文字の `docs/Benchmarks/` などは対象外。
  - コミット済みの記録の変更、削除、リネームを違反とする。新規追加は許可する。`--allow-status` と `--allow-links` は記録には効かない。
  - 違反のメッセージは `コミット済みの記録（docs/audit/、docs/audits/、docs/benchmarks/、docs/benchmark/）は変更できません: <パス>` と `コミット済みの記録（docs/audit/、docs/audits/、docs/benchmarks/、docs/benchmark/）は削除・移動できません: <パス>`。記録の違反があれば、案内に `記録を直す場合は、新しい記録を作成してください。` を加える。
  - `pre-commit`、`check-range`、`claude-hook`、`codex-hook` で検査する。`claude-hook` と `codex-hook` では、コミット済みの記録の内容を変える編集を終了コード2で拒否する。
  - `links --fix` は、コミット済みの記録をコミット済みのADRと同じく既定では書きかえず、`--include-committed` で書きかえたときは `<パス>: コミット済みの記録です。コミットには ADR-Guard-Allow のトレーラーが要ります` を表示する。
- 意図してADRや記録を書きかえる場合は、次の方法で違反を許可する（[ADR 0008](adr/0008-allow-intentional-adr-rewrites.md)、[0013](adr/0013-protect-audit-and-benchmark-records.md)）。
  - `check-range`：コミットメッセージのトレーラー（最後の段落）に `ADR-Guard-Allow: <理由>` があるコミットでは、ADRと記録の変更・削除・リネームを違反としない。キーの大文字と小文字は区別しない。理由（前後の空白を除いた値）が空なら許可しない。許可したときは `check-range: <コミット>: ADR-Guard-Allow により許可しました: <理由>` を標準エラーに表示する。許可はトレーラーを付けたコミットだけに及ぶ。
  - `pre-commit`：環境変数 `ADR_GUARD_ALLOW` の値（前後の空白を除いたもの）が空でなければ、違反があっても成功し、`pre-commit: ADR_GUARD_ALLOW により許可しました: <値>` を標準エラーに表示する。
  - `claude-hook` と `codex-hook` には、環境変数やトレーラーによる許可の手段がない。
  - トレーラーは緊急用であり、リンクを付ける・外す・リンク先を直す変更には `--allow-links` を使う。
- `pre-commit` と `check-range` に `--allow-links` を付けると、コミット済みのADRの変更のうち、リンクだけが異なる変更を許可する（[ADR 0009](adr/0009-require-links-for-adr-references.md)、[0015](adr/0015-allow-link-changes.md)）。変更の前後で、次の処理をしてから比較する（`--allow-status` があれば、さらにステータスの行を取り除く）。
  - インラインのリンク `[テキスト](リンク先)` と参照形式のリンク `[テキスト][ラベル]` を、ADRの参照かどうかによらず、テキストだけに置きかえる。
  - 画像（直前が `!` のもの）は、`![代替テキスト](リンク先)` を `![代替テキスト]()` に、`![代替テキスト][ラベル]` を `![代替テキスト][]` に置きかえる。画像の追加・削除は許可されない。
  - リンクの参照定義の行（行頭から3つまでの空白、`[ラベル]:`、…）を行ごと取り除く。
  - フェンスで囲んだコードブロックとインラインのコードの中は変えない。自動リンク（`<URL>`）と本文のURLは本文として扱う。
- 文書（`.md`）の中のADRの参照にはリンクが要る（[ADR 0009](adr/0009-require-links-for-adr-references.md)）。
  - 参照は `ADR` と半角の空白と4桁の数字（前後が英数字でないもの）。`、` か `,`（後ろに空白があってもよい）で区切って続く4桁の数字も参照とする。
  - フェンスで囲んだコードブロック、インラインのコード（同じ数の `` ` `` の並びで囲んだ部分）、リンク（インラインと参照形式）の中と、ADR自身の番号は対象外。
  - 参照先は、文書のディレクトリから上にたどった各祖先 `<祖先>`（ルートを含み、`docs` の要素を含むものを除く）について、近い順に、文書が `<祖先>/docs/<モジュール>/` 以下（`<モジュール>` はADRのディレクトリの名前以外）なら `<祖先>/docs/<モジュール>/<名前>/`、続けて `<祖先>/docs/<名前>/` から、ファイル名が `<番号>-` で始まる `.md` がちょうど1つあるものを探す（[ADR 0011](adr/0011-nested-adr-directories.md)）。`<名前>` は `adr`、`adrs`、`decisions`、`decision` の順に並べる。同じ祖先の複数のディレクトリに同じ番号のADRがあれば、先に並ぶものを参照先とする（[ADR 0014](adr/0014-directory-name-variants.md)）。見つからない番号は検査しない。
  - リンクは `[ADR <番号>](<文書のディレクトリからの相対パス>)`、続く番号は `[<番号>](<相対パス>)`。
- ADRの参照の前に付けた修飾子（`golem-driver ADR 0015`、`docs/adr ADR 0187`）は、参照先のディレクトリを決める（[ADR 0019](adr/0019-module-qualified-adr-references.md)）。
  - 修飾子は、`ADR` の直前に半角の空白1つをはさんで置かれた語。語は `A-Z`、`a-z`、`0-9`、`.`、`_`、`/`、`-` の最も長い続きで、英数字で始まる。`/` を含めばディレクトリの修飾子、含まずに `-` を含めば名前の修飾子とする。どちらも含まない語は、その名前のモジュールのADRのディレクトリがあるときだけ名前の修飾子とする。リンクのテキストの先頭の `<語> ADR <番号>` も同じ。
  - ADRのディレクトリは、作業ツリーにADR（`<4桁の数字>-` で始まる `.md`）が1つ以上あるディレクトリ。名前の修飾子 `<名前>` は、文書の各祖先（近い順）の `<祖先>/docs/<名前>/<ADRのディレクトリの名前>` のうち最初にあるもの。ディレクトリの修飾子 `<パス>` は、先頭の `./` と末尾の `/` を除いたリポジトリのルートからのパスで、それ自身がADRのディレクトリならそれ、そうでなければ `<パス>/<ADRのディレクトリの名前>` のうち最初にあるもの。
  - そのディレクトリの直下の `<番号>-*.md` がちょうど1つなら参照先とする。文書による候補の順は使わない。参照先が文書自身なら対象外。列挙の続く番号は最初の番号の修飾子で解決する。
  - リンクの無い参照は、`<修飾子> ADR <番号>` 全体をリンクのテキストにする（`[golem-driver ADR 0015](../golem-driver/adr/0015-vertical-dashes.md)`）。報告は `<修飾子> ADR <番号> にリンクがありません`。
  - 参照先が無ければリンクにせず、`<修飾子> ADR <番号>: <修飾子> のADRのディレクトリがありません`、`…: <ディレクトリ> に ADR <番号> がありません`、`…: <ディレクトリ> に ADR <番号> が2つ以上あります` のどれかを誤りとして報告する。`--fix` でも直さない。
  - すでにリンクがあるもの（`[<修飾子> ADR <番号>](…)`、`<修飾子> [ADR <番号>](…)`、それらに区切りで続く `[<番号>](…)`）は、リンク先（URL を除く。`#` 以降を除いて文書のディレクトリから解決する）を参照先と比べる。違えば `<修飾子> ADR <番号> のリンク先 <リンク先> は、<参照先> ではありません`、同じでも修飾子がリンクの外にあれば `<修飾子> ADR <番号> の修飾子がリンクの外にあります` を報告する。`--fix` は `[<修飾子> ADR <番号>](<参照先への相対パス><#以降>)` に書きかえる。修飾子の無いリンクは検査しない。
- シンボリックリンクの `.md`（作業ツリーではパスそのものがシンボリックリンクのもの、インデックスではモードが `120000` のもの）は、`links`（`--fix` を含む）と pre-commit hook のADRの参照の検査と、`renumber` のリンクの書きかえと警告の対象にしない。参照先を探すADRの一覧からは除かない（[ADR 0020](adr/0020-skip-symlinked-documents.md)）。

### サブコマンド

- オプションは、サブコマンドの後ろならどの位置に置いてもよい。同じオプションを2回付けた場合や、受け付けないオプションを付けた場合は引数の誤りとする。
- 引数が次のいずれでもなければ、使い方の1行（`usage: adr-guard <pre-commit [--allow-status] [--allow-links] [--allow-renumber]|claude-hook [--allow-status]|codex-hook [--allow-status]|check-range [--allow-status] [--allow-links] [--allow-renumber] <base> <head>|trace [--list|--graph] <path>|list [--status]|proposed [--status]|unimplemented [--status]|links [--fix [--include-committed]]|check-status|check-numbers|renumber [--dry-run]|--version|--help>`）を標準エラーに表示して終了コード64で終わる。`--version`、`--help` に余分な引数が付いた場合や、`list`、`proposed` に `--status` 以外の引数が付いた場合も同じ。
- `adr-guard --version`、`adr-guard -V`（[ADR 0003](adr/0003-version-and-help-options.md)）
  - `adr-guard <Cargo.toml の version>` の1行を標準出力に表示し、終了コード0で終わる。
- `adr-guard --help`、`adr-guard -h`（[ADR 0003](adr/0003-version-and-help-options.md)）
  - 1行目に `--version` と同じ行、続けて概要と、各サブコマンドとオプションの説明を標準出力に表示し、終了コード0で終わる。
  - サブコマンドごとのヘルプ（`adr-guard pre-commit --help` など）はない。
- `adr-guard pre-commit [--allow-status] [--allow-links]`
  - ステージされたADRの変更のうち、HEADに存在するファイルについて、HEADとインデックスの内容を比較する。HEADがない（初回コミット）なら、追加したADRを空のツリーと比較する。
  - ステージされた追加・変更の `.md` のファイルの、インデックスの内容にリンクの無いADRの参照か、修飾子の付いた参照の誤り（[ADR 0019](adr/0019-module-qualified-adr-references.md)）があれば違反とする。参照先の解決には作業ツリーのADRを使う。`ADR_GUARD_ALLOW` はこの違反を許可しない（[ADR 0009](adr/0009-require-links-for-adr-references.md)）。
  - 違反があれば、`pre-commit: ` で始まるメッセージを表示して終了コード1で終わる。
- `adr-guard claude-hook [--allow-status]`
  - Claude CodeのPreToolUse hookとして、標準入力のJSONを読む。Write・Edit・MultiEditを対象とする。
  - 対象ファイルがADRで、かつHEADに存在する場合、ツール実行後の内容を計算してHEADと比較する。違反があれば終了コード2で終わる。
  - それ以外（JSONを読めない、gitリポジトリでない、対象外のファイルなど）は終了コード0で終わる。
- `adr-guard codex-hook [--allow-status]`（[ADR 0021](adr/0021-codex-hook.md)）
  - CodexのPreToolUse hookとして、標準入力のJSONを読む。`tool_name` が `apply_patch` か `Bash` のものを対象とし、`tool_input.command` の中のパッチを読む。パッチは、前後の空白を除くと `*** Begin Patch` になる最初の行から、`*** End Patch` になる次の行まで（ヒアドキュメントで包まれたものも読む）。
  - パッチのパスは、JSON の `cwd`（無ければ現在のディレクトリ）からの相対パスか絶対パスとして解決する。リポジトリのルートは `cwd` で `git rev-parse --show-toplevel` を実行して求める。
  - パッチのハンクごとに、パスがコミット済みのADRか記録（HEADに存在するもの）なら次のとおり判定する。
    - `*** Add File:`：新しい内容をHEADと比べる。ADRは上の比べ方（`--allow-status` があればステータスの行を除く）、記録はまったく同じときだけ許可する。
    - `*** Delete File:`：拒否する。`*** Move to:` のある `*** Update File:` は、元か移動先がコミット済みなら拒否する。
    - `*** Move to:` の無い `*** Update File:`：消す行（`-`）と足す行（`+`）で判定する。ADRで `--allow-status` があれば、それらがすべてステータスの行のとき許可する。それ以外は、消す行も足す行も無いときだけ許可する。パッチを当てた後の内容は計算しない。
  - 違反があれば、違反ごとのメッセージ（`claude-hook` と `pre-commit` と同じ文）と案内を標準エラーに表示して終了コード2で終わる。それ以外（JSONを読めない、対象外のツール、パッチが無いか `*** End Patch` で終わらない、gitリポジトリでないなど）は終了コード0で終わる。
- `adr-guard check-range [--allow-status] [--allow-links] <base> <head>`
  - `<base>..<head>` のマージ以外の各コミットを、親コミットとの差分で検査する。
  - `<base>` がコミットとして存在しなければ、`<head>` から辿れるすべてのコミットを検査する。
  - 違反したコミットとファイルをすべて表示し、1件でもあれば終了コード1で終わる。
- `adr-guard trace [--list|--graph] <path>`（[ADR 0023](adr/0023-trace-amendments.md)）
  - 作業ツリー内の起点からステータス行の修正先を記載順に深さ優先でたどる。パス引数は絶対または呼び出し元からの相対パスで指定し、出力のパスはリポジトリルートからの相対パスとする。
  - 既定の `--list` はヘッダーなしの4列TSVで種別、元のパス、修正先のパス、部分名を出す。起点は `root` で第三列にそのパスを置き、修正先ごとに `edge` を出す。合流先の子は一度だけ展開する。
  - `--graph` は枝線で親子関係を描き、合流先に `[表示済み]` を付ける。両形式でフィールド内のバックスラッシュ、タブ、CR、LF はエスケープする。
  - 正常時は終了コード0とする。到達範囲の循環、参照先の欠落や曖昧さ、読み込みや構文のエラーは標準エラーに出し、標準出力を空にして1で終わる。オプションの併用や引数の過不足は64で終わる。
- `adr-guard list [--status]`（[ADR 0005](adr/0005-list-and-proposed-subcommands.md)、[0007](adr/0007-list-status-option.md)）
  - 作業ツリーにあるすべてのADRのパスを、1行に1つずつ標準出力に出力し、終了コード0で終わる。
  - `--status` では、`<ステータス><タブ><パス>` の形で出力する。ステータスは元の値のまま。ステータスの行が無いか2つ以上あるADRはステータスを空にする（行はタブで始まる）。
- `adr-guard proposed [--status]`（[ADR 0005](adr/0005-list-and-proposed-subcommands.md)、[0006](adr/0006-proposed-status-option.md)）
  - 作業ツリーにあるADRのうち、ステータスが提案のもののパスを、1行に1つずつ標準出力に出力し、終了コード0で終わる。
  - `--status` では、`<ステータス><タブ><パス>` の形で出力する。
  - ステータスの行がちょうど1つあり、その状態が Proposed のADRを対象とする（[ADR 0012](adr/0012-status-line-and-status-adt.md)）。
- `adr-guard links [--fix [--include-committed]]`（[ADR 0009](adr/0009-require-links-for-adr-references.md)）
  - 作業ツリーの `.md`（`list` と同じ列挙）にある、リンクの無いADRの参照を `<パス>:<行>: ADR <番号> にリンクがありません` の形で、修飾子の付いた参照の誤りを上の文で、標準エラーに表示する。1件でもあれば修正の方法を表示して終了コード1、無ければ終了コード0で終わる。
  - `--fix` では、コミット済みのADR（ADRのパスにあって `HEAD` に存在するもの）を除いて、参照をリンクに置きかえ、修飾子の付いたリンクを直す。直せない誤り（修飾子の参照先が無いもの）は報告し、終了コード1で終わる。コミット済みのADRに参照が残れば終了コード1で終わる。
  - `--include-committed`（`--fix` と一緒にだけ使える）では、コミット済みのADRも書きかえる。
- `adr-guard check-status`（[ADR 0012](adr/0012-status-line-and-status-adt.md)）
  - 作業ツリーのすべてのADR（`.md`）のステータス行がちょうど1つあるか、予約した `Status: superseded [` で始まる行が新形式の構文に従うかを検査し、違反を `check-status: ` に続けて標準エラーに表示する。違反があれば終了コード1、無ければ終了コード0で終わる。
- `adr-guard unimplemented [--status]`（[ADR 0016](adr/0016-accepted-not-implemented-status.md)）
  - ステータスの行がちょうど1つあり、その状態が AcceptedNotImplemented のADRのパスを、1行に1つずつ標準出力に出力し、終了コード0で終わる。
  - `--status` では、`<ステータス><タブ><パス>` の形で出力する。ステータスは元の値のまま。
- `adr-guard check-numbers`（[ADR 0018](adr/0018-repair-duplicate-adr-numbers.md)）
  - 作業ツリーの各ADRのディレクトリの直下にある、ファイル名が `<4桁の数字>-` で始まる `.md` のADRについて、同じディレクトリで同じ番号のものが2つ以上あれば重複とする。別のディレクトリの同じ番号は重複としない。
  - 重複ごとに `check-numbers: <ディレクトリ>: ADR <番号> が重複しています: <ファイル名>, <ファイル名>` を標準エラーに表示し、`adr-guard renumber で修復できます。` を表示して終了コード1で終わる。重複が無ければ終了コード0。pre-commit hookでは検査しない。
- `adr-guard renumber [--dry-run]`（[ADR 0018](adr/0018-repair-duplicate-adr-numbers.md)）
  - 重複した番号のADRのうち、`git log --diff-filter=A` で最初に追加されたコミットの時刻が最も早いもの（同じか未コミットならファイル名の昇順で先のもの）を残し、残りに、ディレクトリの最大の番号の次から順に番号を割り当てる。
  - ファイル名の番号を変え（`<名前>` の部分は変えない）、1行目が `# ADR <旧番号>:` ならその番号を変える。`git mv` はしない。
  - 作業ツリーの `.md`（記録を除く）のインラインのリンクのうち、リンク先（文書のディレクトリからの相対パス）が振りなおしたADRを指すものについて、リンク先のファイル名を変え、テキストが `ADR <旧番号>` か `<旧番号>` なら番号を変える。URLや絶対パスのリンク先は変えない。
  - 振りなおしの一覧（`<ディレクトリ>: <旧ファイル名> -> <新ファイル名>`）と、直した参照の件数を標準エラーに表示する。リンクの無い参照のうち、振りなおした旧番号で、そのディレクトリが参照先の候補になるものは、`<パス>:<行>: ADR <番号> はどちらのADRを指すか分からないため、直していません` を表示する。
  - `--dry-run` では、ファイルを変えずに一覧だけを表示する。重複が無ければ何もせずに終了コード0。
- `--allow-renumber`（`pre-commit` と `check-range`。[ADR 0018](adr/0018-repair-duplicate-adr-numbers.md)）
  - 削除されたADRと追加されたADRが同じディレクトリにあり、ファイル名の番号の後ろが同じで番号が違い、変更前の状態で削除されたADRの番号がそのディレクトリで重複しており、追加されたADRの内容がタイトルの行の番号を除いて削除されたADRの内容と同じ（`--allow-status`、`--allow-links` の正規化を含む）組を、番号の振りなおしとして違反にしない。
  - 同じ変更のほかのADRの変更では、リンクのテキストの番号のうち、振りなおした新しい番号を古い番号に読みかえてから比較する。
- `list` と `proposed` と `unimplemented` と `links` と `check-status` と `check-numbers` と `renumber` で共通
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

- 入力は `base`、`head`（必須）と `version`、`allow-status`（既定は `false`）、`allow-links`（既定は `false`）、`links`（既定は `true`）、`check-status`（既定は `true`）、`check-numbers`（既定は `true`）、`allow-renumber`（既定は `false`）（任意）。
- `version` を省略すると、action自身の `Cargo.toml` のバージョン（`v<version>`）を使う。
- リリースから `x86_64-unknown-linux-gnu` のアーカイブと `SHA256SUMS` をダウンロードし、チェックサムを検証してから、ワークスペースで `adr-guard check-range <base> <head>` を実行する。`allow-status`、`allow-links` が `true` なら、それぞれ `--allow-status`、`--allow-links` を付ける。`links` が `true` なら、続けて `adr-guard links` を実行する（[ADR 0009](adr/0009-require-links-for-adr-references.md)）。`check-status` が `true` なら、続けて `adr-guard check-status` を実行する（[ADR 0012](adr/0012-status-line-and-status-adt.md)）。`check-numbers` が `true` なら、続けて `adr-guard check-numbers` を実行する。`allow-renumber` が `true` なら、`check-range` に `--allow-renumber` を付ける（[ADR 0018](adr/0018-repair-duplicate-adr-numbers.md)）。
- Linux（x86_64）以外のランナーではエラーにする。

## 開発プロセス

### ADR

- 実装の前に `docs/adr/` にADRを作成し、人間のレビューを経てからコミットする。
- コミット済みのADRはステータスの行を除いて変更できない（このリポジトリのhookとCIは `--allow-status --allow-links` で検査する）。仕様を変更する場合は新しいADRを作成する。
- このリポジトリでは、開発中の adr-guard 自身（`cargo run`）で、pre-commit hook、Claude Codeのhook（`.claude/settings.json`）、Codexのhook（`.codex/hooks.json`、[ADR 0021](adr/0021-codex-hook.md)）、CIで強制する。

### CI

- `main` へのpushとプルリクエストで、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` と、ADRの検査（`cargo run -- check-range --allow-status --allow-links`）、リンクの検査（`cargo run -- links`）、ステータスの行の検査（`cargo run -- check-status`）を実行する。

## 不明な挙動

利用する側のリポジトリで観察したが、原因を突き止めていない挙動をここに記録する。
再現の手順がわかったら、ADR を作成して直すか、仕様どおりであることをここに書く。

### `core.hooksPath` が絶対パスに書き換わる

- 観察した場所：tett23/golem（2026-10-15 から 2026-10-16）。一つのリポジトリに十を超える git worktree を作り、それぞれで並行してコミットした。
- 観察したこと：共有の `.git/config` の `core.hooksPath` を相対パスの `.githooks` にしておいたが、三回、元のチェックアウトの `.githooks` を指す絶対パスに書き換わっていた。
- 影響：元のチェックアウトは古いブランチにあり、その `.githooks/pre-commit` は `--allow-status` と `--allow-links` を持たない古いものだった。そのため、各 worktree のコミットが古いフックで検査され、ステータスの行の変更が拒否された。
- わかっていないこと：書き換えたのが adr-guard か、git か、worktree を作る道具か、それ以外かはわからない。golem のリポジトリの中のスクリプトと文書は、どれも相対パスの `git config core.hooksPath .githooks` だけを書いている。
- 回避：`git -c core.hooksPath=.githooks commit` で、その worktree のフックを指定してコミットした。

### pre-commit がマージのコミットのステータスの行の変更を拒否する

- 観察した場所：tett23/golem（2026-10-15 から 2026-10-16）、adr-guard 0.8.1、`exec adr-guard pre-commit --allow-status --allow-links`。
- 観察したこと：`origin/main` をブランチにマージし、`origin/main` の側でコミット済みの ADR のステータスの行が変わっているとき、マージのコミットが pre-commit で拒否されることがあった。複数の worktree で、別々に起きた。
- 再現しないこと：同じ状態で `adr-guard pre-commit --allow-status --allow-links` を手で実行すると、終了コードは 0 だった。同じ範囲を `adr-guard check-range` にかけても通った。フックに環境変数を書き出す行を一時的に足すと、同じコミットが通った。
- 推測：フックの中では `GIT_DIR` などが worktree の gitdir に設定される。上の `core.hooksPath` の書き換えで、古いフックが動いていた可能性もある。どちらも確かめていない。
- 回避：フックを飛ばさずに、マージをやり直すか、`git -c core.hooksPath=.githooks` でコミットした。

## 破棄したADR

なし
