#!/usr/bin/env python3
"""Assemble the same emitter with the two issue #707 opcode selectors."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[3]
source = (root / "crates/vtlxx/6502/compiler/emitter.s").read_text()

base = "byte_base: .byte $a9, $89, $29, $09, $49, $69, $e9"
table = """byte_opcodes:
    .byte $a9, $00, $29, $09, $49, $69, $e9
    .byte $a5, $85, $25, $05, $45, $65, $e5
    .byte $ad, $8d, $2d, $0d, $4d, $6d, $ed"""
delta = "byte_delta: .byte 0, $fc, 4"
offset = "byte_mode_offset: .byte 0, 7, 14"
base_select = """    ldx em_index
    lda byte_base,x
    ldx em_patch
    clc
    adc byte_delta,x
    sta em_patch+1"""
table_select = """    ldx em_patch
    lda byte_mode_offset,x
    clc
    adc em_index
    tax
    lda byte_opcodes,x
    sta em_patch+1"""

# Verify all legal entries independently of the size comparison.
bases = (0xa9, 0x89, 0x29, 0x09, 0x49, 0x69, 0xe9)
rows = ((0xa9, 0x00, 0x29, 0x09, 0x49, 0x69, 0xe9),
        (0xa5, 0x85, 0x25, 0x05, 0x45, 0x65, 0xe5),
        (0xad, 0x8d, 0x2d, 0x0d, 0x4d, 0x6d, 0xed))
for mode, adjustment in enumerate((0, -4, 4)):
    for op, value in enumerate(bases):
        if (mode, op) != (0, 1):
            assert (value + adjustment) & 0xff == rows[mode][op]

def replace_once(text, old, new):
    if text.count(old) != 1:
        raise SystemExit(f"expected one occurrence of {old!r}")
    return text.replace(old, new, 1)

direct = replace_once(source, base, table)
direct = replace_once(direct, delta, offset)
direct = replace_once(direct, base_select, table_select)
with tempfile.TemporaryDirectory(prefix="issue707-") as directory:
    directory = Path(directory)
    # Dummy definitions are identical for both links and carry no code/data.
    # They exist only to resolve the real emitter's imports, not to execute it.
    stubs = directory / "stubs.s"
    stubs.write_text(""".exportzp zp_phase, cc_cursor, cc_arg, rt_regs, rt_lhs
zp_phase = $20
cc_cursor = $10
cc_arg = $12
rt_regs = $80
rt_lhs = $14
.export owner_require_active, rt_read_static, rt_print_bytes
.export rt_store_reg, rt_load_reg, rt_pop_condition, rt_push
.export cc_track_patch, cc_patch, cc_mark, cc_emit_byte
.export cc_write_byte_raw, cc_reserve, session_backend_error
owner_require_active = $1000
rt_read_static = $1000
rt_print_bytes = $1000
rt_store_reg = $1000
rt_load_reg = $1000
rt_pop_condition = $1000
rt_push = $1000
cc_track_patch = $1000
cc_patch = $1000
cc_mark = $1000
cc_emit_byte = $1000
cc_write_byte_raw = $1000
cc_reserve = $1000
session_backend_error = $1000
""")
    config = directory / "compare.cfg"
    config.write_text("""MEMORY { RAM: start = $1000, size = $7000, file = %O; }
SEGMENTS {
  CODE: load = RAM, type = ro;
  RODATA: load = RAM, type = ro;
  BSS: load = RAM, type = bss;
}
""")
    stub_obj = directory / "stubs.o"
    subprocess.run(["ca65", str(stubs), "-o", str(stub_obj)], check=True)
    for name, data in (("delta", source), ("direct", direct)):
        asm = directory / f"{name}.s"
        obj = directory / f"{name}.o"
        asm.write_text(data)
        subprocess.run(["ca65", str(asm), "-o", str(obj)], check=True)
        subprocess.run(["ld65", "-C", str(config), "-o", str(directory / f"{name}.bin"),
                        "-m", str(directory / f"{name}.map"), str(obj), str(stub_obj)],
                       check=True)
        print(f"--- {name} ---", flush=True)
        subprocess.run(["od65", "--dump-segments", str(obj)], check=True)
        print((directory / f"{name}.map").read_text(), flush=True)
