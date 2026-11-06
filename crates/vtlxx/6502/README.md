# VTLXX 6502 ターゲット

このディレクトリは VTLXX の sim65 固有コードを置く。`target/sim65_adapter.s` が byte と EOF を carry flag で区別する `serial_in`、1 byte の `serial_out`、8-bit status の `halt` を提供する。`serial_in` は A に任意の byte（`$ff` を含む）を返し carry clear、EOF は carry setで返す。A register を受け渡しに使う呼び出し規約と `sim6502.lib` への接続はこの target 内だけの取り決めであり、portable VTLXX runtime ABI ではない。

host 側の `tests/sim65_6502.rs` が `ca65 -t sim6502`、`ld65 -t sim6502`、`sim65 -x` を起動する。生成物は OS の一時ディレクトリに作られ、テスト終了時に削除される。

```sh
cargo test -p vtlxx --test sim65_6502 -- --ignored
```

通常の `cargo test --workspace` は cc65 を必要としない。

## Runtime helper（#274）

`runtime/state.s`、`runtime/arithmetic.s`、`runtime/primitives.s` は現在の6502 target の実行状態と primitive を実装する。native emitter は `rt_init` を一度呼び、その後に以下の entry point を JSR で呼ぶ。いずれも target 内の link contract であり、portable VTLXX API ではない。

sim65 では、全 source の compile と driver 完成の後に `rt_init` へ移るため、compile 専用 Zero Page scratch と runtime 状態を `target/zp_layout.s` の同じ **100 B**（`zp_phase`）に時間的に重ねて配置する。compile view は offset `0..79`、runtime view は `0..99` を使用する。driver entry、frontend/backend の結果 status、および両フェーズで使う cc65 library の領域は別予約であり、この100 Bに含めない。compile failure は runtime 開始前に戻る。runtime から新たな source compile へ戻る経路はない。

| entry | 入力 | value stack effect |
| --- | --- | --- |
| `rt_push` | A = Cell low byte、X = high byte | `( -- value )` |
| `rt_load_reg` / `rt_store_reg` | X = fixed register index 0..25 | `( -- value )` / `( value -- )` |
| `rt_load_storage` / `rt_store_storage` | stack 上の address | `( address -- value )` / `( address value -- )` |
| `rt_add`, `rt_sub`, `rt_mul`, `rt_div`, `rt_rem` | stack 上の operands | `( lhs rhs -- result )` |
| `rt_eq`, `rt_ne`, `rt_lt`, `rt_le`, `rt_gt`, `rt_ge` | stack 上の operands | `( lhs rhs -- 0\|1 )` |
| `rt_print_number`, `rt_print_char` | stack 上の value | `( value -- )` |
| `rt_read_char` | — | `( -- char_or_eof )` |

helper は A/X/Y と processor flags を保存しない。`rt_push` の引数と register helper の X 以外に CPU register の入力契約はない。Cell は little-endian の signed 16-bit、stack は 16 Cell、A-Z は 26 Cell、storage は下位8 bitで選ぶ256 Cellの ring。A-Z と stack は zero page、storage は通常 RAM に置く。`rt_init` は register、storage、stack depth を初期化する。

fatal status は underflow = 1、overflow = 2、division by zero = 3、remainder by zero = 4 とし、いずれも #273 の `halt` へ移る。これは sim65 fixture が success と runtime error を区別するための target 内の値である。error 検出前に stack operand を消費しない。

sim65 の失敗時状態確認だけに `RT_TEST_PROBE` を指定すると、HALT 直前に fixture の `rt_test_probe` を呼ぶ。通常の target build にはこの hook は入らない。

`rt_read_char` は `$` PrimaryRead の固定runtime targetである。stack空き容量をtransport readより先に検査し、NULを読み捨てて1..255またはEOF=-1を1 Cellとしてpushする。EOFはruntime session内で記憶し、後続readも-1を返す。sim65 adapterの `_getchar` はbyteとEOFだけを区別し、recoverable I/O failureを個別に表せないため、このtargetではそのerror分類を検証できない。

```sh
cargo test -p vtlxx --test sim65_runtime -- --ignored
```

