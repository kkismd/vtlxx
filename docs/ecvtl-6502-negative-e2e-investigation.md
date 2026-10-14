# ECVTL 6502 異常系E2E fixture 調査（#409）

## 調査範囲と再現環境

- 対象: #380 の着手前具体化。production code / test code は変更しない。
- 調査対象 SHA: `eb0f3663cd2b1b82667fa169d58999663bebb270`（`main`、2026-10-14）。
- `ca65`, `ld65`, `sim65`: V2.18 - Ubuntu 2.19-1。
- `cargo`: 1.99.0 (`5f94df478 2026-08-27`)。
- build: `cargo build -p vtlxx-poc-extended-classic-vtl --bin ecvtl6502`。
- 各 CLI 実測: `target/debug/ecvtl6502 <一時 source>`。resource 上限の探索では `--cycle-limit 4000000` を指定。
- source は `/tmp` の一時 directory に raw bytes で生成。改行は LF。repository の tracked file は変更していない。
- sim65 の既定 cycle limit は 4,000,000。全ての失敗で stderr は compiler/syntax error を示し、cycle-limit error ではなかった。N=169/170 の境界探索を含め、個別の消費 cycle 数は計測していない。

## status と公開観測

| 内部段階 | 内部 status | frontend status | CLI exit | CLI diagnostic |
| --- | ---: | ---: | ---: | --- |
| source/transport error | — | 1 | 129 | source / transport error |
| syntax error | — | 2 | 130 | syntax error |
| literal range error | — | 3 | 131 | literal range error |
| backend error | `cc_status` を保持 | 4 | 132 | compiler resource error |
| binding 未定義 | 8（`binding.s::cc_resolve`） | 4（`basic.s::backend_error`） | 132 | compiler resource error |
| shared arena 容量超過 | 1（`arena.s::cc_reserve_shared`） | 4 | 132 | compiler resource error |

`sim65_cli.s::_main` は失敗時に `fe_compile_status | $80` で halt し、Rust CLI は frontend status 4 を一律 `compiler resource error` と表示する。したがって公開 exit/stderr では未定義 binding と arena 超過を区別できない。既存 `sim65_frontend` の E2E helper も frontend status を返すだけで、backend 原因は出力しない。既存 `sim65_backend` は arena / binding API の各単体契約を確認するが、#380 の source fixture が残した `cc_status` を直接確認しない。

**#380 の原因別判定方式:** CLI fixture は本番 `ecvtl6502` を使い、非ゼロ exit・診断・raw stdout をassertする。原因確認は別の test-only sim65 driver を追加して行う。この driver は同じ `fe_compile_run`、同じ frontend/backend/runtime object、同じ framed source bytes を使い、成功時は通常どおり generated entry を実行する。失敗時だけ `fe_compile_status == 4` なら halt status に `cc_status` を返し、それ以外は `fe_compile_status` を返す。例えば未定義 binding=8、arena=1、syntax=2 となる。観測位置は `fe_compile_run` の戻り直後（失敗時の source abort を含む本番処理の後）とする。`basic.s::@failed` は `cc_source_is_active` が真の場合だけ abortし、`owner.s::cc_source_abort` / `arena.s::cc_data_abort` は active append を取り消した後も元の `cc_status` を保持する（inactive呼び出しは別途 misuse=3を報告するが、frontendはinactive時に呼ばない）。#380のprobeはこの分岐とstatus保持をコード確認してから、同じcompiler objectsとframe bytesで実行する。test-only driver の違いは失敗 halt 値だけであり、runtime出力には触れない。CLI観測と probe 観測を同じ raw source で対にし、production API/diagnostic は変更しない。probe は内部状態の根拠であり、本番CLIの公開status契約を検査する代用にはしない。

## 原因別 fixture と runtime 未開始の確認

すべて `?=42\n` を先頭に置く。正常対照は `?=42\n` で、CLI exit 0、stdout は ASCII `42`。失敗時の stdout は表示上の空ではなく raw `b""`。

