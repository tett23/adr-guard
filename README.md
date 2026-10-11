# adr-guard

[![CI](https://github.com/tett23/adr-guard/actions/workflows/ci.yml/badge.svg)](https://github.com/tett23/adr-guard/actions/workflows/ci.yml)

コミット済みのADR（`docs/adr/`）を、ステータスの行（`ステータス:` で始まる行）を除いて変更できないようにするツール。

ADRは決定の記録であり、あとから書きかえると経緯が失われる。
仕様を変える場合は、元のADRを書きかえずに新しいADRを作成する。
adr-guard は、この規則をgitのpre-commit hook、Claude Codeのhook、CIの三か所で強制する。

## 規則

- 対象は `docs/adr/` 以下のファイル。
- コミット済みのADRは、`ステータス:` で始まる行以外を変更できない。
- コミット済みのADRは、削除・リネームできない。
- 新しいADRの追加と、未コミットのADRの編集は自由にできる。

## サブコマンド

| サブコマンド | 用途 |
|---|---|
| `adr-guard pre-commit` | gitのpre-commit hookとして、ステージされた変更を検査する |
| `adr-guard claude-hook` | Claude CodeのPreToolUse hookとして、Write・Edit・MultiEditによるコミット済みADRの変更を拒否する |
| `adr-guard check-range <base> <head>` | `<base>..<head>` のマージ以外の各コミットを、親コミットとの差分で検査する。`<base>` が無いコミットならルートコミットから検査する |

## インストール

### リリースから

[Releases](https://github.com/tett23/adr-guard/releases) から、環境に合うアーカイブと `SHA256SUMS` をダウンロードする。

| 環境 | ターゲット |
|---|---|
| macOS（Apple Silicon） | `aarch64-apple-darwin` |
| Linux（x86_64） | `x86_64-unknown-linux-gnu` |

```sh
tag=v0.1.0
target=aarch64-apple-darwin
archive="adr-guard-$tag-$target.tar.gz"
curl -fsSLO "https://github.com/tett23/adr-guard/releases/download/$tag/$archive"
curl -fsSLO "https://github.com/tett23/adr-guard/releases/download/$tag/SHA256SUMS"
grep "$archive" SHA256SUMS | shasum -a 256 -c   # Linuxでは sha256sum -c
tar -xzf "$archive"
mv "${archive%.tar.gz}/adr-guard" ~/.local/bin/
```

macOS向けのバイナリは、Apple Developer IDによる署名と公証をしていない。
`curl` や `gh` でダウンロードすれば検疫属性は付かない。
ブラウザでダウンロードした場合は、チェックサムを確かめてから `xattr -d com.apple.quarantine <パス>` で検疫を外す。

### ソースから

```sh
cargo install --git https://github.com/tett23/adr-guard --tag v0.1.0
```

## リポジトリへの導入

### gitのpre-commit hook

`.githooks/pre-commit` を作り、実行権限を付ける。

```sh
#!/bin/sh
exec adr-guard pre-commit
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
        "hooks": [{ "type": "command", "command": "adr-guard claude-hook" }]
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
      - uses: tett23/adr-guard@v0.1.0
        with:
          base: ${{ github.event_name == 'pull_request' && github.event.pull_request.base.sha || github.event.before }}
          head: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}
```

- 範囲内の全コミットを読むため、`fetch-depth: 0` でチェックアウトする。
- actionはタグかコミットのSHAで指定する。使う adr-guard のバージョンは、actionの `Cargo.toml` のバージョンになる。`version` 入力（例：`v0.1.0`）で変えられる。

### 注意

- adr-guard が手元に無いと、pre-commit hookはコマンドが見つからずにコミットを拒否し、Claude Codeのhookは編集を止めない。どちらの場合もCIで検査される。
- `git commit --no-verify` やGitHubのWeb UIでの編集はhookを通らない。CIで検査する。
- CIでは、範囲内で一度でもADRを書きかえたコミットがあれば、後のコミットで戻していても失敗する。履歴を書きかえて（rebaseなどで）直す。

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
