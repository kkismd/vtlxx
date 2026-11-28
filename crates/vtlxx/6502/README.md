# VTLXX 6502 ターゲット

このディレクトリは VTLXX の 6502 target と host 別 adapter を置く。`target/sim65_adapter.s` が byte と EOF を carry flag で区別する `serial_in`、readinessを提供できない場合に Fatal statusを返す `serial_try_in`、1 byte の `serial_out`、8-bit status の `halt` を提供する。`serial_in` は A に任意の byte（`$ff` を含む）を返し carry clear、EOF は carry setで返す。`serial_try_in` はsim65 stdinを読まず X=3 (Fatal/unsupported) を返す。A register を受け渡しに使う呼び出し規約と `sim6502.lib` への接続はこの target 内だけの取り決めであり、portable VTLXX runtime ABI ではない。

`target/sim65_scripted_adapter.s` はnative試験専用で、通常adapterと同時にはlinkしない。fixtureが所有するタグ付きイベント列を `serial_in` と `serial_try_in` で共有し、TryのNotReady/Byte/EOF/FatalとWait入力の順序を再現する。2-byte eventの範囲外・不完全event・tag 0..3以外はfixture破損としてsim65 status 64で終了する。これは入力FatalのX=3や同期入力失敗status 6とは別である。明示EOFのみstickyになり、明示FatalはTryでX=3、Waitでstatus 6となる。この経路はCLI optionや標準sim65 stdinのreadinessを提供しない。

host 側の `tests/sim65_6502.rs` が `ca65 -t sim6502`、`ld65 -t sim6502`、`sim65 -x` を起動する。生成物は OS の一時ディレクトリに作られ、テスト終了時に削除される。

```sh
cargo test -p vtlxx --test sim65_6502 -- --ignored
```

通常の `cargo test --workspace` は cc65 を必要としない。

`entropy16` は target-private adapter symbol で、成功時は carry clear / A=low / X=high、取得不可は carry setを返す。`vtlxx6502` はOS由来raw16値または `--seed` のraw16値を選択したadapterのprivate assemble定数として渡す。OS取得に失敗した場合は取得不可adapterをlinkし、`!`未使用sourceの実行を妨げない。実機adapterは同じsymbol/ABIを実装して差し替える。

## Runtime helper（#274）

`runtime/state.s`、`runtime/arithmetic.s`、`runtime/primitives.s` は現在の6502 target の実行状態と primitive を実装する。native emitter は `rt_init` を一度呼び、その後に以下の entry point を JSR で呼ぶ。いずれも target 内の link contract であり、portable VTLXX API ではない。

sim65 では、全 source の compile と driver 完成の後に `rt_init` へ移るため、compile 専用 Zero Page scratch と runtime 状態を `target/zp_layout.s` の同じ **100 B**（`zp_phase`）に時間的に重ねて配置する。compile 時に生存する領域は offset `0..79`、runtime view は `0..99` を使用する。加減乗算の共有 scratch `+85..90` は各算術呼出しの間だけ両フェーズで使用し、compile 時の `0..79` と重複しない。driver entry、frontend/backend の結果 status、および両フェーズで使う cc65 library の領域は別予約であり、この100 Bに含めない。compile failure は runtime 開始前に戻る。runtime から新たな source compile へ戻る経路はない。

| entry | 入力 | value stack effect |
| --- | --- | --- |
| `rt_push` | A = Cell low byte、X = high byte | `( -- value )` |
| `rt_load_reg` / `rt_store_reg` | X = fixed register index 0..25 | `( -- value )` / `( value -- )` |
| `rt_load_storage` / `rt_store_storage` | stack 上の address | `( address -- value )` / `( address value -- )` |
| `rt_add`, `rt_sub`, `rt_mul`, `rt_div`, `rt_rem` | stack 上の operands | `( lhs rhs -- result )` |
| `rt_eq`, `rt_ne`, `rt_lt`, `rt_le`, `rt_gt`, `rt_ge` | stack 上の operands | `( lhs rhs -- 0\|1 )` |
| `rt_print_number`, `rt_print_char` | stack 上の value | `( value -- )` |
| `rt_read_char` | — | `( -- char_or_eof )` |
| `rt_try_read_char` | — | `( -- char_or_status )` |
| `rt_read_entropy` | `entropy16` on first draw | `( -- random_value )` |

