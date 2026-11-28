# #707 opcode 選択方式の比較

`python3 docs/research/byte-direct-emitter/compare.py` は実装中の emitter
から選択部分と表だけを置換し、基準 opcode と mode 差分の方式と 7×3
直接表方式をそれぞれ ca65 で assemble、同一の仮シンボルと設定で ld65
link して segment サイズを表示する。仮シンボルはコードやデータを持たず、
実行には使用できない。owner 検査、引数検証、予約、出力は同じコードであり、
probe の費用を含まない emitter object 全体の比較である。製品 link の増分は
別途測定する。直接表の source と置換差分は compare.py 内に保存している。

ca65/ld65 V2.18 (Ubuntu 2.19-1) での結果（byte）:

| 方式 | CODE | RODATA | BSS | ZEROPAGE | CODE + RODATA |
| --- | ---: | ---: | ---: | ---: | ---: |
| 基準 opcode + 差分 | 804 | 10 | 16 | 0 | 814 |
| 直接表 | 805 | 24 | 16 | 0 | 829 |

基準方式は即値 opcode を基準に、immediate / zero page / absolute の順に
0 / -4 / +4 を加える。STORE の即値は両方式とも共通の検証で拒否する。
合法な全20組について直接表と等しい opcode を得る。CODE + RODATA が
15 byte 小さい基準方式を採用した。値には既存 emitter の共通部分が含まれる。
