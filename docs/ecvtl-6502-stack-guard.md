# EC07 コンパイル時 CPU stack guard の監査（#424）

## 対象・前提

Tracker #271、採用 ADR #417、設計 #418、実装 #424 に対応する。
変更前は `f4af325b430cc27b5d938b578d846b6c08d49e39`（PR #423 merge）。
cc65 tools は V2.18（Ubuntu 2.19-1）、標準 `-t sim6502`、CLI と同じ14 sourceを
同じ数値順でリンクした。変更前のリンク検算を実施してから guard を追加し、変更後も再検算した。
以下は IRQ/NMI のない sim6502 の compile phase 限定。runtime の上界評価は #381 に残す。

## 計算法と被覆

リンク済み命令列を条件分岐の両側へ追跡し、JSR 2 B、PHA/PHP 1 B、
PLA/PLP -1 B、tail JMP 0 B として同時に生存する追加量を算定した。
通常 subroutine の RTS は局所 push が釣り合うことを確認して呼出元へ戻す。
次の4入口への呼出しでは、その入口までの JSR 2 B を含めて区切る。
その呼出しが正常に戻った後の caller の継続も、同じ caller の局所深度で検査する。
callee 内部はその入口の guard から別途保護される。既に保持している frame / PHA は
観測 S に含まれるため再加算しない。各区間の通常終了は釣り合うので、復帰後に
caller の継続へ移っても、新しい guard 起点より深くなる分は caller 側の上界以内である。

- statement → required_block → statement、および operand → value → operand が
  再帰による増加を監視する。Write、IF、While の body も required_block を通る。
- P0a 保存の template_body_statement は再帰せず、展開は statement を通る。
  template_reader_next 内の局所 `JSR @advance` もリンク命令で数える（追加2 B）。
- ループ内の JSR は正常に帰還し、PHA は対応する PLA で回収する。
  guard を除いた呼出しに stack が増え続ける循環はない。
- 通常の syntax/source/backend error は既存の RTS、operand の PLA、
  fe_compile_run の temporary-source abort を通る。error 経路も上界に含める。
- マクロ展開後の longbranch は条件分岐と JMP であり push を追加しない。
  guard 自身および guard NG → halt → exit も追加 push は0 B。

## 4入口の上界と代表経路

変更前後で次の局所上界は同じだった。#418 の18/16/22/19 Bは関数中の
PHAを過大に合算した候補だったが、今回は命令上の生存期間を分けて計数した。
候補 K=24 B を引き下げず、その範囲内であることを確認する。

| guard | 追加上界 B | 最大となる経路の一例 |
| --- | ---: | --- |
| statement | 15 | template_body_statement → template_save_equals → peek → fe_next → serial_in → _getchar → _fgetc → pushax の PHA |
| required_block | 13 | skip_separators → peek → fe_next → serial_in → _getchar → _fgetc → pushax の PHA |
| operand | 15 | operator の PHA×2 → peek → fe_next → serial_in → _getchar → _fgetc → pushax の PHA |
| value | 13 | parse_literal の PHA×2 → peek → fe_next → serial_in → _getchar → _fgetc → pushax の PHA |
| guard NG | 0 | status=4、A=$84、JMP halt、JMP $FFF9 |

_getchar → _fgetc は tail JMP。_fgetc 内の追加上界3 Bは、pushax / push0ax、
内部ポインタ計算、swap のいずれかの JSR 2 Bと局所 PHA 1 B。
これらの PHA は帰還前に回収され、別々の呼出し分を合算しない。
入力の host entry `$FFF6` へは JSR 2 Bを算入するが、sim65 host内部に
guest CPU stack の追加 push はない。EOF/入力失敗分岐と末尾の software-stack
調整も確認した。compile phase は _putchar に到達しない。

### その他の末端経路

表の値は各 helper の入口からの追加量（その helper を呼ぶ JSR は caller 側に加算）。

| helper | 追加上界 B |
| --- | ---: |
| peek | 9 |
| skip_separators / require_form_boundary / template_save_equals / parse_literal | 11 |
| operator / template_body_statement | 13 |
| template_reader_next（局所 @advance 込み） | 2 |
| cc_push_const / cc_call | 2 |
| cc_jz_placeholder / cc_jump_placeholder / cc_patch_here | 4 |
| cc_define_label / cc_label_jump | 6 |
| cc_publish / cc_resolve | 4 / 2 |
| cc_source_begin / cc_source_append / cc_source_finish / cc_source_abort | 2 / 2 / 0 / 0 |
| cc_complete_owner | 4 |

arena の reserve/patch、owner の fixup、binding、work stack、template registry の
失敗分岐も追跡対象。上の backend 最大6 Bに caller 側の深度を足しても4入口の表を超えない。

## guard 以外の区間

