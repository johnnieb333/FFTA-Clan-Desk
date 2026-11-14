//! EUR per-race ability progress, staged in a private record before one commit.
const JOBS:usize=0x6e5464;
const RACE_TABLE:usize=0x6df4d4;
const NAMES:usize=0x724c34;
const ABILITY_NAMES:usize=0x720d40;
fn half(b:&[u8],p:usize)->Result<u16,String>{Ok(u16::from_le_bytes(b.get(p..p+2).ok_or("Truncated ability data")?.try_into().unwrap()))}
pub fn read(m:&gba_core::Machine,expected:&menu::CharacterInfo)->Result<(menu::CharacterInfo,Vec<menu::AbilityJob>),String>{
    let member=crate::character_editor::read(m)?.into_iter().find(|r|r.slot==expected.slot).ok_or("Member no longer present")?;
    if member.record!=expected.record{return Err("Character changed; reopen Jobs & Abilities".into());}
    crate::item_editor::verify_teaching_tables(&m.bus.rom)?;
    let jobs=catalog(&m.bus.rom,&member.record)?;Ok((member,jobs))
}
fn catalog(rom:&[u8],record:&[u8])->Result<Vec<menu::AbilityJob>,String>{
    let race=*record.get(6).ok_or("Character record unavailable")?;
    if !(1..=5).contains(&race){return Err("Unverified character race".into());}
    let p=RACE_TABLE+race as usize*4;
    let base=u32::from_le_bytes(rom.get(p..p+4).ok_or("Missing race table")?.try_into().unwrap()).checked_sub(0x08000000).ok_or("Invalid race table pointer")? as usize;
    let mut jobs=Vec::new();
    for id in 2u8..=43 {
        let at=JOBS+id as usize*52;if rom.get(at+4)!=Some(&race){continue;}
        let start=*rom.get(at+46).ok_or("Missing job abilities")?;let end=rom[at+47];
        if start==0 || end<start || end>140{return Err("Unexpected ability range".into());}
        let mut skills=Vec::new();
        for index in start..=end {
            let p=base+index as usize*8;let kind=*rom.get(p+6).ok_or("Missing ability record")?;
            if kind==0{continue;} // The game's own Master Job predicate excludes these.
            let kind=match kind{1=>"Action",2=>"Reaction",3=>"Support",5=>"Combo",_=>return Err("Unverified ability type".into())};
            let required=*rom.get(p+7).ok_or("Missing ability AP")?;
            if required==0 || required>127{return Err("Unverified AP requirement".into());}
            let raw=*record.get(0x40+index as usize).ok_or("AP outside character record")?;
            let name=crate::item_editor::text_at(rom,ABILITY_NAMES,half(rom,p)? as usize)?;
            if name.is_empty(){return Err("Missing ability name".into());}
            skills.push(menu::AbilityInfo{index,name,kind:kind.into(),required,current:(raw&127).min(required)});
        }
        if skills.is_empty(){return Err("Job has no verified abilities".into());}
        let name=crate::item_editor::text_at(rom,NAMES,half(rom,at)? as usize)?;
        jobs.push(menu::AbilityJob{id,name,skills});
    }
    if jobs.is_empty(){return Err("No playable jobs for this race".into());}Ok(jobs)
}
pub fn prepare(m:&gba_core::Machine,expected:&menu::CharacterInfo,changes:&[(u8,u8)])->Result<(usize,Vec<u8>),String>{
    let (member,jobs)=read(m,expected)?;
    if changes.is_empty(){return Err("No pending AP changes".into());}
    let mut updates=std::collections::BTreeMap::new();
    for &(index,target)in changes {
        let skill=jobs.iter().flat_map(|j|&j.skills).find(|s|s.index==index).ok_or("Ability does not belong to this race")?;
        if target<skill.current || target>skill.required{return Err("Only added AP up to the requirement is enabled".into());}
        if updates.insert(index,target).is_some(){return Err("Duplicate ability change".into());}
    }
    // Original EUR 080d1074 links Viera Red/White Mage Cure entries.
    if member.record[6]==4 {
        if let Some(target)=updates.get(&26).copied().or_else(||updates.get(&33).copied()){
            let target=target.max(updates.get(&26).copied().unwrap_or(0)).max(updates.get(&33).copied().unwrap_or(0))
                .max(member.record[0x40+26]&127).max(member.record[0x40+33]&127).min(10);
            updates.insert(26,target);updates.insert(33,target);
        }
    }
    let mut record=member.record.clone();
    for(index,target)in updates {
        let skill=jobs.iter().flat_map(|j|&j.skills).find(|s|s.index==index).ok_or("Unverified shared ability")?;
        let o=0x40+index as usize;let old=record[o];
        if (old&127)>=target{continue;}
        record[o]=target|if target==skill.required{128}else{old&128};
    }
    if record==member.record{return Err("No AP increase to apply".into());}
    Ok((0x80+(member.slot-1)*0x108,record))
}

