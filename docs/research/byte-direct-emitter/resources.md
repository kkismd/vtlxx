# #707 製品相当リンクと stack の確認

比較基準は `68615c3ff7c13682e50ddd061ad1aa953ebe70c2`。次のコマンドは
基準の source を git から読み、作業 tree と同じ条件で独立に組み立て、map と
主要 object の内訳を表示する。比較用一時ファイルは製品や probe に含めない。

```sh
python3 docs/research/byte-direct-emitter/product_sizes.py
```

ca65/ld65 V2.18 (Ubuntu 2.19-1)、`vtlxx6502` の13 source と順序、
`VTLXX_RAW`、entropy unavailable、製品 `mos6502.cfg`、load `$0200`、
service `$FFEF`。map/labels の追加は生成命令を変えない。基準と差分方式の
結果は以下のとおり。これは frontend 未接続のため最終 compiler の増分ではない。

| 項目 | 基準 B | 実験 B | 差 B |
| --- | ---: | ---: | ---: |
| CODE | 6876 | 7106 | +230 |
| RODATA | 40 | 50 | +10 |
| ZEROPAGE | 108 | 108 | 0 |
| BSS | 29291 | 29293 | +2 |
| arena（BSS 内） | 24576 | 24576 | 0 |
| emitter object CODE | 574 | 804 | +230 |
| emitter object RODATA | 0 | 10 | +10 |
| emitter object BSS | 14 | 16 | +2 |

emitter CODE は #704 の静的目安 105–175 B を超過する。上記には入力の検証、
owner 検査、全 template 予約、Cell 変換および 2 入口の共通出力処理を含む。
実験 link の arena は `$2A06..$8A05`、patch 表は `$8A06`、Cell 配列は
`$29..$5C`。これらは測定条件下の配置であり、物理 address を frontend の
ABI として公開するものではない。

## stack の監査

上記製品 link は新規 emitter を含むが既存 frontend から呼び出さない。既存の
guard 間・入口外・runtime の呼出し命令および既存 helper の実装は変わらず、
`docs/ecvtl-6502-stack-guard.md` の #650 / #672 / #681 の評価（guard 間
K=24 B、成功 S>=25、入口外 19 B）はこの未接続 link に適用される。

新規 `cc_byte_op` と `cc_byte_store_cell` の内部には PHA/PHP がなく、
`owner_require_active`、`cc_reserve`、`cc_write_byte_raw` を逐次 JSR する。
各 helper はこの経路では leaf で、入口からの同時追加は最大 2 B、呼出元の
JSR を含め 4 B。最後の書込みは tail JMP。引数検査、owner 不正、容量不足の
fatal は push なしで `session_backend_error` へ tail JMP し、保存された session
入口 S を復元して caller に戻る。生成本体は PHA/PHP/JSR を追加せず、通常の
owner completion が RTS を付ける。これらは新規入口の局所上界であり、後続
frontend からの呼出し位置とそこで生存する frame を含む guard 間の再監査は
別途必要である。IRQ/NMI、別 adapter や実機の安全性を主張しない。

## probe と cycles

`cargo test -p vtlxx --test sim65_backend -- --ignored --test-threads=1` は
9 件成功。新規 case 58–70 で全20合法 opcode/mode、全 Cell の byte と格納、
通常実行、owner 不正、容量境界、内部不正入力、fatal 診断と復帰を確認する。
既存 guard probe は `cargo test -p vtlxx --test sim65_frontend compile_stack_guard
-- --ignored --test-threads=1` で実行できる。probe で観測した個別経路は、
上記の静的上界そのものの代替ではない。

```sh
python3 docs/research/byte-direct-emitter/probe_cycles.py
```

sim65 V2.18 の `-c` は case 58 が 334381 cycles、case 59 が 305829 cycles。
いずれも sim6502 startup から終了までの総数で、fixture の compile、検査、
runtime、出力、終了を含む。命令生成単独の時間でも、両 case の差を用いた
操作単価でもない。実機の bus や MMIO は測定していない。

生成命令そのものの NMOS 6502 静的 cycles は immediate/zp/abs の順に
LOAD/AND/OR/XOR が 2/3/4、STORE が不可/3/4、ADD/SUB が 4/5/6。
Cell 格納は 8 cycles、通常 call の JSR/RTS は別途 12 cycles。固定
address の `$00FF`/`$0100`/`$FFFF` は opcode 検査であり、その位置を
runtime 試験で実行したという意味ではない。
