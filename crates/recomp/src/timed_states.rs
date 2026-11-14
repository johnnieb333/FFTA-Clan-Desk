//! Ten rotating full-machine snapshots, isolated from manual and native saves.
use std::{fs,path::{Path,PathBuf},time::{Duration,Instant,SystemTime},io::Write};
pub struct TimedStates{dir:PathBuf,quick:PathBuf,pub minutes:u16,last:Option<Instant>,elapsed:Duration}
fn valid_minutes(n:u16)->bool{[0,1,2,5,10].contains(&n)}
fn stamp(path:&Path)->Option<SystemTime>{fs::metadata(path).ok()?.modified().ok()}
fn time_label(path:&Path)->String{stamp(path).and_then(|t|t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d|{let c=gba_core::hostclock::from_linear(d.as_secs()as i64);format!("{:02}/{:02} {:02}:{:02} UTC",c.day,c.month,c.hour,c.min)}).unwrap_or_else(||"Empty".into())}
impl TimedStates{
 pub fn new(save_dir:&Path,hash:&str)->Self{
  let dir=save_dir.join("AutoStates").join(hash);let minutes=fs::read_to_string(dir.join("timer.cfg")).ok().and_then(|s|s.trim().parse::<u16>().ok()).filter(|&n|valid_minutes(n)).unwrap_or(5);
  Self{dir,quick:save_dir.join("QuickSave"),minutes,last:None,elapsed:Duration::ZERO}
 }
 fn path(&self,slot:usize)->PathBuf{self.dir.join(format!("auto-{slot:02}.state"))}
 pub fn reset_clock(&mut self){self.last=None;self.elapsed=Duration::ZERO;}
 pub fn pause(&mut self){self.last=None;}
 pub fn set_minutes(&mut self,n:u16)->Result<(),String>{
  if !valid_minutes(n){return Err("Choose Off, 1, 2, 5 or 10 minutes".into());}
  fs::create_dir_all(&self.dir).map_err(|e|e.to_string())?;let path=self.dir.join("timer.cfg");let temp=self.dir.join("timer.tmp");
  let mut f=fs::File::create(&temp).map_err(|e|e.to_string())?;write!(f,"{n}\n").and_then(|_|f.sync_all()).map_err(|e|e.to_string())?;drop(f);fs::rename(temp,path).map_err(|e|e.to_string())?;
  self.minutes=n;self.reset_clock();Ok(())
 }
 pub fn rows(&self)->Vec<(usize,String)>{
  let mut ids:Vec<_>=(1..=10).collect();ids.sort_by(|&a,&b|stamp(&self.path(b)).cmp(&stamp(&self.path(a))).then(a.cmp(&b)));
  ids.into_iter().map(|id|(id,format!("Auto {id:02}   {}",time_label(&self.path(id))))).collect()
 }
 pub fn bytes(&self,id:usize)->Result<Vec<u8>,String>{
  if id==0{return crate::autosave::recovery_state(&self.quick);}
  if !(1..=10).contains(&id){return Err("Invalid autosave slot".into());}
  [self.path(id),self.path(id).with_extension("bak")].iter().find_map(|p|{
   if fs::metadata(p).ok()?.len()>8*1024*1024{return None;}let b=fs::read(p).ok()?;gba_core::state::preview(&b).ok()?;Some(b)
  }).ok_or_else(||"No valid autosave in this slot".into())
 }
 pub fn card(&self,id:usize)->menu::StateCard{
  let mut c=menu::StateCard::default();if let Ok(b)=self.bytes(id){if let Ok((_,image))=gba_core::state::preview(&b){c.image=image;c.occupied=true;}}
  c.saved=if id==0{format!("Existing QuickSave / {}",time_label(&self.quick))}else{format!("Auto {id:02} / {}",time_label(&self.path(id)))};c
 }
 fn capture(&self,m:&gba_core::Machine)->Result<usize,String>{
  fs::create_dir_all(&self.dir).map_err(|e|e.to_string())?;
  let slot=(1..=10).find(|&i|!self.path(i).exists()).unwrap_or_else(||(1..=10).min_by_key(|&i|stamp(&self.path(i))).unwrap());
  let bytes=m.save_state();let path=self.path(slot);let temp=path.with_extension("tmp");
  if path.is_file(){let old=fs::read(&path).map_err(|e|e.to_string())?;gba_core::state::preview(&old).map_err(|e|format!("Preserving damaged autosave: {e}"))?;
   if old.get(8..72)!=bytes.get(8..72){return Err("Existing autosave belongs to another ROM or BIOS".into());}
   fs::copy(&path,path.with_extension("bak")).map_err(|e|e.to_string())?;
  }
  let mut file=fs::File::create(&temp).map_err(|e|e.to_string())?;file.write_all(&bytes).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);fs::rename(temp,path).map_err(|e|e.to_string())?;Ok(slot)
 }
 pub fn tick(&mut self,m:&gba_core::Machine,eligible:bool,now:Instant)->Option<Result<usize,String>>{
  let previous=self.last.replace(now);if self.minutes==0{self.elapsed=Duration::ZERO;return None;}
  if let Some(last)=previous{self.elapsed+=now.saturating_duration_since(last).min(Duration::from_secs(1));}
  if self.elapsed<Duration::from_secs(self.minutes as u64*60)||!eligible{return None;}
  self.elapsed=Duration::ZERO;Some(self.capture(m))
 }
}
#[cfg(test)]mod save_browser_tests{
 use super::*;
 fn fixture()->(PathBuf,TimedStates,gba_core::Machine){let p=std::env::temp_dir().join(format!("ffta-auto-{}-{}",std::process::id(),SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos()));let m=gba_core::Machine::new(vec![0;256]);let a=TimedStates::new(&p,"test");(p,a,m)}
 #[test]fn save_browser_ten_slots_timer_pause_reload_and_preserve_manual(){
  let(dir,mut a,mut m)=fixture();assert_eq!(a.minutes,5);a.set_minutes(1).unwrap();let now=Instant::now();assert!(a.tick(&m,true,now).is_none());
  for i in 1..60{assert!(a.tick(&m,true,now+Duration::from_secs(i)).is_none());}assert!(a.tick(&m,true,now+Duration::from_secs(60)).unwrap().is_ok());
  a.pause();assert!(a.tick(&m,true,now+Duration::from_secs(600)).is_none());assert_eq!(a.elapsed,Duration::ZERO);
  let manual=crate::manual_states::ManualStates::new(&dir);manual.save(&m).unwrap();let original=fs::read(dir.join("ManualStates/slot-01.state")).unwrap();
  for i in 1..=12{m.bus.ewram[0x80]=i;a.capture(&m).unwrap();}
  assert_eq!((1..=10).filter(|&i|a.path(i).is_file()).count(),10);assert_eq!(fs::read(dir.join("ManualStates/slot-01.state")).unwrap(),original);
  let rows=a.rows();assert_eq!(rows.len(),10);for(id,_)in rows{let b=a.bytes(id).unwrap();let mut fresh=gba_core::Machine::new(vec![0;256]);fresh.load_state(&b).unwrap();}
  a.set_minutes(0).unwrap();assert_eq!(TimedStates::new(&dir,"test").minutes,0);assert!(a.set_minutes(3).is_err());fs::remove_dir_all(dir).unwrap();
 }
 #[test]fn save_browser_external_load_has_undo_and_rejects_corruption(){
  let(dir,a,mut m)=fixture();let manual=crate::manual_states::ManualStates::new(&dir);a.capture(&m).unwrap();m.bus.ewram[0x90]=42;let before=m.save_state();manual.load_external(&mut m,&a.bytes(1).unwrap()).unwrap();assert_eq!(m.bus.ewram[0x90],0);manual.load(&mut m,true).unwrap();assert_eq!(m.save_state(),before);assert!(manual.load_external(&mut m,b"bad").is_err());assert_eq!(m.save_state(),before);fs::remove_dir_all(dir).unwrap();
 }
}

