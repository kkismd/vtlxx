# 6502 target PoC

このディレクトリは EC07 の sim6502 固有コードを置く。`target/sim65_adapter.s` が 1 byte の `serial_in` / `serial_out` と 8-bit status の `halt` を提供する。A register を受け渡しに使う呼び出し規約と `sim6502.lib` への接続はこの target 内だけの取り決めであり、portable ECVTL runtime ABI ではない。入力終端の扱いは後続 issue が定める。

host 側の `tests/sim65_6502.rs` が `ca65 -t sim6502`、`ld65 -t sim6502`、`sim65 -x` を起動する。生成物は OS の一時ディレクトリに作られ、テスト終了時に削除される。

```sh
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_6502 -- --ignored
```

通常の `cargo test --workspace` は cc65 を必要としない。

## Runtime helper（#274）

`runtime/state.s`、`runtime/arithmetic.s`、`runtime/primitives.s` は第一6502 proof profile の実行状態と primitive を実装する。後続の native emitter は `rt_init` を一度呼び、その後に以下の entry point を JSR で呼ぶ。いずれも target 内の link contract であり、portable ECVTL API ではない。

| entry | 入力 | value stack effect |
| --- | --- | --- |
| `rt_push` | A = Cell low byte、X = high byte | `( -- value )` |
| `rt_load_reg` / `rt_store_reg` | X = fixed register index 0..25 | `( -- value )` / `( value -- )` |
| `rt_load_storage` / `rt_store_storage` | stack 上の address | `( address -- value )` / `( address value -- )` |
| `rt_add`, `rt_sub`, `rt_mul`, `rt_div`, `rt_rem` | stack 上の operands | `( lhs rhs -- result )` |
| `rt_eq`, `rt_ne`, `rt_lt`, `rt_le`, `rt_gt`, `rt_ge` | stack 上の operands | `( lhs rhs -- 0\|1 )` |
| `rt_print_number`, `rt_print_char` | stack 上の value | `( value -- )` |

helper は A/X/Y と processor flags を保存しない。`rt_push` の引数と register helper の X 以外に CPU register の入力契約はない。Cell は little-endian の signed 16-bit、stack は 16 Cell、A-Z は 26 Cell、storage は下位7 bitで選ぶ128 Cellの ring。A-Z と stack は zero page、storage は通常 RAM に置く。`rt_init` は register、storage、stack depth を初期化する。

fatal status は underflow = 1、overflow = 2、division by zero = 3、remainder by zero = 4 とし、いずれも #273 の `halt` へ移る。これは sim65 fixture が success と runtime error を区別するための target 内の値である。error 検出前に stack operand を消費しない。

sim65 の失敗時状態確認だけに `RT_TEST_PROBE` を指定すると、HALT 直前に fixture の `rt_test_probe` を呼ぶ。通常の target build にはこの hook は入らない。

```sh
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_runtime -- --ignored
```
