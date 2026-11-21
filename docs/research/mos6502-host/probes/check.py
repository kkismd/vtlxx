import subprocess,time,resource
h='/tmp/vtlxx-592-exp/harness/target/release/probe';base='/tmp/vtlxx-592-exp/vtlxx.prg';alt='/tmp/vtlxx-592-exp/try.prg'
def run(name,exe,src,inp=b'',limit='200000000'):
 frame=len(src).to_bytes(2,'little')+src+inp;p=subprocess.run([h,exe,limit],input=frame,capture_output=True);print(name,repr(p.stdout),repr(p.stderr),p.returncode)
for prog in [base,alt]:
 for n,s,i in [('basic',b'?=65',b''),('wait',b'A=$ ?=A',b'\0A'),('fail',b'?',b''),('eof',b'A=$ B=$ ?=A ?=B',b''),('high',b'A=$ ?=A',b'\xff'),('trybyte',b'A=` ?=A',b'\xff'),('trynul',b'A=` ?=A',b'\0'),('tryeof',b'A=` B=` ?=A ?=B',b'')]:run(prog.split('/')[-1]+' '+n,prog,s,i)
 p=subprocess.run([h,prog,'1'],input=b'\x04\0?=65',capture_output=True);print('limit',repr(p.stdout),repr(p.stderr),p.returncode)
for prog in [base,alt]:
 p=subprocess.Popen([h,prog],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE);s=b'A=$ ?=A';p.stdin.write(len(s).to_bytes(2,'little')+s);p.stdin.flush(); t=time.monotonic();time.sleep(.75);p.stdin.write(b'A');p.stdin.flush();out,err=p.communicate();print('delay',prog,time.monotonic()-t,repr(out),repr(err),p.returncode)
