# ADR 0011: `packages/<パッケージ>/docs/adr/` など、サブディレクトリの中のADRも保護する

ステータス: 採択→ディレクトリの名前は[ADR 0014](0014-directory-name-variants.md)

## 文脈

[ADR 0002](0002-module-adr-directories.md) で、ADRのディレクトリを `docs/adr/` と `docs/<モジュール>/adr/` に広げた。
このとき、`packages/<モジュール>/docs/adr/` のようにリポジトリ直下の `docs/` 以外にあるADRは「実装しないこと」とした。

しかし、モノレポでは、パッケージごとに `docs/` を持つ構成も多い。

- `packages/<パッケージ>/docs/adr/`（npm・pnpmのワークスペースなど）
- `apps/<アプリ>/docs/adr/`、`crates/<クレート>/docs/adr/`、`services/<サービス>/docs/adr/` など

ディレクトリの名前はリポジトリごとに異なり、`packages/` などを決め打ちすると漏れる。
そこで、リポジトリ直下に限らず、どの深さの `docs/adr/` と `docs/<モジュール>/adr/` もADRのディレクトリとして扱う。

## 実装すること

### ADRのパスの判定

リポジトリのルートからの相対パス（区切りは `/`）を `/` で分けた要素の並びについて、次のどちらかがあればADRとする。

- 連続する要素 `docs`、`adr` の後ろに、1つ以上の要素がある（`<前>/docs/adr/<…>`）
- 連続する要素 `docs`、`<モジュール>`、`adr` の後ろに、1つ以上の要素がある（`<前>/docs/<モジュール>/adr/<…>`）。`<モジュール>` は空でない

`<前>` は0個以上の空でない要素で、名前は問わない。`<前>` が0個のときは、これまでの判定（[ADR 0002](0002-module-adr-directories.md)）と同じになる。

| パス | ADRか |
|---|---|
| `docs/adr/0001-a.md` | はい（従来どおり） |
| `docs/server/adr/0001-a.md` | はい（従来どおり） |
| `packages/server/docs/adr/0001-a.md` | はい（従来は対象外） |
| `apps/web/docs/adr/0001-a.md` | はい |
| `packages/server/docs/api/adr/0001-a.md` | はい（パッケージの中のモジュール） |
| `src/docs/server/adr/a.md` | はい（従来は対象外） |
| `docs/a/b/adr/0001-a.md` | いいえ（モジュールが2段。従来どおり） |
| `packages/server/docs/a/b/adr/0001-a.md` | いいえ（モジュールが2段） |
| `packages/server/adr/0001-a.md` | いいえ（`docs` を挟まない） |
| `packages/server/docs/adrs/0001-a.md` | いいえ |

判定は `is_adr_path`（[ADR 0002](0002-module-adr-directories.md)）の1か所で行い、`pre-commit`、`check-range`、`claude-hook`、`list`、`proposed`、`links` のすべてに及ぶ。

### `links` の参照先の解決（[ADR 0009](0009-require-links-for-adr-references.md)）

- 文書のディレクトリから上にたどり、各祖先のディレクトリ `<祖先>`（ルートを含む。`docs` という名前の要素を含む祖先は除く）について、次の順に参照先のADRのディレクトリの候補とする。近い祖先を先に探す。
  - 文書が `<祖先>/docs/<モジュール>/` 以下（`<モジュール>` は `adr` 以外）にあれば、`<祖先>/docs/<モジュール>/adr`
  - `<祖先>/docs/adr`
- 候補のうち、参照の番号のADRがちょうど1つあるディレクトリを参照先とする（[ADR 0009](0009-require-links-for-adr-references.md) と同じ）。
- 例：`packages/server/README.md` の `ADR 0001` は、`packages/server/docs/adr/`、`packages/docs/adr/`、`docs/adr/` の順に探す。
  `packages/server/docs/api/notes.md` は、`packages/server/docs/api/adr/`、`packages/server/docs/adr/`、`packages/docs/adr/`、`docs/adr/` の順に探す。
  存在しないディレクトリには参照先が無いので、実際には `packages/server/docs/adr/` か `docs/adr/` が使われる。
- ルートの文書（`README.md`、`docs/` 以下）の解決は、これまでと同じになる。

### ドキュメントとリリース

