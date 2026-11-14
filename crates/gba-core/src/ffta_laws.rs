//! Optional penalty bypass. Recommendation predicates, JP awards and laws stay native.
use crate::{ffta_rom,Machine};
const PENALTY:u32=0x08138688;
#[derive(Clone,Copy,Debug)]pub struct State{verified:bool,disabled:bool}
impl State{
 pub fn new(rom:&[u8])->Self{Self{verified:ffta_rom::version(rom)==Some(ffta_rom::Version::Europe)&&rom.get(0x138688..0x138694)==Some(&[0x70,0xb5,0x05,0x1c,0x06,0x98,0x08,0x99,0x00,0x06,0x06,0x0e]),disabled:false}}
 pub fn supported(&self)->bool{self.verified}
 pub fn disabled(&self)->bool{self.disabled}
 pub fn set(&mut self,disabled:bool)->bool{if !self.verified{return false;}self.disabled=disabled;true}
 pub fn interprets(&self,key:u32)->bool{self.verified&&self.disabled&&(key&!1)==PENALTY}
}
pub fn before_instruction(m:&mut Machine){
 if m.cpu.thumb()&&m.bus.ffta_laws.interprets(m.cpu.regs[15])&&m.cpu.regs[14]&1!=0{
  // Return "no penalty" before the native function touches JP/cards/status.
  // This function is not the shared law/recommendation predicate.
  m.cpu.regs[0]=0;m.cpu.regs[15]=m.cpu.regs[14]&!1;
 }
}
#[cfg(test)]mod law_penalties_tests{
 use super::*;
 #[test]fn law_penalties_hook_is_opt_in_and_does_not_intercept_recommendations(){
  let mut m=Machine::new(vec![0;1024]);m.bus.ffta_laws=State{verified:true,disabled:false};m.cpu.cpsr|=crate::cpu::FLAG_T;m.cpu.regs[15]=PENALTY;m.cpu.regs[14]=0x03007001;m.cpu.regs[0]=123;
  let before=m.save_state();before_instruction(&mut m);assert_eq!(m.save_state(),before);
  m.bus.ffta_laws.set(true);m.cpu.regs[15]=0x08138040;let before=m.save_state();before_instruction(&mut m);assert_eq!(m.save_state(),before);
  m.cpu.regs[15]=PENALTY;let regs=m.cpu.regs;let ram=m.bus.ewram.clone();before_instruction(&mut m);assert_eq!(m.cpu.regs[0],0);assert_eq!(m.cpu.regs[15],0x03007000);for i in 1..15{assert_eq!(m.cpu.regs[i],regs[i]);}assert_eq!(m.bus.ewram,ram);
  assert!(!State::new(&[0;1024]).set(true));
 }
 fn call(m:&mut Machine,pc:u32,args:[u32;4],extra:&[u32],disabled:bool)->u32{
  assert!(m.bus.ffta_laws.set(disabled));let mut c=crate::Cpu::new();c.cpsr|=crate::cpu::FLAG_T|crate::cpu::FLAG_I;c.regs[..4].copy_from_slice(&args);c.regs[13]=0x03007400;c.regs[14]=0x03007001;c.regs[15]=pc;m.cpu=c;
  // Machine::step can execute the return target in the bypassed step. A
  // private Thumb self-branch keeps that sentinel stable for this test only.
  m.bus.iwram[0x7000..0x7002].copy_from_slice(&0xe7feu16.to_le_bytes());
  for(i,n)in extra.iter().enumerate(){m.bus.iwram[0x7400+i*4..0x7404+i*4].copy_from_slice(&n.to_le_bytes());}
  for _ in 0..2_000_000{if m.cpu.regs[15]==0x03007000{return m.cpu.regs[0];}m.step();}panic!("Native law call did not return");
 }
 #[test]fn law_penalties_real_yellow_red_jp_and_recommendation_comparisons(){
  let(Ok(r),Ok(s))=(std::env::var("FFTA_TEST_ROM"),std::env::var("FFTA_TEST_STATE"))else{return;};let rom=std::fs::read(r).unwrap();let snapshot=std::fs::read(s).unwrap();let mut m=Machine::new(rom.clone());m.load_state(&snapshot).unwrap();let mut cases=0;
  for slot in 2..24{let p=0x80+slot*0x108;m.load_state(&snapshot).unwrap();if m.bus.ewram[p+4]!=1{continue;}
   for kind in 1..=3{for disabled in[false,true]{m.load_state(&snapshot).unwrap();m.bus.ewram[p+0xd6..p+0xd8].copy_from_slice(&3u16.to_le_bytes());m.bus.ewram[p+0x102..p+0x104].fill(0);m.bus.ewram[p+0x28..p+0x2a].fill(0);let before=m.bus.ewram.clone();
    let result=call(&mut m,PENALTY,[0x02000000+p as u32,0,0,0],&[0,0,kind,0,1],disabled);
    if disabled{assert_eq!(result,0);assert_eq!(m.bus.ewram,before);}else{assert_eq!(m.bus.ewram[p+0xd6],0);assert!(m.bus.ewram[p+0x102]>0||m.bus.ewram[p+0x103]>0);}
   }cases+=1;}
   for typ in[0u8,20]{for disabled in[false,true]{m.load_state(&snapshot).unwrap();m.bus.ewram[0x3f000..0x3f010].fill(0);m.bus.ewram[0x3f004]=typ;
    let result=call(&mut m,0x08138040,[0x02000000+p as u32,0x02000000+p as u32,0,0],&[0,0,0,0x0203f000],disabled);assert_eq!(result,if typ==20{1}else{0});
   }}println!("Law penalty and recommendation comparisons passed for slot {}",slot+1);
  }assert!(cases>0);assert_eq!(m.bus.rom,rom);
 }
}
