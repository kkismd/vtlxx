#!/usr/bin/env python3
"""Scratch launcher: fail closed before handing stdin to the actual guest."""
import os
import signal
import subprocess
import sys
from pathlib import Path

SIGNATURE = b"VTLXX-SIM65-TRY-ABI-1\n"
PROBE = os.environ.get("SIM65_CAPABILITY_IMAGE", str(Path(__file__).with_name("capability.bin")))

def check(sim, timeout=0.3):
    p = subprocess.Popen([sim, PROBE], stdin=subprocess.DEVNULL,
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                         start_new_session=True)
    try:
        out, err = p.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        p.communicate()
        return False, "timeout"
    if p.returncode != 0:
        return False, f"probe exit {p.returncode}; stderr={err[:256]!r}"
    if out != SIGNATURE:
        return False, f"unexpected probe output {out[:256]!r}"
    return True, "ok"

if __name__ == '__main__':
    if len(sys.argv) != 3:
        raise SystemExit('usage: launcher.py SIM65 IMAGE')
    try:
        ok, reason = check(sys.argv[1])
    except OSError as exc:
        ok, reason = False, str(exc)
    if not ok:
        print(f'capability rejected: {reason}', file=sys.stderr)
        raise SystemExit(125)
    os.execv(sys.argv[1], [sys.argv[1], sys.argv[2]])