crt0 は `$0200` から CLD、LDX #$FF、TXS を実行する。
zerobss、空の initlib を終え、callmain は _main へ tail JMP する。
_main で S=$FD、`JSR fe_compile_run` 後は S=$FB（251）。
startup の pushax は software stack の引数を構築し、hardware stack の
一時 PHA は回収済み。startup 自体の追加最大は5 B。

| 区間 | fe_compile_run の S=251 からの追加上界 B | 根拠 |
| --- | ---: | --- |
| framing 初期化 | 9 | JSR fe_init 2 + fe_init の serial/library 7 |
| cc_init / template_reset / fe_start_chunk | 6以下 | それぞれ caller JSR 2 + helper上界2 / 0 / 4 |
| top-level separator | 13 | JSR skip_separators 2 + helper 11 |
| finish / driver build | 8 | JSR 2 + helper 6。owner → return emission → reserve を含む |
| 通常失敗 cleanup | 2 | cc_source_is_active / cc_source_abort は追加 push なし |
| rt_init / generated entry への移行 | 2 | rt_init は leaf。帰還後の手製 PHA×2と同時に積まない |

したがって非監視 compile 区間は startup S=$FF から17 B以下、S>=238。
生成 entry へ移った後の runtime 深度は本監査に混ぜない。

## 比較・レジスタ・停止の契約

`TSX; CPX #25; BCS success; JMP compile_stack_exhausted` をinlineに置く。
A/Y は変更せず、X/N/Z/C は各使用点で次に使う前に上書きされる。
statement は look_byte、required_block は skip_separators、value と operand の
operators 側は peek から読み直す。operand の入力 Z だけは最初の BNE が使うため、
その直後の value 側と @operators 側の両方に guard を置く。
operand に保留されている演算子の A/X は既存 PHA×2 に保存済みで、value の帰還後に復元する。

成功 S>=25 から K=24 B消費しても S>=1。
次入口の呼出し、局所退避、ライブラリ消費を K に含め、NG経路は追加 push なしで停止する。
NGでのみ source abort / unwind を省略する。生成runtimeの開始前なので stdout は空。
通常の失敗復帰、source framing、parser の深さ上限は変更しない。

## リンクの証拠とサイズ

`-g` / map / label はアドレス確認用で、生成命令を増やさない。

| symbol / segment | 変更前 | guard追加後 |
| --- | --- | --- |
| statement | $03B6 | $03BF |
| required_block | $0DE7 | $0DF8 |
| operand | $0E59 | $0E72 |
| value | $0EB4 | $0EDD |
| _getchar / _fgetc | $1E18 / $1D35 | $1E49 / $1D66 |
| halt | $1CF8 | $1D29 |
| exit | $FFF9 | $FFF9 |
| CODE | $0229..$1E95, 7277 B | $0229..$1EC6, 7326 B |
| RODATA | 49 B | 49 B |
| ZEROPAGE | 239 B | 239 B |
| BSS | $1F0E..$2E5F, 3922 B | $1F3F..$2E90, 3922 B |
| PRG（12 B header込み） | 7450 B | 7499 B |

ROM相当ロード量は **49 B増加**（8 B ×5箇所 + fatal 9 B）。RAM予約量の増加は0 B。
BSS の配置アドレスはコード増分だけ後方へ動く。exit は C runtime の `_exit` ではなく
sim65 の直接終了入口。`halt` のリンク命令は `4C F9 FF` であり、終了時flushや隠れたpushはない。

再現は `src/bin/ecvtl6502.rs` の14 source配列の順に、各sourceを
`ca65 -t sim6502 -g SOURCE -o N.o -l N.lst`、続いて
`ld65 -t sim6502 -o main.prg -m main.map -Ln main.lbl 0.o … 13.o sim6502.lib`
でリンクする。PRGの先頭12 Bを除き `da65 --start-addr 512 main.bin` で逆アセンブルできる。
前版の同じsourceと現版の比較で上記アドレスと呼出し経路を検算する。

## 動作検証

- production CLI で PR #415 の201 B fixture（16 block +16 group）を再現。
  Rust reference と raw stdout `0` / exit 0が一致。
- 一時的な assembly copy に push を行わない最小Sの観測を追加。
  同fixtureの全guardの最小 S は **83**。これはguard時点の観測であり、
  ライブラリ内部までの動的高水位を測った値ではない。
- 比較入力だけを24/25へ注入し、24はexit132/空stdout、25はexit0/stdout42。
  `TXS` を変更しない。通常operandとstack-root operandの両方を実行する。
- 深いstatement、required_block、operand、value、P0a展開に200の厳しい閾値を
  test-onlyで注入。各fixtureが注入前には正常なことも確認。
  全例exit132/空stdout。rt_initへの到達は専用exit99に置き換えて非開始を検出する。
  `$0100..$0118` の番兵がfatal時に保持され、観測 S>0 を確認。
  実際の256 B近辺までstackを消費させる試験ではなく、停止動作と非wrapの検証であり、
  物理下端付近の予防性はリンク上界と24/25の比較試験を合わせて裏付ける。
