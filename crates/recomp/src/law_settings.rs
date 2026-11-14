//! Law penalties are a host preference, never a saved card or clan edit.
use std::{fs,path::Path};
fn parse(s:&str)->Result<u8,String>{let n=s.trim().strip_prefix("mode=").ok_or("Invalid law setting")?.parse::<u8>().map_err(|_|"Invalid law mode")?;if n>1{return Err("Invalid law mode".into());}Ok(n)}
pub fn load(p:&Path)->Result<Option<u8>,String>{match fs::read_to_string(p){Ok(s)=>parse(&s).map(Some),Err(e)if e.kind()==std::io::ErrorKind::NotFound=>Ok(None),Err(e)=>Err(e.to_string())}}
pub fn save(p:&Path,mode:u8)->Result<(),String>{if mode>1{return Err("Invalid law mode".into());}if let Some(parent)=p.parent(){fs::create_dir_all(parent).map_err(|e|e.to_string())?;}let tmp=p.with_extension("cfg.tmp");fs::write(&tmp,format!("mode={mode}\n")).map_err(|e|e.to_string())?;if p.exists(){fs::copy(p,p.with_extension("cfg.bak")).map_err(|e|e.to_string())?;}fs::rename(tmp,p).map_err(|e|e.to_string())}
#[cfg(test)]mod law_penalties_settings_tests{use super::*;
 #[test]fn law_penalties_preferences_persist_and_restore_default(){let dir=std::env::temp_dir().join(format!("ffta-law-penalties-test-{}",std::process::id()));let p=dir.join("law-penalties.cfg");for mode in 0..=1{save(&p,mode).unwrap();assert_eq!(load(&p),Ok(Some(mode)));}save(&p,0).unwrap();assert_eq!(load(&p),Ok(Some(0)));for s in["mode=2","mode=-1","garbage","mode=1\nmode=2"]{assert!(parse(s).is_err());}fs::remove_dir_all(dir).unwrap();}
}