## Native compiler backend（#275）

`compiler/` は source syntax を扱わない 6502 固有 backend である。`cc_init` は 1 回の compile-run に先立って 24,576 B のコード専用 arena、binding、owner を初期化する。generated code は arena の RAM に残り、owner 完了後の entry address は u16 の絶対 address として利用できる。compiler の失敗は `cc_status` に保持し、後続の compile-run driver が runtime を開始せず HALT する。

`compiler/arena.s` は生成コード専用の24,576 B arenaと、arena全域のbyte位置を追跡する`patch_bits`・`completed_bits`（各3,072 B）を保持する。手続テンプレートのsource保存領域や仮想readerは廃止済みであり、通常のsource frameは`frontend/source.s`が読み取る。コードの予約・patch・owner完成と、完成済みbindingのみ公開する順序は維持する。

target 内の呼び出し規約は次の通り。u16 の主引数・戻り値は A=low、X=high。`cc_patch` は A/X に operand low-byte address、`cc_arg` に書き込む u16 を取る。`cc_is_bound`、`cc_publish`、`cc_resolve` は `cc_name_len` と `cc_name` に入れた ASCII 小文字1〜8文字の通常 Write 名を取る。`cc_is_bound` は bound なら carry set、unbound なら carry clear を返し、有効な問い合わせでは `cc_status` と binding table を変更しない。`cc_publish` は `cc_arg`=completed target を取り、`cc_resolve` は完全名から A/X に target を返す。raw arena pointer、table layout、scratch は frontend 向け contract に含めない。

| 操作 | 役割 |
| --- | --- |
| `cc_mark`、`cc_emit_byte`、`cc_emit_u16`、`cc_patch` | append と既存 operand の little-endian patch |
| `cc_push_const`、`cc_call`、`cc_return`、`cc_jump`、`cc_jz` | 固定のnative命令列を生成（廃止された手続テンプレートとは別） |
| `cc_load_reg`、`cc_store_reg` | X = register index 0..25 を受け取り、index setup と register helper call を backend で emit |
| `cc_jump_placeholder`、`cc_jz_placeholder`、`cc_patch_here`、`cc_jump_to` | absolute target の後方 patch と直接 jump |
| `cc_begin_owner`、`cc_complete_owner` | owner の開始、未解決参照の検証、final RTS、completed target |
| `cc_define_label`、`cc_label_jump` | owner-local の numeric label と forward fixup |

`cc_status = 0` は成功。非ゼロは compile-run failure で、1 arena full、2 invalid patch、3 owner misuse、4 duplicate label、5 label full、6 fixup full、7 unresolved reference、8 binding error。失敗後の継続は契約に含めない。placeholder は patch 完了まで追跡し、未解決のまま owner を完了できない。完了済み owner の bytes は patch できない。binding には完了済み owner の entry だけを登録できる。

JZ用の生成命令列が呼ぶ `rt_pop_condition` は runtime value stack の top Cell を 1 つ consume し、0 の場合に zero flag を立てる target 内 helper。source control の解釈は持たない。

```sh
cargo test -p vtlxx --test sim65_backend -- --ignored
```

## Frontend framing reader（#290）

`frontend/source.s` は `u16` little-endian length headerを読み、指定 byte 数だけ `serial_in` から取得する bounded reader (`fe_init` / `fe_next`) を提供する。sourceを保存せず、remaining countが0になった後は入力 helperを呼ばない。premature EOFは `fe_status = 1` と carry setで返す。`frontend_framing` sim65 fixture は source bytesの後ろにある runtime input sentinelを読めることと、truncated sourceをfailureにすることを確認する。

```sh
cargo test -p vtlxx --test sim65_6502 source_framing -- --ignored
cargo test -p vtlxx --test sim65_6502 truncated_source_frame -- --ignored
```

## Basic source frontend（#276 / #293 / #301 / #304）

