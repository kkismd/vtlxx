#!/usr/bin/env bash
set -euo pipefail

# A new directory supplied by the caller keeps all generated files isolated.
root=$(git rev-parse --show-toplevel)
evidence="$root/docs/research/mos6502-host"
work=${1:?Pass a new absolute scratch directory}
[[ "$work" = /* && ! -e "$work" ]] || { echo 'scratch must be a new absolute path' >&2; exit 2; }
mkdir -p "$work/source" "$work/image"
git -C "$root" archive 246d45c9a6ac85a9f8e4e9f762317db35a88e702 | tar -x -C "$work/source"
git clone https://github.com/mre/mos6502.git "$work/mos6502"
git -C "$work/mos6502" checkout --detach 9330a8e773702d8b8d92734324027fa916490158
cp -R "$evidence/probes/harness" "$evidence/probes/failure-harness" "$work/"
cargo build --locked --release --manifest-path "$work/harness/Cargo.toml"
cargo build --locked --release --manifest-path "$work/failure-harness/Cargo.toml"

base="$work/source/crates/vtlxx"
sources=(6502/target/sim65_cli.s 6502/frontend/basic.s 6502/frontend/source.s
  6502/compiler/arena.s 6502/compiler/emitter.s 6502/compiler/owner.s
  6502/compiler/binding.s 6502/target/zp_layout.s 6502/runtime/state.s
  6502/runtime/primitives.s 6502/runtime/arithmetic.s 6502/target/sim65_adapter.s)
objects=()
for i in "${!sources[@]}"; do
  ca65 -t sim6502 -D SIM65_ENTROPY_UNAVAILABLE "$base/${sources[$i]}" -o "$work/image/$i.o"
  objects+=("$work/image/$i.o")
done
ld65 -t sim6502 -Ln "$work/image/vtlxx.lbl" -o "$work/image/vtlxx.prg" "${objects[@]}" sim6502.lib
ca65 -t sim6502 "$evidence/probes/guest/adapter.s" -o "$work/image/adapter.o"
ca65 -t sim6502 "$evidence/probes/guest/adapter-fatal.s" -o "$work/image/adapter-fatal.o"
ld65 -t sim6502 -o "$work/image/try.prg" "${objects[@]:0:11}" "$work/image/adapter.o" sim6502.lib
ld65 -t sim6502 -o "$work/image/fatal-adapter-cli.prg" "${objects[@]:0:11}" "$work/image/adapter-fatal.o" sim6502.lib

fixture_sources=(6502/target/zp_layout.s 6502/runtime/state.s 6502/runtime/primitives.s 6502/runtime/arithmetic.s)
for i in "${!fixture_sources[@]}"; do
  options=()
  [[ $i != 1 ]] || options=(-D RT_TEST_PROBE)
  ca65 -t sim6502 "${options[@]}" "$base/${fixture_sources[$i]}" -o "$work/image/f$((i+1)).o"
done
for fixture in fatal fatal-wait capacity wait-capacity; do
  options=()
  [[ "$fixture" != *capacity ]] || options=(-D TEST_CASE=16)
  ca65 -t sim6502 "${options[@]}" "$evidence/probes/guest/$fixture.s" -o "$work/image/fixture.o"
  adapter="$work/image/adapter.o"
  [[ "$fixture" != fatal-wait ]] || adapter="$work/image/adapter-fatal.o"
  ld65 -t sim6502 -o "$work/image/$fixture.prg" "$work/image/fixture.o" \
    "$work/image/f1.o" "$work/image/f2.o" "$work/image/f3.o" "$work/image/f4.o" "$adapter" sim6502.lib
done
cc -O2 "$evidence/probes/poll.c" -o "$work/poll"
python3 "$evidence/verify.py" "$work"
