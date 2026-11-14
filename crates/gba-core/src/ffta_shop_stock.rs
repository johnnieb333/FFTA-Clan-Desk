//! Host stock preference changes only the native shop builder's level argument.
use crate::{ffta_rom,Machine};
const ENTRY:u32=0x080cf820;
#[derive(Clone,Copy,Debug)]pub struct State{verified:bool,tier:u8}
impl State{
 pub fn new(rom:&[u8])->Self{Self{verified:ffta_rom::version(rom)==Some(ffta_rom::Version::Europe)&&rom.get(0xcf820..0xcf82c)==Some(&[0xf0,0xb5,0x57,0x46,0x4e,0x46,0x45,0x46,0xe0,0xb4,0x85,0xb0]),tier:0}}
 pub fn supported(&self)->bool{self.verified}
 pub fn tier(&self)->u8{self.tier}
 pub fn set(&mut self,tier:u8)->bool{if tier>3||!self.verified{return false;}self.tier=tier;true}
 pub fn interprets(&self,pc:u32)->bool{self.verified&&self.tier!=0&&(pc&!1)==ENTRY}
}
pub fn before_instruction(m:&mut Machine){let s=m.bus.ffta_shop_stock;if m.cpu.thumb()&&s.interprets(m.cpu.regs[15]){m.cpu.regs[2]=[0,0,10,20][s.tier as usize];}}
#[cfg(test)]mod shop_stock_tests{
 use super::*;
 #[test]fn shop_stock_hook_changes_only_builder_level_argument(){
  let mut m=Machine::new(vec![0;1024]);m.bus.ffta_shop_stock=State{verified:true,tier:2};m.cpu.cpsr|=crate::cpu::FLAG_T;m.cpu.regs=[123;16];m.cpu.regs[15]=ENTRY;
  let before=m.save_state();let regs=m.cpu.regs;before_instruction(&mut m);for i in 0..16{assert_eq!(m.cpu.regs[i],if i==2{10}else{regs[i]});}
  m.cpu.regs=regs;m.bus.ffta_shop_stock.tier=0;before_instruction(&mut m);assert_eq!(m.save_state(),before);
  m.bus.ffta_shop_stock.tier=3;m.cpu.regs[15]=ENTRY+2;let regs=m.cpu.regs;before_instruction(&mut m);assert_eq!(m.cpu.regs,regs);assert!(!m.bus.ffta_shop_stock.set(4));
  assert!(!State::new(&[0;1024]).set(1));
 }
 fn list(rom:&[u8],snapshot:&[u8],tier:u8,level:u32,category:u32)->Vec<u16>{
  let mut m=Machine::new(rom.to_vec());m.load_state(snapshot).unwrap();assert!(m.bus.ffta_shop_stock.set(tier));
  m.bus.ewram[0x3e000..0x3e800].fill(0);let mut c=crate::Cpu::new();c.cpsr|=crate::cpu::FLAG_T|crate::cpu::FLAG_I;c.regs[..4].copy_from_slice(&[0x0203e000,category,level,0]);c.regs[13]=0x03007400;c.regs[14]=0x03007001;c.regs[15]=ENTRY;m.cpu=c;
  for _ in 0..2_000_000{if m.cpu.regs[15]==0x03007000{let n=m.cpu.regs[0]as usize;assert!(n<=375);assert_eq!(m.bus.rom,rom);return(0..n).map(|i|{let p=0x3e000+i*4;u16::from_le_bytes(m.bus.ewram[p..p+2].try_into().unwrap())}).collect();}m.step();}panic!("Shop builder did not return");
 }
 #[test]fn shop_stock_real_tiers_match_native_lists_without_clan_edits(){
  let(Ok(r),Ok(s))=(std::env::var("FFTA_TEST_ROM"),std::env::var("FFTA_TEST_STATE"))else{return;};let rom=std::fs::read(r).unwrap();let snapshot=std::fs::read(s).unwrap();
  for(tier,level)in[(1,0),(2,10),(3,20)]{let mut total=0;for category in 0..6{let native=list(&rom,&snapshot,0,level,category);let selected=list(&rom,&snapshot,tier,2,category);assert_eq!(selected,native);total+=selected.len();}println!("Shop stock tier {tier}: {total} items; all categories match native stock");}
 }
}