| ID | source bytes / 生成規則 | 境界 | 狙う最初の失敗 | 内部原因・公開結果 | runtime未開始 | 成功対照・cycle |
| --- | --- | --- | --- | --- | --- | --- |
| SYN | `?=42\nA=`（7 bytes） | 固定 | RHS欠落の構文解析 | frontend=2、CLI exit 130、syntax diagnostic。backend原因なし | 実測: stdout `b""`。成功対照は `42` | `?=42\n` → `42`。4M budget、実測でcycle limitに未到達 |
| BND | `?=42\na=()\n`（10 bytes、`a`未定義） | 固定 | Write binding 解決 | `cc_resolve` により cc=8 と静的追跡。frontend=4、CLI exit 132、resource diagnostic。cc=8 のE2E probeは未実施 | 実測: stdout `b""`。`fe_compile_run` は失敗枝から戻り `rt_init` / generated entry に進まない | `?=42\n` → `42`。4M budget |
| ARENA | `?=42\n` に `A=1\n` を N 回連結 | N=169 成功 (681 bytes)、N=170 失敗 (685 bytes) | 有効なstatement列を compile する途中の shared 2,048-byte arena 予約 | cc=1 は `cc_reserve_shared` の容量経路から静的推定。frontend=4、CLI exit 132。cc=1 のE2E probeは未実施 | 実測: N=170 stdout `b""`。N=169 は exit 0、stdout `42` | 4M budget。N=169/170 の境界を実行し、cycle-limit diagnosticなし |

SYN/BND/ARENA はいずれも同じ本番CLIで実行した。CLI の stderr は上記 status と整合した。BND/ARENA の狙い原因は source と実装分岐からの静的推定であり、#380 で test-only probe を追加するまで「内部原因を実測済み」と扱わない。

### raw source bytes の決定的な生成規則

以下の Python 風 byte 生成式で再現できる。`marker = b"?=42\n"`、statement区切りはASCII space、指定した `\n` はLF byte。明記のない末尾改行は付けない。Nは括弧/IF/Whileで16または17、ARENAで169または170、label/fixupで32または33とする。

```python
marker = b"?=42\n"
syntax = marker + b"A="
undefined_write = marker + b"a=()\n"
arena = lambda n: marker + b"A=1\n" * n
parentheses = lambda n: marker + b"A=" + b"(" * n + b"1" + b")" * n
if_nesting = lambda n: marker + b"%=1 [ " * n + b"?=9" + b" ]" * n
while_nesting = lambda n: (
    marker + b"~=0 " + b"*=() [ ~=0 ] [ " * n + b"?=9" + b" ]" * n
)
labels = lambda n: marker + b" ".join(
    b"^=" + str(i).encode("ascii") for i in range(n)
)
fixups = lambda n: marker + b" ".join(
    [*(b"#=" + str(i).encode("ascii") for i in range(n)),
     *(b"^=" + str(i).encode("ascii") for i in range(n))]
)
chunks = b"A=1\n" + b"".join(
    b"&=" + bytes([identity]) + b" [ ]\nA=1\n"
    for identity in range(ord("a"), ord("z") + 1)
)
```

labels/fixupsは10以上の数値も通常の10進表記で、末尾にspace/LFを追加しない。ARENAのN=169/170はこのSHAとtoolchainにおける観測境界であり、将来の実装へ固定する意味論上の定数ではない。

`basic.s::fe_compile_run` は全 statement compile、`fe_finish_chunk`、`fe_build_driver` の成功後にだけ `rt_init` と generated entry へ移る。失敗は `@failed` から返る。よって先行 `?=42` が出ないことは CLI で実測でき、コード構造は runtime が開始されない根拠になる。これは register/storage/stack 等の全内部状態を直接観測した証拠ではない。#380 の受け入れ表現も「先行出力がない + compile failure時に runtime entry へ進まない構造確認」とし、全 mutation の直接実測とはしない。

## 資源上限の到達性

