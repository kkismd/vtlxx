# 長い通常Write名の適合・資源検証台帳

根拠は採用ADR [#453](https://github.com/kkismd/vtlxx/issues/453)、[stack方針 #520](https://github.com/kkismd/vtlxx/issues/520)、親Tracker [#458](https://github.com/kkismd/vtlxx/issues/458)。#457の検証基準mainは `7e64f883459c093d6f906cfb58dd5ce47f475af3`、長名実装前の比較基準は `1826eb00c4a7fd285a7ad6c68e64aed375895aa9`。実行環境はsim6502、IRQ/NMIなし、ca65/ld65/sim65 V2.18（Ubuntu 2.19-1）。

## 共通sourceの期待値と観測

`long_write_conformance.rs` の `rust_shared_success_and_stack`（R）、`rust_shared_failures_keep_completed_effects_and_bindings`（F）、ignored `sim65_shared_sources_match_expected_bytes_and_failure_profile`（S）が検査する。成功fixtureはそのまま両処理系に入力し、下表のstdoutはLFを含まないraw bytes。6502の内部stackを直接測定したとは主張しない。

| source | 可搬な期待・対象 | Rust限定の状態検査 | 6502限定の結果 |
| --- | --- | --- | --- |
| `success.vtl` | 成功、stdout `abr8`。1/2/5/8文字、同長・異長の接頭辞、callee-first、`=()`/RHS/`~=...`、IF/While内呼出 | R: 空stack | S: 成功・raw stdout一致 |
| `template_first.vtl` | 成功、stdout `12`。`a{}`を`ab`より先に定義 | R: 空stack | S: 成功・raw stdout一致 |
| `write_first.vtl` | 成功、stdout `12`。`ab`を`a{}`より先に定義 | R: 空stack | S: 成功・raw stdout一致 |
| `invalid.vtl` | 各行を独立入力とし空名、9文字、数字混在、大文字混在、完全名重複、未定義、forward call、未完成owner、P0a同名衝突の両順序、長名template呼出を拒否 | F: compile error、先行flush済みstdout `ok` / A=7、完成済み `persist` を別executeで再利用。未完成 `open` は未公開 | S: 全入力でnonzero、空stdout、compile failure診断。WholeProgramCompileRunのため先行出力も実行されない |

不正例には各処理系で同一の先行source `A=7 ?="ok" &=persist [ B=~ ]` を付加する。空stdout単独では無出力runtimeの開始を検出できないため、runtime入口trapによる非開始の直接証拠は既存 `sim65_frontend.rs::p0a_shared_failures_preserve_phase_publication_and_never_start_runtime` とcompile guardのprobe、およびproduction frontendの全体compile後のruntime入口構造を併せて参照する。

## 既存テストによる容量・公開・回帰の証拠

テスト名は `tests/` 以下のファイル名と関数名。6502側はcc65が必要なignored testである。異なるtestの検査を同じsourceによる比較と読み替えない。

| 条件 | Rust側の直接検査 | 6502側の直接検査 |
| --- | --- | --- |
| 65件の通常Write定義と呼出 | `long_write_names.rs::rust_accepts_more_than_sixty_four_completed_names` は65件を定義・呼出しA=65 | `sim65_long_write_names.rs::sixty_four_definitions_and_deep_calls_succeed_but_sixty_five_fail` は64件・64段の成功と65件目のcompile失敗、空stdoutを検査。件数制限はtarget profile固有 |
| 64通常Write + 26 template + 91 chunk | 件数上限をportable仕様としない | `sim65_long_write_names.rs::sixty_four_writes_and_twenty_six_templates_fit_ninety_one_chunks` は全定義・呼出と出力 `6426`、さらに65件目の失敗を検査。arena等の別上限は独立 |
| 完成済み定義の公開継続と失敗 | `long_write_names.rs::completed_long_names_survive_failures_and_stay_machine_local` はflush済み出力/register、完成済み定義の別execute間保持、未完成定義の非公開、重複拒否と別Machineからの未定義を検査 | `sim65_long_write_names.rs::invalid_names_and_definitions_do_not_start_runtime` は先行出力を含む入力の失敗と空stdoutを検査。process終了後の状態継続は要求しない |
| P0aの完全名衝突・共存 | `long_write_names.rs::template_and_normal_names_use_complete_name_for_collisions` は両定義順、同名衝突と長名template呼出の拒否を検査 | `sim65_long_write_names.rs::template_and_normal_prefix_coexist_in_both_orders` および `invalid_names_and_definitions_do_not_start_runtime` |
| 既存builtin・label・named integer | `long_write_names.rs::existing_builtins_labels_and_named_integers_keep_their_meaning` | 既存 `sim65_frontend` / `sim65_cli` / `sim65_negative` のignored回帰を維持 |
| P0a/P0b | `template_p0a_fixtures` 等の既存検査 | `sim65_frontend.rs::p0a_shared_success_sources_match_reference_and_empty_stack`、`p0a_shared_failures_preserve_phase_publication_and_never_start_runtime`。詳細は隣の `template_p0a/README.md` |

## 資源とstackの照合

[#456の監査](../../../../../../docs/ecvtl-6502-long-write-resources.md)は上記両基準を同一ツールで比較した記録。今回、両commit間の対象ソース差分を照合し、runtime・adapter・計測scriptには変更がなく、`zp_layout.s` はコメントのみ変更、frontend/bindingが変更対象であることを確認した。CLIの15 moduleとsim6502.libのリンク順も現行sourceで確認した。同じ測定条件・リンク経路に矛盾を見つけていないため、数値を複製して再測定せず既存証拠を参照する。以下は監査の値であり今回の新規計測ではない。

| 対象 | 旧 → 現基準 | 出典・範囲 |
| --- | --- | --- |
| linked CODE / RODATA | 7745 → 7890 B / 49 → 49 B | #456 linked map、15 module、sim6502.lib |
| linked ZEROPAGE / BSS | 141 → 143 B / 34473 → 35117 B | 同上。現BSS `$2173..$AA9F`、arena `$2C84` から24576 B、リンク配置は非重複 |
| Micro TREK arena 使用 / 残 | 15570 / 9006 → 15570 / 9006 B | `tests/fixtures/micro_trek/measure_sim65.py` の代表workload、全入力の上界ではない |
| EOF / displays / retry cycles | 30211835 / 31352775 / 47418752 → 30275815 / 31418147 / 47495133 | 同script、compileとruntimeの合計。代表入力の実測で全入力の上界ではない |
| compile stack guard | S>=25、K=24 B、下端予約1 B | [guard監査](../../../../../../docs/ecvtl-6502-stack-guard.md)の4入口・入口外区間に#456の長名scanner/binding経路を照合。NGはstatus 4、exit 132、空stdout、runtime非開始のtail HALT。`sim65_frontend.rs::compile_stack_guard_comparison_boundary_preserves_operand_modes` が24/25境界、`compile_stack_guard_halts_deep_sources_before_runtime_or_wrap` が強制NGとruntime入口trap、`compile_stack_guard_accepts_201_byte_fixture_and_observes_headroom` が正常例を検査。全compile入力の動的高水位を実測した主張ではない |
| runtime stack | 最大64段、保守的上界145 B | #456は[#426](https://github.com/kkismd/vtlxx/issues/426)・[#381](https://github.com/kkismd/vtlxx/issues/381)のlinked helper監査を現行15 moduleとV2.18 libraryに照合。startup 2 + main 2 + entry return 2 + driver 2 + 64段×2 = 136 B、入力helper最大9 Bを加算。初期S=$FFから非wrap許容量255 Bに対し余裕110 B。64段testの成功は補助証拠であり、145 Bは実測high-waterではない |

runtimeの上界はこのadapter・library・IRQ/NMIなしのsim6502に限る。他targetの保証ではない。compile guardはruntimeを監視しない。[#520](https://github.com/kkismd/vtlxx/issues/520)は旧一律128 B設計予算を廃止し、物理stackの非wrapを要求する。library、adapter、呼出経路、guard被覆などの変更時には再監査が必要。
