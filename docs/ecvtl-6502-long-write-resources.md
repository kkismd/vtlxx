# 6502 長い通常手続名の資源と stack 監査（#456）

対象は sim6502、IRQ/NMI なし、ca65/ld65/sim65 V2.18（Ubuntu 2.19-1）。
`tests/fixtures/micro_trek/measure_sim65.py` を同じツールで変更前 main
`1826eb0` と変更後に実行した。総 cycle は compile と runtime の合計である。

| 項目 | 変更前 | 変更後 |
| --- | ---: | ---: |
| CODE | 7745 B | 7890 B |
| RODATA | 49 B | 49 B |
| ZEROPAGE | 141 B | 143 B |
| BSS | 34473 B | 35117 B |
| Micro TREK arena 使用 / 残 | 15570 / 9006 B | 15570 / 9006 B |
| EOF cycles | 30211835 | 30275815 |
| displays cycles | 31352775 | 31418147 |
| retry cycles | 47418752 | 47495133 |

変更後の BSS は `$2173..$AA9F`、arena は `$2C84` から 24576 B。
sim6502 のリンクで領域は重複せず、従来と同じ software stack 配置に収まる。
BSS 増分 644 B は名前表・chunk 表・名前 scratch 等を含む。
代表入力の arena と cycles は全入力の上界ではない。

## compile 時の hardware stack

既存監査 `ecvtl-6502-stack-guard.md` の4入口と tail HALT を維持する。
新しい `statement → scan_write_name_tail → peek` は入口から最大13 B
（JSR 2 + JSR 2 + peek 内9）、`take` は逐次実行で加算しない。
`cc_is_bound` / `cc_resolve` の `binding_find` は leaf、`cc_publish` は
`cc_is_completed → owner_bit_address` を呼ぶ。caller を含む新規の binding
経路は既存 statement 上界15 B以内であり、name copy は push しない。
required_block、operand、value、入口外の finish/driver の呼出しは変更していない。
guard の監視区間 K=24 B と S>=25 の条件を維持し、残り1 Bを下端に予約する。
guard 24/25 境界と深い入力の HALT・空 stdout・runtime 非開始は
`sim65_frontend -- --ignored` で検証した。compile 全入力の高水位は実測していない。

## runtime 時の hardware stack

名前検索は compile 時のみで `cc_call` は解決済み target の JSR を出力する。
完成済み定義のみ参照可能で再定義を拒否するため call graph は非循環、最大64段。
driver は最大91 chunk を逐次呼び出し、chunk 間の frame は重ならない。
runtime、adapter、cc65 V2.18 library と call emitter の経路は変更していない。
production の15 module を `ca65 -g -t sim6502`、`ld65 -t sim6502 -m -Ln` の順に
リンクし、12 B header を除いて `da65 -S 0x200` で確認した。`serial_in=$1F4D`
は `_getchar=$207D` を JSR し `_fgetc=$1F9A` へ tail JMP、`serial_out=$1F58`
は `_putchar=$20B6`、`_fputc=$2026` へ tail JMP、`halt=$1F5D` は `$FFF9` へ
tail JMP する。#426 の linked helper 経路を現行 source・同一 library と照合し、同時生存する
startup 2 + main 2 + entry return 2 + driver 2 + 手続64段×2 = 136 B に、
入力 helper の最大9 Bを加えると **145 B**。比較は142 B、出力は143 B、
storage 等は140 B以下である。これらは同時成立を許した保守的上界で、
初期 S=$FF から非wrap許容量255 Bに対して110 Bの余裕がある。
64段の実行テストは補助的な動作確認であり、上界の実測ではない。
異なる library、割込み、adapter、target にはこの数値を適用しない。
