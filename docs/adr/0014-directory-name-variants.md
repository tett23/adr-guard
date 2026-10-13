# ADR 0014: ADRと記録のディレクトリの名前の揺れ（`adrs`、`decisions`、`decision`、`audits`、`benchmark`）も認める

ステータス: 採択

## 文脈

adr-guard が保護するディレクトリの名前は、次のとおり決め打ちしている。

- ADR：`adr`（`docs/adr/`、`docs/<モジュール>/adr/`。[ADR 0002](0002-module-adr-directories.md)、[0011](0011-nested-adr-directories.md)）
- 記録：`audit` と `benchmarks`（`docs/audit/`、`docs/benchmarks/` など。[ADR 0013](0013-protect-audit-and-benchmark-records.md)）

しかし、リポジトリによっては、ADRを `docs/adrs/` や `docs/decisions/` に、監査や計測の記録を `docs/audits/` や `docs/benchmark/` に置く。
名前が少し違うだけで保護されず、気づかないままコミット済みのファイルを書きかえられてしまう。
[ADR 0013](0013-protect-audit-and-benchmark-records.md) は、このことをトレードオフとして認めていた。

そこで、よく使われる名前の揺れとして、ADRの `adrs`、`decisions`、`decision` と、記録の `audits`、`benchmark` を加える。

## 実装すること

### ディレクトリの名前

- ADRのディレクトリの名前を `adr`、`adrs`、`decisions`、`decision` にする。
- 記録のディレクトリの名前を `audit`、`audits`、`benchmarks`、`benchmark` にする。
- パスの形（どの深さの `docs/<名前>/` と `docs/<モジュール>/<名前>/` か）は、[ADR 0011](0011-nested-adr-directories.md)、[0013](0013-protect-audit-and-benchmark-records.md) のまま変えない。

| パス | 種類 |
|---|---|
| `docs/adr/0001-a.md` | ADR（従来どおり） |
| `docs/adrs/0001-a.md`、`docs/decisions/0001-a.md`、`docs/decision/0001-a.md` | ADR（新しく対象になる） |
| `docs/server/adrs/0001-a.md`、`packages/server/docs/decisions/0001-a.md` | ADR（新しく対象になる） |
| `docs/audits/a.md`、`docs/benchmark/v1.json`、`docs/server/benchmark/v1.json` | 記録（新しく対象になる） |
| `docs/benchmarks/v1.json`、`docs/audit/a.md` | 記録（従来どおり） |
| `docs/adr-old/a.md`、`docs/architecture/a.md`、`docs/decided/a.md`、`docs/Decisions/a.md` | 対象外 |

ADRのディレクトリの判定は `is_adr_path` の1か所で行うため、`pre-commit`、`check-range`、`claude-hook`、`list`、`proposed`、`links`、`check-status` のすべてに及ぶ。記録の判定も `is_record_path` の1か所で行う。

### `links` の参照先の解決（[ADR 0009](0009-require-links-for-adr-references.md)、[0011](0011-nested-adr-directories.md)）

- 各祖先 `<祖先>` について、候補のディレクトリを次の順にする。近い祖先を先に探すことは変えない。
  1. 文書が `<祖先>/docs/<モジュール>/` 以下（`<モジュール>` はADRのディレクトリの名前以外）にあれば、`<祖先>/docs/<モジュール>/<名前>` を、名前を `adr`、`adrs`、`decisions`、`decision` の順に並べたもの
  2. `<祖先>/docs/<名前>` を、同じ順に並べたもの
- 候補のうち、参照の番号のADRがちょうど1つあるディレクトリを参照先とする（従来どおり）。同じ祖先の複数のディレクトリに同じ番号のADRがあれば、先に並ぶもの（`adr`、`adrs`、`decisions`、`decision` の順）を参照先とする。

### メッセージ

- 記録の違反のメッセージの括弧の中を、`docs/audit/、docs/audits/、docs/benchmarks/、docs/benchmark/` にする。

### ドキュメントとリリース

