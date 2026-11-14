//! Native clone creation runs privately; commit copies only a validated member record.
use gba_core::{Machine,Cpu,cpu::{FLAG_T,FLAG_I}};
const START:usize=0x80;const STRIDE:usize=0x108;
fn half(b:&[u8],p:usize)->u16{u16::from_le_bytes(b[p..p+2].try_into().unwrap())}
fn word(b:&[u8],p:usize)->u32{u32::from_le_bytes(b[p..p+4].try_into().unwrap())}
fn eligible(r:&[u8],slot:usize)->bool{r.len()==STRIDE&&(2..24).contains(&slot)&&r[4]==1&&(1..=5).contains(&r[6])&&(1..=50).contains(&r[9])&&half(r,0x16)==0&&half(r,0x28)==0}
fn free(ram:&[u8])->Result<usize,String>{
 for s in 2..24{let p=START+s*STRIDE;let r=ram.get(p..p+STRIDE).ok_or("Roster memory unavailable")?;
  if r[4]==0&&word(r,0)==0{if !r.iter().enumerate().all(|(i,&b)|b==0||(i==0x104&&b==255)){return Err("Unused clan slot contains unverified data".into());}return Ok(s);}
 }Err("Clan is full (24 members). No member will be replaced.".into())
}
pub fn open(m:&Machine,expected:&menu::CharacterInfo)->Result<(menu::CharacterInfo,usize),String>{
 let r=crate::character_editor::read(m)?.into_iter().find(|r|r.slot==expected.slot).ok_or("Member is no longer present")?;
 if r.record!=expected.record{return Err("Member changed; reopen Clone member".into());}
 if !eligible(&r.record,r.slot-1){return Err("Select a regular member with no mission or status flags.".into());}
 let dest=free(&m.bus.ewram)?;Ok((r,dest+1))
}
fn call(m:&mut Machine,pc:u32,args:[u32;3])->Result<u32,String>{
 let mut c=Cpu::new();c.cpsr|=FLAG_T|FLAG_I;c.regs[..3].copy_from_slice(&args);c.regs[13]=0x03007400;c.regs[14]=0x03007001;c.regs[15]=pc;
 for _ in 0..2_000_000{if c.regs[15]==0x03007000{return Ok(c.regs[0]);}gba_core::exec::step_hle(&mut c,&mut m.bus);}
 Err("Native member calculation did not return; no clone applied".into())
}
pub fn prepare(m:&Machine,expected:&menu::CharacterInfo,destination:usize,name:&str)->Result<(usize,Vec<u8>,gba_core::ffta_names::NameBank),String>{
 let(r,slot)=open(m,expected)?;if slot!=destination{return Err("Roster changed; reopen Clone member".into());}
 let name=name.trim();if crate::character_editor::read(m)?.iter().any(|r|r.name.eq_ignore_ascii_case(name)){return Err("Choose a different name from the existing clan members".into());}
 let dest=slot-1;let mut bank=m.bus.ffta_names.clone();let ptr=bank.set(dest,name)?;
 let source=&r.record;let at=START+dest*STRIDE;let addr=0x02000000+at as u32;
 let snapshot=m.save_state();let mut private=Machine::new(m.bus.rom.clone());private.load_state(&snapshot)?;
 private.bus.ewram[0x3f000..0x3f040].fill(0);private.bus.ewram[0x3f000]=1;private.bus.ewram[0x3f001]=source[7];private.bus.ewram[0x3f002]=source[8];
 call(&mut private,0x080ccf9c,[addr,0x0203f000,source[9]as u32])?;
 let mut record=private.bus.ewram[at..at+STRIDE].to_vec();
 if record[4]!=1||record[6]!=source[6]||record[7]!=source[7]||record[9]!=source[9]||record[0x2a..0x34].iter().any(|&b|b!=0)||half(&record,0x16)!=0||half(&record,0x28)!=0{return Err("Native member initialization failed verification".into());}
 let count=source[0x34]as usize;if count==0||count>146||record[0x34]as usize!=count{return Err("Unverified ability layout".into());}
 record[0x18..0x28].copy_from_slice(&source[0x18..0x28]);record[0xd2..0xd4].copy_from_slice(&source[0xd2..0xd4]);record[0xd6..0xd8].copy_from_slice(&source[0xd6..0xd8]);record[10]=source[10];
 record[0x35..0x3d].copy_from_slice(&source[0x35..0x3d]);record[0x40..0x40+count].copy_from_slice(&source[0x40..0x40+count]);
 let base=word(&m.bus.rom,0x6df4d4+source[6]as usize*4).checked_sub(0x08000000).ok_or("Unverified ability table")?as usize;
 for index in 1..count{let p=base+index*8;let table=m.bus.rom.get(p..p+8).ok_or("Missing ability data")?;let raw=record[0x40+index];if table[6]!=0&&table[7]>0&&(raw&127)<table[7]{record[0x40+index]&=127;}}
 let hp=half(&record,0x1a);let mp=half(&record,0x1e);record[0x18..0x1a].copy_from_slice(&hp.to_le_bytes());record[0x1c..0x1e].copy_from_slice(&mp.to_le_bytes());record[..4].copy_from_slice(&ptr.to_le_bytes());
 // Discard every global initializer side effect before deriving movement/evade.
 private.load_state(&snapshot)?;private.bus.ffta_names=bank.clone();private.bus.ewram[at..at+STRIDE].copy_from_slice(&record);call(&mut private,0x080cdd30,[addr,0,0])?;
 let derived=&private.bus.ewram[at..at+STRIDE];if derived.iter().zip(&record).enumerate().any(|(i,(a,b))|a!=b&&!(0xfc..0x100).contains(&i)){return Err("Unexpected derived member changes".into());}record.copy_from_slice(derived);
 Ok((at,record,bank))
}
#[cfg(test)]mod clone_tests{
 use super::*;
 #[test]fn clone_guards_protected_dispatched_and_full_roster(){
  let mut r=vec![0;STRIDE];r[4]=1;r[6]=1;r[9]=3;assert!(eligible(&r,2));for id in[2,8]{r[4]=id;assert!(!eligible(&r,2));}r[4]=1;assert!(!eligible(&r,1));
  r[0x16]=1;assert!(!eligible(&r,2));r[0x16]=0;r[0x28]=4;assert!(!eligible(&r,2));
  let mut ram=vec![0;262144];assert_eq!(free(&ram).unwrap(),2);ram[START+2*STRIDE+20]=1;assert!(free(&ram).is_err());for s in 2..24{ram[START+s*STRIDE+4]=1;}assert!(free(&ram).is_err());
 }
 #[test]fn clone_real_native_records_state_reload_and_disk_undo(){
  let(Ok(rom_path),Ok(state_path))=(std::env::var("FFTA_TEST_ROM"),std::env::var("FFTA_TEST_STATE"))else{println!("Set FFTA_TEST_ROM and FFTA_TEST_STATE for real clone checks");return;};
  let rom=std::fs::read(rom_path).unwrap();let state=std::fs::read(state_path).unwrap();let mut m=Machine::new(rom.clone());m.load_state(&state).unwrap();let rows=crate::character_editor::read(&m).unwrap();let mut tested=0;
  for source in &rows{
   m.load_state(&state).unwrap();if !eligible(&source.record,source.slot-1){assert!(open(&m,source).is_err());continue;}
   let(_,slot)=open(&m,source).unwrap();let name=format!("Copy{}",source.slot);let before=m.save_state();
   for text in [&source.name[..],"","bad@name"]{assert!(prepare(&m,source,slot,text).is_err());}
   let mut stale=source.clone();stale.record[9]+=1;assert!(prepare(&m,&stale,slot,&name).is_err());
   let(at,record,bank)=prepare(&m,source,slot,&name).unwrap();assert_eq!(m.save_state(),before);
   let temp=std::env::temp_dir().join(format!("ffta-clone-test-{}-{}-{}",std::process::id(),source.slot,std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
   let states=crate::manual_states::ManualStates::new(&temp);states.backup_editor(&m).unwrap();let ram=m.bus.ewram.clone();m.bus.ewram[at..at+STRIDE].copy_from_slice(&record);m.bus.ffta_names=bank;
   assert!(m.bus.ewram.iter().zip(&ram).enumerate().all(|(i,(a,b))|a==b||(at..at+STRIDE).contains(&i)));
   assert!(record[0x2a..0x34].iter().all(|&b|b==0));assert_eq!(half(&record,0x28),0);assert_eq!(half(&record,0x16),0);
   for o in[0x1a,0x1e,0x20,0x22,0x24,0x26,0xd2,0xd6]{assert_eq!(half(&record,o),half(&source.record,o));}for i in 0..record[0x34]as usize{assert_eq!(record[0x40+i]&127,source.record[0x40+i]&127);}
   let after=crate::character_editor::read(&m).unwrap();assert_eq!(after.len(),rows.len()+1);assert_eq!(after.iter().find(|r|r.slot==slot).unwrap().name,name);
   let mut fresh=Machine::new(rom.clone());fresh.load_state(&m.save_state()).unwrap();assert_eq!(fresh.bus.ffta_names,m.bus.ffta_names);assert_eq!(fresh.bus.ewram,m.bus.ewram);
   // Query the original game's mastery predicate in an isolated copy.
   for job in 2u8..=43{if rom[0x6e5464+job as usize*52+4]!=source.record[6]{continue;}
    let old=call(&mut fresh,0x080d0d18,[0x02000000+(START+(source.slot-1)*STRIDE)as u32,job as u32,0]).unwrap();
    let new=call(&mut fresh,0x080d0d18,[0x02000000+at as u32,job as u32,0]).unwrap();assert_eq!(old,new);
   }
   let sum:u32=rows.iter().map(|r|r.level as u32).sum();
   assert_eq!(call(&mut fresh,0x080cdad8,[0,0,0]).unwrap(),((sum+source.level as u32)/(rows.len()as u32+1)).max(1));
   states.undo_editor(&mut m).unwrap();assert_eq!(m.save_state(),before);std::fs::remove_dir_all(temp).unwrap();tested+=1;println!("Clone source {}: private build, record, reload and disk Undo passed",source.slot);
  }assert!(tested>0);
 }
}
