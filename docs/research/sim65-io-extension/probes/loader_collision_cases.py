#!/usr/bin/env python3
from pathlib import Path
import os, subprocess
root=Path(__file__).resolve().parent
template=Path(os.environ['SIM65_TEMPLATE_IMAGE'])
bins=[Path(os.environ[k]) for k in ('SIM65_STOCK','SIM65_V218','SIM65_MASTER','SIM65_V218_RESERVED','SIM65_MASTER_RESERVED')]
orig=template.read_bytes(); h=bytearray(orig[:12]); start=0xffe0; h[8:10]=h[10:12]=start.to_bytes(2,'little')
for end in (0xffef,0xfff0,0xfff1,0xfff3,0xfff4):
 body=bytes([0xa9,42,0x4c,0xf9,0xff])+bytes([0xea])*(end-start-4); (root/f'end-{end:04x}.bin').write_bytes(h+body)
code=bytes([0xa2,0xff,0x9a,0xa9,42,0x20,0xf0,0xff,0x4c,0xf9,0xff]); (root/'collision.bin').write_bytes(h+code+bytes([0xea])*(16-len(code))+bytes([0x60]))
for f in ['end-ffef.bin','end-fff0.bin','end-fff1.bin','end-fff3.bin','end-fff4.bin','collision.bin']:
 for sim in bins:
  p=subprocess.run([str(sim),str(root/f)],input=b'B',capture_output=True,timeout=3);print(f,sim,'status',p.returncode,'stderr',p.stderr.decode().strip())
