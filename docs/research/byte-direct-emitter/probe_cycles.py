#!/usr/bin/env python3
"""Measure complete sim65 backend probe runs (startup through exit)."""
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[3] / "crates/vtlxx"
SOURCES = ["tests/fixtures/sim65/backend_probe.s", "6502/compiler/session.s",
           "6502/compiler/arena.s", "6502/compiler/emitter.s", "6502/compiler/owner.s",
           "6502/compiler/binding.s", "6502/target/zp_layout.s", "6502/runtime/state.s",
           "6502/runtime/primitives.s", "6502/runtime/arithmetic.s", "6502/target/sim65_adapter.s"]


def main():
    with tempfile.TemporaryDirectory(prefix="vtlxx-707-cycles-") as tmp:
        directory = pathlib.Path(tmp)
        for case in (58, 59):
            objects = []
            for index, source in enumerate(SOURCES):
                obj = directory / f"{index}.o"
                args = ["ca65", "-t", "sim6502", "-I", str(ROOT / "tests/fixtures/sim65")]
                if index == 0:
                    args += ["-D", f"TEST_CASE={case}"]
                subprocess.run(args + [str(ROOT / source), "-o", str(obj)], check=True)
                objects.append(str(obj))
            program = directory / "probe.prg"
            subprocess.run(["ld65", "-t", "sim6502", "-o", str(program)] + objects +
                           ["sim6502.lib"], check=True)
            result = subprocess.run(["sim65", "-c", "-x", "30000000", str(program)],
                                    check=True, capture_output=True, text=True)
            print(f"case {case}: stdout={result.stdout.strip()!r} {result.stderr.strip()}")


if __name__ == "__main__":
    main()
