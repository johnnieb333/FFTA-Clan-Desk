//! Native EUR level growth runs only in an isolated machine; commit copies a validated record.
use gba_core::{Machine,Cpu,cpu::{FLAG_I,FLAG_T}};
const JOBS:usize=0x6e5464;
const FIELDS:[usize;7]=[0x1a,0x1e,0xd2,0x20,0x22,0x24,0x26];
fn stats(r:&[u8])->Result<[u16;7],String>{
 if r.len()!=0x108{return Err("Character record unavailable".into());}
 let v=FIELDS.map(|o|u16::from_le_bytes([r[o],r[o+1]]));
 if v.iter().any(|&s|s>999){return Err("Unverified base stat value".into());}Ok(v)
}
fn member(m:&Machine,expected:&menu::CharacterInfo)->Result<menu::CharacterInfo,String>{
 let r=crate::character_editor::read(m)?.into_iter().find(|r|r.slot==expected.slot).ok_or("Member no longer present")?;
 if r.record!=expected.record{return Err("Character changed; reopen Levels & Growth".into());}
 if !(1..=50).contains(&r.level){return Err("Unsupported character level".into());}stats(&r.record)?;Ok(r)
}
fn jobs(m:&Machine,r:&menu::CharacterInfo)->Result<Vec<menu::GrowthJob>,String>{
 let race=r.record[6];let mut rows=Vec::new();
 for id in 2u8..=43{let at=JOBS+id as usize*52;
  if m.bus.rom.get(at+4)!=Some(&race){continue;}
  let name=crate::item_editor::unit_job(&m.bus.rom,id,race)?;
  let growth:[u8;7]=m.bus.rom.get(at+32..at+39).ok_or("Missing growth data")?.try_into().unwrap();
  rows.push(menu::GrowthJob{id,name,growth});
 }
 if !rows.iter().any(|j|j.id==r.record[7]){return Err("This member's job is not supported for level edits".into());}
 Ok(rows)
}
pub fn open(m:&Machine,expected:&menu::CharacterInfo)->Result<(menu::CharacterInfo,Vec<menu::GrowthJob>,[u16;7]),String>{
 let r=member(m,expected)?;let rows=jobs(m,&r)?;let values=stats(&r.record)?;Ok((r,rows,values))
}
fn call(m:&mut Machine,pc:u32,args:[u32;3],r4:u32,stop:u32)->Result<u32,String>{
 let stack=m.bus.iwram[0x7000..0x7400].to_vec();let mut cpu=Cpu::new();cpu.cpsr|=FLAG_T|FLAG_I;
 cpu.regs[..3].copy_from_slice(&args);cpu.regs[4]=r4;cpu.regs[13]=0x03007400;cpu.regs[14]=0x03007001;cpu.regs[15]=pc;
 for _ in 0..100_000{
  if cpu.regs[15]==stop{m.bus.iwram[0x7000..0x7400].copy_from_slice(&stack);return Ok(cpu.regs[0]);}
  gba_core::exec::step_hle(&mut cpu,&mut m.bus);
 }Err("Native growth calculation did not return; no edit applied".into())
}
fn allowed(i:usize)->bool{i==9||i==10||FIELDS.iter().any(|&o|i==o||i==o+1)}
pub fn preview(m:&Machine,expected:&menu::CharacterInfo,target:u8,job:u8)->Result<menu::LevelPlan,String>{
 let r=member(m,expected)?;let rows=jobs(m,&r)?;
 let growth=rows.iter().find(|j|j.id==job).ok_or("Growth job does not belong to this race")?.growth;
 if target<r.level||target>50{return Err("Choose the current level or a higher level, up to 50".into());}
 let before=stats(&r.record)?;
 if target==r.level{return Ok(menu::LevelPlan{target,job,record:r.record,before,after:before});}
 let at=0x80+(r.slot-1)*0x108;let addr=0x02000000+at as u32;
 let mut private=Machine::new(m.bus.rom.clone());private.load_state(&m.save_state())?;
 // The original job accessor follows +5 for many ordinary members and +7
 // for inherited job records. Set both solely inside this private calculation.
 private.bus.ewram[at+5]=job;private.bus.ewram[at+7]=job;
 for(i,&raw)in growth.iter().enumerate(){
  let effective=call(&mut private,0x080ccc48,[addr,0x29+i as u32,0],0,0x03007000)?;
  if effective!=raw as u32{return Err("Native growth does not match the selected job".into());}
 }
 for _ in r.level..target{
  let previous=private.bus.ewram[at+9];
  if call(&mut private,0x080cd4d8,[addr,0,0],0,0x03007000)?!=1||private.bus.ewram[at+9]!=previous+1{
   return Err("Native level-up could not be verified".into());
  }
 }
 let mut record=private.bus.ewram[at..at+0x108].to_vec();record[5]=r.record[5];record[7]=r.record[7];
 if record.iter().zip(&r.record).enumerate().any(|(i,(a,b))|a!=b&&!allowed(i))||record[9]!=target||record[10]!=0{
  return Err("Growth changed an unexpected character field".into());
 }
 let after=stats(&record)?;
 if after.iter().zip(before).any(|(&new,old)|new<old){return Err("Unexpected stat decrease".into());}
 Ok(menu::LevelPlan{target,job,record,before,after})
}
pub fn prepare(m:&Machine,expected:&menu::CharacterInfo,plan:&menu::LevelPlan)->Result<(usize,Vec<u8>),String>{
 if plan.target<=expected.level{return Err("No level increase to apply".into());}
 let verified=preview(m,expected,plan.target,plan.job)?;
 if verified!=*plan{return Err("Preview changed; reopen Levels & Growth before applying".into());}
 Ok((0x80+(expected.slot-1)*0x108,verified.record))
}
#[cfg(test)]mod levels_tests{
 use super::*;
 fn fixture()->Option<(Machine,Vec<u8>)>{
  let(Ok(r),Ok(s))=(std::env::var("FFTA_TEST_ROM"),std::env::var("FFTA_TEST_STATE"))else{println!("Set FFTA_TEST_ROM and FFTA_TEST_STATE for native level checks.");return None;};
  let state=std::fs::read(s).unwrap();let mut m=Machine::new(std::fs::read(r).unwrap());m.load_state(&state).unwrap();Some((m,state))
 }
 #[test]fn levels_real_all_42_selected_jobs_match_battle_and_keep_original_record_fields(){
  let Some((m,state))=fixture()else{return;};let members=crate::character_editor::read(&m).unwrap();let mut count=0;
  let before=m.save_state();let mut battle=Machine::new(m.bus.rom.clone());
  for race in 1..=5{
   let member=members.iter().find(|r|r.record[6]==race).expect("Backup needs all five races");
   for job in jobs(&m,member).unwrap(){
    let target=member.level+1;let plan=preview(&m,member,target,job.id).unwrap();
    assert_eq!(m.save_state(),before,"Preview must leave active machine and RNG unchanged");
    let(at,record)=prepare(&m,member,&plan).unwrap();assert_eq!(record,plan.record);
    assert!(record.iter().zip(&member.record).enumerate().all(|(i,(a,b))|a==b||allowed(i)));
    battle.load_state(&state).unwrap();let addr=0x02000000+at as u32;
    battle.bus.ewram[at+5]=job.id;battle.bus.ewram[at+7]=job.id;
    for(i,raw)in job.growth.iter().enumerate(){assert_eq!(call(&mut battle,0x080ccc48,[addr,0x29+i as u32,0],0,0x03007000).unwrap(),*raw as u32,"Selected growth job must be honored for every race");}
    battle.bus.ewram[0x3f000..0x3f004].copy_from_slice(&0x0203f010u32.to_le_bytes());battle.bus.ewram[0x3f010..0x3f014].copy_from_slice(&addr.to_le_bytes());
    call(&mut battle,0x080aa8d4,[0,0,0],0x0203f000,0x080aa8e4).unwrap();
    battle.bus.ewram[at+5]=member.record[5];battle.bus.ewram[at+7]=member.record[7];
    assert_eq!(&battle.bus.ewram[at..at+0x108],plan.record.as_slice(),"Preview must equal the original battle EXP path for {}",job.name);
    assert_eq!(preview(&m,member,target,job.id).unwrap(),plan,"Repeated preview must be identical");
    let high=preview(&m,member,50,job.id).unwrap();assert_eq!(high.record[9],50);assert_eq!(high.record[10],0);
    let levels=(50-member.level)as u32;
    for i in 0..7{let old=high.before[i]as u32;let g=job.growth[i]as u32;
     let low=if old==0{0}else{(old+levels*(g/10)).min(999)};
     let top=if old==0{0}else{(old+levels*(g/10+u32::from(g%10>0))).min(999)};
     assert!((low..=top).contains(&(high.after[i]as u32)),"{} growth out of bounds",job.name);
    }
    count+=1;
   }
  }
  assert_eq!(count,42);assert_eq!(m.save_state(),before);
  println!("Levels: all 42 selected growth jobs matched their tables and the battle EXP path; previews to 50 passed. Active state and RNG unchanged.");
 }
 #[test]fn levels_real_guards_save_reload_and_full_editor_undo(){
  let Some((mut m,state))=fixture()else{return;};let row=crate::character_editor::read(&m).unwrap().remove(0);let job=row.record[7];
  assert!(preview(&m,&row,row.level-1,job).is_err());assert!(preview(&m,&row,51,job).is_err());assert!(preview(&m,&row,row.level+1,255).is_err());
  let foreign=(2u8..=43).find(|&j|m.bus.rom[JOBS+j as usize*52+4]!=row.record[6]).unwrap();assert!(preview(&m,&row,row.level+1,foreign).is_err());
  let no_change=preview(&m,&row,row.level,job).unwrap();assert!(prepare(&m,&row,&no_change).is_err());assert_eq!(no_change.record,row.record);
  let mut stale=row.clone();stale.record[10]^=1;assert!(preview(&m,&stale,row.level+1,job).is_err());
  let plan=preview(&m,&row,row.level+2,job).unwrap();let mut tampered=plan.clone();tampered.record[0x40]^=1;assert!(prepare(&m,&row,&tampered).is_err());
  assert_eq!(m.save_state(),state);
  let dir=std::env::temp_dir().join(format!("ffta-level-undo-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let saves=crate::manual_states::ManualStates::new(&dir);saves.backup_editor(&m).unwrap();
  let(at,record)=prepare(&m,&row,&plan).unwrap();m.bus.ewram[at..at+record.len()].copy_from_slice(&record);
  let edited=m.save_state();let mut fresh=Machine::new(m.bus.rom.clone());fresh.load_state(&edited).unwrap();assert_eq!(fresh.bus.ewram,m.bus.ewram);
  saves.undo_editor(&mut m).unwrap();assert_eq!(m.save_state(),state);assert!(saves.has_editor_backup());
  assert!(!dir.join("ManualStates/slot-01.state").exists());std::fs::remove_dir_all(dir).unwrap();
  println!("Levels: invalid/stale/tampered requests rejected; fresh load and full editor Undo passed. User save files unchanged.");
 }
}