#[cfg(test)]mod save_browser_guard_tests{
 use super::*;
 #[test]fn save_browser_waits_for_gameplay_and_quicksave_recovery_is_read_only(){
  let dir=std::env::temp_dir().join(format!("ffta-auto-guard-{}-{}",std::process::id(),SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos()));let mut m=gba_core::Machine::new(vec![0;256]);let original=m.save_state();let mut a=TimedStates::new(&dir,"test");a.set_minutes(1).unwrap();
  fs::write(&a.quick,&original).unwrap();let now=Instant::now();for i in 0..=60{assert!(a.tick(&m,false,now+Duration::from_secs(i)).is_none());}assert!(a.tick(&m,true,now+Duration::from_secs(61)).unwrap().is_ok());assert_eq!(fs::read(&a.quick).unwrap(),original);assert_eq!(a.bytes(0).unwrap(),original);
  fs::write(a.quick.with_file_name("QuickSave.bak"),&original).unwrap();fs::write(&a.quick,b"corrupt").unwrap();assert_eq!(a.bytes(0).unwrap(),original);assert_eq!(fs::read(&a.quick).unwrap(),b"corrupt");
  let other=gba_core::Machine::new(vec![1;256]).save_state();let before=m.save_state();let manual=crate::manual_states::ManualStates::new(&dir);assert!(manual.load_external(&mut m,&other).is_err());assert_eq!(m.save_state(),before);fs::remove_dir_all(dir).unwrap();
 }
}
