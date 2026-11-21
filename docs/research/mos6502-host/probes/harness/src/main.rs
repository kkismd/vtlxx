use mos6502::{cpu::CPU,instruction::Nmos6502,memory::{Bus,Memory}};
use std::io::{Read,Write};
#[repr(C)] struct PollFd {fd:i32,events:i16,revents:i16}
unsafe extern "C" {fn poll(fds:*mut PollFd,nfds:usize,timeout:i32)->i32;}
fn word(m:&mut Memory,a:u16)->u16 {m.get_byte(a) as u16 | (m.get_byte(a.wrapping_add(1)) as u16)<<8}
fn main(){
 let args:Vec<String>=std::env::args().collect(); let b=std::fs::read(args.get(1).map(String::as_str).unwrap_or("/tmp/vtlxx-592-exp/vtlxx.prg")).unwrap();
 assert!(b.len()>=12 && &b[..7]==b"sim65\x02\x00"); let zp=b[7] as u16; let load=u16::from_le_bytes([b[8],b[9]]);let entry=u16::from_le_bytes([b[10],b[11]]); assert!((load as usize)+b.len()-12<=0xfff1);
 let mut m=Memory::new(); for(i,x)in b[12..].iter().enumerate(){m.set_byte(load+i as u16,*x)} let mut c=CPU::new(m,Nmos6502);c.registers.program_counter=entry;c.registers.stack_pointer.0=0xff;
 let limit=args.get(2).map(|x|x.parse::<u64>().unwrap()).unwrap_or(u64::MAX);let mut hooks=[0u64;9];
 loop{let pc=c.registers.program_counter;if c.cycles>=limit {eprintln!("limit cycles={} hooks={hooks:?}",c.cycles);std::process::exit(126)}
 if pc==0xffef || pc==0xfff0 { let mut p=PollFd{fd:0,events:1,revents:0}; let ready=if pc==0xffef {unsafe{poll(&mut p,1,0)}} else {1}; let mut byte=[0u8;1]; let result=if pc==0xffef && std::env::var_os("PROBE_FATAL").is_some() {3} else if ready==0 {1} else if ready<0 {3} else {match std::io::stdin().read(&mut byte){Ok(0)=>2,Ok(_)=>0,Err(_)=>3}}; c.registers.accumulator=byte[0]; c.registers.index_x=result; c.registers.stack_pointer.increment(); let lo=c.memory.get_byte(c.registers.stack_pointer.to_u16());c.registers.stack_pointer.increment();let hi=c.memory.get_byte(c.registers.stack_pointer.to_u16());c.registers.program_counter=u16::from_le_bytes([lo,hi]).wrapping_add(1); continue; }
 if (0xfff1..=0xfff9).contains(&pc){hooks[(pc-0xfff1) as usize]+=1; if pc==0xfff9{eprintln!("cycles={} hooks={hooks:?}",c.cycles);std::process::exit(c.registers.accumulator as i32)}
 if pc!=0xfff6 && pc!=0xfff7 {eprintln!("unsupported {pc:04x} {:?}",c.registers);std::process::exit(125)}
 let count=c.registers.accumulator as u16|((c.registers.index_x as u16)<<8);let sp=word(&mut c.memory,zp);let buf=word(&mut c.memory,sp);let fd=word(&mut c.memory,sp.wrapping_add(2));let next=sp.wrapping_add(4);c.memory.set_byte(zp,next as u8);c.memory.set_byte(zp+1,(next>>8) as u8);
 let mut data=vec![0;count as usize];let n=if pc==0xfff6 && fd==0 {std::io::stdin().read(&mut data).unwrap()} else if pc==0xfff7 && (fd==1||fd==2){for i in 0..count{data[i as usize]=c.memory.get_byte(buf.wrapping_add(i));} if fd==1{std::io::stdout().write_all(&data).unwrap()}else{std::io::stderr().write_all(&data).unwrap()} count as usize}else{eprintln!("unsupported fd={fd}");std::process::exit(125)};
 if pc==0xfff6{for(i,x)in data[..n].iter().enumerate(){c.memory.set_byte(buf.wrapping_add(i as u16),*x)}} c.registers.accumulator=n as u8;c.registers.index_x=(n>>8) as u8;
 c.registers.stack_pointer.increment();let lo=c.memory.get_byte(c.registers.stack_pointer.to_u16());c.registers.stack_pointer.increment();let hi=c.memory.get_byte(c.registers.stack_pointer.to_u16());c.registers.program_counter=u16::from_le_bytes([lo,hi]).wrapping_add(1);
 }else{c.single_step();}}
}
