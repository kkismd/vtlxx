#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
: "${CC65_ROOT:?set CC65_ROOT to the V2.18 cc65 checkout with the experimental patch}"
: "${VTLXX_ROOT:?set VTLXX_ROOT to crates/vtlxx at the recorded VTLXX SHA}"
: "${OUT_DIR:?set OUT_DIR to a writable scratch directory}"
mkdir -p "$OUT_DIR"

"$CC65_ROOT/bin/ca65" -t sim6502 "$here/wait_fatal_guest.s" -o "$OUT_DIR/fixture.o"
"$CC65_ROOT/bin/ca65" -t sim6502 "$here/wait_fatal_adapter.s" -o "$OUT_DIR/adapter.o"
"$CC65_ROOT/bin/ca65" -t sim6502 "$VTLXX_ROOT/6502/target/zp_layout.s" -o "$OUT_DIR/layout.o"
"$CC65_ROOT/bin/ca65" -t sim6502 -D RT_TEST_PROBE "$VTLXX_ROOT/6502/runtime/state.s" -o "$OUT_DIR/state.o"
"$CC65_ROOT/bin/ca65" -t sim6502 "$VTLXX_ROOT/6502/runtime/primitives.s" -o "$OUT_DIR/primitives.o"
"$CC65_ROOT/bin/ca65" -t sim6502 "$VTLXX_ROOT/6502/runtime/arithmetic.s" -o "$OUT_DIR/arithmetic.o"
"$CC65_ROOT/bin/ld65" -t sim6502 -o "$OUT_DIR/wait_fatal.prg" \
    "$OUT_DIR/fixture.o" "$OUT_DIR/adapter.o" "$OUT_DIR/layout.o" \
    "$OUT_DIR/state.o" "$OUT_DIR/primitives.o" "$OUT_DIR/arithmetic.o" \
    "$CC65_ROOT/lib/sim6502.lib"
