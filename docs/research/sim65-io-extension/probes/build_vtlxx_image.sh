#!/bin/bash
set -eu
here=$(cd "$(dirname "$0")" && pwd)
: "${CC65_ROOT:?set CC65_ROOT to the patched cc65 checkout}"
: "${VTLXX_ROOT:?set VTLXX_ROOT to crates/vtlxx at the recorded VTLXX SHA}"
: "${OUT_DIR:?set OUT_DIR to a writable scratch directory}"
mkdir -p "$OUT_DIR"
sources=(6502/target/sim65_cli.s 6502/frontend/basic.s 6502/frontend/source.s 6502/compiler/arena.s 6502/compiler/emitter.s 6502/compiler/owner.s 6502/compiler/binding.s 6502/target/zp_layout.s 6502/runtime/state.s 6502/runtime/primitives.s 6502/runtime/arithmetic.s)
objects=()
for i in "${!sources[@]}"; do
  "$CC65_ROOT/bin/ca65" -t sim6502 "$VTLXX_ROOT/${sources[$i]}" -o "$OUT_DIR/vtl-$i.o"
  objects+=("$OUT_DIR/vtl-$i.o")
done
"$CC65_ROOT/bin/ca65" -t sim6502 -D SIM65_ENTROPY_UNAVAILABLE "$here/experimental_adapter.s" -o "$OUT_DIR/vtl-adapter.o"
"$CC65_ROOT/bin/ld65" -t sim6502 -o "$OUT_DIR/vtlxx.prg" "${objects[@]}" "$OUT_DIR/vtl-adapter.o" "$CC65_ROOT/lib/sim6502.lib"