helper は A/X/Y と processor flags を保存しない。`rt_push` の引数と register helper の X 以外に CPU register の入力契約はない。Cell は little-endian の signed 16-bit、stack は 16 Cell、A-Z は 26 Cell、storage は下位8 bitで選ぶ256 Cellの ring。A-Z と stack は zero page、storage は通常 RAM に置く。`rt_init` は register、storage、stack depth を初期化する。

fatal status は underflow = 1、overflow = 2、division by zero = 3、remainder by zero = 4、entropy unavailable = 5、input failure = 6、static byte bounds = 7 とし、いずれも #273 の `halt` へ移る。これは sim65 fixture が success と runtime error を区別するための target 内の値である。error 検出前に stack operand を消費しない。

sim65 の失敗時状態確認だけに `RT_TEST_PROBE` を指定すると、HALT 直前に fixture の `rt_test_probe` を呼ぶ。通常の target build にはこの hook は入らない。

`rt_read_char` は `$` PrimaryRead の固定runtime targetである。stack空き容量をtransport readより先に検査し、NULを読み捨てて1..255またはEOF=-1を1 Cellとしてpushする。EOFはruntime session内で記憶し、後続readも-1を返す。sim65 adapterの `_getchar` はbyteとEOFだけを区別し、recoverable I/O failureを個別に表せないため、このtargetではそのerror分類を検証できない。

`rt_try_read_char` はバッククォート PrimaryRead の固定runtime targetである。stack容量を先に確認してから `serial_try_in` を1回だけ呼び、ByteはNULを含めてその値、NotReadyは0、EOFはstickyな-1として1 Cell pushする。Fatalはstatus 6で停止し、CellやEOF状態を変更しない。通常sim65 adapterはreadinessを提供しないため、入力を消費せずstatus 6を返す。NotReady・NUL・共有EOFのnative検証には通常adapterと別リンクするscripted adapterを使う。

#336変更前後の`ca65` object比較ではfrontend CODE +17 B、runtime primitives CODE +53 B、runtime state CODE +5 B。合計production CODE +75 Bで、Zero Page、BSS、RODATAは増加しない。scripted adapterの呼出回数probeはCODE +3 B、DATA +1 B。`rt_try_read_char`は常駐Zero Page scratchを使わず、provider callのJSR段数は既存`rt_read_char`と同じ。追加PHAやhardware-stack guardはない。

`rt_read_entropy` は `!` PrimaryRead の固定runtime targetである。16 Cell容量を先に検査し、PRNG stateが未初期化の場合だけ `entropy16` からseedを一度取得する。seed 0は`$ACE1`へ正規化し、各評価で16bit xorshift `<<7, >>9, <<8` を順に適用して、更新後stateの上位15bitを1 Cell pushする。stateのゼロ値を未初期化sentinelとして使い、`rt_init`でクリアする。取得失敗はstatus 5でHALTし、sentinel、stack、他の言語stateを変更しない。adapterを差し替えない共通runtime fixtureは引き続き取得不可を返す。

変更前の`main`と`ca65` object segment sizeを比較すると、frontend CODE +17 B、runtime primitives CODE +298 B、runtime state CODE +11 B、sim65 adapter CODE +2 B、合計CODE +328 B。runtime state BSS +2 B（16bit state）、primitives BSS +2 B（16bit scratch）、合計BSS +4 B。ZPとRODATAは増加せず、100 B phase shareも変更しない。初回adapter呼出しのJSR return addressは2 Bで、既存`$` readと同じruntime呼出し段数。追加のPHAやhardware-stack guard経路はない。CPU stack高水位の実測・実機上限保証は行っていない。サイズは変更前後の各module object segmentの差であり、リンク後の総配置比較ではない。

```sh
cargo test -p vtlxx --test sim65_runtime -- --ignored
```

## 静的byte列のbackend契約（#620）

`cc_static_begin` は実行中のownerに追跡付きskipを出力し、`cc_static_byte` は検証済みraw byteをAで順次受け取る。`cc_static_finish` が成功した場合のみ `cc_static_start` と `cc_static_length` の各2 byteを完成済みの絶対位置と長さとして保存できる。失敗時はcompile-run全体を破棄する。frontendはtop-levelでのみ呼び、名前管理と式の入れ子中の値保持を担当する。読出しのemit時は保存済みの組を同じ領域へ設定して `cc_static_read` を呼ぶ。生成コードは `rt_read_static` に位置と長さを渡し、stack topのsigned indexを範囲検査してunsigned byteへ置換する。未解決skipはowner完成を妨げ、patchは追跡中のoperandに限られる。

