#!/usr/bin/env python3
import os, subprocess, sys, time
from pathlib import Path
sim=os.environ['SIM65_BIN']
time_file=os.environ.get('SIM65_TIME_FILE', str(Path(__file__).resolve().parent.parent / 'results' / 'source-delay-time.txt'))
fd=0
header=b''
while len(header)<2:
    chunk=os.read(fd,2-len(header))
    if not chunk: sys.exit(125)
    header+=chunk
length=header[0] | (header[1]<<8)
child=subprocess.Popen(['/usr/bin/time','-f','wall=%e user=%U sys=%S','-o',time_file,sim,'-c',*sys.argv[1:]],stdin=subprocess.PIPE)
child.stdin.write(header); child.stdin.flush()
time.sleep(0.25)
left=length
while left:
    chunk=os.read(fd,min(left,4096))
    if not chunk: child.kill(); sys.exit(125)
    child.stdin.write(chunk); child.stdin.flush(); left-=len(chunk)
child.stdin.close()
sys.exit(child.wait())
