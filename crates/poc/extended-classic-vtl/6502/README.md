# 6502 target PoC

このディレクトリは EC07 の sim6502 固有コードを置く。`target/sim65_adapter.s` が 1 byte の `serial_in` / `serial_out` と 8-bit status の `halt` を提供する。A register を受け渡しに使う呼び出し規約と `sim6502.lib` への接続はこの target 内だけの取り決めであり、portable ECVTL runtime ABI ではない。入力終端の扱いは後続 issue が定める。

host 側の `tests/sim65_6502.rs` が `ca65 -t sim6502`、`ld65 -t sim6502`、`sim65 -x` を起動する。生成物は OS の一時ディレクトリに作られ、テスト終了時に削除される。

```sh
cargo test -p vtlxx-poc-extended-classic-vtl --test sim65_6502 -- --ignored
```

通常の `cargo test --workspace` は cc65 を必要としない。