/// Per-stat envelope for ordinary recruitment and subsequent level growth.
/// This is not proof that one complete build can roll every maximum together.
/// Level-down cycles, consumable boosts and external edits are outside this model.
pub fn game_limits(m:&Machine,expected:&menu::CharacterInfo,target:u8)->Result<[u16;7],String>{
 let r=member(m,expected)?;let rows=jobs(m,&r)?;
 if !(1..=50).contains(&target){return Err("Limits require a level from 1 to 50".into());}
 let mut private=Machine::new(m.bus.rom.clone());private.load_state(&m.save_state())?;
 let mut starting=[0u32;7];let mut gain=[0u32;7];
 for j in &rows{for i in 0..7{
  let base=call(&mut private,0x080cbec8,[j.id as u32,j.id as u32,0x17+i as u32],0,0x03007000)?;
  if base>999{return Err("Unverified recruitment base stat".into());}
  // EUR cc f9c: base + floor(base/20) - signed_random(2*floor(base/20)).
  // 0289c returns half the difference of four inclusive random residues,
  // truncated toward zero. Its outer range is [-2*k, 2*k].
  let high=base+3*(base/20);
  starting[i]=starting[i].max(high);
  gain[i]=gain[i].max((j.growth[i]as u32+9)/10);
 }}
 // Named characters can inherit a different base template than their current job.
 let addr=0x02000080+(r.slot as u32-1)*0x108;
 for i in 0..7{let base=call(&mut private,0x080ccc48,[addr,0x17+i as u32,0],0,0x03007000)?;
  if base>999{return Err("Unverified character template stat".into());}
  starting[i]=starting[i].max(base+3*(base/20));
 }
 Ok(std::array::from_fn(|i|(starting[i]+(target as u32-1)*gain[i]).min(999).max(if i==1{0}else{1})as u16))
}

