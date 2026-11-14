//! Optional save-state-backed character strings, outside cartridge and guest RAM.
//! No names are active unless explicitly staged by the character editor.
pub const BASE:u32=0x09fff000;
const WIDTH:usize=16;
const BYTES:usize=24*WIDTH;
const TAG:&[u8;8]=b"FFTANM01";
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct NameBank {data:[u8;BYTES]}
impl Default for NameBank{fn default()->Self{Self{data:[0;BYTES]}}}
impl NameBank {
    pub fn set(&mut self,slot:usize,text:&str)->Result<u32,String>{
        if slot>=24{return Err("Invalid clan slot".into());}
        if text.trim()!=text || text.is_empty() || text.len()>9{return Err("Use 1-9 characters, without leading or trailing spaces".into());}
        let mut encoded=[0;WIDTH];encoded[0]=1;
        for(i,c)in text.bytes().enumerate(){encoded[i+1]=match c{
            b'A'..=b'Z'=>0xb1+(c-b'A'),b'a'..=b'z'=>0xcb+(c-b'a'),b'0'..=b'9'=>0xa7+(c-b'0'),
            b' '=>0xa0,b'\''=>0xf4,b'-'=>0xff,_=>return Err("Use letters, numbers, spaces, apostrophe or hyphen".into()),
        };}
        self.data[slot*WIDTH..(slot+1)*WIDTH].copy_from_slice(&encoded);Ok(BASE+slot as u32*WIDTH as u32)
    }
    pub fn text(&self,ptr:u32)->Option<String>{
        if self.read(ptr)?!=1{return None;}let mut text=String::new();
        for i in 1..16 {let c=self.read(ptr+i)?;if c==0{return Some(text);}
            text.push(match c{0xb1..=0xca=>(b'A'+(c-0xb1))as char,0xcb..=0xe4=>(b'a'+(c-0xcb))as char,
                0xa7..=0xb0=>(b'0'+(c-0xa7))as char,0xa0=>' ',0xf4=>'\'',0xff=>'-',_=>return None});
        }None
    }
    pub fn read(&self,addr:u32)->Option<u8>{
        if !(0x08..=0x0d).contains(&(addr>>24)){return None;}
        let o=(addr&0x01ffffff).checked_sub(BASE&0x01ffffff)? as usize;
        if o>=BYTES || self.data[o/WIDTH*WIDTH]!=1{return None;}Some(self.data[o])
    }
    pub(crate) fn append_extension(&self,out:&mut Vec<u8>){
        if self.data.iter().all(|&b|b==0){return;}out.extend_from_slice(TAG);out.extend_from_slice(&self.data);
    }
    pub(crate) fn from_extension(bytes:&[u8])->Result<Self,String>{
        if bytes.is_empty(){return Ok(Self::default());}
        if bytes.len()!=8+BYTES || bytes.get(..8)!=Some(&TAG[..]){return Err("Unsupported state extension".into());}
        let mut names=Self::default();names.data.copy_from_slice(&bytes[8..]);
        for row in names.data.chunks_exact(WIDTH){
            if row.iter().all(|&b|b==0){continue;}
            if row[0]!=1{return Err("Invalid saved character name".into());}
            let end=row[1..].iter().position(|&b|b==0).map(|i|i+1).ok_or("Unterminated saved character name")?;
            if end<2 || end>10 || row[end..].iter().any(|&b|b!=0) || row[1..end].iter().any(|&c|!matches!(c,0xa0|0xa7..=0xe4|0xf4|0xff)){
                return Err("Invalid saved character name".into());
            }
        }
        Ok(names)
    }
}
#[cfg(test)]mod character_names_tests {
    use super::*;
    use crate::{Machine,Bus};
    #[test]fn character_names_live_reads_snapshot_reload_and_undo_preserve_original_rom(){
        let mut m=Machine::new(vec![0;16]);let original=m.save_state();let rom=m.bus.rom.clone();
        let ptr=m.bus.ffta_names.set(2,"Cooper").unwrap();
        assert_eq!(m.bus.ffta_names.text(ptr).as_deref(),Some("Cooper"));
        assert_eq!(m.bus.read8(ptr),1);assert_eq!(m.bus.read16(ptr),0xb301);
        assert_eq!(m.bus.read32(ptr),0xd9d9b301);
        assert_eq!(m.bus.read8(ptr+0x02000000),1);
        let first=m.save_state();m.bus.ffta_names.set(2,"Johnnie").unwrap();let second=m.save_state();
        let mut restored=Machine::new(rom.clone());restored.load_state(&second).unwrap();assert_eq!(restored.bus.ffta_names,m.bus.ffta_names);
        restored.load_state(&first).unwrap();assert_eq!(restored.bus.read16(ptr),0xb301);
        restored.load_state(&original).unwrap();assert_eq!(restored.bus.ffta_names,NameBank::default());
        assert_eq!(restored.save_state(),original);assert_eq!(m.bus.rom,rom);
    }
    #[test]fn character_names_reject_invalid_input_and_corrupt_extension_without_writes(){
        let mut b=NameBank::default();for s in [""," too","too ","abcdefghij","é","A/B"]{assert!(b.set(2,s).is_err());assert_eq!(b,NameBank::default());}
        assert!(b.set(24,"Test").is_err());b.set(2,"Test").unwrap();let mut ext=Vec::new();b.append_extension(&mut ext);
        assert_eq!(NameBank::from_extension(&ext).unwrap(),b);ext[8+2*WIDTH]=2;assert!(NameBank::from_extension(&ext).is_err());
        assert!(b.read(0x02000080).is_none());assert!(b.read(BASE).is_none());
    }
    #[test]fn character_names_preview_validates_extensions_and_rejects_corruption(){
        use sha2::{Digest,Sha256};
        let mut m=Machine::new(vec![0;16]);let original=m.save_state();assert!(crate::state::preview(&original).is_ok());
        m.bus.ffta_names.set(2,"aB3-Test").unwrap();let saved=m.save_state();
        let (ram,image)=crate::state::preview(&saved).unwrap();assert_eq!(ram,m.bus.ewram);assert_eq!(image,m.bus.framebuffer);
        let mut bad=saved.clone();let at=bad.len()-BYTES+2*WIDTH;bad[at]=2;
        let digest=Sha256::digest(&bad[104..]);bad[72..104].copy_from_slice(&digest);
        assert!(crate::state::preview(&bad).is_err());
        let mut bad_checksum=saved;bad_checksum[104]^=1;assert!(crate::state::preview(&bad_checksum).is_err());
    }

}