#[cfg(test)]mod abilities_tests {
    use super::*;
    pub(super) fn native_mastered(m:&mut gba_core::Machine,at:usize,job:u8)->bool{
        let stack=m.bus.iwram[0x7000..0x7400].to_vec();
        let mut cpu=gba_core::Cpu::new();
        cpu.cpsr|=gba_core::cpu::FLAG_T|gba_core::cpu::FLAG_I;
        cpu.regs[0]=0x02000000+at as u32;cpu.regs[1]=job as u32;
        cpu.regs[13]=0x03007400;cpu.regs[14]=0x03007001;cpu.regs[15]=0x080d0d18;
        let mut returned=false;
        for _ in 0..100_000{if cpu.regs[15]==0x03007000{returned=true;break;}gba_core::exec::step_hle(&mut cpu,&mut m.bus);}
        m.bus.iwram[0x7000..0x7400].copy_from_slice(&stack);
        assert!(returned,"Native mastery predicate did not return");cpu.regs[0]!=0
    }
    #[test]fn abilities_real_all_jobs_native_mastery_and_restore(){
        let(Ok(r),Ok(s))=(std::env::var("FFTA_TEST_ROM"),std::env::var("FFTA_TEST_STATE"))else{
            println!("Real-ROM abilities check skipped: set FFTA_TEST_ROM and FFTA_TEST_STATE.");return;
        };
        let rom=std::fs::read(r).unwrap();let state=std::fs::read(s).unwrap();
        let mut m=gba_core::Machine::new(rom.clone());m.load_state(&state).unwrap();
        let members=crate::character_editor::read(&m).unwrap();let mut count=0;
        for race in 1..=5 {
            let member=members.iter().find(|r|r.record[6]==race).expect("Test state must include all five races");
            let jobs=catalog(&rom,&member.record).unwrap();
            for job in &jobs {
                m.load_state(&state).unwrap();let before=m.bus.ewram.clone();
                let changes:Vec<_>=job.skills.iter().map(|s|(s.index,s.required)).collect();
                if job.skills.iter().all(|s|s.current==s.required){assert!(native_mastered(&mut m,0x80+(member.slot-1)*0x108,job.id));count+=1;continue;}
                let(at,record)=prepare(&m,member,&changes).unwrap();assert_eq!(m.bus.ewram,before,"Preparation must be read-only");
                let mut allowed:Vec<_>=job.skills.iter().map(|s|at+0x40+s.index as usize).collect();
                if race==4&&job.skills.iter().any(|s|matches!(s.index,26|33)){allowed.extend([at+0x40+26,at+0x40+33]);}
                m.bus.ewram[at..at+record.len()].copy_from_slice(&record);
                assert!(m.bus.ewram.iter().zip(&before).enumerate().all(|(i,(a,b))|a==b||allowed.contains(&i)),"Only AP bytes may change");
                assert!(native_mastered(&mut m,at,job.id),"Game did not recognize {} as mastered",job.name);
                let saved=m.save_state();let mut fresh=gba_core::Machine::new(rom.clone());fresh.load_state(&saved).unwrap();assert_eq!(fresh.bus.ewram,m.bus.ewram);
                m.load_state(&state).unwrap();assert_eq!(m.save_state(),state,"Undo must restore exact snapshot");
                count+=1;
            }
        }
        assert_eq!(count,42);assert_eq!(m.bus.rom,rom);
        println!("All 42 jobs passed the native mastery predicate, AP-only writes, fresh-state load and exact Undo. No files written.");
    }
    #[test]fn abilities_real_guards_and_partial_ap(){
        let(Ok(r),Ok(s))=(std::env::var("FFTA_TEST_ROM"),std::env::var("FFTA_TEST_STATE"))else{return;};
        let mut m=gba_core::Machine::new(std::fs::read(r).unwrap());m.load_state(&std::fs::read(s).unwrap()).unwrap();
        let snapshot=m.save_state();let members=crate::character_editor::read(&m).unwrap();
        let member=&members[0];let(_,jobs)=read(&m,member).unwrap();
        let skill=jobs.iter().flat_map(|j|&j.skills).find(|s|s.current+1<s.required).unwrap();
        let target=skill.current+1;let(at,record)=prepare(&m,member,&[(skill.index,target)]).unwrap();
        assert_eq!(record[0x40+skill.index as usize],target|(member.record[0x40+skill.index as usize]&128));
        assert_eq!(at,0x80+(member.slot-1)*0x108);
        for changes in [vec![],vec![(0,1)],vec![(255,1)],vec![(skill.index,skill.required+1)],vec![(skill.index,target),(skill.index,target)]]{assert!(prepare(&m,member,&changes).is_err());}
        let mut stale=member.clone();stale.record[9]^=1;assert!(prepare(&m,&stale,&[(skill.index,target)]).is_err());
        let mastered=jobs.iter().flat_map(|j|&j.skills).find(|s|s.current>0).unwrap();assert!(prepare(&m,member,&[(mastered.index,mastered.current-1)]).is_err());
        assert_eq!(m.save_state(),snapshot,"All preparation and rejection paths must be read-only");
        let v=members.iter().find(|r|r.record[6]==4).unwrap();
        let(at,record)=prepare(&m,v,&[(26,10)]).unwrap();assert_eq!(record[0x40+26],138);assert_eq!(record[0x40+33],138);
        assert_eq!(at,0x80+(v.slot-1)*0x108);
    }
}