/// Mode 0 uses native increases and an explicitly approximate reverse growth.
/// Mode 1 changes level alone. Per-field overrides are applied last.
pub fn preview_edit(m:&Machine,expected:&menu::CharacterInfo,target:u8,job:u8,mode:u8,overrides:[Option<u16>;7])->Result<menu::LevelPlan,String>{
 let restricted=mode&2!=0;let level_mode=mode&1;
 let r=member(m,expected)?;let rows=jobs(m,&r)?;
 let growth=rows.iter().find(|j|j.id==job).ok_or("Growth job does not belong to this race")?.growth;
 if !(1..=50).contains(&target)||mode>3{return Err("Choose a level from 1 to 50 and a supported stat mode".into());}
 let before=stats(&r.record)?;
 let mut plan=if level_mode==0 && target>=r.level {preview(m,&r,target,job)?}
 else {
  let mut after=before;
  if level_mode==0 {for i in 0..7 {
   // Mean table growth, not a claim to recover unrecorded random rolls.
   let loss=((r.level-target)as u32*growth[i]as u32+5)/10;
   after[i]=before[i].saturating_sub(loss as u16).max(if i==1{0}else{1});
  }}
  menu::LevelPlan{target,job,record:r.record.clone(),before,after}
 };
 for i in 0..7 {if let Some(value)=overrides[i]{
  if value>999 || value==0 && i!=1{return Err("Base stats: 1-999; maximum MP may be 0".into());}
  plan.after[i]=value;
 }}
 if restricted{let limits=game_limits(m,&r,target)?;for i in 0..7{plan.after[i]=plan.after[i].min(limits[i]);}}
 for (i,&o) in FIELDS.iter().enumerate(){plan.record[o..o+2].copy_from_slice(&plan.after[i].to_le_bytes());}
 plan.record[9]=target;
 if target!=r.level{plan.record[10]=0;}
 // Lowering maximum HP/MP must not leave current HP/MP above their maxima.
 for(current,maximum)in[(0x18,plan.after[0]),(0x1c,plan.after[1])]{
  let value=u16::from_le_bytes([plan.record[current],plan.record[current+1]]);
  plan.record[current..current+2].copy_from_slice(&value.min(maximum).to_le_bytes());
 }
 Ok(plan)
}
pub fn prepare_edit(m:&Machine,expected:&menu::CharacterInfo,plan:&menu::LevelPlan,mode:u8,overrides:[Option<u16>;7])->Result<(usize,Vec<u8>),String>{
 let verified=preview_edit(m,expected,plan.target,plan.job,mode,overrides)?;
 if &verified!=plan{return Err("Preview changed; reopen Level & Stats before applying".into());}
 if verified.record==expected.record{return Err("No character changes to apply".into());}
 Ok((0x80+(expected.slot-1)*0x108,verified.record))
}
#[cfg(test)]mod stat_editor_tests{
 use super::*;
 #[test]fn stat_editor_recorded_level_down_direct_stats_clamping_reload_and_undo(){
  let(Ok(rom),Ok(state))=(std::env::var("FFTA_TEST_ROM"),std::env::var("FFTA_STATS_STATE"))else{println!("Set FFTA_TEST_ROM and FFTA_STATS_STATE for recorded character checks");return;};
  let rom=std::fs::read(rom).unwrap();let state=std::fs::read(state).unwrap();let mut m=Machine::new(rom);m.load_state(&state).unwrap();
  let row=crate::character_editor::read(&m).unwrap().into_iter().find(|r|r.slot==3).expect("Billy in slot 3");let original=m.save_state();let job=row.record[7];let stats=stats(&row.record).unwrap();
  let keep=preview_edit(&m,&row,1,job,1,[None;7]).unwrap();assert_eq!(keep.after,stats);assert_eq!(keep.record[9],1);assert_eq!(keep.record[10],0);
  let lower=preview_edit(&m,&row,1,job,0,[None;7]).unwrap();assert!(lower.after.iter().zip(stats).all(|(&new,old)|new<=old));
  let mut direct=[None;7];for i in 0..7{direct[i]=Some(if i==2{132}else{stats[i].saturating_add(2)});}
  let plan=preview_edit(&m,&row,row.level,job,1,direct).unwrap();assert_eq!(plan.after[2],132);assert_eq!(plan.record[10],row.exp);
  for i in 0..7{assert_eq!(plan.after[i],direct[i].unwrap());}
  assert_eq!(m.save_state(),original,"preview cannot change active state/RNG");
  let mut bad=direct;bad[2]=Some(1000);assert!(preview_edit(&m,&row,3,job,1,bad).is_err());bad[2]=Some(0);assert!(preview_edit(&m,&row,3,job,1,bad).is_err());
  let mut low=[None;7];low[0]=Some(1);low[1]=Some(0);let low=preview_edit(&m,&row,1,job,1,low).unwrap();assert!(u16::from_le_bytes([low.record[0x18],low.record[0x19]])<=1);assert_eq!(&low.record[0x1c..0x1e],&[0,0]);
  let mut tampered=plan.clone();tampered.record[0x40]^=1;assert!(prepare_edit(&m,&row,&tampered,1,direct).is_err());
  let(at,record)=prepare_edit(&m,&row,&plan,1,direct).unwrap();
  for(i,(a,b))in record.iter().zip(&row.record).enumerate(){if a!=b{assert!(FIELDS.iter().any(|&o|i==o||i==o+1));}}
  let dir=std::env::temp_dir().join(format!("ffta-stat-check-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
  let saves=crate::manual_states::ManualStates::new(&dir);saves.backup_editor(&m).unwrap();m.bus.ewram[at..at+0x108].copy_from_slice(&record);
  let edited=m.save_state();let mut fresh=Machine::new(m.bus.rom.clone());fresh.load_state(&edited).unwrap();assert_eq!(fresh.bus.ewram,m.bus.ewram);
  saves.undo_editor(&mut m).unwrap();assert_eq!(m.save_state(),original);std::fs::remove_dir_all(dir).unwrap();
  println!("Character stats: keep/reduce level, seven direct fields, limits, current HP/MP clamps, isolated previews, reload and Undo verified");
 }
}

