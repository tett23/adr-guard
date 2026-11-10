# ADR 0021: Codex の PreToolUse hook で、コミット済みのADRと記録の編集を拒否する

ステータス: 採択（未実装）

## 文脈

adr-guard は、コミット済みのADRと記録（[ADR 0013](0013-protect-audit-and-benchmark-records.md)）の変更を、gitのpre-commit hook、Claude Codeのhook（`claude-hook`）、CIの三か所で止める（[ADR 0001](0001-standalone-adr-guard.md)）。
このうち `claude-hook` は、AIのエージェントが書きかえようとした時点で編集を止め、理由をエージェントに返す。
pre-commit hookとCIは、書きかえた後のコミットの時点でしか止められない。

OpenAI の Codex CLI（以下 Codex）にも、Claude Code と同じ形の PreToolUse hook がある。
次の事実は、openai/codex のタグ `rust-v0.166.0`（`6ff1e1ca3b`）のソースで確かめた。手元に入っている Codex は `codex-cli 0.166.0` である。

- hook の機能（`hooks`）は Stable で、既定で有効である（`codex-rs/features/src/lib.rs`）。
- hook の設定は、設定のフォルダ（`~/.codex/` と、リポジトリの `.codex/`）の `hooks.json` に、Claude Code と同じ形（`{"hooks":{"PreToolUse":[{"matcher":…,"hooks":[{"type":"command","command":…}]}]}}`）で書く（`codex-rs/hooks/src/engine/discovery.rs` の `load_hooks_json`）。`matcher` は正規表現である。
- hook は、標準入力に `tool_name`、`tool_input`、`cwd` などを含む JSON を受け取る（`codex-rs/hooks/schema/generated/pre-tool-use.command.input.schema.json`）。
- hook が終了コード2で終わり、標準エラーに理由を書くと、ツールの実行を止める（`codex-rs/hooks/src/events/pre_tool_use.rs`）。標準エラーが空なら、止めずに hook の失敗として扱う。
- Codex は、ファイルの編集に `apply_patch` のツールを使う。hook には、`tool_name` が `apply_patch`、`tool_input` が `{"command": <パッチの全文>}` で渡る（`codex-rs/core/src/tools/handlers/apply_patch.rs`）。`matcher` では、`apply_patch` は `Write` と `Edit` の名前でも一致する（`codex-rs/core/src/tools/hook_names.rs`）。
- シェルのコマンド（`exec_command`）の中身がパッチであれば、Codex はそれを横取りして `apply_patch` として適用する。このとき hook には、`tool_name` が `Bash`、`tool_input` が `{"command": <コマンドの全文>}` で渡る（`codex-rs/core/src/tools/handlers/unified_exec/exec_command.rs`）。

`claude-hook` は、`tool_input` の `file_path` が無い入力を対象外として終了コード0で終わる。
そのため、Codex に `claude-hook` を設定すると、すべての編集を通してしまう。
Codex の編集を止めるには、パッチを読む別のサブコマンドが要る。

パッチの形式は、`codex-rs/apply-patch/src/parser.rs` の文法による。
パッチは `*** Begin Patch` の行で始まり、`*** End Patch` の行で終わる。
その間に、ファイルごとの塊（以下、ハンク）を並べる。

- `*** Add File: <パス>`：ファイルを作る。続く `+` で始まる行が、`+` を除いて新しい内容になる。ファイルがあれば上書きする。
- `*** Delete File: <パス>`：ファイルを消す。
- `*** Update File: <パス>`：ファイルを変える。直後に `*** Move to: <パス>` があれば、変えた内容を移動先に書き、元のファイルを消す。続く行は、`@@` で始まる位置の目印の行と、` `（前後の文脈）、`-`（消す行）、`+`（足す行）で始まる行と、`*** End of File` の行である。

パスは、hook に渡る `cwd` からの相対パスか、絶対パスである。
パーサーは、目印の行の前後の空白を許す。