- `docs/specifications.md`、`README.md`、`--help` の、ディレクトリの名前の記述を更新する。
- `v0.8.1` としてリリースする。保護の対象が広がり、これまで通っていたコミットが拒否されうるが、名前の揺れを足すだけの変更のためパッチバージョンとする。
- 採択後、次のADRのステータスを変更する。
  - [ADR 0002](0002-module-adr-directories.md)：`採択→ADRのディレクトリの深さは[ADR 0011](0011-nested-adr-directories.md)、ディレクトリの名前は[ADR 0014](0014-directory-name-variants.md)`
  - [ADR 0011](0011-nested-adr-directories.md)：`採択→ディレクトリの名前は[ADR 0014](0014-directory-name-variants.md)`
  - [ADR 0013](0013-protect-audit-and-benchmark-records.md)：`採択→記録のディレクトリの名前は[ADR 0014](0014-directory-name-variants.md)`

## 実装しないこと

- `architecture`、`adr-log`、`records` など、ほかの名前。必要になったらADRで追加する。
- ディレクトリの名前を設定ファイルやオプションで指定する機能（[ADR 0001](0001-standalone-adr-guard.md) から引き続き実装しない）。
- 大文字の名前（`ADR`、`Benchmarks`）。
- ADRのディレクトリを複数（`adr` と `decisions` など）持つリポジトリへの警告。

## テスト設計

### 既存のテスト

この変更でADRや記録になるパスを「対象外」として確かめている既存の単体テストは、対象のパスのテストに移す。

- `non_adr_paths` の `docs/server/adrs/0001-a.md` と `packages/server/docs/adrs/0001-a.md`
- `record_paths` の対象外の側の `docs/audits/a.md` と `docs/benchmark/a.md`

そのほかの既存のテストは変えずにすべて通す。記録の違反のメッセージを部分一致（`コミット済みの記録`）で確かめているテストは、そのまま通る。

### 単体テスト

- `is_adr_path`：上の表のADRのパスがADRで、対象外のパスがADRでない。
- `is_record_path`：上の表の記録のパスが記録で、対象外のパスが記録でない。`docs/adrs/0001-a.md`、`docs/decisions/0001-a.md` は記録でない。
- `candidate_dirs`：
  - `README.md` → `docs/adr`、`docs/adrs`、`docs/decisions`、`docs/decision`
  - `docs/server/notes.md` → `docs/server/adr`、`docs/server/adrs`、`docs/server/decisions`、`docs/server/decision`、`docs/adr`、`docs/adrs`、`docs/decisions`、`docs/decision`
  - `docs/decisions/0002-b.md` → `docs/adr`、`docs/adrs`、`docs/decisions`、`docs/decision`（`decisions` はモジュールとみなさない）
  - そのほかの既存の例は、各祖先の `adr` の直後に `adrs`、`decisions`、`decision` が加わる。
- `adr_number`：`docs/adrs/0003-c.md`、`docs/decisions/0004-d.md` の番号が `0003`、`0004` になる。

### 結合テスト

- `docs/adrs/0001-a.md` と `docs/decisions/0002-b.md` をコミットした一時リポジトリで、`pre-commit` が両方の本文の変更を拒否し、`list` が両方を出力し、`check-status` がステータスの行を検査する。`README.md` の `ADR 0001` と `ADR 0002` を `links --fix` が `[ADR 0001](docs/adrs/0001-a.md)` と `[ADR 0002](docs/decisions/0002-b.md)` に直す。
- `docs/audits/a.md` と `docs/benchmark/v1.json` をコミットした一時リポジトリで、`pre-commit` が両方の変更を拒否し、メッセージに `docs/audits/` と `docs/benchmark/` が含まれる。

### リリース

`v0.8.1` のリリース後、ダウンロードしたバイナリで、一時リポジトリの `docs/decisions/` のADRの本文の変更と、`docs/audits/` の記録の変更を `pre-commit` が拒否することを確かめる。

## トレードオフ

- 名前を増やすため、ADRや記録のつもりでない `docs/adrs/`、`docs/decisions/`、`docs/audits/`、`docs/benchmark/` も、コミット後は変更できなくなる。特に `decisions` と `decision` は、ADR以外の決定の記録（会議の議事録など）に使われることもある。その場合も、決定の記録はあとから書きかえないものであるため、保護して差し支えないと判断した。ADRとして扱われるため、ステータスの行の検査（[ADR 0012](0012-status-line-and-status-adt.md)）とリンクの検査も及ぶ点に注意する。
- 名前を決め打ちで増やす方式のため、ほかの名前の揺れには、そのつどADRが要る。設定を持たない単純さを優先する。
- 複数のADRのディレクトリに同じ番号のADRがあるリポジトリでは、`links` は `adr`、`adrs`、`decisions`、`decision` の順に先に並ぶものを参照先とする。番号の重複そのものは検出しない。