- テスト観測用のRAM・分岐は一時ファイルのみに存在し、production ABI/CLIへ公開しない。
- sim65_frontend、sim65_cli、sim65_negative、sim65_examples のignored suiteで
  IF/While/P0a、6例（Queens両版92解）、通常syntax/transport/backend失敗を確認する。

## 文字列出力追加時の監査（#472）

`statement` の guard 後、文字列の処理へ tail JMP する。source の読取は既存の `peek` を直接呼ぶため追加は最大 11 B。backend の最深経路は `cc_literal_output_byte` → `cc_literal_output_finish` → `cc_patch_here` → `cc_patch` → `cc_untrack_patch` → `cc_patch_bit_address` で、同時に生存する JSR は5個、追加10 B。placeholder の追跡も同じ上界内であり、局所 PHA はない。従来の statement 上界15 Bと K=24 B を超えない。runtime helper は `serial_out` を JSR し、runtime stack の既存条件を維持する。

同じ sim6502 CLI の14 source をリンクした比較では、ZEROPAGE は239→241 B、BSS は3922→3929 B、CODE は7361→7633 B、RODATA は49 Bのまま。2 KiB arena は変更していない。

## 再監査条件

4入口の追加・迂回、JSR/tail JMPの変更、PHA/PHPや保存レジスタの追加、
helper・マクロ・通常失敗cleanup・cc65 library・adapter・startup・リンク設定を
変更するときは K と入口外の上界を再監査する。IRQ/NMIや常駐再利用を導入する場合は
#417の前提外であり、新たな設計判断が必要。128 B予算の例外はcompile限定で、
EC07全体やruntimeの256 B安全ゲートの完了をこの変更だけから宣言しない。

## 加減乗算の共用（#636）

現行 sim6502 のリンクでは `rt_add` / `rt_sub` / `rt_mul` はそれぞれ
`rt_need_two`、`cell_add` / `cell_sub` / `cell_mul` を順に JSR し、
`rt_commit_binary` へ tail JMP する。各共用 entry は leaf で、入口からの
追加 hardware stack は 0 B、呼出しの JSR を含めると 2 B。以前から存在した
`rt_need_two` の JSR と同時には積まれないため、各 wrapper の最大深度は
増えない。#520 の 69 B は通常手続26件時の条件付き値であり、現行の
64件には適用しない。#456 の `ecvtl-6502-long-write-resources.md` は、
sim6502・IRQ/NMIなし・cc65 V2.18 のリンクで、64段の呼出しを含む
136 B と入力 helper 9 B を合計した 145 B を保守的上界とした。
現行 `binding.s` の上限も64件で、生成呼出しは解決済み target への JSR、
chunk は逐次実行される。現行の `rt_read_char → serial_in → _getchar`
と `_fgetc` の経路は同じ 9 B（JSR 3個と library 内最大3 B）であり、
今回の sim6502・同一 V2.18 library のリンクでも確認した。追加された
非同期入力・乱数・静的 byte 読取・文字列出力も、現行 sim65 adapter
では入力経路を超える同時 stack 消費を持たない。145 B はこの対象と
リンク条件に限る先行監査の値であり、別 adapter・library・割込み条件や
将来の呼出し構造に対する無条件の上界ではない。
compile 側からはまだ呼ばれないため guard 間・入口外経路も変わらず、
S>=25 と失敗時の status 4 / exit 132 / 空 stdout / 即時停止を維持する。
#637 で呼出しを追加する際にはその経路を再監査する。

共用 entry は D=0、`rt_lhs` / `rt_rhs` の Cell を入力し `rt_result` に
mod 65536 の結果を返す。入力と Y を保持し、A と N/Z/C/V を変更する。
乗算はさらに X と BSS の `work_q` / `work_d` を変更する。各呼出しで
必要な作業領域を初期化し、runtime 初期化には依存しない。
現行 link map では `zp_phase=$06`、compile の +0..79 は $06..$55、
算術用 +85..90 は $5B..$60。永続 entry/status と cc65 の ZP は別予約で、
`emitter.s` が埋め込む `rt_lhs=$5B` と各 alias の相対位置も不変。
sim6502 CLI の同一 source 順・同一 toolchain での map 比較は CODE
7480→7492 B、RODATA 49→49 B、BSS 35317→35317 B、ZEROPAGE
132→132 B（CODE +12 B、他は増減なし）。

## compile session の一括中止（#650）

本節は上記の履歴中にある旧 temporary-source cleanup や旧 P0a 経路を
現行経路として扱わない。`vtlxx6502` の現行 source 配列を順に ca65 V2.18
（Ubuntu 2.19-1）で assemble し、sim6502 設定・同梱 library で link して
命令と呼出しを確認した。sim65 adapter と mos6502 adapter の両方で
共通 session の中止経路を確認した。