## 実装すること

### サブコマンド

- `adr-guard codex-hook [--allow-status]` を足す。Codex の PreToolUse hook から呼ぶ。
- `--allow-status` は、`claude-hook` と同じく、コミット済みのADRのステータスの行の変更を許可する。`--allow-links` は受け付けない。
- 使い方の1行、`--help`、README、`docs/specifications.md` に `codex-hook` を加える。

### 入力の読み方

- 標準入力の JSON の `tool_name` が `apply_patch` なら、`tool_input.command` をパッチを含む文字列とする。
- `tool_name` が `Bash` なら、`tool_input.command` をパッチを含む文字列とする。ただし、パッチの始まりの行が無ければ対象外とする。
- パッチは、文字列の中の、前後の空白を除くと `*** Begin Patch` になる最初の行から、前後の空白を除くと `*** End Patch` になる次の行までとする。ヒアドキュメント（`apply_patch <<'EOF'` など）で包まれたパッチも、この切り出しで読む。
- パスの基準のディレクトリは、JSON の `cwd` とする。無ければ、現在のディレクトリとする。リポジトリのルートは、基準のディレクトリで `git rev-parse --show-toplevel` を実行して求める。
- 次の場合は、何もせず終了コード0で終わる。JSON を読めない。`tool_name` が上の2つ以外である。パッチが無いか、`*** End Patch` で終わらない。基準のディレクトリがgitのリポジトリの中でない。

### 判定

パッチの各ハンクを、次の規則で判定する。
「コミット済みのADR」と「コミット済みの記録」は、リポジトリのルートからの相対パスがADRか記録のパスで、HEADに存在するものをいう。リポジトリの外のパスは対象外とする。

- `*** Add File:`：パスがコミット済みのADRなら、新しい内容とHEADの内容を、`claude-hook` の Write と同じく比べる（`--allow-status` があればステータスの行を除いて比べる）。パスがコミット済みの記録なら、内容がHEADとまったく同じときだけ許可する。
- `*** Delete File:`：パスがコミット済みのADRか記録なら拒否する。
- `*** Update File:` に `*** Move to:` があるもの：元のパスか移動先のパスがコミット済みのADRか記録なら拒否する。
- `*** Update File:` に `*** Move to:` が無いもの：パスがコミット済みのADRか記録なら、消す行（`-` で始まる行）と足す行（`+` で始まる行）を、先頭の1文字を除いて調べる。
  - コミット済みのADRで `--allow-status` があれば、消す行と足す行がすべてステータスの行（`ステータス:` か `Status:` で始まる行）のときだけ許可する。
  - それ以外は、消す行も足す行も無いときだけ許可する。

1つでも拒否するハンクがあれば、拒否するハンクのメッセージをすべて標準エラーに表示し、終了コード2で終わる。
拒否するハンクが無ければ、終了コード0で終わる。

### メッセージ

メッセージには、`pre-commit` と `claude-hook` と同じ文を使う。

- ADRの変更：`claude-hook` と同じ（`--allow-status` の有無で文が変わる）。
- ADRの削除と移動：`コミット済みのADRは削除・移動できません: <パス>`。
- 記録の変更：`コミット済みの記録（docs/audit/、docs/audits/、docs/benchmarks/、docs/benchmark/）は変更できません: <パス>`。
- 記録の削除と移動：`コミット済みの記録（docs/audit/、docs/audits/、docs/benchmarks/、docs/benchmark/）は削除・移動できません: <パス>`。

メッセージの後には、ADRの違反があれば `仕様を変更する場合は、新しいADRを作成してください。` を、記録の違反があれば `記録を直す場合は、新しい記録を作成してください。` を表示する。

### このリポジトリの設定