// Native job changes, including equipment removal, run in an isolated machine.
const INVENTORY_START:usize=0x1940;
const INVENTORY_END:usize=0x1f1c;
fn job_call(m:&mut gba_core::Machine,pc:u32,args:[u32;3])->Result<u32,String>{
 let mut c=gba_core::Cpu::new();c.cpsr|=gba_core::cpu::FLAG_T|gba_core::cpu::FLAG_I;
 c.regs[..3].copy_from_slice(&args);c.regs[13]=0x03007400;c.regs[14]=0x03007001;c.regs[15]=pc;
 for _ in 0..2_000_000{if c.regs[15]==0x03007000{return Ok(c.regs[0]);}gba_core::exec::step_hle(&mut c,&mut m.bus);}
 Err("Native job change did not return; no edit applied".into())
}
fn job_inventory(ram:&[u8])->Result<std::collections::BTreeMap<u16,(u8,u8)>,String>{
 let mut entries=std::collections::BTreeMap::new();let mut worn=std::collections::BTreeMap::<u16,u8>::new();
 for slot in 0..24{let at=0x80+slot*0x108;let r=ram.get(at..at+0x108).ok_or("Roster unavailable")?;
  if r[..4]==[0,0,0,0]{continue;}
  for o in (0x2a..0x34).step_by(2){let id=half(r,o)?;if id!=0{let n=worn.entry(id).or_default();*n=n.checked_add(1).ok_or("Equipped count overflow")?;}}
 }
 for at in (INVENTORY_START..INVENTORY_END).step_by(4){
  let id=half(ram,at)?;let total=ram[at+2];let equipped=ram[at+3];
  if id==0&&total==0&&equipped==0{continue;}
  if id==0||id>375||total>99||equipped>total||entries.insert(id,(total,equipped)).is_some(){return Err("Inventory format differs; no job change applied".into());}
  if worn.remove(&id).unwrap_or(0)!=equipped{return Err("Equipped inventory counters differ from the roster; no edit applied".into());}
 }
 if !worn.is_empty(){return Err("Equipped item is missing from inventory; no edit applied".into());}
 Ok(entries)
}
pub fn prepare_job(m:&gba_core::Machine,expected:&menu::CharacterInfo,target:u8)->Result<(usize,Vec<u8>,Vec<u8>),String>{
 let(member,jobs)=read(m,expected)?;
 if !jobs.iter().any(|j|j.id==target){return Err("Choose a regular job belonging to this race".into());}
 if target==member.record[7]{return Err("This job is already equipped".into());}
 if !(1..=50).contains(&member.level)||half(&member.record,0x16)?!=0||half(&member.record,0x28)?!=0{return Err("Select a member with no dispatch or status flags".into());}
 let before=job_inventory(&m.bus.ewram)?;let mut removed=std::collections::BTreeMap::<u16,u8>::new();
 for o in (0x2a..0x34).step_by(2){let id=half(&member.record,o)?;if id!=0{*removed.entry(id).or_default()+=1;}}
 let at=0x80+(member.slot-1)*0x108;let addr=0x02000000+at as u32;
 let snapshot=m.save_state();let mut private=gba_core::Machine::new(m.bus.rom.clone());private.load_state(&snapshot)?;
 job_call(&mut private,0x080cc57c,[addr,target as u32,0])?;
 let record=private.bus.ewram[at..at+0x108].to_vec();
 if record[7]!=target||record[6]!=member.record[6]||record[0x2a..0x34].iter().any(|&b|b!=0){return Err("Native job/equipment result failed validation".into());}
 // The game's routine keeps special character sprites and resets only ordinary sprites.
 if (record[4]==1&&record[5]!=target)||(record[4]!=1&&record[5]!=member.record[5]){return Err("Unexpected character sprite change".into());}
 for (i,(&new,&old)) in record.iter().zip(&member.record).enumerate(){
  let allowed=i==5||i==7||i==8||(0xb..0x15).contains(&i)||(0x2a..0x34).contains(&i)||(0x35..0x3e).contains(&i)||(0xf1..0x100).contains(&i);
  let progress=(0x41..0x40+member.record[0x34]as usize).contains(&i)&&new&127==old&127;
  if new!=old&&!allowed&&!progress{return Err(format!("Native job change modified unexpected member field {i:02x}"));}
 }
 let after=job_inventory(&private.bus.ewram)?;
 if before.len()!=after.len(){return Err("Inventory records changed unexpectedly".into());}
 for (&id,&(total,equipped)) in &before{
  let n=removed.get(&id).copied().unwrap_or(0);let expected=equipped.checked_sub(n).ok_or("Equipment counter underflow")?;
  if after.get(&id)!=Some(&(total,expected)){return Err(format!("Inventory conservation failed for item {id}; no edit applied"));}
 }
 let inventory=private.bus.ewram[INVENTORY_START..INVENTORY_END].to_vec();
 // Only equipped counters may change. Total ownership and record ordering stay fixed.
 for(i,(&new,&old))in inventory.iter().zip(&m.bus.ewram[INVENTORY_START..INVENTORY_END]).enumerate(){if new!=old&&i%4!=3{return Err("Unexpected inventory write".into());}}
 Ok((at,record,inventory))
}
#[cfg(test)]mod job_change_tests{
 use super::*;
 #[test]fn job_change_real_all_race_jobs_inventory_native_fields_reload_and_undo(){
  let rom=std::env::var("FFTA_TEST_ROM").expect("Set FFTA_TEST_ROM");let state=std::env::var("FFTA_JOB_STATE").expect("Set FFTA_JOB_STATE");
  let state=std::fs::read(state).unwrap();let mut m=gba_core::Machine::new(std::fs::read(rom).unwrap());m.load_state(&state).unwrap();let before=m.save_state();let members=crate::character_editor::read(&m).unwrap();let mut count=0;
  for race in 1..=5{
   let member=members.iter().find(|r|r.record[6]==race && half(&r.record,0x16).unwrap()==0 && half(&r.record,0x28).unwrap()==0).expect("Recording must contain an available member of all five races");
   for job in catalog(&m.bus.rom,&member.record).unwrap(){
    if job.id==member.record[7]{assert!(prepare_job(&m,member,job.id).is_err());count+=1;continue;}
    let(at,record,inventory)=prepare_job(&m,member,job.id).unwrap_or_else(|e|panic!("{}: {e}",job.name));
    assert_eq!(m.save_state(),before,"Preview must not change original state or RNG");
    assert_eq!(record[9],member.level);assert_eq!(record[10],member.exp);assert_eq!(&record[0x18..0x28],&member.record[0x18..0x28]);assert_eq!(&record[0xd2..0xd8],&member.record[0xd2..0xd8]);
    let mut expected=gba_core::Machine::new(m.bus.rom.clone());expected.load_state(&before).unwrap();job_call(&mut expected,0x080cc57c,[0x02000000+at as u32,job.id as u32,0]).unwrap();
    assert_eq!(record,&expected.bus.ewram[at..at+0x108]);assert_eq!(inventory,&expected.bus.ewram[INVENTORY_START..INVENTORY_END]);
    count+=1;
   }
  }assert_eq!(count,42);
  let member=members.iter().find(|r|r.slot==3).unwrap();let target=catalog(&m.bus.rom,&member.record).unwrap().into_iter().find(|j|j.id!=member.record[7]).unwrap().id;
  let foreign=(2..=43).find(|&j|m.bus.rom[JOBS+j as usize*52+4]!=member.record[6]).unwrap();assert!(prepare_job(&m,member,foreign).is_err());
  let mut stale=member.clone();stale.record[9]^=1;assert!(prepare_job(&m,&stale,target).is_err());
  let(at,record,inventory)=prepare_job(&m,member,target).unwrap();
  let dir=std::env::temp_dir().join(format!("ffta-job-change-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));let states=crate::manual_states::ManualStates::new(&dir);states.backup_editor(&m).unwrap();
  m.bus.ewram[at..at+0x108].copy_from_slice(&record);m.bus.ewram[INVENTORY_START..INVENTORY_END].copy_from_slice(&inventory);
  let mut fresh=gba_core::Machine::new(m.bus.rom.clone());fresh.load_state(&m.save_state()).unwrap();assert_eq!(fresh.bus.ewram,m.bus.ewram);states.undo_editor(&mut m).unwrap();assert_eq!(m.save_state(),before);std::fs::remove_dir_all(dir).unwrap();
  println!("All 42 race jobs: native job-change records, unchanged base stats, correct item counts, isolated previews, reload and Undo passed");
 }
}