- `docs/specifications.md`、`README.md`、`--help` の、ADRのディレクトリの記述を更新する。
- 保護とリンクの検査の対象が広がり、これまで通っていたコミットが拒否されうるため、マイナーバージョンを上げて `v0.6.0` としてリリースする。
- 採択後、[ADR 0002](0002-module-adr-directories.md) のステータスを `採択→ADRのディレクトリの深さは[ADR 0011](0011-nested-adr-directories.md)` にする。

## 実装しないこと

- ADRのディレクトリを設定ファイルやオプションで指定する機能（[ADR 0001](0001-standalone-adr-guard.md) から引き続き実装しない）。
- 特定のディレクトリ（`node_modules/`、`vendor/` など）を除外すること。追跡されていない・無視されたファイルはもとから対象外になる（`git ls-files --exclude-standard` と、`git diff` がステージ・コミットされたものだけを見るため）。
- 2段以上のモジュール（`docs/a/b/adr/`）。
- パッケージをまたぐ参照（`packages/a/` の文書から `packages/b/docs/adr/` のADRを指す場合など）の自動の解決。人がリンクを書く。
- パッケージの間でのADRの移動の許可。従来どおり、コミット済みのADRの移動は削除として拒否する。

## テスト設計

### 単体テスト

- `is_adr_path`：上の表のすべてのパスを確かめる。
  - 既存のテスト `non_adr_paths` のうち、この変更でADRになる `src/docs/server/adr/a.md` と `packages/server/docs/adr/0001-a.md` は、ADRのパスのテストに移す。
  - そのほかの既存の判定は変えない。
- `candidate_dirs`：
  - `packages/server/README.md` → `packages/server/docs/adr`、`packages/docs/adr`、`docs/adr`
  - `packages/server/docs/api/notes.md` → `packages/server/docs/api/adr`、`packages/server/docs/adr`、`packages/docs/adr`、`docs/adr`
  - `packages/server/docs/adr/0002-b.md` → `packages/server/docs/adr`、`packages/docs/adr`、`docs/adr`
  - 既存の `README.md`、`docs/specifications.md`、`docs/adr/0002-b.md`、`docs/server/adr/0002-b.md`、`docs/server/notes.md` は、結果を変えない。
  - 既存の `src/docs/a.md` は、`src/docs/adr`、`docs/adr` に変わる（`src/` も祖先として探すため）。
- `adr_number`：`packages/server/docs/adr/0003-c.md` の番号が `0003` になる。

### 結合テスト

既存のテストは、上の単体テストの移しかえを除き、変えずにすべて通す。そのうえで `packages/server/docs/adr/0001-a.md` をコミットした一時リポジトリで、次を確かめる。

- `pre-commit`：本文の変更を拒否し、`--allow-status` でステータスの行だけの変更を許可する。
- `check-range`：本文を変えたコミットを拒否する。
- `claude-hook`：本文の編集を終了コード2で拒否する。
- `list`：`docs/adr/` のADRと並んで、`packages/server/docs/adr/0001-a.md` を昇順に出力する。
- `links`：`packages/server/README.md` の `ADR 0001` を `[ADR 0001](docs/adr/0001-a.md)`（`packages/server/` からの相対パス）に直し、ルートの `README.md` の `ADR 0001` はルートの `docs/adr/` へのリンクに直す。

### リリース

`v0.6.0` のリリース後、ダウンロードしたバイナリで、一時リポジトリの `packages/server/docs/adr/` のADRの本文の変更を `pre-commit` が拒否することを確かめる。

## トレードオフ

- 深さを問わないため、ADRのつもりでない `<どこか>/docs/adr/` も、コミット後は変更できなくなる。追跡しているサードパーティのコード（`vendor/` にコミットしたライブラリなど）にそのようなディレクトリがあると、その更新が止まる。そのような場合は、[ADR 0008](0008-allow-intentional-adr-rewrites.md) の手段で許可するか、ADRで除外の仕組みを追加する。
- `links` の参照先を近い祖先から探すため、パッケージの文書の `ADR 0001` は、パッケージのADRがあればそちらを指す。ルートのADRを指したい場合は、人がリンクを書く。
- 判定が広がるため、v0.6.0 に上げると、パッケージの中のADRを書きかえる既存の運用は止まる。意図した変更は、ステータスの行（`--allow-status`）とリンク（`--allow-links`）に限られる。