#[cfg(test)]mod stat_limits_tests{
 use super::*;
 #[test]fn stat_limits_native_all_races_caps_preview_apply_guards_and_unchanged_machine(){
  let rom=std::env::var("FFTA_TEST_ROM").expect("Set FFTA_TEST_ROM");let state=std::env::var("FFTA_STATS_STATE").expect("Set FFTA_STATS_STATE");
  let mut m=Machine::new(std::fs::read(rom).unwrap());m.load_state(&std::fs::read(state).unwrap()).unwrap();let snapshot=m.save_state();let rows=crate::character_editor::read(&m).unwrap();
  for race in 1..=5{let row=rows.iter().find(|r|r.record[6]==race).unwrap();let job=row.record[7];let mut previous=[0;7];
   for target in [1,3,50]{let cap=game_limits(&m,row,target).unwrap();assert!(cap.iter().zip(previous).all(|(&a,b)|a>=b));previous=cap;
    let limited=preview_edit(&m,row,target,job,3,[Some(999);7]).unwrap();assert_eq!(limited.after,cap);let(_,record)=prepare_edit(&m,row,&limited,3,[Some(999);7]).unwrap();assert_eq!(record,limited.record);
    let free=preview_edit(&m,row,target,job,1,[Some(999);7]).unwrap();assert_eq!(free.after,[999;7]);
    assert_eq!(&limited.record[0x2a..0x34],&row.record[0x2a..0x34]);assert_eq!(limited.record[7],row.record[7]);
    let mut tampered=limited.clone();tampered.after[2]=999;assert!(prepare_edit(&m,row,&tampered,3,[Some(999);7]).is_err());
   }
   let limits=game_limits(&m,row,3).unwrap();if race==1{assert!(limits[2]>=132,"Natural level-3 high-Speed recruits must fit");}
  }
  assert_eq!(m.save_state(),snapshot);println!("Stat limits: all five races, levels 1/3/50, unrestricted values, native guards and isolated previews passed");
 }
 #[test]fn stat_limits_recruitment_rng_extreme_witnesses(){
  // The decoded native generator uses four consecutive 15-bit LCG outputs.
  // Find actual seeds attaining both theoretical extremes for every spread
  // used by the 42 ordinary job bases (k = floor(base/20), 0..6).
  for k in 0..=6i32{let mut low=false;let mut high=false;
   for seed in 0..1_000_000u32{let mut state=seed;let mut r=[0i32;4];for v in &mut r{state=state.wrapping_mul(0x41c64e6d).wrapping_add(0x3039);*v=((state>>16)&0x7fff)as i32%(2*k+1);}
    let roll=(r[0]+r[1]-r[2]-r[3])/2;low|=roll== -2*k;high|=roll==2*k;if low&&high{break;}
   }assert!(low&&high,"Missing native RNG extrema witnesses for k={k}");
  }
 }
}
