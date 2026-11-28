#!/usr/bin/env python3
"""Compare raw product links at #707 baseline and the working tree.

Run from the repository root: python3 docs/research/byte-direct-emitter/product_sizes.py
Requires git, ca65 and ld65. Maps and objects are temporary; stdout records results.
"""
import pathlib
import re
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[3]
BASE = "68615c3ff7c13682e50ddd061ad1aa953ebe70c2"
CRATE = pathlib.Path("crates/vtlxx")
SOURCES = [
    "6502/target/sim65_cli.s", "6502/frontend/basic.s", "6502/frontend/source.s",
    "6502/compiler/session.s", "6502/compiler/arena.s", "6502/compiler/emitter.s",
    "6502/compiler/owner.s", "6502/compiler/binding.s", "6502/target/zp_layout.s",
    "6502/runtime/state.s", "6502/runtime/primitives.s", "6502/runtime/arithmetic.s",
    "6502/target/mos6502_adapter.s",
]
CONFIG = "6502/target/mos6502.cfg"


def run(args):
    return subprocess.run(args, cwd=ROOT, check=True, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, text=True).stdout


def build(directory, baseline):
    objects = []
    for index, source in enumerate(SOURCES):
        path = CRATE / source
        inp = directory / (str(index) + ".s")
        inp.write_bytes(subprocess.check_output(["git", "show", f"{BASE}:{path}"], cwd=ROOT)
                        if baseline else (ROOT / path).read_bytes())
        obj = directory / (str(index) + ".o")
        args = ["ca65", "-D", "VTLXX_RAW"]
        if source.endswith("mos6502_adapter.s"):
            args += ["-D", "SIM65_ENTROPY_UNAVAILABLE"]
        run(args + [str(inp), "-o", str(obj)])
        objects.append(obj)
    config = directory / "raw.cfg"
    path = CRATE / CONFIG
    config.write_bytes(subprocess.check_output(["git", "show", f"{BASE}:{path}"], cwd=ROOT)
                       if baseline else (ROOT / path).read_bytes())
    mapfile = directory / "raw.map"
    labels = directory / "raw.lbl"
    run(["ld65", "-C", str(config), "-D", "VTLXX_LOAD=0x0200", "-D",
         "VTLXX_SERVICE=0xffef", "-o", str(directory / "raw.bin"),
         "-m", str(mapfile), "-Ln", str(labels)] + [str(obj) for obj in objects])
    data = mapfile.read_text()
    print("\n" + ("baseline " + BASE if baseline else "working tree"))
    print(data.split("Segment list:")[1].split("Exports list")[0].strip())
    modules = data.split("Modules list:")[1].split("Segment list:")[0]
    for index in (4, 5, 9):
        match = re.search(rf"(?m)^{index}\.o:\n(?:(?:    ).*\n)*", modules)
        if match:
            print(match.group().strip())
    for line in labels.read_text().splitlines():
        if re.search(r"\.(cc_arena|cc_patch_table|zp_phase|rt_regs)$", line):
            print(line)


def main():
    print(run(["ca65", "--version"]).strip() or "ca65 version is printed on stderr")
    with tempfile.TemporaryDirectory(prefix="vtlxx-707-sizes-") as tmp:
        root = pathlib.Path(tmp)
        for name, baseline in (("base", True), ("current", False)):
            directory = root / name
            directory.mkdir()
            build(directory, baseline)


if __name__ == "__main__":
    main()