`fe_compile_run` は `TSX; STX session_entry_sp` を最初に実行する。保存時は
caller の JSR の復帰アドレスが既に stack 上にあり、追加の JSR frame はない。
backend の確定 fatal は詳細を A に入れて `JMP session_backend_error` し、
同ルーチンは status を記録して `LDX session_entry_sp; TXS; SEC; RTS` へ
続く。記録から RTS まで追加 push は 0 B。中間 frame や PHA があっても
保存 S まで戻して caller へ復帰する。frontend の確定 fatal も同じ
`session_abort` に tail JMP する。定数値の暫定 error は通常 RTS で分類元に
戻り、分類後に中止する。`fe_init` / `fe_next` は serial_in の復帰後に
失敗を判定するので cc65 software stack を使用中に脱出しない。
stack guard NG は従来どおり push なしで halt へ tail JMP し、mos6502
adapter の Fatal も adapter から halt へ tail JMP する。

4入口間は JSR 2 B、PHA/PHP 1 B、tail JMP 0 B で数え、次の guard 入口への
JSR も含める。既存の入力側最大は `peek → fe_next → serial_in → _getchar
→ _fgetc` で peek 入口から 9 B。`parse_literal` の 2 PHA は peek の JSR
より前に両方回収される。guard 後の主な最大経路は、statement から label
解決と literal 読取を経る 15 B、required_block から separator 読取の 13 B、
operand から operator 読取の 15 B、value から literal 読取の 13 B である。
文字列・静的 payload の patch 側は nested JSR と局所 PHA の生存期間を含め
K=24 B 内であり、backend fatal への tail JMP は追加しない。mos6502 の入力
adapter は host `$FFF0` を JSR し、sim65 の cc65 入力経路より深くない。
これらにより既存 K=24、残り 1 B のための S>=25 を維持する。

入口外では fe_compile_run の S=$FB（251）を基準に、framing 初期化は 9 B、
通常の top-level separator は 13 B、finish / driver は 8 B 以下。top-level
定数定義の `fe_definition → constant_expression → constant_expression_value
→ parse_literal → peek` は、それぞれの JSR 10 B と peek 内 9 B の計 19 B。
静的定義の source 読取・backend patch 経路もこの値を超えない。したがって
入口外 compile 区間は S>=232 であり、保存 S 自体も wrap しない。startup は
従来どおり S=$FF から fe_compile_run まで 4 B を保持する。

現行の `cargo test -p vtlxx --test sim65_frontend compile_stack_guard --
--ignored --nocapture` では4件が成功し、比較入力 S=24 は exit 132 / 空
stdout、S=25 は成功した。201-byte fixture の guard 観測最小 S は83、16段の
静的読取は181。深い各入口の試験は runtime 非開始と下端番兵を確認した。
観測 S は guard 時点の値であり、library 内の動的高水位ではない。これらは
sim6502 とこの toolchain、割込みなしの条件に限り、実機の IRQ/NMI や別の
library の安全性を主張しない。

## 完成 owner の公開（#651）

sim6502 の実リンクで完成・公開経路を再確認した。`cc_complete_owner` は
終端生成と freeze を順に呼び、従来の bitmap 更新 helper を呼ばない。
`cc_publish` は名前検索後に `cc_completed_entry` を JSR し、返された2 byteを
PHA で保持して登録後に回収し、件数更新後 `cc_consume_completed` を JSR
する。取得・消費は leaf であり、保持中に別の JSR はない。公開内部の最大
追加量は JSR または2 PHA の2 Bで、従来の完成判定から bit address helper
への二重 JSR より深くならない。失敗は push を増やさず session 中止へ
飛ぶ。入口外の chunk / driver の完成も従来より深くならず、先行監査の
K=24、S>=25 と入口外 S>=232 を維持する。実機の割込み等には適用しない。


## 未解決 patch 表（#652）

正式 sim6502 CLI の link と命令を確認した。`cc_track_patch` と `cc_patch`
は表の登録・探索・消費を自身で行う leaf で、検索 index は register に保持し、
新たな PHA/PHP や JSR はない。従来の `cc_patch` → `cc_untrack_patch` →
`cc_patch_bit_address` の JSR はなくなり、文字列・静的 payload や label
解決の最深経路を増やさない。容量超過や未登録の失敗は追加 push なしで
`session_backend_error` へ tail JMP する。完成・公開、source、adapter、
cc65 library は変更せず、先行監査の K=24、S>=25、入口外 S>=232 と
runtime の条件付き上界145 Bを維持する。これは sim6502、同一 library、
割込みなしの静的な評価であり、実機 IRQ/NMI の保証ではない。
