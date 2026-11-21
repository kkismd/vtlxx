import subprocess, os
cli=os.environ['VTLXX6502']
src=os.environ['WAIT_FATAL_SOURCE']
env0=os.environ.copy();env0['PATH']=os.environ['CC65_BIN']+':'+env0['PATH']
for label,n in [('EOF',None),('header-low',0),('header-high',1),('payload-first',2),('payload-last',9),('runtime',10)]:
 env=env0.copy()
 if n is not None: env['SIM65_WAIT_FAIL_AFTER']=str(n)
 p=subprocess.run([cli,src,'--interactive'],stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env,timeout=5)
 expected=(0,b'-1') if n is None else (6,b'')
 assert (p.returncode,p.stdout)==expected,(label,p.returncode,p.stdout,p.stderr)
 if n is None:
  assert p.stderr==b'',(label,p.stderr)
 else:
  assert b'target halted with status 6' in p.stderr,(label,p.stderr)
 print(label,'status',p.returncode,'stdout',repr(p.stdout),'stderr',repr(p.stderr))
for n in [None,0]:
 env=env0.copy()
 if n is not None: env['SIM65_WAIT_FAIL_AFTER']=str(n)
 r,w=os.pipe();os.write(w,b'Z');os.close(w)
 p=subprocess.run([os.environ['SIM65_V218'],os.environ['WAIT_FATAL_IMAGE']],stdin=r,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env,timeout=5)
 remaining=os.read(r,1);os.close(r)
 expected=(99,b'',b'',b'') if n is None else (6,b'K',b'',b'Z')
 assert (p.returncode,p.stdout,p.stderr,remaining)==expected,(n,p.returncode,p.stdout,p.stderr,remaining)
 print('probe fail_after',n,'status',p.returncode,'stdout',repr(p.stdout),'stderr',repr(p.stderr),'remaining',repr(remaining))
