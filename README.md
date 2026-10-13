# adr-guard

[![CI](https://github.com/tett23/adr-guard/actions/workflows/ci.yml/badge.svg)](https://github.com/tett23/adr-guard/actions/workflows/ci.yml)

コミット済みのADR（`docs/adr/`、モノレポでは `docs/<モジュール>/adr/` や `packages/<パッケージ>/docs/adr/` なども）を変更できないようにするツール。ステータスの行（`ステータス:` で始まる行）の変更と、ADRの参照のリンクだけの変更は、オプションで許可する。

ADRは決定の記録であり、あとから書きかえると経緯が失われる。
仕様を変える場合は、元のADRを書きかえずに新しいADRを作成する。
adr-guard は、この規則をgitのpre-commit hook、Claude Codeのhook、CIの三か所で強制する。

## 規則

- 対象は `docs/adr/` と `docs/<モジュール>/adr/`（例：`docs/server/adr/`）以下のファイル。どの深さにあってもよく、`packages/server/docs/adr/` や `apps/web/docs/api/adr/` も対象になる（[ADR 0011](docs/adr/0011-nested-adr-directories.md)）。モジュールは1段だけで、`docs/a/b/adr/` は対象外。
- 設定は不要。モノレポでモジュールごとにADRを分けても、導入の手順は変わらない。
- コミット済みのADRは変更できない。ただし、検査に `--allow-status` を付けると `ステータス:` で始まる行の変更を、`--allow-links` を付けるとADRの参照のリンクだけの変更を許可する。ADRの採択などでステータスを変えるため、通常は両方を付ける。
- コミット済みのADRは、削除・リネームできない。
- 新しいADRの追加と、未コミットのADRの編集は自由にできる。
- `docs/adr/` から `docs/<モジュール>/adr/` への移動も、コミット済みのADRの移動として拒否する。移す場合は、新しい場所に新しいADRを作成する。
- 文書（`.md`）の中のADRの参照（`ADR 0001` の形）には、ADRのファイルへのリンクを付ける（例：`[ADR 0001](docs/adr/0001-xxx.md)`、続く番号は `[ADR 0005](...)、[0006](...)`）。インラインのコード、コードブロック、リンクのテキストの中と、ADR自身の番号は対象外。参照先のADRが見つからない番号は検査しない。

## サブコマンド

| サブコマンド | 用途 |
|---|---|
| `adr-guard pre-commit [--allow-status] [--allow-links]` | gitのpre-commit hookとして、ステージされた変更を検査する。ステージされた `.md` のリンクの無いADRの参照も拒否する |
| `adr-guard claude-hook [--allow-status]` | Claude CodeのPreToolUse hookとして、Write・Edit・MultiEditによるコミット済みADRの変更を拒否する |
| `adr-guard check-range <base> <head> [--allow-status] [--allow-links]` | `<base>..<head>` のマージ以外の各コミットを、親コミットとの差分で検査する。`<base>` が無いコミットならルートコミットから検査する |
| `adr-guard list [--status]` | 作業ツリーにあるADR（未コミットのものも含む）のパスを一覧する。`--status` では `<ステータス><タブ><パス>` で出力する（ステータスの行が無ければステータスは空） |
| `adr-guard proposed [--status]` | ステータスが提案のADRのパスを一覧する。`--status` では `<ステータス><タブ><パス>` で出力する |
| `adr-guard links [--fix [--include-committed]]` | 作業ツリーの `.md` にある、リンクの無いADRの参照を報告する。`--fix` でリンクに置きかえる（コミット済みのADRは `--include-committed` を付けたときだけ） |
| `adr-guard --version`、`-V` | バージョンを表示する |
| `adr-guard --help`、`-h` | ヘルプを表示する。AIがヘルプだけで使い方を把握できるよう、規則、各サブコマンドの出力の形と終了コード、例まで載せている |

- `--allow-status`：コミット済みのADRで、ステータスの行の変更を許可する（`pre-commit`、`claude-hook`、`check-range`）。付けないと、ステータスの行も変更できない。
- `--allow-links`：コミット済みのADRで、ADRの参照のリンクだけが異なる変更（リンクを付ける・外す・リンク先を変える）を許可する。
- オプションは、サブコマンドの後ろならどの位置に置いてもよい。

レビュー待ちのADRを開く例：

```sh
adr-guard proposed | xargs "$EDITOR"
```

## インストール

### リリースから

[Releases](https://github.com/tett23/adr-guard/releases) から、環境に合うアーカイブと `SHA256SUMS` をダウンロードする。

| 環境 | ターゲット |
|---|---|
| macOS（Apple Silicon） | `aarch64-apple-darwin` |
| Linux（x86_64） | `x86_64-unknown-linux-gnu` |

```sh
tag=v0.6.0
target=aarch64-apple-darwin
archive="adr-guard-$tag-$target.tar.gz"
curl -fsSLO "https://github.com/tett23/adr-guard/releases/download/$tag/$archive"
curl -fsSLO "https://github.com/tett23/adr-guard/releases/download/$tag/SHA256SUMS"
grep "$archive" SHA256SUMS | shasum -a 256 -c   # Linuxでは sha256sum -c
tar -xzf "$archive"
mv "${archive%.tar.gz}/adr-guard" ~/.local/bin/
adr-guard --version   # adr-guard 0.6.0
```

macOS向けのバイナリは、Apple Developer IDによる署名と公証をしていない。
`curl` や `gh` でダウンロードすれば検疫属性は付かない。
ブラウザでダウンロードした場合は、チェックサムを確かめてから `xattr -d com.apple.quarantine <パス>` で検疫を外す。

### ソースから

```sh
cargo install --git https://github.com/tett23/adr-guard --tag v0.6.0
```

手元の adr-guard のバージョンは `adr-guard --version` で確かめる。

## リポジトリへの導入

### gitのpre-commit hook

`.githooks/pre-commit` を作り、実行権限を付ける。

```sh
#!/bin/sh
exec adr-guard pre-commit --allow-status --allow-links
```

```sh
chmod +x .githooks/pre-commit
git config core.hooksPath .githooks
```

`core.hooksPath` はクローンごとの設定なので、導入先のREADMEにも手順を書いておく。

### Claude Codeのhook

`.claude/settings.json` に追加する。

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Write|Edit|MultiEdit",
        "hooks": [{ "type": "command", "command": "adr-guard claude-hook --allow-status" }]
      }
    ]
  }
}
```

### CI（GitHub Actions）

このリポジトリのcomposite actionを使う。リリースのバイナリをダウンロードし、`SHA256SUMS` で検証してから `check-range` を実行する。
Linux（x86_64）のランナーだけに対応する。

```yaml
jobs:
  adr:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
          persist-credentials: false
      - uses: tett23/adr-guard@v0.6.0
        with:
          base: ${{ github.event_name == 'pull_request' && github.event.pull_request.base.sha || github.event.before }}
          head: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}
          allow-status: true
          allow-links: true
