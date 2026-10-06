# 6502 target PoC

このディレクトリは EC07 の sim6502 固有コードを置く。`target/sim65_adapter.s` が byte と EOF を carry flag で区別する `serial_in`、1 byte の `serial_out`、8-bit status の `halt` を提供する。`serial_in` は A に任意の byte（`$ff` を含む）を返し carry clear、EOF は carry setで返す。A register を受け渡しに使う呼び出し規約と `sim6502.lib` への接続はこの target 内だけの取り決めであり、portable ECVTL runtime ABI ではない。

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

## Native compiler backend（#275）

`compiler/` は source syntax を扱わない 6502 固有 backend である。`cc_init` は 1 回の compile-run に先立って 2048 B の code arena、binding、owner、source work stack を初期化する。generated code は arena の RAM に残り、owner 完了後の entry address は u16 の絶対 address として利用できる。compiler の失敗は `cc_status` に保持し、後続の compile-run driver が runtime を開始せず HALT する。

target 内の呼び出し規約は次の通り。u16 の主引数・戻り値は A=low、X=high。`cc_patch` は A/X に operand low-byte address、`cc_arg` に書き込む u16 を取る。`cc_publish` は A=role（0 Write、1 SourceProcedure）、X=`a..z`、`cc_arg`=completed target を取る。`cc_resolve` は同じ role と identity から A/X に target を返す。raw arena pointer、table layout、scratch は frontend 向け contract に含めない。

| 操作 | 役割 |
| --- | --- |
| `cc_mark`、`cc_emit_byte`、`cc_emit_u16`、`cc_patch` | append と既存 operand の little-endian patch |
| `cc_push_const`、`cc_call`、`cc_return`、`cc_jump`、`cc_jz` | fixed native template |
| `cc_load_reg`、`cc_store_reg` | X = register index 0..25 を受け取り、index setup と register helper call を backend で emit |
| `cc_jump_placeholder`、`cc_jz_placeholder`、`cc_patch_here`、`cc_jump_to` | absolute target の後方 patch と直接 jump |
| `cc_begin_owner`、`cc_complete_owner` | owner の開始、未解決参照の検証、final RTS、completed target |
| `cc_define_label`、`cc_label_jump` | owner-local の numeric label と forward fixup |
| `cc_work_push`、`cc_work_pop`、`cc_work_swap`、`cc_work_reset` | runtime value stack と別の 16×u16 source work stack |

`cc_status = 0` は成功。非ゼロは compile-run failure で、1 arena full、2 invalid patch、3 owner misuse、4 duplicate label、5 label full、6 fixup full、7 unresolved reference、8 binding error、9 work stack error。失敗後の継続は契約に含めない。placeholder は patch 完了まで追跡し、未解決のまま owner を完了できない。完了済み owner の bytes は patch できない。binding には完了済み owner の entry だけを登録できる。

JZ template が呼ぶ `rt_pop_condition` は runtime value stack の top Cell を 1 つ consume し、0 の場合に zero flag を立てる target 内 helper。source control の解釈は持たない。

```sh
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_backend -- --ignored
```

## Frontend framing reader（#290）

`frontend/source.s` は `u16` little-endian length headerを読み、指定 byte 数だけ `serial_in` から取得する bounded reader (`fe_init` / `fe_next`) を提供する。sourceを保存せず、remaining countが0になった後は入力 helperを呼ばない。premature EOFは `fe_status = 1` と carry setで返す。`frontend_framing` sim65 fixture は source bytesの後ろにある runtime input sentinelを読めることと、truncated sourceをfailureにすることを確認する。

```sh
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_6502 source_framing -- --ignored
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_6502 truncated_source_frame -- --ignored
```

## Basic source frontend（#276）

`frontend/basic.s` は `fe_init` / `fe_next` の 1 byte 先読みだけで basic top-level statement を読み、`cc_begin_owner` から `cc_complete_owner` までの 1 owner に native code を生成してから実行する。`fe_compile_run` は成功時 carry clear、compile 失敗時 carry set で返し、`fe_compile_status` は source/transport = 1、syntax = 2、literal range = 3、backend = 4 を示す。失敗時は生成途中の code を実行しない。source frame の終端以降は reader を呼ばず、残りの serial bytes は runtime input として保持する。

現在の対象は signed i16 literal、A-Z register、`@(address)` read / `@=address,value` write、算術・比較、`?` / `$` output、grouping、左結合の式、左から右への comma operand、RHS 先頭だけの `~` である。`[` の旧 stack surface、named integer、label、block、user-defined Write は後続 issue の対象である。

```sh
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_frontend -- --ignored
```
