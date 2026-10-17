# ECVTL 6502 hardware stack 続行監査（#381）

## 対象main SHAと検証環境

- 対象main: `b60ef772a2b7ba7fb3977a5d6909a2d85da14fdd`（PR #414 merge後）
- `ca65` / `ld65` / `sim65`: V2.18（Ubuntu package 2.19-1）
- `sim65 --trace` は `Unknown option: --trace` で利用できず、`sim65 -h` にも掲載されていない。`$FFCB` へtrace modeを書き込む小さいsim65 programも試したが、trace出力は得られなかった。
- `src/bin/ecvtl6502.rs::build_and_run` と同じ14 assembly sourceをassembleし、`sim6502.lib` とlinkしたmap・label・機械語を監査した。以下のstack値はSP初期値 `$FF` からの静的導出であり、動的SP trace測定値ではない。
- JSRは2B、PHA/PHPは各1Bとして同時使用量を加算し、JMP/tail JMPは加算しない。

## Runtime phase

深い経路は `crt0 → _main → fe_compile_run → generated entry → driver → chunk → 最大26手続き連鎖 → runtime helper → serial input`。`callmain` は `_main` へtail JMPし、software stack引数の `pushax` はその前にreturnするため、runtime中に残るcrt0 frameは2B。

| 同時に積む要素 | B | 根拠 |
| --- | ---: | --- |
| crt0 `_start → callmain` の保持frame | 2 | linked `sim6502.lib` startup/callmain。callmain内の引数pushはreturn後に消える |
| `_main → fe_compile_run` | 2 | `6502/target/sim65_cli.s:11-12` |
| generated entryへの手動復帰先 | 2 | `6502/frontend/basic.s:127-131` |
| driver → top-level chunk | 2 | `basic.s:192-218`、generated call template |
| 最大26個のWrite手続き呼出し | 52 | 26 identities × 2B。完成済みbindingのみ解決し、定義順から再帰不可（`basic.s:742-799`, `compiler/binding.s`） |
| generated code → `rt_read_char` → `serial_in` → `_getchar` | 6 | 各JSR 2B（`runtime/primitives.s:22-43`, `target/sim65_adapter.s:10-18`）。`_getchar` は `_fgetc` へtail JMP |
| linked `_fgetc` 内部の最大追加量 | 3 | map上 `_fgetc` は `0x1D25`。nested `JSR pushax` の2Bとpushax内PHA 1Bが同時 |
| **sim65上のruntime静的上界** | **69** | 2 + 2 + 2 + 2 + 52 + 6 + 3 |

#239のtarget-visible分はcrt0 2Bとcc65 `_fgetc` 内部3Bを除き64Bで、128B予算内。物理256Bに対しては187Bの余裕がある。serial output経路もlinked `_fputc` を確認したが、このinput経路の69Bを超えない。

## Compile phase

`structured_depth` と `nesting` は別guardであり、16段のblockと16段の式groupは同時に成立する。次の201-byte sourceを生成し、compileを最後まで通すことを確認した。outer IFはfalseなので深い式はruntimeで評価されない。

```text
"%=0 [ " + "%=1 [ " × 15
+ ("A=" + "0+(" × 16 + "0+0" + ")" × 16)
+ " ]" × 16 + " ?=A"
```

Rust referenceと6502 CLIの双方がexit 0、raw stdout `0` で一致した。これは有効な正常系compile経路の確認であり、SPの実測ではない。

| 同時に積む要素 | B | 算定 |
| --- | ---: | --- |
| crt0 frame | 2 | `_start → callmain` |
| `_main → fe_compile_run → top-level statement` | 4 | 2Bずつ |
| 16段block内の `required_block` とbody `statement` | 64 | 16 × (2B + 2B) |
| innermost statementから16 groupを通りterminal valueへ至るJSR | 68 | statement→operand 2B + 16段のrecursive value/operand call 64B + terminal RHS value 2B |
| operandがnested valueを待つtemporary PHA | 34 | 17 operands × (operator 1B + X 1B) |
| terminal literalのlookahead/input path | 13 | `parse_literal` 2B + `peek` 2B + `fe_next` 2B + `serial_in` 2B + `_getchar` call 2B + `_fgetc`内部3B |
| **sim65上のcompile静的上界** | **185** | 上記の同時使用量 |

深い式を読み進める時点では17個のoperandがtemporaryを保持し、各呼出しframeも同時に残る。`basic.s:1586-1632` の `operand` / `value` recursionとPHA、`basic.s:1771-1778` のnesting guard、`basic.s:1537-1580` のblock recursionを根拠とする。

emitter/backend helper callはこのreader経路より浅く、PHA保持中にsource再帰を継続しない。control patchやbinding helperの最大call chainも、式recursionが解けた後かblock深度の小さい時点で実行される。separator/comment走査は末尾JMP loopでstackを再帰的に増やさない。P0a template展開は一段のstatement再呼出しだが、template bodyはassignmentのみでtemplate/control文へ再帰できず、template readerの追加callも通常source reader/input callより浅い。startup時の最大は `callmain → pushax` の5Bで、compile上界を超えない。

crt0 2Bとcc65 `_fgetc` 内部3Bを除いたtarget-visible compile上界は180B。さらに、source read helperへ入る前のtarget-visible部分だけで少なくとも**170B**（2B `_main → fe_compile_run` + 2B top-level statement + 64B block frames + 68B expression JSR + 34B PHA）となる。したがって#239の128B通常経路予算は正常sourceで少なくとも42B超過する。sim65全体の185B上界は物理256Bを71B下回る。

## 他資源と再現確認

- `cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_examples -- --ignored --nocapture`: 6例成功、Queens通常/Storage各92解、Rust/6502 raw stdout一致、exit 0。
- `cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_negative -- --ignored --nocapture`: 成功。
- 上記201-byte fixtureは `target/debug/ecvtl` と `target/debug/ecvtl6502` に同一sourceを渡し、exit 0 / stdout `0` / raw bytes一致。
- 他資源の現行値: shared arena 2048B（P0a sourceと共有、P0a固有評価は#354）、runtime value stack 16 Cells、indexed storage 128 Cells、source work stack 16×u16、labels/fixups 32/owner、structured/While depth 16、top-level chunks 53。既存E2Eと#380の代表failure経路で動作確認。全workload高水位は本監査で測っていない。

## 判定と残件

| 判定項目 | 結果 |
| --- | --- |
| 物理256B容量 | **通過**。compile 185B、runtime 69Bの静的上界。phaseは非同時 |
| #239 target-visible 128B予算 | **超過**。compileのtarget-visible部分だけで170B以上。runtimeは64B |

production assembly、CLI、semantics、resource profileは変更していない。128B超過を是正または許容する判断は別途必要で、#381では予算を緩和しない。sim65 traceは本環境で利用できず、数値はlink後のcall graphとdepth guardからの静的導出である。
