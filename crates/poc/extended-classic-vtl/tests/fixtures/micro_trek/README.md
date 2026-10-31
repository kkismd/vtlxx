# マイクロTREK 銀河セルの局所操作

実装 issue [#383](https://github.com/kkismd/vtlxx/issues/383)、親 Tracker
[#321](https://github.com/kkismd/vtlxx/issues/321) / [#429](https://github.com/kkismd/vtlxx/issues/429)、
採用 ADR [#378](https://github.com/kkismd/vtlxx/issues/378) に対応する代表 source。
ゲーム本体への統合は各 caller の後続 issue が担当する。
名前付き手続き・テンプレート・include 機能は追加せず、既存の整数演算、storage、局所 block だけを使う。

内容は `base=100*k+10*b+s`（`k=0..5,b=0..1,s=0..7`）、
保存値は未観測なら `base`、観測済みなら `base+1000`。
合法値はそれぞれ `0..517`、`1000..1517` 内にあり、途中の乗算・加減算も signed16 に収まる。
各範囲内の全整数が合法という意味ではない。不正な外部書込みの補正は行わない。

| source | 値スタックへの入出力 | caller-saved register | storage 書込み |
| --- | --- | --- | --- |
| `galaxy_encode.vtl` | `( index k b s observed -- )` | A,C,D,E,I | 対象セル |
| `galaxy_decode.vtl` | `( index -- k b s observed )` | A,C,D,E,F,I | なし |
| `galaxy_observe.vtl` | `( index -- )` | I | 未観測の対象セルのみ |
| `galaxy_remove_enemy.vtl` | `( index -- )` | F,I | 敵数が正の対象セルのみ |
| `galaxy_remove_base.vtl` | `( index -- )` | F,I | 基地数が正の対象セルのみ |
| `galaxy_remove_star.vtl` | `( index -- )` | F,I | 星数が正の対象セルのみ |

`index=0..63`、encode の `observed=0/1` は caller の前提。
decode は `stored>=1000` で観測を判定して flag を除去し、商と剰余で各個数を取り出す。
削除例では合法値に対して同値な `stored%1000` で元の内容を抽出し、caller の個数検査を局所 block で示す。
個数がゼロなら書き込まない。正なら保存値から100/10/1を減らし、観測 flag と他の個数を保つ。
観測は未観測時だけ1000を加えるため冪等。

正常終了時は stack prefix を保持し、指定した register と対象セル以外を変更しない。
K/B/N register、全 storage[64..]（装置・sector・敵・scalar 状態・PRNG[192]・scratch を含む）、出力は不変。
ゲーム全体の敵数や敵HP等の整合更新は各 caller が担当する。
共有 A-Z の保存は [#408](https://github.com/kkismd/vtlxx/issues/408) に従い caller が行う。
この fixture 自体は named callee を呼ばず、scratch storage も使わない。

`micro_trek_galaxy.rs` は既存 `Machine::execute_source` で各 source を実行する。
全96組合せ×観測2状態を index=0,63 で検証し、全64アドレスで連続操作も検証する。
期待値は Rust 側で独立に計算し、decode の戻り値、観測2回、各個数1→0、
ゼロ個数での削除拒否、全65536セル、非scratch register、出力、stack prefix を比較する。
現行 Rust `Machine` には入力cursor/入力primitiveがないため、入力不変は source が入力操作を含まないことの確認による。
入力用の host API は本 issue では追加しない。

```sh
cargo test --manifest-path crates/poc/extended-classic-vtl/Cargo.toml --test micro_trek_galaxy
cargo test --manifest-path crates/poc/extended-classic-vtl/Cargo.toml
cargo test --workspace
```
