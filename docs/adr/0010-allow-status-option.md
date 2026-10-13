# ADR 0010: ステータスの行の変更も `--allow-status` オプションで許可する

ステータス: 採択

## 文脈

adr-guard は、コミット済みのADRのうち、ステータスの行（行頭の `ステータス:`）の変更だけを常に許可してきた（[ADR 0001](0001-standalone-adr-guard.md)）。
[ADR 0009](0009-require-links-for-adr-references.md) では、ADRの参照にリンクを付けるだけの変更を、常にではなく `--allow-links` オプションを付けたときだけ許可するようにした。

同じ「許可される変更」なのに、ステータスの行は常に許可され、リンクはオプションで許可されるため、どの変更が許可されるかがhookやCIの設定から読み取れない。
ステータスの行の変更もオプションで許可するようにし、許可する変更をすべて呼び出し側で明示する形にそろえる。

## 実装すること

### `--allow-status`

- `pre-commit`、`check-range`、`claude-hook` に `--allow-status` を加える。
  - `--allow-status` を付けると、コミット済みのADRのステータスの行の変更を許可する（これまでの既定の挙動）。
  - `--allow-status` を付けないと、ステータスの行の変更も違反とする。コミット済みのADRは、`--allow-links` で許可したリンクの変更を除いて、一切変更できない。
- `--allow-status` と `--allow-links` は組み合わせられる。両方を付けると、ステータスの行の変更とリンクの変更の両方を許可する。
- 判定：変更の前後の内容に、`--allow-status` ならステータスの行の正規化を、`--allow-links` ならリンクをテキストに戻す処理を施してから比較し、一致すれば許可する。どちらも付けなければ、前後の内容がまったく同じでなければ違反とする。
- 違反のメッセージは、許可したものに合わせる。
  - `--allow-status` あり：`コミット済みのADRはステータスの行以外を変更できません: <パス>`（従来どおり）
  - `--allow-status` なし：`コミット済みのADRは変更できません（ステータスの行の変更は --allow-status で許可できます）: <パス>`
- [ADR 0008](0008-allow-intentional-adr-rewrites.md) の `ADR_GUARD_ALLOW` とトレーラー、削除・移動の禁止は変えない。

### composite action（`action.yml`）

- 入力 `allow-status`（既定は `false`）を加え、`true` のときは `check-range` に `--allow-status` を付ける。`allow-links` と同じく、許可する変更を使う側で明示する。

### `--help`、README、仕様

- 使い方の1行を次に変える。

  ```
  usage: adr-guard <pre-commit [--allow-status] [--allow-links]|claude-hook [--allow-status]|check-range [--allow-status] [--allow-links] <base> <head>|list [--status]|proposed [--status]|links [--fix [--include-committed]]|--version|--help>
  ```

- `--help` の「規則」「サブコマンド」「意図してADRを書きかえる場合」「例」に `--allow-status` を加える。例は `exec adr-guard pre-commit --allow-status --allow-links`、`adr-guard claude-hook --allow-status`、`check-range --allow-status --allow-links` とする。
- READMEの導入の手順（pre-commit hook、Claude Codeのhook、CIの例）に `--allow-status` を加え、既存の利用者向けに、v0.5.0 に上げるときは hook・Claude Codeのhook・actionに `--allow-status`（`allow-status: true`）を付けないとステータスの行も変更できなくなることを記載する。
- `docs/specifications.md` の規則を、許可する変更をオプションで選ぶ形に書きかえる。

### このリポジトリ

- `.githooks/pre-commit` を `pre-commit --allow-status --allow-links` に、`.claude/settings.json` を `claude-hook --allow-status` に、CIの `check-range` を `check-range --allow-status --allow-links` にする。

### リリース

- 既定の挙動が変わる（オプションを付けないとステータスの行も変更できなくなる）ため、マイナーバージョンを上げて `v0.5.0` としてリリースする。

## 実装しないこと

- ステータスの行の値の検査（`提案` から `採択` への変更だけを許可する、など）。
- `--allow-status` を既定で有効にする設定ファイルや環境変数。
- `claude-hook` の `--allow-links`（[ADR 0009](0009-require-links-for-adr-references.md) のとおり、リンクの修正は `adr-guard links --fix` をシェルから実行する）。

## テスト設計

### 既存のテスト

- ステータスの行の変更を許可することを確かめている既存のテスト（`pre-commit`、`claude-hook`、`check-range` と、モジュールのADRのもの）は、呼び出しに `--allow-status` を付ける。確かめる内容（許可されること）は変えない。
- [ADR 0009](0009-require-links-for-adr-references.md) の `--allow-links` のテストのうち、ステータスの行を含めて比較しているものは、必要に応じて `--allow-status` を付ける。
- そのほかの既存のテストは変えずにすべて通す。

### 追加するテスト

単体テスト（判定）：

- オプションなしでは、ステータスの行の変更もリンクの変更も許可しない。まったく同じ内容は許可する。
- `--allow-status` だけでは、ステータスの行の変更を許可し、リンクの変更を許可しない。
- `--allow-links` だけでは、リンクの変更を許可し、ステータスの行の変更を許可しない。
- 両方では、ステータスの行とリンクを同時に変えた変更を許可する。本文の変更はどの組み合わせでも許可しない。

結合テスト：

- `pre-commit`：ステータスの行だけの変更を、オプションなしでは拒否し、メッセージに `--allow-status` が含まれる。`--allow-status` では許可する。
- `check-range`：同様に、オプションなしでは拒否し、`--allow-status`（前に置く場合と後ろに置く場合）では許可する。
- `claude-hook`：ステータスの行だけの編集を、オプションなしでは終了コード2で拒否し、`--allow-status` では終了コード0で許可する。
- `claude-hook --allow-links`、`pre-commit --allow-status --allow-status` は終了コード64。
- `--help` の出力に `--allow-status` が含まれる。

### リリース

`v0.5.0` のリリース後、ダウンロードしたバイナリで、一時リポジトリのステータスの行だけの変更を `pre-commit` が拒否し、`pre-commit --allow-status` が許可することを確かめる。

## トレードオフ

- 既定の挙動が変わるため、v0.5.0 に上げてhookとactionに `--allow-status` を付け忘れると、ADRの採択などでステータスの行を変えられなくなる。失敗のメッセージで `--allow-status` を案内し、READMEに移行の手順を記載して補う。安全側（変更を拒否する側）に倒れるため、意図しない書きかえを通してしまうことはない。
- Claude Codeのhookは全体の設定（`~/.claude/settings.json`）に置かれることが多く、v0.5.0 に上げると、その設定にも `--allow-status` を付ける必要がある。付けないと、Claude CodeはADRのステータスを変えられなくなる。
- 呼び出しが長くなるが、許可する変更が設定から読み取れることを優先する。
