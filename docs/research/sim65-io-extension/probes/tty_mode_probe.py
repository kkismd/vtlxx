#!/usr/bin/env python3
import os,pty,termios,tty,subprocess
sim=os.environ['SIM65_BIN']; image=os.environ['SIM65_IMAGE']
master,slave=pty.openpty(); attrs=termios.tcgetattr(slave); attrs[3] &= ~termios.ECHO; termios.tcsetattr(slave,termios.TCSANOW,attrs)
def probe():
 p=subprocess.run([sim,image],stdin=slave,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=2); return p.returncode,p.stdout.decode().strip(),p.stderr.decode()
os.write(master,b'A'); print('canonical before newline',probe()); os.write(master,b'\n'); print('canonical after newline first',probe()); print('canonical newline',probe()); tty.setraw(slave); os.write(master,b'B'); print('raw immediate',probe()); os.close(master); os.close(slave)
