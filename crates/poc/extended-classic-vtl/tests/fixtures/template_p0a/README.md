# P0a template conformance fixtures

各 `.vtl` は Rust / 6502 共通 source、隣接する `.expected` は期待条件です。
`stdout` の `\n` は LF 1 byte を表します。不正形式のファイルは**各行を独立した source**として実行します。
説明文を含む `.expected` 全体を機械的に parse/assert しているわけではありません。以下の台帳で各条件と観測手段を対応付けます。

根拠: 採用 ADR [#339](https://github.com/kkismd/vtlxx/issues/339)、[#359](https://github.com/kkismd/vtlxx/issues/359)。
親 Tracker: [#354](https://github.com/kkismd/vtlxx/issues/354)、[#271](https://github.com/kkismd/vtlxx/issues/271)。
[#427](https://github.com/kkismd/vtlxx/issues/427) の確認基準 main は `7338275be36dae3a17a5e48b06b76ebf2c51574a`。

## 期待条件と観測台帳（8群）

表の source 名は拡張子 `.vtl`、expected 名は `.expected` を省略しています。
`S` は `sim65_frontend.rs::p0a_shared_success_sources_match_reference_and_empty_stack`、
`F` は `sim65_frontend.rs::p0a_shared_failures_preserve_phase_publication_and_never_start_runtime`。
Rust の対応 test は `template_p0a_fixtures.rs` に置き、calibration のみ `template_calibration_example.rs` です。

| source / expected | 可搬な期待条件と Rust の検査 | 6502 の検査・観測手段 | Rust 限定条件 | 6502 固有条件 |
| --- | --- | --- | --- | --- |
| `minimal_two_arguments` / 同名 | success、stdout `1\n11\n`、A=1/B=11、stack empty。Rust は output/register/stack を直接 assert | S: 原source全体を両CLIで exit0 / raw stdout 完全一致。A/B は原source末尾の出力。private probe で rt_depth=0、a の全保存本文も照合 | なし | 1つの length-prefixed frame |
| `block_then_else` / 同名 | success、stdout `1\n7\n0\n4\n`、A=1/B=7/C=0/D=4、stack empty。Rust は output/register/stack を直接 assert | S: 原source全体で exit0 / raw stdout 一致。then の呼出、else 非選択、後続文と A〜D は末尾出力。private probe で rt_depth=0 | なし | 1 frame |
| `examples/p0a_template_calibration.vtl` / 隣接 expected なし | success、stdout `58\n20\n40\n`、A=58/B=20/C=40、stack empty。Rust は `assert_calibration` で直接 assert | 既存 `sim65_cli.rs::p0a_template_calibration_example_matches_reference_output_bytes` と `sim65_examples.rs` の全例台帳: 両CLI exit0、raw stdout 一致。register は原source出力。6502 stack は下記の構造根拠（この例専用probeは追加しない） | Machine API による内部state検査 | 既存examplesハーネスを再利用 |
| `deferred_rhs_definition` + `deferred_rhs_call` / 各同名 | definition success / phase definition / stdout empty / a published。call compile_error / phase invocation / stdout empty / registers unchanged / stack empty | F: definition原本単独 exit0、空stdout、a=`{}=A+` をprobe。definition+callを**同一frameに連結**して exit130 / 空stdout。probeで syntax_error 時 virtual_active=1、a原本保持、runtime非開始 | 同じMachineで別execute_source間の再利用、別MachineではUndefinedBinding。RustでB=0/empty stack | process終了後の再利用は要求しない。register/stackのruntime副作用なしは入口trapで確認 |
| `invalid_call_forms` / 同名 | 全行 compile_error / phase invocation / stdout empty / registers, stack unchanged | F: 有効なa定義＋各行を別frame。5行すべて exit130 / 空stdout、virtual RHSに入る前のsyntax失敗、a本文保持、runtime非開始。最初の4行は呼出形式、`z{A}=()` は未定義template（z未公開をprobe） | RustでA=0/empty stack | 6502は呼出形式違反と未定義templateをともにstatus2へ分類。原因は入力とregistry検査で区別 |
| `invalid_definitions` / 同名 | 全行 compile_error / phase definition / no incomplete template / stdout empty | F: 7行を各々別frame、全行 exit130 / 空stdout、virtual RHS前のsyntax失敗。probeでa〜z全slot未公開、runtime非開始 | Rustは失敗後の呼出がUndefinedBindingになることも確認 | 失敗後の同一process内lookupで確認し、別frameへのstate継続を要求しない |
| `partial_expansion_failure` / 同名 | compile_error / phase invocation expansion / A=0,B=0 / stack empty / aは両原statementを保持 | F: 原source全体 exit130 / 空stdout、virtual_active=1でsyntax失敗。a=`A=3`+LF+`{}=A+` の全byte/長さをprobe、他slot未公開。runtime入口trapでA=3/B=9や部分ownerの非実行を確認 | RustはA/B/stackと失敗後a再呼出を検査 | 6502 runtime stateは未初期化のため数値0を直接要求せず、runtime未開始を検査 |
| `chunk_failure_profiles` / 同名（profile別説明） | 後続c定義が失敗、a完成済み/c未公開 | F: 原source全体 exit130 / 空stdout、virtual RHS前のsyntax失敗。a=`A={}` 保持、cを含む他slot未公開。runtime入口trapで先行B=7も実行されないことを確認 | 先行chunk B=7を保持。失敗後a再利用可、c未公開をAPIで検査 | WholeProgramCompileRunはruntimeを開始しない。expectedの「B remains at its initial value」はruntime副作用なしの意味で扱い、rt_init前のRAM内容を初期値0とは扱わない |

## 6502 の観測方法と制約

- source は u16 LE 長＋source bytes の frame とし、成功例の原sourceを加工しません。新しい検査ではruntime入力・stdout sentinelを付加せず、raw bytes（LF込み）を比較します。
- まず**変更していない** `6502/target/sim65_cli.s` とproduction frontendを実行します。success=exit0、今回の全不正例=exit130（128+fe_compile_status 2）、stdoutとstderrはそれぞれ独立assert（失敗時はいずれも空）。これはtarget adapterの結果であり、host `ecvtl6502` の診断付きstderrとは別です。
- 次に同じsourceをprivate probeで実行します。成功時は空value stackを直接確認。失敗時は一時コピーの `syntax_error` で `virtual_active` を記録し、cleanupで消える前のreaderを区別します。`rt_init` への唯一の到達箇所をexit99のtrapに置換し、**空stdoutだけでは検出できない無出力runtimeの開始**も拒否します。probe自身の不一致はexit98、全検査成功はexit0です。
- registryはprocess終了前に `template_lookup` で26slotを検査。aがある場合はu16長と全保存byteを比較し、他slotは未公開を要求します。失敗後のcompile再開・再実行は行いません。test用adapterとbasicの一時コピーだけを使い、release ABIへexportやswitchを追加しません。
- 定義単独成功は空のdriverを通るため、`rt_init` 自体の未到達を要求しません。「runtimeなし」はtemplate本文をruntimeとして実行しないことです。通常RHSとして不正な `{}=A+` の定義成功と保存本文検査が、この遅延検証を裏付けます。
- definition/呼出エラーで完成済みaが不変であることは上記probeで直接観測します。公開順序の構造根拠は `basic.s` の本文/終端検証→`cc_source_finish`成功→`template_publish`、`template.s` のvalid最終書込みです。`template_reader.s` は保存本文を読み取り専用で走査し、展開はcaller ownerの通常assignment compilerへ渡します。`fe_compile_run` は全source・driver完成後にのみruntimeへ入ります。
- calibrationの6502 stack根拠: 原sourceには明示stack操作がなく、通常assignmentはRHS値の生成とstoreで相殺、`?=`は値を消費、`?=()`はstack不変、`=()` template呼出はruntime operandを追加しません。Rustは直接stackを検査し、同じ展開機構を使う最小/IF fixtureは6502 probeでも直接検査します。calibrationそのものの6502 rt_depthを動的に観測したとは主張しません。
- exit132は本fixtureの期待値ではありません。stack guard NGと一般backend resourceの両方がCLIでは132になり得ます。既存 `compile_stack_guard_*` と `sim65_negative.rs` の内部原因probeを維持し、今回のsyntax適合と混同しません。

## 実行

cc65（ca65 / ld65 / sim65）が必要です。ignored suiteを実行しない通常workspaceテストだけで6502適合を宣言しません。

```sh
cargo test -p vtlxx-poc-extended-classic-vtl --test template_p0a_fixtures
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_frontend -- --ignored --nocapture
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_cli -- --ignored --nocapture
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_examples -- --ignored --nocapture
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_negative -- --ignored --nocapture
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

資源使用量・容量評価は #428、全workload proofは #278 の責務です。
