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

## 再監査条件

4入口の追加・迂回、JSR/tail JMPの変更、PHA/PHPや保存レジスタの追加、
helper・マクロ・通常失敗cleanup・cc65 library・adapter・startup・リンク設定を
変更するときは K と入口外の上界を再監査する。IRQ/NMIや常駐再利用を導入する場合は
#417の前提外であり、新たな設計判断が必要。128 B予算の例外はcompile限定で、
EC07全体やruntimeの256 B安全ゲートの完了をこの変更だけから宣言しない。
