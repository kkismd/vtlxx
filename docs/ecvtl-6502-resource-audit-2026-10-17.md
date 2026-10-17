# ECVTL 6502 資源監査（#381）

## 対象main SHAと検証環境

- 監査対象 `main`: `48cc385f6ce86e755fbe82604dd9b354cca18ab4`
- `ca65`、`ld65`、`sim65`: V2.18（Ubuntu package `2.19-1`）
- ignored sim65 テストを実行した。今回は静的監査で確認できる範囲を扱い、`sim65 --trace` は実行していない。
- 対象は現在の sim6502 target と CLI 経路。実機6502のadapterと割り込みは対象外。

## hardware stack の内訳

6502 の `JSR` は復帰アドレス2 byteを積む。`PHA` と `PHP` はそれぞれ1 byteを積む。`JMP` と末尾の `JMP` は復帰アドレスを追加しない。runtime入口は `_main → fe_compile_run` で、compile後に `fe_compile_run` が生成entryへの復帰アドレスを積み、そこへjumpする。生成driverは完成済みsource chunkを呼び、chunkは静的に解決済みのWrite手続きを呼ぶ。

| runtimeで同時に積まれる要素 | byte数 | 根拠・補足 |
| --- | ---: | --- |
| `_main → fe_compile_run` | 2 | `6502/target/sim65_cli.s:11-12`。`fe_compile_run` が生成entryを実行する間も保持される |
| `fe_compile_run → generated entry` | 2 | `6502/frontend/basic.s:127-130` の手動high/low `PHA` |
| generated driver → top-level chunk | 2 | `cc_call` が `JSR` を生成。`6502/frontend/basic.s:192-218`、`6502/compiler/emitter.s:42-57` |
| 名前付き手続きの最大連鎖 | 52 | 小文字identityは最大26個（`a..z`）なので、26 × 2 byte。呼出先は完成済みbindingのみ解決し、definitionは完成後に公開する。現在のsource orderでは直接・相互再帰を作れない（`6502/frontend/basic.s:742-799`、`6502/compiler/binding.s`） |
| generated code → runtime helper | 2 | `cc_call` 等の固定templateが `JSR` を生成 |
| helper → nested helper | 例として2以上 | `rt_pop_condition → rt_need_one`、`rt_div → unsigned_divide`、storage helper → `rt_need_two` 等。同時に通らない呼出し経路を単純加算しない（`runtime/state.s:65-77, 153-181`、`runtime/arithmetic.s:74-109`） |
| helper内のpush byte | `rt_push` で2以上 | `runtime/state.s:46-63`。`rt_load_reg → rt_push` では `rt_load_reg` のPHA 1 byteも同時に残る（`runtime/state.s:122-132`）。helper呼出し自身のJSRは上段に含む |
| serial出力ライブラリ経路 | 未確認 | 最大の手続き連鎖下で `rt_print_number → serial_out → _putchar` が動き得る。cc65 library内の到達可能な呼出し経路を逆アセンブルして上界化していない |

名前付き手続きの最深連鎖と一つのnested helperまでの**静的部分小計は62B**（2 + 2 + 2 + 52 + 2 + 2）。serial出力経路などを含まず、全体の上界ではない。26手続きの項は現在のbinding数から得た構造上の最大値であり、全workloadがこの深さに達するという意味ではない。

## compile/runtime各phaseの最大上界と予算比較

| phase | 最大上界 | 物理256B | #239 target-visible予算128B |
| --- | --- | --- | --- |
| runtime | **未解決**。上記62Bの部分小計に、cc65 `_putchar` とstartup/libraryの到達経路を加えた全体上界が未算定 | **未解決** | **未確認** |
| compile | **未解決**。式/groupとstructured blockの再帰は各16段guard（`nesting`、`structured_depth`）。While frameも別に16件制限。各段の同時生存JSRとPHA/PHP、およびbackend・serial入力helperの最大ネストは未列挙 | **未解決** | **未確認** |

compileとruntimeは順次実行される。compile完了後に `fe_compile_run` が `rt_init` を呼び、その後に生成コードへ入るため、両phaseの最大値は加算しない。`_main` の呼出しframeは両phaseを通じて残る。この監査では、8-bit SP wrapを有効な挙動として必要とする経路は見つからなかったが、wrapしないことを裏付ける全経路の列挙も未完了。

**物理容量の判定: 未解決。** 利用できる静的根拠では、両phaseが256B未満と証明できていない。

**128B設計予算の判定: 未確認。** #239の予算は維持し、変更を提案しない。

## 根拠の関数・ソース行と再現手順

主な参照箇所:

- CLI入口: `crates/poc/extended-classic-vtl/6502/target/sim65_cli.s:11-21`
- compile、driver構築、runtimeへの移行、復帰先push: `crates/poc/extended-classic-vtl/6502/frontend/basic.s:84-145, 190-230`
- Write ownerの完了と公開順: `crates/poc/extended-classic-vtl/6502/frontend/basic.s:742-800`
- binding解決と完了確認: `crates/poc/extended-classic-vtl/6502/compiler/binding.s`
- runtime helper呼出しとpush: `crates/poc/extended-classic-vtl/6502/runtime/state.s`、`runtime/arithmetic.s`、`runtime/primitives.s`
- cc65 adapter: `crates/poc/extended-classic-vtl/6502/target/sim65_adapter.s:10-25`

成功workloadの確認コマンド:

```sh
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_examples -- --ignored --nocapture
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_negative -- --ignored --nocapture
```

両方とも成功した。`sim65_examples` はQueens両版（各92解）を含む全6 fixtureを実行し、Rustと6502のraw stdout一致およびexit 0を確認した。negative suiteも成功した。これらはworkloadの根拠であり、stack high-water測定ではない。

## その他の資源の簡易評価

| 資源 | 現行profileと根拠 | 評価 |
| --- | --- | --- |
| shared arena | 2048B（`compiler/arena.s`）。codeと保持中のP0a template sourceが共有。P0a calibration成功例とarena exhaustionのnegative testあり | byte単位の占有量は報告しない。予約容量を使用済みcodeと見なさない。P0a固有の容量評価は#354 |
| runtime value stack | 16 Cells（`runtime/state.s`）、`rt_push` にoverflow guard。既存workloadとruntime failure経路を検証済み | 動的な最大depthは本監査で未測定 |
| indexed storage | 128 Cells / 256B。`runtime/state.s:152-182` はaddress下位7 bitを選択に使う。Queens storage fixture成功 | 現行profileのworkload動作は確認済み。異なるCellの使用数は未測定 |
| compile table | source work stack 16×u16、numeric labels 32/owner、unresolved fixups 32/owner、structured/While深さ16、top-level chunks 53。到達可能な代表failure pathを既存テストで確認 | 固定profileとguarded failure経路を確認。最新#381 scopeでは全workloadの高水位表を必須としない |

## 残存リスクと判定

現行の代表workload成功と複数のcompile-time resource failure経路は確認できた。一方、compile時とruntime時のhardware stack安全性は、call pathの算定が未完了のため証明できていない。production assembly、ABI、CLI option、容量profileは変更していない。

次の調査では、到達可能なcc65 startup/serial routineとfrontendの再帰経路を列挙し、同時に積まれるJSR/PHA/PHPを算定する。静的算定でなお不明な場合は、compileとruntimeを分けた短い代表ケースで `sim65 --trace` のCPU `S` を確認する。これは補助観測であり、到達可能な全経路の上界の代替にはしない。#239の128B比較も別欄で維持する。両phaseの根拠が揃うまでは、物理256Bの判定を通過としない。
