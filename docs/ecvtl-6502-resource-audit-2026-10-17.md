# ECVTL 6502 resource audit (#381)

## Target main SHA and verification environment

- Audited `main`: `48cc385f6ce86e755fbe82604dd9b354cca18ab4`.
- `ca65`, `ld65`, and `sim65`: V2.18 (Ubuntu package `2.19-1`).
- The ignored sim65 tests were run locally. No `--trace` run was needed for this static audit.
- The audit covers the current sim6502 target and CLI path. Physical adapters and interrupts on an actual 6502 board are outside this profile.

## Hardware stack breakdown

6502 `JSR` pushes a two-byte return address. `PHA` and `PHP` each add one byte. `JMP` and tail `JMP` add no return address. The relevant runtime entry is `_main → fe_compile_run`; after compilation, `fe_compile_run` pushes the return address for the generated entry and jumps to it. The generated driver calls each completed source chunk, and a chunk can call a statically resolved Write procedure.

| Runtime contribution | Bytes | Evidence / qualification |
| --- | ---: | --- |
| `_main → fe_compile_run` | 2 | `6502/target/sim65_cli.s:11-12`; remains live while `fe_compile_run` runs the generated entry |
| `fe_compile_run → generated entry` | 2 | Manual high/low `PHA` at `6502/frontend/basic.s:127-130` |
| generated driver → top-level chunk | 2 | `cc_call` emits `JSR`; `6502/frontend/basic.s:192-218`, `6502/compiler/emitter.s:42-57` |
| Maximum named-procedure chain | 52 | 26 lowercase identities (`a..z`) × 2 bytes. Calls resolve only completed bindings; definitions publish only after completion. Direct and mutual recursion cannot be formed in the current source order (`6502/frontend/basic.s:742-799`, `6502/compiler/binding.s`). |
| generated code → runtime helper | 2 | `JSR` emitted by `cc_call` or other fixed templates |
| helper → nested helper | at least 2 on paths such as `rt_pop_condition → rt_need_one`, `rt_div → unsigned_divide`, or storage helpers → `rt_need_two` | These helper calls are sequential alternatives; do not sum them. `runtime/state.s:65-77, 153-181`, `runtime/arithmetic.s:74-109` |
| helper push bytes | at least 2 on `rt_push`; up to 3 extra on `rt_load_reg → rt_push` | `runtime/state.s:46-63, 122-132`. The helper call itself is already counted above. |
| serial output library path | Unknown | `rt_print_number → serial_out → _putchar` can execute beneath the deepest procedure chain. `serial_out` tail-jumps to `_putchar`; the reached cc65 library call graph was not disassembled and bounded here. |

The source-visible subtotal through the deepest procedure and one generated helper call is **62 B** (2 + 2 + 2 + 52 + 2 + 2 for the helper's nested call). This is a partial subtotal, not a proven maximum. The serial output path and all helper alternatives must be compared at their deepest simultaneous points before it can serve as an upper bound. The 26-procedure term is a structural maximum for current bindings, not a claim that every workload reaches it.

## Compile/runtime upper bounds and budget comparison

| Phase | Maximum upper bound | Physical 256 B | #239 target-visible budget 128 B |
| --- | --- | --- | --- |
| Runtime | **Unresolved.** The 62 B subtotal above omits the reached cc65 `_putchar`/startup library path and is not a whole-program bound. | **Unresolved** | **Unconfirmed** |
| Compile | **Unresolved.** Expression/group and structured-block recursion each have a 16-level guard (`nesting`, `structured_depth`); While frames have a separate 16-entry limit. The per-level live JSR frames and PHA/PHP bytes, plus backend and serial input helper nesting, have not been exhaustively enumerated. | **Unresolved** | **Unconfirmed** |

Compile and runtime occur in sequence: `fe_compile_run` calls `rt_init` only after compilation and then enters generated code. Their independent phase maxima must not be added. The `_main` caller frame remains present across both phases. No path in this audit was found to depend on 8-bit SP wrap as a valid behavior, but a complete path enumeration is still needed to substantiate the no-wrap condition.

**Physical-capacity decision: 未解決.** The available static evidence does not prove both phase maxima below 256 B.

**128 B design-budget decision: 未確認.** The 128 B target-visible budget remains unchanged; no adjustment is proposed.

## Evidence and reproduction

Relevant source locations:

- CLI entry: `crates/poc/extended-classic-vtl/6502/target/sim65_cli.s:11-21`.
- Compile, driver construction, runtime handoff, and manual return-address push: `crates/poc/extended-classic-vtl/6502/frontend/basic.s:84-145, 190-230`.
- Write owner completion and publish order: `crates/poc/extended-classic-vtl/6502/frontend/basic.s:742-800`.
- Binding resolution and completion checks: `crates/poc/extended-classic-vtl/6502/compiler/binding.s`.
- Runtime helper calls and pushes: `crates/poc/extended-classic-vtl/6502/runtime/state.s`, `runtime/arithmetic.s`, `runtime/primitives.s`.
- cc65 adapter: `crates/poc/extended-classic-vtl/6502/target/sim65_adapter.s:10-25`.

Reproduce the successful workload checks with:

```sh
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_examples -- --ignored --nocapture
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_negative -- --ignored --nocapture
```

Both commands passed in this environment. `sim65_examples` ran all six fixtures, including both Eight Queens variants (92 solutions each), with Rust/6502 raw stdout agreement and exit 0. The negative suite passed its compile-failure/no-runtime-effect checks. These are workload evidence, not stack high-water measurements.

## Other resources: concise assessment

| Resource | Current profile / evidence | Assessment |
| --- | --- | --- |
| Shared arena | 2048 B (`compiler/arena.s`); code and retained P0a template source share it. The successful P0a calibration example and arena-exhaustion negative test demonstrate a working and detected limit. | No byte-by-byte occupancy claim made here. Reserved capacity is not treated as used code. P0a-specific capacity evaluation belongs to #354. |
| Runtime value stack | 16 Cells (`runtime/state.s`), overflow guarded by `rt_push`; example workloads and runtime failure paths are exercised. | No dynamic maximum depth measured in this audit. |
| Indexed storage | 128 Cells / 256 B; access uses the low seven address bits (`runtime/state.s:152-182`). Queens storage fixture succeeds. | Fixture success supports current profile operation; distinct-cell usage is not measured. |
| Compile tables | Source work stack: 16 × u16; numeric labels: 32/owner; unresolved fixups: 32/owner; structured and While depth: 16; top-level chunks: 53. Current failure tests exercise representative exhaustion paths where reachable. | Fixed profile and guarded failure paths; no complete per-workload high-water table is required by the latest #381 scope. |

## Residual risks and decision

The current evidence establishes successful representative workloads and several compile-time resource failure paths. It does not establish hardware-stack safety because the compile call-chain accounting and the linked serial library path are incomplete. No production assembly, ABI, CLI option, or capacity profile was changed.

The next focused investigation should enumerate the reachable cc65 startup/serial routines and all frontend recursive call paths with simultaneous JSR/PHA/PHP accounting. If static accounting remains unclear, use a short, representative `sim65 --trace` run to inspect CPU `S` separately for compile and runtime; this is supporting observation, not a substitute for an upper bound over all reachable paths. Keep the #239 128 B comparison separate. Do not mark the physical 256 B checkpoint passed until both phase bounds are supported.