- `.codex/hooks.json` を足し、開発中の adr-guard（`cargo run`）の `codex-hook --allow-status` を、`matcher` を `^(apply_patch|Bash)$` として呼ぶ。`cargo run` の `--manifest-path` は、`git rev-parse --show-toplevel` で求めたルートの `Cargo.toml` とする。
- README の導入の節に、Codex の hook の設定の例を加える。

## 実装しないこと

- パッチを実際に当てた後の内容の計算。`*** Update File:` は、消す行と足す行だけで判定する。Codex の位置合わせ（空白を無視した一致など）を再実装しない。
- `--allow-links` の受け付け。`claude-hook` と同じく、リンクの変更はコミットの時点の検査に任せる。
- パッチ以外のシェルのコマンド（`sed -i`、`mv`、`rm` など）による変更の検査。`claude-hook` も Bash を検査しない。これらは pre-commit hook とCIで止める。
- `claude-hook` の変更。`claude-hook` は、これまでどおり Claude Code の入力だけを扱う。
- `~/.codex/hooks.json` など、利用者の環境の設定の変更。
- 環境変数やトレーラーによる許可の手段。`claude-hook` と同じく持たない。

## テスト設計

### 既存のテスト

既存のテストは変えずにすべて通す。

### 単体テスト（パッチの読み方）

- `*** Begin Patch` から `*** End Patch` までの、Add、Delete、Update、Update と Move のハンクを、パスと消す行と足す行に分けて読む。
- `*** Environment ID:` の行、`@@` の目印の行、` ` で始まる文脈の行、`*** End of File` の行を、消す行と足す行に含めない。
- 目印の行の前後に空白があっても読む。
- ヒアドキュメント（`apply_patch <<'EOF'` の行と `EOF` の行）で包まれたパッチを読む。
- `*** Begin Patch` が無いか、`*** End Patch` で終わらない文字列は、パッチとして読まない。

### 結合テスト（`tests/hooks.rs`）

`docs/adr/0001-a.md`（ステータスの行と本文を持つ）と `docs/audit/a.md` をコミットした一時リポジトリで、`codex-hook` に JSON を渡して次を確かめる。

- `apply_patch` で `docs/adr/0001-a.md` のステータスの行だけを変えるパッチは、`--allow-status` があれば終了コード0、無ければ終了コード2で終わる。
- 本文を変えるパッチは、`--allow-status` があっても終了コード2で終わり、標準エラーに `docs/adr/0001-a.md` と `仕様を変更する場合は、新しいADRを作成してください。` を含む。
- `*** Add File:` で `docs/adr/0001-a.md` を同じ内容で上書きするパッチは終了コード0、違う内容では終了コード2で終わる。
- `*** Delete File: docs/adr/0001-a.md` は終了コード2で終わり、標準エラーに `コミット済みのADRは削除・移動できません: docs/adr/0001-a.md` を含む。
- `docs/adr/0001-a.md` を `*** Move to:` で移すパッチと、ほかのファイルを `docs/adr/0001-a.md` へ移すパッチは、どちらも終了コード2で終わる。
- 未コミットのADR（`docs/adr/0002-b.md`）とADRでないファイル（`README.md`）を変えるパッチは、終了コード0で終わる。
- `docs/audit/a.md` のステータスの行だけを変えるパッチは、`--allow-status` があっても終了コード2で終わり、標準エラーに `記録を直す場合は、新しい記録を作成してください。` を含む。
- 2つのファイルを変えるパッチで、1つが違反なら終了コード2で終わり、標準エラーには違反のファイルだけを含む。
- `cwd` が `docs` のとき、`adr/0001-a.md` の本文を変えるパッチは終了コード2で終わる。絶対パスで書いたパッチも同じく拒否する。
- `tool_name` が `Bash` で、`apply_patch <<'EOF'` で包んだ本文を変えるパッチは終了コード2で終わる。パッチを含まない `Bash`（`ls`）は終了コード0で終わる。
- 読めない JSON、`tool_name` が `Read` の入力、`*** End Patch` の無いパッチ、gitのリポジトリの外の `cwd` は、終了コード0で終わる。

