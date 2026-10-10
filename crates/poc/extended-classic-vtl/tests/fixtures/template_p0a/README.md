# P0a template conformance fixtures

Each `.vtl` file is a source input shared with the future 6502 compiler. Its adjacent `.expected` file records the result, phase, observable output/state, and profile where relevant. `\n` in `stdout` denotes one LF byte. A fixture marked “run independently” treats each logical statement as its own source input; this permits one file to group related malformed forms without requiring runtime continuation after a compile error.

| Fixture | Rust check | 6502 profile |
| --- | --- | --- |
| `minimal_two_arguments.vtl` | Run and compare stdout, A/B, and empty stack | Same source and visible output |
| `p0a_template_calibration.vtl` in `examples/` | Existing integration test compares output, A/B/C, and stack | Shared successful example |
| `block_then_else.vtl` | Run and compare then-arm call, else selection, following statement, and output | Same source and visible output |
| `deferred_rhs_definition.vtl`, `deferred_rhs_call.vtl` | Definition succeeds; invocation fails at compile time without partial runtime effect | Compile failure before runtime |
| `invalid_call_forms.vtl` | Each listed call fails compilation | Same source forms |
| `invalid_definitions.vtl` | Each listed definition fails and publishes no template | Same source forms |
| `partial_expansion_failure.vtl` | Failed expansion leaves caller owner unexecuted and completed template intact | Same compile failure; runtime does not start |
| `chunk_failure_profiles.vtl` | Rust retains the flushed earlier chunk and published template | Whole-program compile failure starts no runtime |

The Rust-only API lifetime check uses the same `deferred_rhs_definition.vtl` and `deferred_rhs_call.vtl` sources in one `Machine`, then verifies that a second `Machine` has no template. It is profile-specific because the current 6502 interface accepts one serial source and compiles the whole program.

Run Rust fixtures with:

```sh
cargo test --manifest-path crates/poc/extended-classic-vtl/Cargo.toml --test template_p0a_fixtures
```
