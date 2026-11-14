//! Read-only EUR roster inspection. Editing, dispatch and removal are not enabled.
const START:usize=0x80;
const STRIDE:usize=0x108;
pub fn read(machine:&gba_core::Machine)->Result<Vec<menu::CharacterInfo>,String>{
    if gba_core::ffta_rom::version(&machine.bus.rom)!=Some(gba_core::ffta_rom::Version::Europe){return Err("Roster requires the verified European ROM".into());}
    if !crate::autosave::editor_world_map(&machine.bus){return Err("Return to the world map to inspect the roster.".into());}
    decode(&machine.bus.rom,&machine.bus.ewram,&machine.bus.ffta_names)
}
fn decode(rom:&[u8],ram:&[u8],names:&gba_core::ffta_names::NameBank)->Result<Vec<menu::CharacterInfo>,String>{
    let mut rows=Vec::new();
    for slot in 0..24 {
        let at=START+slot*STRIDE;let r=ram.get(at..at+STRIDE).ok_or("Roster memory unavailable")?;
        let ptr=u32::from_le_bytes(r[..4].try_into().unwrap());if ptr==0{continue;}
        let race=match r[6]{1=>"Human",2=>"Bangaa",3=>"Nu Mou",4=>"Viera",5=>"Moogle",_=>return Err(format!("Unverified race in clan slot {}",slot+1))};
        let name=if let Some(name)=names.text(ptr){name}else{crate::item_editor::unit_name(rom,ram,ptr)?};
        let job=crate::item_editor::unit_job(rom,r[7],r[6])?;
        let jp=u16::from_le_bytes([r[0xd6],r[0xd7]]);
        if name.is_empty() || job.is_empty() || r[9]==0 || r[10]>100 || jp>10{return Err(format!("Unverified character values in slot {}",slot+1));}
        rows.push(menu::CharacterInfo{slot:slot+1,identity:r[4],record:r.to_vec(),name,race:race.into(),job,level:r[9],exp:r[10],jp});
    }
    if rows.is_empty(){return Err("No clan members found".into());}Ok(rows)
}
pub fn prepare_rename(machine:&gba_core::Machine,expected:&menu::CharacterInfo,text:&str)->Result<(usize,u32,gba_core::ffta_names::NameBank),String>{
    let current=read(machine)?.into_iter().find(|r|r.slot==expected.slot).ok_or("Clan member is no longer present")?;
    if current.slot<=2 || current.identity!=1{return Err("Marche, Montblanc and unverified character identities cannot be renamed".into());}
    if current.record!=expected.record{return Err("Character changed; reopen Rename".into());}
    if text==current.name{return Err("No name change to apply".into());}
    let mut bank=machine.bus.ffta_names.clone();let ptr=bank.set(current.slot-1,text)?;
    Ok((START+(current.slot-1)*STRIDE,ptr,bank))
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn character_preview_reads_ram_names_and_rejects_unverified_job_identity(){
        let mut rom=vec![0;0x730000];let mut ram=vec![0;262144];
        ram[START..START+4].copy_from_slice(&0x02001f1cu32.to_le_bytes());
        ram[START+6]=1;ram[START+7]=2;ram[START+9]=3;ram[START+10]=18;ram[START+0xd6]=1;
        ram[0x1f1c..0x1f20].copy_from_slice(&[1,0xb1,0xcb,0]); // Aa
        let job=0x6e5464+2*52;rom[job+4]=1;rom[job..job+2].copy_from_slice(&400u16.to_le_bytes());
        let p=0x724c34+400*4;rom[p..p+4].copy_from_slice(&0x08728000u32.to_le_bytes());
        rom[0x728000..0x728004].copy_from_slice(&[1,0xc3,0xdd,0]); // Ss
        let before=ram.clone();let rows=decode(&rom,&ram,&Default::default()).unwrap();assert_eq!(rows.len(),1);assert_eq!(rows[0].name,"Aa");
        assert_eq!((rows[0].level,rows[0].exp,rows[0].jp),(3,18,1));assert_eq!(ram,before);
        rom[job+4]=2;assert!(decode(&rom,&ram,&Default::default()).is_err());
    }
}

#[cfg(test)]mod character_rename_real_tests {
    use super::*;
    #[test]fn character_rename_real_backup_preserves_main_members_and_restores_names(){
        let (Ok(rom_path),Ok(state_path))=(std::env::var("FFTA_TEST_ROM"),std::env::var("FFTA_TEST_STATE"))else{return;};
        let rom=std::fs::read(rom_path).unwrap();let state=std::fs::read(state_path).unwrap();
        let mut m=gba_core::Machine::new(rom.clone());m.load_state(&state).unwrap();let rows=read(&m).unwrap();
        for expected in rows.iter().filter(|r|r.slot<=2 || r.identity!=1){
            let mut different=expected.clone();different.name="Other".into();assert!(prepare_rename(&m,&different,"TestName").is_err());
        }
        let expected=rows.iter().find(|r|r.slot>2 && r.identity==1).unwrap().clone();
        let protected=m.bus.ewram[0x80..0x290].to_vec();let before=m.bus.ewram.clone();
        let (at,ptr,bank)=prepare_rename(&m,&expected,"TestName").unwrap();assert_eq!(m.bus.ewram,before);
        m.bus.ffta_names=bank;m.bus.ewram[at..at+4].copy_from_slice(&ptr.to_le_bytes());
        let renamed=read(&m).unwrap().into_iter().find(|r|r.slot==expected.slot).unwrap();assert_eq!(renamed.name,"TestName");
        assert_eq!(&m.bus.ewram[0x80..0x290],protected.as_slice());
        assert!(m.bus.ewram.iter().zip(&before).enumerate().all(|(i,(a,b))|(at..at+4).contains(&i)||a==b));
        let saved=m.save_state();let mut fresh=gba_core::Machine::new(rom.clone());fresh.load_state(&saved).unwrap();
        assert_eq!(read(&fresh).unwrap().into_iter().find(|r|r.slot==expected.slot).unwrap().name,"TestName");
        m.load_state(&state).unwrap();assert_eq!(read(&m).unwrap().into_iter().find(|r|r.slot==expected.slot).unwrap().name,expected.name);
        assert_eq!(m.bus.rom,rom);assert_eq!(m.save_state(),state);
        println!("Rename, protected identities, fresh load and Undo checks passed. No save files written.");
    }
}
