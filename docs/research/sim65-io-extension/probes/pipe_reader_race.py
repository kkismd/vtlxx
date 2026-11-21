#!/usr/bin/env python3
import os, select, threading, time, fcntl, errno
r,w=os.pipe(); copied=os.dup(r)
assert not (fcntl.fcntl(r,fcntl.F_GETFL)&os.O_NONBLOCK)
os.write(w,b'A')
p=select.poll(); p.register(r,select.POLLIN)
events=p.poll(0)
assert events and events[0][1]&select.POLLIN
stolen=os.read(copied,1)
started=threading.Event(); done=threading.Event(); result=[]
def victim():
 started.set(); result.append(os.read(r,1)); done.set()
t=threading.Thread(target=victim); t.start(); started.wait(); time.sleep(.1)
blocked=not done.is_set()
os.write(w,b'B'); t.join(timeout=1)
print(f'poll={events!r} competitor={stolen!r} victim_blocked_100ms={blocked} victim_after_next_write={result!r} writer_open=True')
assert blocked and result==[b'B']
# O_NONBLOCK is shared by descriptors created by dup: affects blocking Wait too.
flags=fcntl.fcntl(copied,fcntl.F_GETFL);fcntl.fcntl(copied,fcntl.F_SETFL,flags|os.O_NONBLOCK)
print(f'dup_sets_original_nonblock={bool(fcntl.fcntl(r,fcntl.F_GETFL)&os.O_NONBLOCK)}')
try: os.read(r,1)
except BlockingIOError as e: print(f'empty_read_errno={e.errno} ({errno.errorcode[e.errno]})')
for fd in (copied,r,w): os.close(fd)