| 資源 | 実装上限・失敗段階 | 現行 source surface からの境界 / 到達性 | #380 での扱い |
| --- | --- | --- | --- |
| expression の括弧 nesting | `basic.s::enter_nesting` は16まで、17で syntax status 2 | `A=` + `(` を16/17個 + `1` + `)` を同数生成。先頭 `?=42\n` 付きで16は40 bytes / exit 0 / stdout `42`、17は42 bytes / exit 130 / stdout `b""` | 到達可能。17を runtime 未開始 fixture にできる |
| IF structured depth / compile work stack | structured depth 16、17段目は `basic.s` が先に syntax status 2。work stackは16 u16、超過時 cc=9 | `?=42\n` + `%=1 [ ` を16/17回 + `?=9` + ` ]`を同数生成。16は136 bytes / exit 0 / stdout `429`、17は144 bytes / exit 130 / stdout `b""`。IFの未解決patchが16段重なるためwork stackは16まで到達し、17段目はwork stack pushより前にfrontendが拒否 | 17段目の構文制限を代表 fixture にできる。cc=9 の work-stack capacity failure はsource経路から到達不能。backend単体 test case 14 は16件push成功後の17件目で cc=9 を確認 |
| While depth / structured depth | `while_depth` 16、17段目は syntax status 2。structured blockも16 | `?=42\n~=0 ` + `*=() [ ~=0 ] [ ` を16/17回 + `?=9` + ` ]`を同数生成。16は284 bytes / exit 0 / stdout `42`、17は301 bytes / exit 130 / stdout `b""` | 到達可能。17段目はruntime未開始 fixtureにできる |
| label table | ownerあたり32 label。33個目で cc=5、frontend=4 | `?=42\n` + `^=0` から `^=32`。32個は成功 / stdout `42`、33個は159 bytes / exit 132 / stdout `b""` | 到達可能。既存 frontend test は33 labelsでstatus 4を確認 |
| unresolved fixup table | ownerあたり32 pending fixup。33件目で cc=6。owner complete時の未解決は cc=7 | `?=42\n` + `#=0..31` 後に `^=0..31` を置くと304 bytesで成功 / stdout `42`。33件目を追加してから全labelを置くと314 bytesでexit 132 / stdout `b""`。probeなしでは cc=6 と cc=7 をCLIで区別不能 | 到達可能。32/33 fixtureで原因 probe を利用 |
| top-level chunk table | 53 entry。`fe_finish_chunk` の54個目要求で frontend=4 | 現行source-defined Write identityは `a`〜`z` の26個。各定義の前後に top-level runtime chunkを置いても最大27 chunk。26定義の生成 source は316 bytesで成功 | 53上限は現行 surface では到達不能。無理に対象化しない |
| compile source work stack | 16 u16。17回目pushで cc=9 | frontendからの push は IF の未解決patch用のみ。structured depthも16なので17回目より先に syntax error。SourceProcedure向けの汎用work-stack source経路なし | capacity error はsource E2E到達不能。SourceProcedure/work stack を新規実装しない |
| shared arena | 2,048 bytes共有領域、予約失敗 cc=1 | ARENA fixtureでN=169/170の境界。685-byte sourceで失敗するためu16 source長上限65,535 byteには届かない | 到達可能。上記原因別probeを必須とする |
| runtime value stack | runtime Cell stackの容量/underflow | compile完了後のruntime実行エラーであり compile failure ではない | #380のcompile failure分類に混ぜない |

label/fixup の現行frontend testは `sim65_frontend.rs` の `too_many_labels` / `too_many_fixups` を含み、frontend status 4と空stdoutを確認している。現在のCLI統合testに同じ表はない。IF nesting、While、括弧、26 identities のケースを本番CLIで上記のとおり別途実測した。

## 既存回帰確認と引き渡し

次の ignored sim65 test をすべて実行し成功した。

```text
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_examples -- --ignored --nocapture
  1 passed: six examples (Rust / 6502 portable success and raw stdout comparison)
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_cli -- --ignored --nocapture
  5 passed
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_frontend -- --ignored --nocapture
  10 passed
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_backend -- --ignored --nocapture
  7 passed
```

生成物は各既存 test helper と CLI が一時 directory に置き、repositoryへ残さない。本調査では test/production codeを変更していない。

### #380 の実装条件

1. 本番 CLI に SYN/BND/ARENA を入れ、exit・stderr分類・stdout raw bytes・cycle-limit誤判定なしをassertする。成功対照 `?=42\n` は `b"42"`。
2. #380内の test-only sim65 driver で同一 source を再実行し、SYN=frontend 2、BND=`cc_status` 8、ARENA=`cc_status` 1を区別する。probe は `fe_compile_run` の失敗戻り直後に `cc_status` をhalt statusへ写すだけにし、runtime outputを生成しない。source compile/run経路は本番frontend・backend・runtimeと共有する。
3. runtime未開始は先行印字の不在を実測し、`fe_compile_run` の `rt_init`/entry呼出位置をコード確認する。内部register/storage等の全state mutation未発生を直接測ったとは主張しない。
4. 到達可能な追加代表例は IF nesting 17、label 33、resolved fixup 33。work stack 17、top-level chunk 54 はfrontend guard / identity上限により到達不能なので受け入れ対象にしない。
5. SourceProcedure、production diagnostic/status、資源上限を変更しない。#381の高水位計測も含めない。