正式sim65 CLIの同一moduleを変更前後でリンクした結果、CODEは6711→6992 B、BSSは34907→34915 B、ZEROPAGEは132 Bで不変。arenaは24576 B、patch/completed bitsetは各3072 Bで不変。compile時の既存patch経路は追加JSRの深さが従来のuntrack呼出しと同じであり、既存guardを変更しない。新しいbackend入口は未接続で、frontend接続時に呼出し元を含むguard監査が必要。runtimeのreadはJSRに加え一時PHAを1 B使用する。IRQ/NMIを含む実機の動的高水位は未測定。

## Native compiler backend（#275）

`compiler/` は source syntax を扱わない 6502 固有 backend である。`cc_init` は 1 回の compile-run に先立って 24,576 B のコード・静的byte列共有 arena、binding、owner を初期化する。generated code は arena の RAM に残り、owner 完了後の entry address は u16 の絶対 address として利用できる。backend の確定した失敗は `cc_status` に原因を記録し、共通 session 中止処理で caller へ戻る。runtime は開始しない。失敗した session の内部状態は再利用しない。

`compiler/arena.s` は生成コードと静的byte列の24,576 B arenaと、同時に未解決のoperand addressを追跡する49件の表（98 B）を保持する。手続テンプレートのsource保存領域や仮想readerは廃止済みであり、通常のsource frameは`frontend/source.s`が読み取る。コードの予約・patch・owner完成と、完成済みbindingのみ公開する順序は維持する。

target 内の呼び出し規約は次の通り。u16 の主引数・戻り値は A=low、X=high。`cc_patch` は A/X に operand low-byte address、`cc_arg` に書き込む u16 を取る。`cc_is_bound`、`cc_publish`、`cc_resolve` は `cc_name_len` と `cc_name` に入れた ASCII 小文字1〜8文字の通常 Write 名を取る。`cc_is_bound` は bound なら carry set、unbound なら carry clear を返し、有効な問い合わせでは `cc_status` と binding table を変更しない。`cc_publish` は直前に完成した未公開 owner の entry を取得し、公開成功時に権限を消費し、`cc_resolve` は完全名から A/X に target を返す。raw arena pointer、table layout、scratch は frontend 向け contract に含めない。

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

