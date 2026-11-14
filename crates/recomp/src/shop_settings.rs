//! Shop stock is a host preference, never a clan or cartridge edit.
use std::{fs,path::Path};
fn parse(s:&str)->Result<u8,String>{let n=s.trim().strip_prefix("tier=").ok_or("Invalid shop setting")?.parse::<u8>().map_err(|_|"Invalid stock tier")?;if n>3{return Err("Invalid stock tier".into());}Ok(n)}
pub fn load(p:&Path)->Result<Option<u8>,String>{match fs::read_to_string(p){Ok(s)=>parse(&s).map(Some),Err(e)if e.kind()==std::io::ErrorKind::NotFound=>Ok(None),Err(e)=>Err(e.to_string())}}
pub fn save(p:&Path,tier:u8)->Result<(),String>{if tier>3{return Err("Invalid stock tier".into());}if let Some(parent)=p.parent(){fs::create_dir_all(parent).map_err(|e|e.to_string())?;}let tmp=p.with_extension("cfg.tmp");fs::write(&tmp,format!("tier={tier}\n")).map_err(|e|e.to_string())?;if p.exists(){fs::copy(p,p.with_extension("cfg.bak")).map_err(|e|e.to_string())?;}fs::rename(tmp,p).map_err(|e|e.to_string())}
#[cfg(test)]mod shop_stock_settings_tests{use super::*;
 #[test]fn shop_stock_preferences_persist_and_restore_default(){let dir=std::env::temp_dir().join(format!("ffta-shop-stock-test-{}",std::process::id()));let p=dir.join("shop-stock.cfg");for tier in 0..=3{save(&p,tier).unwrap();assert_eq!(load(&p),Ok(Some(tier)));}save(&p,0).unwrap();assert_eq!(load(&p),Ok(Some(0)));for s in["tier=4","tier=-1","garbage","tier=1\ntier=2"]{assert!(parse(s).is_err());}fs::remove_dir_all(dir).unwrap();}
}
