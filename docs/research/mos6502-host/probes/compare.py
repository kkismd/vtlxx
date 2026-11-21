import subprocess,time,resource
p='/tmp/vtlxx-592-exp/vtlxx.prg'; mos='/tmp/vtlxx-592-exp/harness/target/release/probe';vr='/tmp/vtlxx-592-exp/vr'
for cmd in [['sim65',p],[mos,p],[vr,p]]:
 for label,frame in [('short-header',b'\x01'),('short-source',b'\x04\x00?=6')]:
  x=subprocess.run(cmd,input=frame,capture_output=True);print(cmd[0],label,repr(x.stdout),repr(x.stderr),x.returncode)
 for label,src in [('wait',b'A=$ ?=A'),('source',b'?=65')]:
  before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic();c=subprocess.Popen(cmd,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE);frame=len(src).to_bytes(2,'little')+src;c.stdin.write(frame if label=='wait' else frame[:3]);c.stdin.flush();time.sleep(.75);sent=time.monotonic();c.stdin.write(b'A' if label=='wait' else frame[3:]);c.stdin.flush();o,e=c.communicate();end=time.monotonic();after=resource.getrusage(resource.RUSAGE_CHILDREN);print(cmd[0],label,'wall',round(end-start,4),'user',round(after.ru_utime-before.ru_utime,4),'sys',round(after.ru_stime-before.ru_stime,4),'response',round(end-sent,4),repr(o),repr(e),c.returncode)
