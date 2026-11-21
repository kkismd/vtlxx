#!/usr/bin/env python3
"""Reconstructed #596 runner; all inputs are automated and files stay in scratch."""
import os
from pathlib import Path
import resource
import subprocess
import sys
import time

work = Path(sys.argv[1])
image = work / 'image'
host = work / 'harness/target/release/probe'
failure = work / 'failure-harness/target/release/probe'

def frame(source, tail=b''):
    return len(source).to_bytes(2, 'little') + source + tail

def check(name, cmd, data, expected, status=0, env=None, stderr_contains=None):
    proc = subprocess.run([str(x) for x in cmd], input=data, capture_output=True,
                          env={**os.environ, **(env or {})}, timeout=20)
    assert (proc.stdout, proc.returncode) == (expected, status), (name, proc)
    if stderr_contains is not None:
        assert stderr_contains in proc.stderr, (name, proc.stderr)
    print(name, 'stdout=', repr(proc.stdout), 'stderr=', repr(proc.stderr), 'status=', proc.returncode, flush=True)

stock = image / 'vtlxx.prg'
alt = image / 'try.prg'
for runner in (['sim65'], [host]):
    check(f'{runner[0]} basic', runner + [stock], frame(b'?=65'), b'65')
    check(f'{runner[0]} wait/nul', runner + [stock], frame(b'A=$ ?=A', b'\0A'), b'65')
    check(f'{runner[0]} syntax', runner + [stock], frame(b'?'), b'', 130)
    check(f'{runner[0]} truncated', runner + [stock], b'\x04\0?=6', b'', 129)
check('stock cycle', ['sim65', '-c', stock], frame(b'?=65'), b'65466196 cycles\n')
check('mos cycle', [host, stock], frame(b'?=65'), b'65', stderr_contains=b'cycles=466289')
check('stock adapter try unsupported', [host, stock], frame(b'A=` ?=A', b'A'), b'', 6)
for source, data, output in [(b'A=` ?=A', b'\0', b'0'),
                             (b'A=` ?=A', b'\xff', b'255'),
                             (b'A=` B=` ?=A ?=B', b'', b'-1-1'),
                             (b'A=` B=$ C=` ?=A ?=B ?=C', b'ABC', b'656667')]:
    check('try ' + repr(source) + repr(data), [host, alt], frame(source, data), output)
for fixture, code, env in [('fatal', 6, {'PROBE_FATAL': '1'}),
                           ('fatal-wait', 6, {'PROBE_WAIT_FATAL': '1'}),
                           ('capacity', 2, {}), ('wait-capacity', 2, {})]:
    check(fixture, [failure, image / (fixture + '.prg')], b'A', b'K', code, env)
source = b'A=$ ?=A'
check('source read failure', [failure, stock], frame(source, b'A'), b'', 120,
      {'FAIL_READ_AT': '3'}, b'HOST_IO_ERROR read')
check('wait read failure', [failure, stock], frame(source, b'A'), b'', 120,
      {'FAIL_READ_AT': '10'}, b'depth=0 eof=0')
check('read error mistaken for eof', [failure, stock], frame(source, b'A'), b'-1', 0,
      {'FAIL_READ_AT': '10', 'RETURN_ERROR': '1'})
check('write failure injection', [failure, stock], frame(b'?=65'), b'', 121,
      {'FAIL_WRITE': '1'}, b'HOST_IO_ERROR write')
if Path('/dev/full').exists():
    with open('/dev/full', 'wb', buffering=0) as full:
        proc = subprocess.run([str(failure), str(stock)], input=frame(b'?=65'), stdout=full, stderr=subprocess.PIPE)
    assert proc.returncode == 121 and b'HOST_IO_ERROR write' in proc.stderr
    print('/dev/full', repr(proc.stderr), proc.returncode, flush=True)
    with open('/dev/full', 'wb', buffering=0) as full:
        old = subprocess.run([str(host), str(stock)], input=frame(b'?=65'), stdout=full, stderr=subprocess.PIPE)
    assert old.returncode == 0, (old.stderr, old.returncode)
    print('old buffered /dev/full', repr(old.stderr), old.returncode, flush=True)
fd = os.open('/tmp', os.O_RDONLY | os.O_DIRECTORY)
try:
    proc = subprocess.run([str(failure), str(image / 'fatal.prg')], stdin=fd, capture_output=True)
    assert proc.returncode == 6 and proc.stdout == b'K'
    print('OS read error', repr(proc.stdout), repr(proc.stderr), proc.returncode, flush=True)
finally:
    os.close(fd)

# Keep the pipe open to distinguish NotReady from EOF.
proc = subprocess.Popen([str(host), str(alt)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
proc.stdin.write(frame(b'A=` ?=A')); proc.stdin.flush()
proc.wait(timeout=5)
stdout, stderr = proc.stdout.read(), proc.stderr.read()
assert proc.returncode == 0 and stdout == b'0', (stdout, stderr, proc.returncode)
proc.stdin.close()
print('try not ready', repr(stdout), repr(stderr), proc.returncode, flush=True)

for label, cmd, program in [('sim65', ['sim65'], stock), ('mos', [host], stock), ('mos adapter', [host], alt)]:
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.monotonic()
    proc = subprocess.Popen([str(x) for x in cmd + [program]], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    proc.stdin.write(frame(source)); proc.stdin.flush()
    time.sleep(.75); sent = time.monotonic()
    proc.stdin.write(b'A'); proc.stdin.flush()
    out, err = proc.communicate(timeout=10)
    elapsed = time.monotonic() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    assert out == b'65' and proc.returncode == 0
    print(label, 'wall', round(elapsed, 4), 'response', round(time.monotonic()-sent, 4),
          'user', round(after.ru_utime-before.ru_utime, 4), 'system', round(after.ru_stime-before.ru_stime, 4),
          'stderr', repr(err), flush=True)
    assert elapsed >= .70

proc = subprocess.Popen([str(host), str(alt)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
payload = frame(b'?=65')
start = time.monotonic()
proc.stdin.write(payload[:3]); proc.stdin.flush(); time.sleep(.75)
sent = time.monotonic(); proc.stdin.write(payload[3:]); proc.stdin.flush()
out, err = proc.communicate(timeout=10)
assert out == b'65' and proc.returncode == 0
print('delayed source', 'wall', round(time.monotonic()-start, 4), 'response', round(time.monotonic()-sent, 4), repr(err), flush=True)

for runner, limit, status in [(['sim65', '-x'], 466196, 126), (['sim65', '-x'], 466197, 0)]:
    check('sim65 limit ' + str(limit), runner + [str(limit), stock], frame(b'?=65'), b'65', status)
for limit, status in [(466289, 126), (466290, 0)]:
    check('mos limit ' + str(limit), [host, stock, str(limit)], frame(b'?=65'), b'65', status)
check('try busy limit', [host, alt, '10000000'], frame(b'*=1 [ A=` ]'), b'', 126, stderr_contains=b'limit cycles=')
subprocess.run([str(work / 'poll')], check=True)
print('PASS: representative cases', flush=True)
