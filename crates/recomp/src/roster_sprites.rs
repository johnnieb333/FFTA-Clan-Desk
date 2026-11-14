//! Read-only ROM sprite decoding. No game code, image assets or disk cache bundled.
use std::collections::BTreeMap;
const JOBS:usize=0x6e5464;
fn bytes(rom:&[u8],at:usize,n:usize)->Result<&[u8],String>{rom.get(at..at.checked_add(n).ok_or("Sprite extent overflow")?).ok_or_else(||"Sprite resource outside ROM".into())}
fn half(rom:&[u8],at:usize)->Result<u16,String>{Ok(u16::from_le_bytes(bytes(rom,at,2)?.try_into().unwrap()))}
fn word(rom:&[u8],at:usize)->Result<u32,String>{Ok(u32::from_le_bytes(bytes(rom,at,4)?.try_into().unwrap()))}
fn pointer(rom:&[u8],at:usize)->Result<usize,String>{let p=word(rom,at)?.checked_sub(0x08000000).ok_or("Invalid sprite pointer")?as usize;bytes(rom,p,1)?;Ok(p)}
fn job_record(rom:&[u8],base:u8,current:u8)->Result<&[u8],String>{
 let mut id=base;let mut seen=[false;116];for _ in 0..116{
  if id as usize>=116||seen[id as usize]{return Err("Invalid sprite job redirect".into());}seen[id as usize]=true;
  let r=bytes(rom,JOBS+id as usize*52,52)?;
  match r[5]{0=>return Ok(r),255=>id=current,next=>id=next}
 }Err("Sprite redirect did not resolve".into())
}
fn binding(rom:&[u8],record:&[u8],mode:u8)->Result<(u16,usize),String>{
 if record.len()!=0x108{return Err("Invalid roster record".into());}
 let (id,bank,custom)=if record[4]>1{
  if record[4]>94{return Err("Unverified special-unit sprite identity".into());}
  let r=bytes(rom,0x6e6f50+record[4]as usize*14,14)?;
  let selector=(r[8]>>4)|((r[9]&15)<<4);
  (u16::from_le_bytes([r[4],r[5]]),r[8]&15,selector.checked_sub(1))
 }else{let r=job_record(rom,record[5],record[7])?;(u16::from_le_bytes([r[7],r[8]]),r[11]&15,None)};
 let variant=match mode&3{1=>1,2=>2,_=>0};
 let source=if let Some(index)=custom{
  if index>=9{return Err("Unverified special sprite palette".into());}
  [0x40fa90,0x4110f0,0x410610][variant]+index as usize*32
 }else{[0x40f290,0x4108f0,0x40fe10][variant]+bank as usize*32};
 bytes(rom,source,32)?;Ok((id,source))
}
fn decode(rom:&[u8],id:u16,palette:usize)->Result<menu::RosterSprite,String>{
 for(at,expected)in[(0x2219c,0x394834),(0x221b8,0x898a40),(0x221c0,0xa50020)]{if pointer(rom,at)?!=expected{return Err("Sprite reader root differs".into());}}
 let index=pointer(rom,0x394834+id as usize*4)?;let sequence=pointer(rom,index)?;let count=half(rom,sequence)?as usize;
 if !(1..=64).contains(&count){return Err("Unverified sprite sequence".into());}
 let frame=(0..count).map(|i|sequence+4+i*20).find(|&at|rom.get(at+9)==Some(&1)).ok_or("Sprite has no initial image")?;
 let objects=0xa50020usize.checked_add(word(rom,frame+4)?as usize).ok_or("Sprite object overflow")?;
 let pixel=0x898a40usize.checked_add(word(rom,frame)?as usize).ok_or("Sprite pixel overflow")?;
 let n=half(rom,objects)?as usize;if !(1..=32).contains(&n){return Err("Unverified sprite object count".into());}
 let mut parts=Vec::new();let mut tile_count=0;
 for i in 0..n{let at=objects+2+i*6;let a0=half(rom,at)?;let a1=half(rom,at+2)?;let a2=half(rom,at+4)?;
  let shape=(a0>>14)as usize;let size=(a1>>14)as usize;
  if shape>=3||a0&0x2300!=0{return Err("Unverified sprite affine/color mode".into());}
  let(w,h)=[[(8,8),(16,16),(32,32),(64,64)],[(16,8),(32,8),(32,16),(64,32)],[(8,16),(8,32),(16,32),(32,64)]][shape][size];
  tile_count+=w*h/64;
  let x=(((a1&511)as i16)<<7)>>7;let y=(a0 as u8 as i8)as i16;
  parts.push((x,y,w,h,a1,a2));
 }
 let data=bytes(rom,pixel,tile_count*32)?;let mut pixels=vec![[0u8;4];96*96];
 for(x,y,w,h,a1,a2)in parts{for yy in 0..h{for xx in 0..w{
  let sx=if a1&0x1000!=0{w-1-xx}else{xx};let sy=if a1&0x2000!=0{h-1-yy}else{yy};
  let at=(a2 as usize&1023)*32+(sy/8*(w/8)+sx/8)*32+(sy%8)*4+sx%8/2;
  let packed=*data.get(at).ok_or("Sprite tile outside transfer")?;let color=(packed>>(4*(sx%2)))&15;if color==0{continue;}
  let dx=48+x as i32+xx as i32;let dy=72+y as i32+yy as i32;
  if !(0..96).contains(&dx)||!(0..96).contains(&dy){return Err("Sprite does not fit roster bounds".into());}
  let c=half(rom,palette+color as usize*2)?;pixels[dy as usize*96+dx as usize]=[((c&31)*255/31)as u8,(((c>>5)&31)*255/31)as u8,(((c>>10)&31)*255/31)as u8,255];
 }}}
 let visible:Vec<_>=pixels.iter().enumerate().filter(|(_,p)|p[3]!=0).map(|(i,_)|(i%96,i/96)).collect();
 if visible.is_empty(){return Err("Empty roster sprite".into());}
 let left=visible.iter().map(|p|p.0).min().unwrap();let right=visible.iter().map(|p|p.0).max().unwrap();let top=visible.iter().map(|p|p.1).min().unwrap();let bottom=visible.iter().map(|p|p.1).max().unwrap();
 let width=right-left+1;let height=bottom-top+1;let mut cropped=Vec::with_capacity(width*height);
 for y in top..=bottom{cropped.extend_from_slice(&pixels[y*96+left..y*96+right+1]);}
 Ok(menu::RosterSprite{width,height,pixels:cropped})
}
pub fn read(machine:&gba_core::Machine,rows:&[menu::CharacterInfo])->BTreeMap<usize,menu::RosterSprite>{
 let mut out=BTreeMap::new();if gba_core::ffta_rom::version(&machine.bus.rom)!=Some(gba_core::ffta_rom::Version::Europe){return out;}
 let mode=machine.bus.ewram.get(0x2fc2).copied().unwrap_or(0);let mut cache:BTreeMap<(u16,usize),menu::RosterSprite>=BTreeMap::new();
 for row in rows{let result=(||{let key=binding(&machine.bus.rom,&row.record,mode)?;if let Some(image)=cache.get(&key){return Ok(image.clone());}let image=decode(&machine.bus.rom,key.0,key.1)?;cache.insert(key,image.clone());Ok::<_,String>(image)})();
  match result{Ok(image)=>{out.insert(row.slot,image);},Err(e)=>eprintln!("Roster sprite slot {}: {e}",row.slot)}
 }out
}
#[cfg(test)]mod roster_sprite_tests{
 use super::*;
 #[test]fn roster_sprite_actual_rom_all_jobs_specials_and_no_machine_changes(){
  let rom=std::env::var("FFTA_TEST_ROM").expect("Set FFTA_TEST_ROM");let state=std::env::var("FFTA_STATS_STATE").expect("Set FFTA_STATS_STATE");
  let mut m=gba_core::Machine::new(std::fs::read(rom).unwrap());m.load_state(&std::fs::read(state).unwrap()).unwrap();let before=m.save_state();let rows=crate::character_editor::read(&m).unwrap();let images=read(&m,&rows);assert_eq!(images.len(),rows.len());
  for job in 2..=43u8{let mut record=vec![0;0x108];record[4]=1;record[5]=job;record[7]=job;let(id,palette)=binding(&m.bus.rom,&record,0).unwrap();let image=decode(&m.bus.rom,id,palette).unwrap();assert!(image.width<=64&&image.height<=64);assert!(image.pixels.iter().any(|p|p[3]==255));}
  for row in rows.iter().filter(|r|r.identity>1){let old=binding(&m.bus.rom,&row.record,0).unwrap();let mut record=row.record.clone();record[7]=6;assert_eq!(binding(&m.bus.rom,&record,0).unwrap(),old,"Special identities keep their own sprite");}
  assert_eq!(m.save_state(),before);println!("Roster sprites: all 42 jobs and actual clan members decoded; special identities preserved; machine unchanged");
 }
 #[test]fn roster_sprite_invalid_rom_fails_without_panicking(){assert!(decode(&[],0,0).is_err());assert!(binding(&[],&[],0).is_err());}
}
