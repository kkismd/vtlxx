import subprocess, pathlib, struct, tempfile

# Run from the repository root. Scratch files are outside the source tree.
root=pathlib.Path('crates/poc/extended-classic-vtl')
scratch = tempfile.TemporaryDirectory(prefix='micro-trek-measure-')
out = pathlib.Path(scratch.name)
sources=['target/sim65_cli','frontend/basic','frontend/source','compiler/arena','compiler/emitter','compiler/owner','compiler/binding','target/zp_layout','runtime/state','runtime/primitives','runtime/arithmetic','target/sim65_adapter']
objs=[]
for i,s in enumerate(sources):
 p=root/'6502'/f'{s}.s'; o=out/f'{i}.o'; subprocess.run(['ca65','-t','sim6502',str(p),'-o',str(o)],check=True); objs.append(str(o))
subprocess.run(['ld65','-t','sim6502','-o',str(out/'game'),'-m',str(out/'map'),'-Ln',str(out/'labels'),*objs,'sim6502.lib'],check=True)
print((out/'map').read_text().split('Segment list:', 1)[1].split('Exports list', 1)[0])
for line in (out/'labels').read_text().splitlines():
 if line.endswith((' .cc_arena', ' .cc_cursor', ' .cc_status', ' .fe_compile_status', ' .zp_phase')): print(line)
src=(root/'examples/micro_trek.vtl').read_bytes()
for name,inp in [('eof',b''),('displays',b'G\nL\nS\nR\n?\nX\n'),('retry',b'W\n1\n0\n'*30+b'bad\nY\n'+b'W\n1\n0\n'*30+b'N\n')]:
 p=subprocess.run(['sim65','-c','-x','4000000000',str(out/'game')],input=struct.pack('<H',len(src))+src+inp,capture_output=True,check=True)
 print(name, 'stdout_tail',repr(p.stdout[-100:]),'stderr',repr(p.stderr))
basic=(root/'6502/frontend/basic.s').read_text().replace('    lda entry\n    ldx entry+1\n    jsr rt_init','    .import serial_out, halt\n    .importzp cc_cursor\n    lda cc_cursor\n    jsr serial_out\n    lda cc_cursor+1\n    jsr serial_out\n    lda #0\n    jmp halt\n    lda entry\n    ldx entry+1\n    jsr rt_init',1)
(out/'basic-probe.s').write_text(basic)
subprocess.run(['ca65','-t','sim6502',str(out/'basic-probe.s'),'-o',str(out/'probe.o')],check=True)
objs[1]=str(out/'probe.o')
subprocess.run(['ld65','-t','sim6502','-o',str(out/'probe'),'-Ln',str(out/'probe.labels'),*objs,'sim6502.lib'],check=True)
p=subprocess.run(['sim65','-x','4000000000',str(out/'probe')],input=struct.pack('<H',len(src))+src,capture_output=True,check=True)
labels=(out/'probe.labels').read_text().splitlines(); arena=int(next(x.split()[1] for x in labels if x.endswith(' .cc_arena')),16)
cursor=int.from_bytes(p.stdout,'little'); print('probe',hex(arena),hex(cursor),'native',cursor-arena,'remaining',24576-(cursor-arena))