#[cfg(test)]mod editor_comfort_race_master_tests{
 use super::*;
 #[test]fn editor_comfort_batch_master_every_race_matches_game_predicate(){
  let rom=std::env::var("FFTA_TEST_ROM").expect("Set FFTA_TEST_ROM");let state=std::env::var("FFTA_TEST_STATE").expect("Set FFTA_TEST_STATE");let mut m=gba_core::Machine::new(std::fs::read(rom).unwrap());m.load_state(&std::fs::read(state).unwrap()).unwrap();let original=m.save_state();let rows=crate::character_editor::read(&m).unwrap();let mut total=0;
  for race in 1..=5{let row=rows.iter().find(|r|r.record[6]==race).unwrap();let(_,jobs)=read(&m,row).unwrap();let changes:std::collections::BTreeMap<_,_>=jobs.iter().flat_map(|j|j.skills.iter().map(|s|(s.index,s.required))).collect();let list:Vec<_>=changes.into_iter().collect();
   let mut private=gba_core::Machine::new(m.bus.rom.clone());private.load_state(&original).unwrap();
   if jobs.iter().flat_map(|j|&j.skills).any(|s|s.current<s.required){let(at,record)=prepare(&m,row,&list).unwrap();private.bus.ewram[at..at+0x108].copy_from_slice(&record);}
   for job in jobs{assert!(super::abilities_tests::native_mastered(&mut private,0x80+(row.slot-1)*0x108,job.id));total+=1;}
  }assert_eq!(total,42);assert_eq!(m.save_state(),original);println!("Batch mastery: all 42 jobs across five races recognized by the game; active machine unchanged");
 }
}