`frontend/basic.s` は `fe_init` / `fe_next` の 1 byte 先読みだけで source を読み、top-level runtime code を ownerごとの completed chunk にする。top-level の `&=p [ ... ]` は ASCII 小文字1〜8文字の Write definitionを開始する。既存 bindingを body owner開始前に `cc_is_bound` で調べ、既に定義済みなら拒否する。未定義なら現在のchunkを完了し、required blockを新しい named ownerへcompileし、owner completion成功後にだけ `cc_publish` する。definition owner自体はruntime sequenceへ含めない。block内の label / IF / While は named ownerへinline compileされる。EOF後に最大65個の chunk targetをsource順に直接callする final driver ownerを完成し、そのtargetだけをruntime entryとして実行する。chunk table容量はfrontend所有のcompile-only資源であり、超過は frontend の compile-run failureにする。したがってcompile failureでは、先に完成したchunkがあってもruntimeを開始しない。`p=...` は compile時に `cc_resolve` したtargetへの直接callとなり、`p=()` は operandなしのcallとなる。両方ともWriteの共有value stackを使う。`fe_compile_run` は成功時 carry clear、compile 失敗時 carry setで返し、`fe_compile_status` は source/transport = 1、syntax = 2、literal range = 3、compile/backend resource = 4を示す。source frameの終端以降はreaderを呼ばず、残りのserial bytesはruntime inputとして保持する。

