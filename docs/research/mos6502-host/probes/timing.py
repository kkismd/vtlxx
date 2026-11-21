import subprocess,time,resource
h='/tmp/vtlxx-592-exp/harness/target/release/probe';prg='/tmp/vtlxx-592-exp/try.prg'
for name,src,delay in [('try',b'A=` ?=A','hold'),('wait',b'A=$ ?=A','wait'),('source',b'?=65','source'),('loop',b'*=1 [ A=` ]','loop')]:
 before=resource.getrusage(resource.RUSAGE_CHILDREN);t=time.monotonic();p=subprocess.Popen([h,prg,'10000000'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 frame=len(src).to_bytes(2,'little')+src
 p.stdin.write(frame[:3] if delay=='source' else frame);p.stdin.flush()
 if delay in ('wait','source'):
  time.sleep(.75);sent=time.monotonic();p.stdin.write(b'A' if delay=='wait' else frame[3:]);p.stdin.flush()
 else:sent=t
 if delay=='hold':
  p.wait(timeout=4);p.stdin.close();out=p.stdout.read();err=p.stderr.read()
 else:out,err=p.communicate()
 after=resource.getrusage(resource.RUSAGE_CHILDREN); print(name,'wall',round(time.monotonic()-t,4),'response',round(time.monotonic()-sent,4),'user',round(after.ru_utime-before.ru_utime,4),'sys',round(after.ru_stime-before.ru_stime,4),'out',repr(out),'err',repr(err),'status',p.returncode)