現行の直接テストは [`tests/sim65_long_write_names.rs`](../tests/sim65_long_write_names.rs)（64件成功、65件目失敗、65 chunk順序）と [`tests/long_write_names.rs`](../tests/long_write_names.rs)（Rustの65件以上、完全名照合、失敗時binding）です。長名導入時のROM/RAM/stack比較は [当時の監査記録](../../../../docs/ecvtl-6502-long-write-resources.md) にあり、その数値は現在の配置ではありません。[PR #537](https://github.com/kkismd/vtlxx/pull/537) のCLIのZP/BSS/CODE/RODATA測定も当時の構成による歴史的な値です。いずれも全経路の動的stack高水位の実測を意味しません。

コンパイル時のCPU hardware stackは、statement / required_block / operand / valueで残量を検査する。Sが25未満の場合に限り、通常の復帰を行わず、`fe_compile_status=4`、exit132で即時HALTする。stdoutは空で生成runtimeは開始しない。通常のsyntax/source/backend失敗は従来どおり呼出元へ返す。閾値・linked library・入口外経路の根拠と再監査条件は[stack guard監査](../../../../docs/ecvtl-6502-stack-guard.md)を参照。

現在の対象は signed i16 十進 literal、raw16 の16進 literal、A-Z register、`@(address)` read / `@=address,value` write、算術・比較、`?` / `$` output、grouping、左結合の式、左から右への comma operand、RHS 先頭だけの `~`、`&=p [ ... ]` Write definition と lowercase Write call、top-level named integer 定義と RHS / numeric label operand 参照、anonymous block `[ ... ]`、one/two-arm IF `%=`、native While `*=condition` である。definition bodyは独立ownerへcompileされ、block内の label / control はそのownerを使う。ordinary blockは surrounding owner へ inline compileされ、numeric label namespaceを共有する。IF の else は次 source form が `[` のときだけ読み、block 深さは16までである。While は IF と同じ通常の bounded RHS 条件式の後に required body block を1つ取り、条件コードをループ先頭へ生成する。backend の JZ placeholder、direct jump、patch を使って loop を生成する。IF の forward patch と While の exit patch / loop target は、block の入れ子深さに対応する frontend-private な各16-entry table に保持する。式の入れ子上限は独立している。条件式の結果は runtime で各反復1 Cell消費される。named integer はトップレベルで `.NAME := value` と定義し、`.NAME` と参照する。名前本体は `[A-Z][A-Z0-9_]+` の2〜16 byteで、接頭の `.` は名前の長さに含めず表にも保存しない。右辺は signed i16 decimal literal、16進 literal または定義済み整数名を `+` / `-` / `*` でつないだ式で、左結合・優先順位なし、各演算は16 bitで wrap する（`2+3*4` は20）。十進リテラルは `-32768..32767` で、負号を付けられるのは十進リテラルだけである。例えば `.WIDTH:=8 .HEIGHT:=8 .SIZE:=.WIDTH*.HEIGHT .BASE:=.SIZE+7` と領域を配置できる。整数と静的byte列の合計は compile-run あたり128件までである。通常 statement と同じ空白・改行・コメント・source 終端で区切り、`.MAX:=10 .MIN:=-1 A=.MAX` のように同一行に置ける。`:=` の前後には ASCII space / HTAB を許すが、式内部の空白・改行は許さない。括弧、除余、比較、register、入力、乱数、stack、静的byte列名・添字は式に使えず、例えば `.AA:=1 +2`、`.BB:=(1+2)`、`.CC:=-.AA` は失敗する。通常式では grouping や除余も使える。静的byte列の数値要素には十進または16進のリテラルを使い、式や定数名は使えない。定義後のみ参照でき、重複定義と旧形式 `==NAME,value` や裸の複数文字定数参照は compile failure となる。constant table / spelling は compile-only で runtime code へ出力しない。`^=operand` は owner-local numeric label を定義し、`#=operand` は同じ owner 内へ無条件 jump する。operand は decimal、16進 literal または `.NAME` 形式の定義済み named integer で、解決後の範囲は `0..32767`。label / fixup の重複、未解決、容量超過は backend compile failure となり runtime を開始しない。

```sh
cargo test -p vtlxx --test sim65_frontend -- --ignored
```

採用 ADR [#679](https://github.com/kkismd/vtlxx/issues/679) の16進 literal は `$` と大文字 ASCII の16進数字1〜4桁であり、raw16 pattern を Cell に格納する。通常式の値位置では `$` 直後が16進数字なら literal、それ以外は従来の待機入力となる。`$=$41` は文字 `A` を出力する。名前付き整数・静的byte列・直接ラベルでは裸の `$` は無効であり、静的byte要素は `0..255`、直接ラベルは `0..32767` の範囲に限る。詳細と例は [crate README](../README.md#expression) を参照する。

`?="..."` は ASCII bytes を改行なしで出力する。文字列は生成コード arena 内に最大255 byteずつ格納し、実行時に固定長 helper で出力する。空文字列は出力せず、escape と複数行文字列には対応しない。arena の容量不足や不正な文字列では runtime を開始しない。

## 手動実行 CLI（#365）

`target/sim65_cli.s` は CLI 専用entryで、`_main` から `fe_compile_run` を一度呼ぶ。compile成功時は既にfrontendが実行したruntimeの後で `halt(0)` し、compile error時は `fe_compile_status` を `$80` と OR したtarget statusで終了する。test probe用entryとは分離し、追加serial readやsentinel出力を行わない。

crate root の `vtlxx6502` host binary は source を raw bytes で読み、長さを u16（最大65,535 bytes）で検証し、little-endian length header を付ける。`ca65` / `ld65` を使い mos6502 adapter を固定 raw image に link し、専用 host で実行する。一時 directory は終了時に削除する。正式なホスト動作保証は Linux であり、Linux 以外の実行要求は source 読込前に拒否する。独立した sim65 fixture は従来の設定と `sim6502.lib` を使う。

repository root からの例（製品実行には `ca65` と `ld65` が必要）:

```sh
printf 'A=$ ?=A' > /tmp/input.vtl
printf 'A' > /tmp/input.bin
cargo run -p vtlxx --bin vtlxx6502 -- /tmp/input.vtl --runtime-input /tmp/input.bin
```

このコマンドは `65` を出力する。非対話では `--runtime-input` が指定されなければ runtime 入力は空の EOF であり、stdin を待たない。`--seed` は符号付き16 bit の初期 seed を指定し、省略時は OS entropy を使う。固定 seed により `!` の値を再現できる。`--cycle-limit` は正の guest cycle 数で、非対話の既定は 4,000,000、対話時の既定は無制限である。上限到達時は status 126 で終了する。入力待ち時間は guest cycles に含まれない。

mos6502 は対話・非対話のどちらでも guest 出力を標準出力へ逐次送出する。host の診断は標準エラーへ出す。対話では runtime 入力を標準入力に接続する。`--interactive` と `--runtime-input` は併用できない。端末の既定 canonical mode では通常、改行まで入力が届かない。

```sh
cargo run -p vtlxx --bin vtlxx6502 -- /tmp/input.vtl --interactive
```

逐字入力が必要な場合は Linux の TTY 上で raw mode を明示する。次の例では Try がその時点の入力を一度だけ読み、その後 Wait が次の文字を待つ。

```sh
printf '?=` ?=$' > /tmp/try.vtl
cargo run -p vtlxx --bin vtlxx6502 -- /tmp/try.vtl --interactive --raw-tty
```

`--raw-tty` は mos6502 の `--interactive` と TTY stdin が必要で、echo を無効にする。正常終了・エラーおよび SIGINT / SIGTERM / SIGHUP / SIGQUIT では端末設定を復元する。SIGTSTP では停止前に復元し、SIGCONT 後に再適用する。捕捉できない SIGKILL / SIGSTOP ではこの復元を保証できない。

source と runtime は同じ入力 session を共有するが、source frame の読み取りと compile 完了後に runtime が開始する。Wait (`$`) は入力を blocking で待ち、NUL を読み捨て、byte または EOF の -1 を返す。Try（バッククォート）は待たずに最大1 byteを消費し、未到着と raw NUL を 0、EOF を -1 として返す。EOF は後続の Wait / Try でも維持され、Fatal は guest status 6 で停止する。通常 sim65 の Try は未対応で入力を消費せず status 6 となる。

mos6502 用 adapter と host の間では、private service `$FFEF` が Try、`$FFF0` が Wait、`$FFF1` が A の 1 byte 出力、`$FFF2` が A の status による非復帰の終了を担う。出力は byte をそのまま書き、flush が成功してから guest に戻る。これらは portable runtime ABI ではなく、通常の sim65 adapter とは別である。`$FFF3..$FFFF` は未提供 service として拒否する。

製品は内部 link した header のない raw image のみを使う。配置は zero page `$0000..$00FF`、hardware stack `$0100..$01FF`、load / entry `$0200`、service 予約 `$FFEF..$FFFF` である。host は実行ごとにゼロ初期化 RAM に payload を置き、PC=`$0200`、S=`$FF`、D=0、I=1、cycles=0、IRQ/NMI なしで開始する。BSS と zero page は file に含めず、C startup、software stack、`sim6502.lib` は使用しない。loader は payload の空と最大長（65,007 B）を検査するが、任意の短縮・破損、旧形式や CPU 違いを識別できない。外部 image の入力は受け付けない。source frame の長さ header はこの image と別である。

mos6502 host は標準の [`mos6502` 0.10.1](https://crates.io/crates/mos6502/0.10.1)（BSD-3-Clause）を使い、固定された依存元と version は [Cargo.lock](../../../Cargo.lock) に記録する。CPU core と cc65 は改造しない。

## 現行の6502資源構成

現在のsim65用コンパイラは、生成コードと不変の静的byte列を保持する **24,576 B（24 KiB）arena** を使用する。コードと手続テンプレートsourceを共有する旧方式は[#528](https://github.com/kkismd/vtlxx/issues/528)と[#529 / PR #534](https://github.com/kkismd/vtlxx/pull/534)により廃止された。patch追跡には最大49件のoperand address表（98 B）を使用し、成功したpatchは一度だけ消費する。完成履歴のbitsetは廃止し、直前に完成したownerからのみ公開することで、ownerの完成・公開とコード境界の検査を維持している。

以下は #652 当時の sim65 CLI の同一 module・cc65 V2.18 による歴史的な静的リンク計測であり、現行製品の配置ではない:

| 領域 | 前提 main | 変更後 |
| --- | ---: | ---: |
| ZEROPAGE | 134 B | 134 B |
| BSS | 32,265 B | 29,291 B |
| CODE | 6,979 B | 6,913 B |
| RODATA | 41 B | 33 B |

変更後のarenaは `$2993..$8992`、patch表は `$8993..$89f4`、BSSは `$1d92..$8ffc`。CPU stackは `$0100..$01ff`、cc65 software stackは `$f7f0..$ffef` で、BSS末尾からsoftware stackまで26,611 B空く。共有ZPの `zp_phase` は100 B予約で、entry、status、cc65共通ZPは別領域である。アドレスはABIではなく、実際の配置はlink mapを正本とする。IRQ/NMIを含む実機安全性はこの計測の対象外。

## 過去の計測記録: Micro TREK の sim65 E2E と資源（#513）

この節のアドレス、領域、cycles、stackに関する記述は **#513で計測した当時の構成を保存する歴史的記録** である。現在のarenaはコードと静的byte列を共有し、汎用work stackはすでに削除されている。以下のテストや測定スクリプトを最新`main`で実行しても、当時の配置やcyclesが再現されるとは限らない。現行の値は上節と最新の実装・計測PRを参照する。

基準 main `07350a863c450529d098fbd5f2bf896deb9d00cf`、Ubuntu cc65 package 2.19-1 の ca65/ld65/sim65 V2.18 で測定。正本 `examples/micro_trek.vtl` を変更せず固定 seed 0 で使う。以下は repository root から実行する。

```sh
cargo test -p vtlxx --test micro_trek_e2e
cargo test -p vtlxx --test sim65_micro_trek_e2e -- --ignored
python3 crates/vtlxx/tests/fixtures/micro_trek/measure_sim65.py
```

E2E は Rust 側の 5 テストに対応する 7 入力系列を raw stdout/stderr と正常終了で比較し、別に Linux の対話モードで `COMMAND :` を入力前に受信してから X を送る。一括実行は 4,000,000,000 cycles、各子プロセスは 120 秒を上限とする。Rust 側の独立した出力期待値も維持する。CLI は内部 storage や PRNG の最終値を公開しないため、その一致は主張しない。Rust の `micro_trek_lifecycle::replay_initialization_continues_the_random_stream` は storage[192] の継続を、`micro_trek_turn_end::win_precedes_dock_and_attack_even_when_energy_and_days_are_exhausted` は勝利優先を直接検査する。実 CLI の勝利は未到達。6502 の `sim65_runtime::storage_all_cells_aliases_and_reinitialization` は 256 Cell と ring の非 alias を検査する。EOF と敗北・RETRY は上記 E2E で観測する。

正式 module の `ld65 -m/-Ln` 静的配置は ZP `$0000..$008c` (141 B)、CODE `$0229..$2069`、RODATA `$206a..$209a`、DATA `$209b..$20e1`、BSS `$20e2..$a78a`。arena `$2b9e..$8b9d` は 24,576 B、patch/completed bitset は各 3,072 B、定数表は 128×19 = 2,432 B、storage は 256×2 = 512 B。CPU stack は `$0100..$01ff`、cc65 software stack は `$f7f0..$ffef`、BSS 末尾から後者までの空きは 20,581 B。終端 wrap と静的重複はない。compile/runtime ZP overlay は 111 B、entry と両 status は独立した 4 B、cc65 共通 ZP は 26 B。アドレスは ABI ではない。

測定当時の script は同じ module の link map/labels を生成し、sim65 `-c` で総 cycles を別実行で取得した。現在の script は製品 build と source list が異なり、`compiler/session.s` が含まれないため、再利用時には確認が必要である。EOF 30,211,835、表示後 X 31,352,775、2 回敗北・RETRY 47,418,752 cycles。compile/runtime 相別は未計測。script は別途 frontend の一時コピーに成功した `fe_build_driver` の直後、runtime 開始前の `cc_cursor` を出力して停止する診断を挿入する。診断 binary の arena は `$2bad`、cursor は `$687f`、生成量 15,570 B、残り 9,006 B。ゲームは P0a を使わない。この値は診断 binary の測定であり production CLI の直接観測ではない。

動的 CPU S、value/work stack、cc65 software SP の最大使用量は本測定では未計測で、全経路の動的安全性は未証明。静的非重複はその証明ではない。compile guard は S < 25 を検出するが runtime や IRQ/NMI に適用しない。この測定時点の共有 arena と template 専用テストは #528 によって廃止された。現行の arena と owner の検証は `sim65_backend` を参照。EC09 #463 の完了判定には、未測定の stack と相別 cycles、および実 CLI 勝利未到達を含めて評価が必要。