IfElseの分岐先パッチとWhileの出口パッチ・ループ先頭は、`frontend/basic.s`の構造化された入れ子深さに対応する2つのprivateな16要素テーブルに保持する。旧`compiler/work_stack.s`と`cc_work_*`は[#524 / PR #537](https://github.com/kkismd/vtlxx/pull/537)で撤去済みであり、汎用source work stackは使用しない。`frontend/source.s`はsource frameの有界readerとして引き続き使用する。

### 通常Writeの名前と定義件数（EC10 #453）

通常のユーザー定義Write手続は `&=readnum [ ... ]` で定義し、`readnum=RHS` または `readnum=()` で呼びます。名前はASCII小文字1〜8文字で、`a` と `ab`、`read` と `reader` を完全名で区別します。同名の再定義、未定義呼出、forward callはコンパイルエラーです。callee-firstの順序で定義し、本文の完成後だけbindingを公開します。実行時の呼出はコンパイル時に解決したtargetを使用し、名前の再検索はしません。

**6502初期targetの通常Write上限は64件**です。65件目はコンパイル失敗となり、runtimeは開始しません。64件すべてを定義するsourceでも、arena等の別の資源上限は独立に適用されます。最大64定義に対しtop-level source chunkは最大65件です。Rust referenceには64件という件数制限を設けていません。6502 `compiler/binding.s` の現在のprivate名前表は `length:u8 | name[8] | completed target:u16` の11B×64件（704B）であり、これはportableな言語仕様やruntime APIではありません。

現行の直接テストは [`tests/sim65_long_write_names.rs`](../tests/sim65_long_write_names.rs)（64件成功、65件目失敗、65 chunk順序）と [`tests/long_write_names.rs`](../tests/long_write_names.rs)（Rustの65件以上、完全名照合、失敗時binding）です。長名導入時のROM/RAM/stack比較は [当時の監査記録](../../../../docs/ecvtl-6502-long-write-resources.md) にあり、その数値は現在の配置ではありません。現在の正式CLIのZP/BSS/CODE/RODATA測定は [PR #537](https://github.com/kkismd/vtlxx/pull/537) に記録されています。いずれも全経路の動的stack高水位の実測を意味しません。

コンパイル時のCPU hardware stackは、statement / required_block / operand / valueで残量を検査する。Sが25未満の場合に限り、通常の復帰を行わず、`fe_compile_status=4`、exit132で即時HALTする。stdoutは空で生成runtimeは開始しない。通常のsyntax/source/backend失敗は従来どおり呼出元へ返す。閾値・linked library・入口外経路の根拠と再監査条件は[stack guard監査](../../../../docs/ecvtl-6502-stack-guard.md)を参照。

現在の対象は signed i16 literal、A-Z register、`@(address)` read / `@=address,value` write、算術・比較、`?` / `$` output、grouping、左結合の式、左から右への comma operand、RHS 先頭だけの `~`、`&=p [ ... ]` Write definition と lowercase Write call、top-level named integer 定義と RHS / numeric label operand 参照、anonymous block `[ ... ]`、one/two-arm IF `%=`、native While `*=condition` である。definition bodyは独立ownerへcompileされ、block内の label / control はそのownerを使う。ordinary blockは surrounding owner へ inline compileされ、numeric label namespaceを共有する。IF の else は次 source form が `[` のときだけ読み、block 深さは16までである。While は IF と同じ通常の bounded RHS 条件式の後に required body block を1つ取り、条件コードをループ先頭へ生成する。backend の JZ placeholder、direct jump、patch を使って loop を生成する。IF の forward patch と While の exit patch / loop target は、block の入れ子深さに対応する frontend-private な各16-entry table に保持する。式の入れ子上限は独立している。条件式の結果は runtime で各反復1 Cell消費される。named integer はトップレベルで `.NAME := value` と定義し、`.NAME` と参照する。名前本体は `[A-Z][A-Z0-9_]+` の2〜16 byteで、接頭の `.` は名前の長さに含めず表にも保存しない。値は単一の signed i16 decimal literal、compile-run あたり128件までである。通常 statement と同じ空白・改行・コメント・source 終端で区切り、`.MAX:=10 .MIN:=-1 A=.MAX` のように同一行に置ける。`:=` の前後だけ ASCII space / HTAB を許し、定義中の改行は許さない。定義後のみ参照でき、重複定義と旧形式 `==NAME,value` や裸の複数文字定数参照は compile failure となる。constant table / spelling は compile-only で runtime code へ出力しない。`^=operand` は owner-local numeric label を定義し、`#=operand` は同じ owner 内へ無条件 jump する。operand は decimal または `.NAME` 形式の定義済み named integer で、解決後の範囲は `0..32767`。label / fixup の重複、未解決、容量超過は backend compile failure となり runtime を開始しない。

```sh
cargo test -p vtlxx --test sim65_frontend -- --ignored
```

`?="..."` は ASCII bytes を改行なしで出力する。文字列は生成コード arena 内に最大255 byteずつ格納し、実行時に固定長 helper で出力する。空文字列は出力せず、escape と複数行文字列には対応しない。arena の容量不足や不正な文字列では runtime を開始しない。

## 手動実行 CLI（#365）

`target/sim65_cli.s` は CLI 専用entryで、`_main` から `fe_compile_run` を一度呼ぶ。compile成功時は既にfrontendが実行したruntimeの後で `halt(0)` し、compile error時は `fe_compile_status` を `$80` と OR したtarget statusで終了する。test probe用entryとは分離し、追加serial readやsentinel出力を行わない。

crate root の `vtlxx6502` host binaryが source/runtime inputをraw bytesで読み、source lengthをu16 LEで検証・framingする。`ca65` / `ld65` は argvで起動し、対象source、既存runtime/frontend/compiler、CLI entry、`sim65_adapter.s` を組み立てて `sim6502.lib` と linkする。生成programは4,000,000 cycle limit付きでsim65へ渡す。一時directoryは各実行で新規作成し、終了時に削除する。

## 現行の6502資源構成

現在のsim65用コンパイラは、生成コードだけを保持する **24,576 B（24 KiB）arena** を使用する。コードと手続テンプレートsourceを共有する旧方式は[#528](https://github.com/kkismd/vtlxx/issues/528)と[#529 / PR #534](https://github.com/kkismd/vtlxx/pull/534)により廃止された。現在のpatch追跡・完成済み位置のbitsetは各3,072 Bで、ownerの完成・公開とコード境界の検査を維持している。

最新の正式sim65 CLIの静的リンク計測（[#524 / PR #537](https://github.com/kkismd/vtlxx/pull/537)、`ca65`/`ld65`のmap）:

| 領域 | 計測値 |
| --- | ---: |
| ZEROPAGE | 132 B |
| BSS | 34,903 B |
| CODE | 6,145 B |
| RODATA | 49 B |

共有ZPの`zp_phase`は100 B予約で、compile viewは80 B、runtime viewは100 Bを使用する。driver entry、compiler/frontend status、cc65共通ZPは別領域である。この表は**PR #537時点の静的配置**であって、portable ABIや全入力の上界ではない。動的CPU hardware stackの高水位、IRQ/NMIを含む実機安全性、compile/runtime別cyclesを実測したものでもない。実際の配置は最新のlink mapとコードを正本とする。

## 過去の計測記録: Micro TREK の sim65 E2E と資源（#513）

この節のアドレス、領域、cycles、stackに関する記述は **#513で計測した当時の構成を保存する歴史的記録** である。現在のarenaはコード専用で、汎用work stackはすでに削除されている。以下のテストや測定スクリプトを最新`main`で実行しても、当時の配置やcyclesが再現されるとは限らない。現行の値は上節と最新の実装・計測PRを参照する。

基準 main `07350a863c450529d098fbd5f2bf896deb9d00cf`、Ubuntu cc65 package 2.19-1 の ca65/ld65/sim65 V2.18 で測定。正本 `examples/micro_trek.vtl` を変更せず固定 seed 0 で使う。以下は repository root から実行する。

```sh
cargo test -p vtlxx --test micro_trek_e2e
cargo test -p vtlxx --test sim65_micro_trek_e2e -- --ignored
python3 crates/vtlxx/tests/fixtures/micro_trek/measure_sim65.py
```

E2E は Rust 側の 5 テストに対応する 7 入力系列を raw stdout/stderr と正常終了で比較し、別に Unix の対話モードで `COMMAND :` を入力前に受信してから X を送る。一括実行は 4,000,000,000 cycles、各子プロセスは 120 秒を上限とする。Rust 側の独立した出力期待値も維持する。CLI は内部 storage や PRNG の最終値を公開しないため、その一致は主張しない。Rust の `micro_trek_lifecycle::replay_initialization_continues_the_random_stream` は storage[192] の継続を、`micro_trek_turn_end::win_precedes_dock_and_attack_even_when_energy_and_days_are_exhausted` は勝利優先を直接検査する。実 CLI の勝利は未到達。6502 の `sim65_runtime::storage_all_cells_aliases_and_reinitialization` は 256 Cell と ring の非 alias を検査する。EOF と敗北・RETRY は上記 E2E で観測する。

正式 module の `ld65 -m/-Ln` 静的配置は ZP `$0000..$008c` (141 B)、CODE `$0229..$2069`、RODATA `$206a..$209a`、DATA `$209b..$20e1`、BSS `$20e2..$a78a`。arena `$2b9e..$8b9d` は 24,576 B、patch/completed bitset は各 3,072 B、定数表は 128×19 = 2,432 B、storage は 256×2 = 512 B。CPU stack は `$0100..$01ff`、cc65 software stack は `$f7f0..$ffef`、BSS 末尾から後者までの空きは 20,581 B。終端 wrap と静的重複はない。compile/runtime ZP overlay は 111 B、entry と両 status は独立した 4 B、cc65 共通 ZP は 26 B。アドレスは ABI ではない。

上記 script は production と同じ module の link map/labels を生成し、sim65 `-c` で総 cycles を別実行で取得する。EOF 30,211,835、表示後 X 31,352,775、2 回敗北・RETRY 47,418,752 cycles。compile/runtime 相別は未計測。script は別途 frontend の一時コピーに成功した `fe_build_driver` の直後、runtime 開始前の `cc_cursor` を出力して停止する診断を挿入する。診断 binary の arena は `$2bad`、cursor は `$687f`、生成量 15,570 B、残り 9,006 B。ゲームは P0a を使わない。この値は診断 binary の測定であり production CLI の直接観測ではない。

動的 CPU S、value/work stack、cc65 software SP の最大使用量は本測定では未計測で、全経路の動的安全性は未証明。静的非重複はその証明ではない。compile guard は S < 25 を検出するが runtime や IRQ/NMI に適用しない。この測定時点の共有 arena と template 専用テストは #528 によって廃止された。現行の code-only arena と owner の検証は `sim65_backend` を参照。EC09 #463 の完了判定には、未測定の stack と相別 cycles、および実 CLI 勝利未到達を含めて評価が必要。

