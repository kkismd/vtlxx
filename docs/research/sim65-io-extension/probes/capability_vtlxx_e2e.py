#!/usr/bin/env python3
import subprocess, os
from pathlib import Path
here=Path(__file__).resolve().parent
s=b'A=` B=$ ?=A ?=B'
frame=len(s).to_bytes(2,'little')+s+b'AZ'
for name,sim in [('v218',Path(os.environ['SIM65_V218'])),('master',Path(os.environ['SIM65_MASTER'])),('stock',Path(os.environ['SIM65_STOCK']))]:
    p=subprocess.run([str(here/'capability_launcher.py'),str(sim),os.environ['VTLXX_IMAGE']],input=frame,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=3)
    expected=(0,b'6590') if name != 'stock' else (125,b'')
    assert (p.returncode,p.stdout)==expected
    print(name,'status',p.returncode,'stdout',repr(p.stdout),'stderr',repr(p.stderr))