### 実機での確認

自動テストを書けないため、実装の後に次を動かして確かめる。

- 手元の Codex（`codex-cli 0.166.0`）で、ADRをコミットした一時リポジトリに `.codex/hooks.json` を置き、`codex exec` でADRの本文の書きかえを頼むと、hook が編集を止め、ADRの内容が変わらないこと。
- 同じリポジトリで、ステータスの行の書きかえを頼むと、編集が通ること。
- Codex がプロジェクトの hook を動かす前に、利用者による信頼の操作を求めるかどうか（`discovery.rs` に信頼の状態の扱いがあるが、操作の手順は未確認）。求める場合は、その手順を README に書く。

## トレードオフ

- `*** Update File:` を、内容を計算せずに消す行と足す行だけで判定する。ステータスの行だけを変えるパッチは、当たれば、ステータスの行を除いた内容を変えないため、許可しても安全である。一方、同じ行を消して足し直すだけのパッチ（内容が変わらないもの）も拒否する。Codex の位置合わせの再実装を避けるため、この過剰な拒否を受け入れる。
- 移動は、元と移動先のどちらかがコミット済みなら一律に拒否する。移動先の内容を計算するには元のファイルとパッチの適用が要るため、判定を単純に保つことを優先する。
- `Bash` の入力は、パッチを含むものだけを調べる。`matcher` に `Bash` を含めると、すべてのシェルのコマンドで adr-guard が起動する。パッチを含まない入力はJSONを読むだけで終わるため、その費用を受け入れる。
- パッチを読めない入力は通す。Codex 自身もそのパッチを当てられないため、ファイルは変わらない。
- `claude-hook` に Codex の入力を足さず、サブコマンドを分ける。2つの入力の形が違い、1つのサブコマンドに混ぜると、どちらの入力として読んだかが判定に絡むためである。

## 参考文献

- openai/codex のタグ `rust-v0.166.0`（`6ff1e1ca3b`）
  - [codex-rs/apply-patch/src/parser.rs](https://github.com/openai/codex/blob/6ff1e1ca3b/codex-rs/apply-patch/src/parser.rs)（パッチの文法）
  - [codex-rs/core/src/tools/handlers/apply_patch.rs](https://github.com/openai/codex/blob/6ff1e1ca3b/codex-rs/core/src/tools/handlers/apply_patch.rs)（`apply_patch` の hook への入力）
  - [codex-rs/core/src/tools/handlers/unified_exec/exec_command.rs](https://github.com/openai/codex/blob/6ff1e1ca3b/codex-rs/core/src/tools/handlers/unified_exec/exec_command.rs)（シェルのコマンドの hook への入力と、パッチの横取り）
  - [codex-rs/core/src/tools/hook_names.rs](https://github.com/openai/codex/blob/6ff1e1ca3b/codex-rs/core/src/tools/hook_names.rs)（`apply_patch` の `matcher` での別名）
  - [codex-rs/hooks/src/events/pre_tool_use.rs](https://github.com/openai/codex/blob/6ff1e1ca3b/codex-rs/hooks/src/events/pre_tool_use.rs)（終了コード2の扱い）
  - [codex-rs/hooks/src/engine/discovery.rs](https://github.com/openai/codex/blob/6ff1e1ca3b/codex-rs/hooks/src/engine/discovery.rs)（`hooks.json` の読みこみと信頼の状態）
  - [codex-rs/hooks/schema/generated/pre-tool-use.command.input.schema.json](https://github.com/openai/codex/blob/6ff1e1ca3b/codex-rs/hooks/schema/generated/pre-tool-use.command.input.schema.json)（hook の入力の形）
  - [codex-rs/features/src/lib.rs](https://github.com/openai/codex/blob/6ff1e1ca3b/codex-rs/features/src/lib.rs)（`hooks` の機能が既定で有効であること）
