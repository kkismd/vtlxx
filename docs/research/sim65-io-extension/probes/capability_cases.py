#!/usr/bin/env python3
import subprocess, os
from pathlib import Path
here=Path(__file__).resolve().parent
cases=[('v218',os.environ['SIM65_V218']),('master',os.environ['SIM65_MASTER']),('stock',os.environ['SIM65_STOCK']),('wrong-response',str(here/'fake_bad.py')),('timeout',str(here/'fake_hang.py')),('error-status',str(here/'fake_error.py'))]
for name, sim in cases:
    # ioerr.bin consumes exactly one byte and exits with its value.
    p=subprocess.run([str(here/'capability_launcher.py'),sim,os.environ['SIM65_BYTE_PROBE']],input=b'A',stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=3)
    expected=65 if name in ('v218','master') else 125
    assert p.returncode==expected,(name,p.returncode,p.stdout,p.stderr)
    print(name,'status',p.returncode,'stdout',repr(p.stdout),'stderr',repr(p.stderr))
