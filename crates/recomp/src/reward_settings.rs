//! Host preferences beside the saves; no cartridge or machine snapshot writes.
use std::{fs,path::Path};
fn valid(n:u8)->bool{gba_core::ffta_rewards::FACTORS.contains(&n)}
fn parse(text:&str)->Result<(u8,u8,u8,u8),String>{
    let mut gil=None;let mut ap=None;let mut exp=None;let mut jp=None;
    for line in text.lines().map(str::trim).filter(|s|!s.is_empty()) {
        let(key,value)=line.split_once('=').ok_or("Invalid reward settings line")?;
        let n=value.trim().parse::<u8>().map_err(|_|"Invalid reward multiplier")?;
        if !valid(n){return Err("Unsupported reward multiplier".into());}
        match key.trim(){"gil" if gil.is_none()=>gil=Some(n),"ap" if ap.is_none()=>ap=Some(n),"exp" if exp.is_none()=>exp=Some(n),"jp" if jp.is_none()=>jp=Some(n),_=>return Err("Unknown or duplicate reward setting".into())}
    }
    Ok((gil.ok_or("Missing Gil setting")?,ap.ok_or("Missing AP setting")?,exp.unwrap_or(1),jp.unwrap_or(1)))
}
pub fn load(path:&Path)->Result<Option<(u8,u8,u8,u8)>,String>{
    match fs::read_to_string(path){Ok(text)=>parse(&text).map(Some),Err(e)if e.kind()==std::io::ErrorKind::NotFound=>Ok(None),Err(e)=>Err(e.to_string())}
}
pub fn save(path:&Path,gil:u8,ap:u8,exp:u8,jp:u8)->Result<(),String>{
    if !valid(gil)||!valid(ap)||!valid(exp)||!valid(jp){return Err("Unsupported reward multiplier".into());}
    if let Some(parent)=path.parent(){fs::create_dir_all(parent).map_err(|e|e.to_string())?;}
    let tmp=path.with_extension("cfg.tmp");
    fs::write(&tmp,format!("gil={gil}\nap={ap}\nexp={exp}\njp={jp}\n")).map_err(|e|e.to_string())?;
    if path.exists(){fs::copy(path,path.with_extension("cfg.bak")).map_err(|e|e.to_string())?;}
    fs::rename(&tmp,path).map_err(|e|e.to_string())
}
#[cfg(test)]mod reward_settings_tests {
    use super::*;
    #[test]fn reward_settings_roundtrip_and_reject_invalid_preferences(){
        for &gil in &gba_core::ffta_rewards::FACTORS{for &ap in &gba_core::ffta_rewards::FACTORS{
            assert_eq!(parse(&format!("gil={gil}\r\nap={ap}\r\n")),Ok((gil,ap,1,1)));
        }}
        for &exp in &gba_core::ffta_rewards::FACTORS{for &jp in &gba_core::ffta_rewards::FACTORS{
            assert_eq!(parse(&format!("gil=2\nap=5\nexp={exp}\njp={jp}\n")),Ok((2,5,exp,jp)));
        }}
        for text in ["gil=0\nap=2","gil=2\nap=4","gil=2","gil=2\nap=2\ngil=5","gil=2\nother=2","gil=2\nap=2\nexp=0","gil=2\nap=2\njp=4","gil=2\nap=2\nexp=2\nexp=3"]{assert!(parse(text).is_err());}
    }
    #[test]fn reward_settings_saved_preferences_reload_and_keep_previous_copy(){
        let dir=std::env::temp_dir().join(format!("ffta-reward-settings-test-{}",std::process::id()));
        fs::create_dir_all(&dir).unwrap();let path=dir.join("rewards.cfg");
        save(&path,2,5,3,10).unwrap();assert_eq!(load(&path),Ok(Some((2,5,3,10))));
        save(&path,1,10,2,5).unwrap();assert_eq!(load(&path),Ok(Some((1,10,2,5))));
        assert_eq!(parse(&fs::read_to_string(path.with_extension("cfg.bak")).unwrap()),Ok((2,5,3,10)));
        fs::remove_dir_all(dir).unwrap();
    }
}