```

- 範囲内の全コミットを読むため、`fetch-depth: 0` でチェックアウトする。
- `check-range` の後に、チェックアウトした作業ツリーで `adr-guard links` を実行する。入力 `links: false` で止められる。
- 入力 `allow-status: true`、`allow-links: true` で、`check-range` に `--allow-status`、`--allow-links` を付ける。どちらも既定は `false`。
- actionはタグかコミットのSHAで指定する。使う adr-guard のバージョンは、actionの `Cargo.toml` のバージョンになる。`version` 入力（例：`v0.6.0`）で変えられる。

### v0.5.0 に上げる場合

v0.5.0 から、ステータスの行の変更も `--allow-status` を付けたときだけ許可する（[ADR 0010](docs/adr/0010-allow-status-option.md)）。
v0.4.0 以前から上げる場合は、pre-commit hook、Claude Codeのhook（`~/.claude/settings.json` を含む）、actionのすべてに `--allow-status`（actionは `allow-status: true`）を付ける。
付けないと、ADRの採択などでステータスの行を変えられなくなる。

### 注意

- adr-guard が手元に無いと、pre-commit hookはコマンドが見つからずにコミットを拒否し、Claude Codeのhookは編集を止めない。どちらの場合もCIで検査される。
- `git commit --no-verify` やGitHubのWeb UIでの編集はhookを通らない。CIで検査する。
- CIでは、範囲内で一度でもADRを書きかえたコミットがあれば、後のコミットで戻していても失敗する。履歴を書きかえて（rebaseなどで）直す。

### 既存のリポジトリでリンクの検査を導入する

1. pre-commit hookを `exec adr-guard pre-commit --allow-status --allow-links` にし、actionに `allow-status: true` と `allow-links: true` を付ける。移行が済むまではactionに `links: false` を付けてもよい。
2. `adr-guard links --fix --include-committed` で、コミット済みのADRも含めて参照をリンクにする。
3. 書きかえをほかの変更と分けて1つのコミットにする。`--allow-links` により、トレーラーは要らない。
4. `adr-guard links` が終了コード0で終わることを確かめ、actionの `links: false` を外す。

### 意図してADRを書きかえる場合

ADRの参照にリンクを付けるだけなら、上の `--allow-links` を使う。
次の手段は、それ以外の書きかえのための緊急用であり、通常は使わない。

書式の修正など、人間が判断したうえでコミット済みのADRを書きかえる場合は、書きかえをほかの変更と分けて1つのコミットにし、次の2つを行う。

- 手元のpre-commit hookは、環境変数 `ADR_GUARD_ALLOW` に理由を入れて通す。`--no-verify` と違い、同じhookのほかの検査は迂回しない。
- CIの検査は、コミットメッセージの最後の段落に `ADR-Guard-Allow: <理由>` のトレーラーを付けて通す。

```sh
ADR_GUARD_ALLOW="ADRの参照をリンクにするため" git commit -F - <<'MSG'
Link ADR references

ADR-Guard-Allow: ADRの参照をリンクにするため
MSG
```

許可はトレーラーを付けたコミットだけに及ぶ。理由は履歴に残るので、プルリクエストのレビューで確かめる。
Claude Codeのhookには許可の手段がないため、Claude Codeに書きかえさせる場合はシェルから書きかえる。

## 開発

クローン後、gitのhookを有効化する。このリポジトリでは、開発中の adr-guard 自身で検査する（`cargo run`）。

```sh
git config core.hooksPath .githooks
```

テストは `cargo test` で実行する。
設計判断は `docs/adr/` にADRとして記録する。仕様は [docs/specifications.md](docs/specifications.md) を参照。

### リリース

`Cargo.toml` の `version` を更新してコミットし、`v<version>` のタグをpushする。
`-` を含むタグ（例：`v0.1.0-rc`）はプレリリースになる。

## License

[MIT](LICENSE)
