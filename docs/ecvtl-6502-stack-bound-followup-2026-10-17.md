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
| **確認したruntime経路の算定値** | **69** | 2 + 2 + 2 + 2 + 52 + 6 + 3 |

#239のtarget-visible分はcrt0 2Bとcc65 `_fgetc` 内部3Bを除き64Bで、確認したruntime経路では128B予算内です。物理256Bに対する余裕は、この経路の算定値との比較で187Bです。serial output経路もlinked `_fputc` を確認しましたが、これだけでは全到達経路の上界を証明しません。

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
| **確認したcompile経路の算定値** | **185** | 上記の同時使用量 |

深い式を読み進める時点では17個のoperandがtemporaryを保持し、各呼出しframeも同時に残る。`basic.s:1586-1632` の `operand` / `value` recursionとPHA、`basic.s:1771-1778` のnesting guard、`basic.s:1537-1580` のblock recursionを根拠とする。

185Bは、到達可能な深い正常compile経路について算出した値です。これは全compile経路の上界とは確認できていません。以下の経路比較では、call depth・生存PHA・helper leafまでの追加frameを含む最大値をlink済み機械語から確定できていないものを「未算定」とします。そのため、185Bより小さいという定量的な比較は主張しません。

| 経路 | 深さ上限・生存PHAの確認 | leafまでの追加frame | 185B以下の比較 |
|---|---|---|---|
| 深いblock内のIF / While条件式とbackend発行・patch | block上限16は確認。条件式の深さと、条件解析後に発行/patchへ進む各call pathの同時最大は未算定 | `cc_*` emitter / patch helperの最大連鎖を未算定 | 未確認 |
| template reader・原sourceとの入力切替 | template本文に制約がありtemplate文/control文への再帰がないこと、およびreader切替を確認。通常source readerと重なる際の最大frame合計は未算定 | `template_reader_*` から既存reader/helperへ進む最大連鎖を未算定 | 未確認 |
| arithmetic/operator後の追加compile helper | 16 group・17 operatorの確認経路は上表に含む。operator別のbackend呼出し・エラー経路を含む全call path比較は未算定 | operator dispatchから `cc_call` / emitter leafまでの全最大を未算定 | 未確認 |
| startup / serial入出力 | runtimeで `_fgetc` 経路69Bを算定。compile中の各serial読込地点およびstartup helperとの最大合成は未算定 | cc65 startup、`_fgetc` / `_fputc`、adapter全組合せのphase別最大を未算定 | 未確認 |

これらは仕様上の再帰guardだけでは排除できず、全経路の保守的上界を求めるcall-path監査が残っています。したがって185Bを物理容量判定の上界として使いません。

crt0 2Bとcc65 `_fgetc` 内部3Bを除いた、確認したcompile経路のtarget-visible算定値は180Bです。さらに、source read helperへ入る前のtarget-visible部分だけで少なくとも**170B**（2B `_main → fe_compile_run` + 2B top-level statement + 64B block frames + 68B expression JSR + 34B PHA）となります。したがって#239の128B通常経路予算は正常sourceで少なくとも42B超過します。この下限判定は、全経路上界が未確認であることに依存しません。

## 他資源と再現確認

- `cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_examples -- --ignored --nocapture`: 6例成功、Queens通常/Storage各92解、Rust/6502 raw stdout一致、exit 0。
- `cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_negative -- --ignored --nocapture`: 成功。
- 上記201-byte fixtureの生成、Rust referenceと6502 CLIの実行、raw stdout比較は次の手順で再現できます（repository root、Python 3とcc65が必要）。

```sh
python3 -c 'from pathlib import Path; s="%=0 [ " + "%=1 [ " * 15 + "A=" + "0+(" * 16 + "0+0" + ")" * 16 + " ]" * 16 + " ?=A"; Path("/tmp/ecvtl-stack-201.vtl").write_bytes(s.encode()); print(len(s.encode()))'
cargo run -p vtlxx-poc-extended-classic-vtl --bin ecvtl -- /tmp/ecvtl-stack-201.vtl > /tmp/ecvtl-reference.out
cargo run -p vtlxx-poc-extended-classic-vtl --bin ecvtl6502 -- /tmp/ecvtl-stack-201.vtl > /tmp/ecvtl-6502.out
cmp /tmp/ecvtl-reference.out /tmp/ecvtl-6502.out
od -An -tx1 /tmp/ecvtl-reference.out
```

最初のコマンドはsource byte数 `201` を表示します。最後の出力はASCII `0` (`30`) です。両CLIが成功終了し、`cmp` が差分なしで完了することを確認します。link後のmap/labelは、CLIの `build_and_run` と同じ14 sourceを次の順でassembleして取得できます。

```sh
tmp=$(mktemp -d)
root=crates/poc/extended-classic-vtl
sources="6502/target/sim65_cli.s 6502/frontend/basic.s 6502/frontend/template.s 6502/frontend/template_reader.s 6502/frontend/source.s 6502/compiler/arena.s 6502/compiler/emitter.s 6502/compiler/owner.s 6502/compiler/binding.s 6502/compiler/work_stack.s 6502/runtime/state.s 6502/runtime/primitives.s 6502/runtime/arithmetic.s 6502/target/sim65_adapter.s"
i=0
objects=()
for source in $sources; do
  ca65 -t sim6502 "$root/$source" -o "$tmp/$i.o" || exit
  objects+=("$tmp/$i.o")
  i=$((i + 1))
done
ld65 -t sim6502 -m "$tmp/audit.map" -Ln "$tmp/audit.lbl" -o "$tmp/audit.prg" "${objects[@]}" sim6502.lib
```

`audit.map` と `audit.lbl` がlink時の配置・symbol根拠です。この `sim6502` linkのload addressは `0x0200` なので、`da65 -S 0x0200 -o "$tmp/audit.dis" "$tmp/audit.prg"` で逆アセンブルできます。これは実行中のSP traceではなく、生成物の静的確認を再現する手順です。
- 他資源の現行値: shared arena 2048B（P0a sourceと共有、P0a固有評価は#354）、runtime value stack 16 Cells、indexed storage 128 Cells、source work stack 16×u16、labels/fixups 32/owner、structured/While depth 16、top-level chunks 53。既存E2Eと#380の代表failure経路で動作確認。全workload高水位は本監査で測っていない。

## 判定と残件

| 判定項目 | 結果 |
| --- | --- |
| 物理256B容量 | **未解決**。185B / 69Bは確認した経路の算定値であり、全到達経路の保守的上界は未証明。phaseは非同時だが容量条件の通過とは判定しない |
| #239 target-visible 128B予算 | **超過**。compileのtarget-visible部分だけで170B以上。runtimeは64B |

production assembly、CLI、semantics、resource profileは変更していない。128B超過を是正または許容する判断は別途必要で、#381では予算を緩和しない。sim65 traceは本環境で利用できず、個別経路の数値はlink後のcall pathとdepth guardからの静的導出です。物理256Bの完了判定には全到達経路の上界監査が残っています。
