# ECVTL 6502 workload 資源計測

## 目的と範囲

Issue [#381](https://github.com/kkismd/vtlxx/issues/381) に基づき、EC07 の現行 profile を成功 workload で計測した。結果は #278 の end-to-end proof に渡す資源 evidence であり、容量 profile の変更・最適化・未使用領域の解放を判断するものではない。

P0a は `p0a_template_calibration.vtl` 1例を測定した。共有 arena における保存 source と生成 code の同時占有を記録している。共通 P0a fixture 全体の適合・容量評価は Tracker [#354](https://github.com/kkismd/vtlxx/issues/354) の別責務として残る。

## 再現条件

- 計測対象 source revision: `4a7cc292432404e44fb1f25776dde014757b3b28`
- 実行: `cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_resource_usage -- --ignored --nocapture`
- 既存 workload 回帰: `cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_examples -- --ignored --nocapture`
- Toolchain: rustc/cargo 1.99.0、ca65/ld65/sim65 V2.18 (Ubuntu 2.19-1)
- small smoke、IF/While、named Write、storage、P0a は 4,000,000 cycles。Queens 2例は #377 と同じ 256,000,000 cycles。

計測 test は通常の `ecvtl6502` build を先に実行し、Rust reference と raw stdout / exit code を比較する。次に `RESOURCE_USAGE_PROBE` を付けた一時 build を同じ framed source で実行し、通常 build の stdout が計測 build の先頭と完全一致すること、exit code も一致することを確認する。計測 record はこの instrumented build の末尾だけに出力し、生成物は一時 directory から削除する。

観測カウンタと追加 storage bitmap は `RESOURCE_USAGE_PROBE` build にだけ置く。計測更新は stack push、storage access、table entry 更新時の inline 記録で行い、計測 hook の追加 JSR は行わない。storage の初期化 clear は access と数えない。hardware stack は `_main` の開始時に stack page の空き領域へ canary を置き、`fe_compile_run` から戻った直後に上書き深さを走査する。canary を `$a5` と `$5a` に変えた2回の実行で他の値と出力が一致することを検査し、hardware high-water は2回の大きい方を採る。値には sim6502 startup から `_main` に入る return address 2 byte を含む。

## 測定結果

容量値は EC06 / EC07 profile と現行コードに基づく。`storage Cells / extent` はそれぞれアクセスされた固有 cell 数と最大 index + 1。arena は `generated code + retained template source` の占有量で比較し、未使用 tail を解放済みとは扱わない。

| workload | code B | template source B | shared arena B / 2048 | value stack Cells / 16 | hardware stack B / 128 | storage Cells / extent (128) | source work stack / 16 | labels / owner (32) | fixups / owner (32) | pending patches / 2048 positions | expression depth / 16 | IF depth / 16 | While depth / 16 | chunks / 53 | constants / 16 | completed owners | Write publishes / 26 | templates / 26 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| small smoke | 45 | 0 | 45 | 2 | 25 | 0 / 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 2 | 0 | 0 |
| IF/While | 109 | 0 | 109 | 2 | 29 | 0 / 0 | 1 | 0 | 0 | 2 | 0 | 1 | 1 | 1 | 0 | 2 | 0 | 0 |
| named Write | 43 | 0 | 43 | 2 | 29 | 0 / 0 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 2 | 0 | 4 | 1 | 0 |
| storage | 45 | 0 | 45 | 2 | 27 | 1 / 1 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 1 | 0 | 2 | 0 | 0 |
| P0a template | 299 | 27 | 326 | 2 | 23 | 0 / 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 2 | 6 | 3 | 0 | 1 |
| Eight Queens | 693 | 0 | 693 | 3 | 45 | 8 / 8 | 3 | 0 | 0 | 5 | 1 | 5 | 1 | 8 | 0 | 16 | 7 | 0 |
| Eight Queens Storage | 656 | 0 | 656 | 3 | 47 | 9 / 9 | 3 | 0 | 0 | 5 | 1 | 5 | 1 | 7 | 0 | 14 | 6 | 0 |

**判定:** 計測した全 workload は各 profile 上限内。最大 shared arena は 693/2048 B、value stack は 3/16 Cells、hardware stack は 47/128 B、storage は 9/128 Cells、source work stack は 3/16 entries。追加確認した expression nesting は最大1/16、IF nesting は5/16、While nesting は1/16。P0a例では 299 B の生成 code と27 Bの保持 sourceが同時にarenaを占有し、合計326/2048 Bだった。

numeric label / owner fixup の高水位 0 は、対象 workload に numeric label / GOTO がないため実測された未使用量である。structured control による machine-code patch は別計数で、最大5件だった。source work stack、named integer、top-level chunk、Write publish、template publish のいずれも実際に使用した値を記録した。

現在の固定 compile-time table も確認した: top-level chunk target 53 entries、named integer 16 entries（最大名長16 byte）、Write / SourceProcedure binding 26 identity × 2 roles、template registry 26 identities、While frame 16、expression nesting 16、structured IF nesting 16、numeric label / unresolved fixup 各32 per owner。現行mainではSourceProcedure roleは実装対象外なので Write publish の上限比較は26 identitiesとした。arena patch bitset と completed-owner bitmap はそれぞれ2048 code positions分の256 Bで、動的 pending patch 数と completed owner 数を表へ記録した。

計測値は run 時点の占有・高水位であり、total linked BSS/RAM、ROM、cycles の評価ではない。成功した compile-run の最終計測は final driver を含み、template source の publish と owner completion が終わって runtime が戻った後に採取する。所有済み template source と生成 code は共有arena内で維持される。余った容量を解放・再利用したという扱いはしない。

## 検証結果

- `cargo test --workspace` — 成功
- `cargo fmt --all -- --check` — 成功
- `cargo clippy --workspace --all-targets -- -D warnings` — 成功
- `cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_examples -- --ignored --nocapture` — 6 workload 成功、raw stdout は Rust と一致。Queens 両例は92解と最終解数を確認
- `cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_resource_usage -- --ignored --nocapture` — 7 workload 成功。各 workload の通常 build と計測 build のstdout raw bytes / exit code一致、profile内を確認
- `git diff --check` — 成功

計測により既存 source semantics、production CLI option、通常 build の resource layout は変更していない。resource probe は test専用で、portable source surface に debug値を公開しない。
