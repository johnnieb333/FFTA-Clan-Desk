//! EUR inventory records and ROM item/name tables checked against the user's dump.
const START: usize=0x1940;
const END: usize=0x1f1c;
const ITEM_TABLE: usize=0x6e0bd0;
const NAME_TABLE: usize=0x724c34;
const ITEM_COUNT: usize=376;
const EQUIPMENT_END:usize=0x1ee4; // Consumables occupy the final fourteen records.
const MYTHRIL:&[(u16,&str,u8)]=&[
    (9,"Mythril Sword",1),
    (27,"Mythril Blade",2),
    (37,"Mythril Saber",3),
    (48,"Mythril Brand",4),
    (85,"Mythril Knife",7),
    (101,"Mythril Rapier",8),
    (117,"Mythril Epee",9),
    (132,"Mythril Staff",10),
    (146,"Mythril Rod",11),
    (159,"Mythril Mace",12),
    (171,"Mythril Bow",13),
    (185,"Mythril Shot",14),
    (197,"Mythril Spear",15),
    (212,"Mythril Bell",16),
    (226,"Mythril Claws",17),
    (238,"Mythril Soul",18),
    (250,"Mythril Gun",19),
];
fn catalog_category(id:u16)->Option<u8>{match id{1..=252=>Some(1),253..=338=>Some(2),339..=361=>Some(3),362..=375=>Some(4),_=>None}}
fn empty_item(ram:&[u8],id:u16)->Option<usize>{
    match catalog_category(id)? {
        4=>(EQUIPMENT_END..END).step_by(4).find(|&o|ram.get(o..o+4)==Some(&[0,0,0,0][..])),
        _=>empty_equipment(ram),
    }
}
fn empty_equipment(ram:&[u8])->Option<usize>{
    (START..EQUIPMENT_END).step_by(4).find(|&o|ram.get(o..o+4)==Some(&[0,0,0,0][..]))
}

const TYPES: [&str;31]=["None","Sword","Blade","Saber","Knight Sword","Great Sword","Broad Sword","Knife","Rapier","Katana","Staff","Rod","Mace","Bow","Great Bow","Spear","Instrument","Knuckles","Soul","Gun","Shield","Helmet","Ribbon","Hat","Armor","Clothing","Robe","Shoes","Armlets","Accessory","Item"];
fn half(b:&[u8],p:usize)->Result<u16,String>{Ok(u16::from_le_bytes(b.get(p..p+2).ok_or("Truncated item data")?.try_into().unwrap()))}
fn glyph(mut c:u8,single:bool)->Option<char>{
    if single {c=c.checked_sub(1)?;}
    Some(match c {
        0xb0..=0xc9=>(b'A'+(c-0xb0)) as char,
        0xca..=0xe3=>(b'a'+(c-0xca)) as char,
        0xa6..=0xaf=>(b'0'+(c-0xa6)) as char,
        0x9f=>' ',0xe4=>'.',0xea=>'?',0xeb=>'!',0xec=>',',0xee=>':',0xef=>'_',
        0xf1=>'/',0xf2=>'~',0xf3|0xf4=>'\'',0xf5|0xf6=>'"',0xf7=>'(',0xf8=>')',
        0xf9=>'{',0xfa=>'}',0xfb=>'[',0xfc=>']',0xfd=>'+',0xfe=>'-',_=>return None,
    })
}
fn name(rom:&[u8],id:usize)->Result<String,String>{
    let record=ITEM_TABLE+id*32;
    let name_id=half(rom,record)? as usize;
    text_at(rom,NAME_TABLE,name_id)
}
pub(crate) fn text_at(rom:&[u8],table:usize,name_id:usize)->Result<String,String>{
    let at=table+name_id*4;
    let ptr=u32::from_le_bytes(rom.get(at..at+4).ok_or("Missing item name pointer")?.try_into().unwrap());
    let p=ptr.checked_sub(0x08000000).ok_or("Invalid item name pointer")? as usize;
    decode_name(rom,p)
}
fn decode_name(rom:&[u8],mut p:usize)->Result<String,String>{
    let table=p;let name_id=0;
    let single=*rom.get(p).ok_or("Invalid item name")?==1;
    if single {p+=1;}
    let mut result=String::new();
    for _ in 0..96 {
        let c=*rom.get(p).ok_or("Truncated item name")?;p+=1;
        if c==0 {return Ok(result);}
        if single {result.push(glyph(c,true).ok_or("Unsupported item name character")?);continue;}
        match c {
            0x80=>{let code=*rom.get(p).ok_or("Truncated item name")?;p+=1;
                if code==0 {return Ok(result);}
                result.push(glyph(code,false).ok_or("Unsupported item name character")?);}
            0x40=>{let sub=*rom.get(p).ok_or("Truncated item name")?;p+=1;
                match sub {0x53=>return Ok(result),0x73=>result.push(' '),0x3c=>p+=1,0x3e=>{p+=1;result.push(' ');},_=>return Err("Unsupported item name control".into())}}
            0x81=>{let code=*rom.get(p).ok_or("Truncated extended name character")?;p+=1;
                result.push(match code {1=>'=',2=>'<',3=>'>',7=>'%',8=>'&',9=>'*',12=>'|',
                    _=>return Err(format!("Unsupported extended name character 81 {code:02x} (table {table:06x}, name {name_id})"))});}
            0x9f=>result.push(' '),_=>return Err(format!("Unsupported name byte {c:02x} at ROM {:06x} (table {table:06x}, name {name_id})",p-1)),
        }
    }
    Err("Item name too long".into())
}
pub(crate) fn unit_name(rom:&[u8],ram:&[u8],ptr:u32)->Result<String,String>{
    if let Some(p)=ptr.checked_sub(0x08000000).filter(|p|(*p as usize)<rom.len()){return decode_name(rom,p as usize);}
    if let Some(p)=ptr.checked_sub(0x02000000).filter(|p|(*p as usize)<ram.len()){return decode_name(ram,p as usize);}
    Err("Unit name pointer is outside verified memory".into())
}
pub(crate) fn unit_job(rom:&[u8],id:u8,race:u8)->Result<String,String>{
    if id>=116 || rom.get(JOBS+id as usize*52+4)!=Some(&race){return Err("Unit race/job mismatch".into());}
    text_at(rom,NAME_TABLE,half(rom,JOBS+id as usize*52)? as usize)
}
const LEARNING:usize=0x6e3ad0;
const JOBS:usize=0x6e5464;
const RACE_ABILITIES:usize=0x6df4d4;
const ABILITY_NAMES:usize=0x720d40;
pub(crate) fn verify_teaching_tables(rom:&[u8])->Result<(),String>{
    for (id,expected) in [(221,"First Aid"),(267,"Cure"),(398,"Fire"),(559,"Combat Combo"),(336,"Boost")] {
        if text_at(rom,ABILITY_NAMES,id)?!=expected{return Err("Ability name table verification failed".into());}
    }
    for (id,expected) in [(2,"Soldier"),(13,"Warrior"),(8,"Black Mage")] {
        if text_at(rom,NAME_TABLE,half(rom,JOBS+id*52)? as usize)?!=expected{return Err("Job table verification failed".into());}
    }
    Ok(())
}
fn teachings(rom:&[u8],id:usize)->Result<Vec<(String,String)>,String>{
    let group=*rom.get(ITEM_TABLE+id*32+29).ok_or("Missing learning group")? as usize;
    if group==0{return Ok(Vec::new());}
    let base=LEARNING+group*20;
    let count=*rom.get(base).ok_or("Missing learning table")? as usize;
    if count>9{return Err("Unexpected learning group size".into());}
    let mut groups:Vec<(u16,String,Vec<String>)>=Vec::new();
    for i in 0..count {
        let pair=rom.get(base+2+i*2..base+4+i*2).ok_or("Truncated learning pair")?;
        let job=pair[0]as usize;let index=pair[1]as usize;
        // 255 marks equipment effects on shoes, not a job learning an ability.
        if job==255{continue;}
        if job>=116{return Err("Invalid learning job".into());}
        let j=JOBS+job*52;
        let race=*rom.get(j+4).ok_or("Missing job race")? as usize;
        if race==0 || race>=24{return Err("Invalid learning race".into());}
        let p=RACE_ABILITIES+race*4;
        let ptr=u32::from_le_bytes(rom.get(p..p+4).ok_or("Missing race ability table")?.try_into().unwrap());
        let ability=ptr.checked_sub(0x08000000).ok_or("Invalid ability pointer")? as usize+index*8;
        let name_id=half(rom,ability)?;
        let ability_name=text_at(rom,ABILITY_NAMES,name_id as usize)?;
        let job_name=text_at(rom,NAME_TABLE,half(rom,j)? as usize)?;
        if ability_name.is_empty() || job_name.is_empty(){return Err("Missing ability or job name".into());}
        if let Some(g)=groups.iter_mut().find(|g|g.0==name_id){if !g.2.contains(&job_name){g.2.push(job_name);}}
        else{groups.push((name_id,ability_name,vec![job_name]));}
    }
    if groups.len()>3{return Err("More than three distinct taught abilities; details unavailable".into());}
    Ok(groups.into_iter().map(|(_,name,jobs)|(name,jobs.join(", "))).collect())
}
fn location(machine:&gba_core::Machine)->Result<(),String>{
    if gba_core::ffta_rom::version(&machine.bus.rom)!=Some(gba_core::ffta_rom::Version::Europe) {return Err("Items require the verified original European ROM".into());}
    if !crate::autosave::editor_world_map(&machine.bus) {return Err("Return to the world map to edit inventory.".into());}
    Ok(())
}
fn records(ram:&[u8])->Result<Vec<(usize,u16,u8,u8)>,String>{
    if ram.len()<END {return Err("Inventory memory unavailable".into());}
    let mut seen=std::collections::BTreeSet::new();let mut result=Vec::new();
    for offset in (START..END).step_by(4) {
        let id=half(ram,offset)?;let total=ram[offset+2];let equipped=ram[offset+3];
        if id==0 && total==0 && equipped==0 {continue;}
        if id==0 || id as usize>=ITEM_COUNT || total>99 || equipped>total || !seen.insert(id) {
            return Err("Inventory layout differs from the verified format; editing locked".into());
        }
        result.push((offset,id,total,equipped));
    }
    Ok(result)
}
pub fn owned(machine:&gba_core::Machine)->Result<Vec<menu::OwnedItem>,String>{
    location(machine)?;
    let rom=&machine.bus.rom;
    verify_teaching_tables(rom)?;
    // An extra table anchor keeps a changed layout from producing plausible names.
    let cap=ITEM_TABLE+0x115*32;
    if name(rom,0x115)?!="Feather Cap"
        || half(rom,cap+4)?!=350 || rom.get(cap+8)!=Some(&23) {
        return Err("Item catalog verification failed".into());
    }
    // Validate the complete named set against the ROM before offering additions.
    for &(id,expected,typ) in MYTHRIL {
        if name(rom,id as usize)?!=expected || rom.get(ITEM_TABLE+id as usize*32+8)!=Some(&typ) {
            return Err("Mythril weapon catalog verification failed".into());
        }
    }
    let mut result=Vec::new();
    let entries=records(&machine.bus.ewram)?;
    for &(offset,id,total,equipped) in &entries {

        let record=ITEM_TABLE+id as usize*32;
        let typ=*rom.get(record+8).ok_or("Truncated item catalog")?;
        let category=match typ {1..=19=>1,20..=26=>2,27..=29=>3,30=>4,_=>5};
        let item_name=name(rom,id as usize)?;
        if item_name.is_empty() {return Err("An owned item's name is missing".into());}
        result.push(menu::OwnedItem{offset,id,name:item_name,kind:TYPES.get(typ as usize).unwrap_or(&"Other").to_string(),category,total,equipped,editable:category<=4,teaches:teachings(rom,id as usize)?});
    }
    // The verified EUR catalog has 361 equipment items and 14 consumables.
    // They are catalog rows only until the user confirms a nonzero quantity.
    for id in 1u16..=375 {
        let typ=*rom.get(ITEM_TABLE+id as usize*32+8).ok_or("Missing weapon type")?;
        let category=match typ {1..=19=>1,20..=26=>2,27..=29=>3,30=>4,_=>return Err("Item catalog verification failed".into())};
        if catalog_category(id)!=Some(category){return Err("Item category verification failed".into());}
        if entries.iter().any(|e|e.1==id){continue;}
        let item_name=name(rom,id as usize)?;
        if item_name.is_empty() || item_name=="-"{return Err("Item name verification failed".into());}
        let empty=empty_item(&machine.bus.ewram,id);
        result.push(menu::OwnedItem{offset:empty.unwrap_or(usize::MAX),id,name:item_name,
            kind:TYPES[typ as usize].into(),category,total:0,equipped:0,editable:empty.is_some(),teaches:teachings(rom,id as usize)?});
    }
    result.sort_by_key(|item|item.name.to_ascii_lowercase());
    Ok(result)
}
pub fn validate(ram:&[u8],expected:&menu::OwnedItem,total:u8)->Result<(),String>{
    let entries=records(ram)?;
    if !expected.editable || expected.category>4 {return Err("This item category is not enabled for editing".into());}
    if total==0 || total>99 || total<expected.equipped {return Err("Keep 1-99 total, at least the equipped count".into());}
    if let Some(record)=entries.into_iter().find(|(_,id,_,_)|*id==expected.id) {
        if record!=(expected.offset,expected.id,expected.total,expected.equipped) {return Err("Inventory changed; reopen the editor".into());}
    }else{
        if expected.total!=0 || expected.equipped!=0 || catalog_category(expected.id)!=Some(expected.category) {
            return Err("Only verified catalog items can be added".into());
        }
        if empty_item(ram,expected.id)!=Some(expected.offset) {return Err("Inventory changed or this storage area is full; reopen the editor".into());}
    }
    Ok(())
}
pub fn write(ram:&mut [u8],expected:&menu::OwnedItem,total:u8)->Result<(),String>{
    validate(ram,expected,total)?;
    if half(ram,expected.offset)?==0 {
        ram[expected.offset..expected.offset+4].copy_from_slice(&[expected.id as u8,(expected.id>>8)as u8,total,0]);
    }else{ram[expected.offset+2]=total;}
    Ok(())
}

/// Prepare the entire edit before taking a backup or changing live memory.
/// Each absent item is assigned its own empty slot in the correct storage area in the prepared copy.
pub fn prepare_batch(ram:&[u8],changes:&[(menu::OwnedItem,u8)])->Result<Vec<u8>,String>{
    if changes.is_empty(){return Err("No pending changes".into());}
    let mut seen=std::collections::BTreeSet::new();
    for (item,total) in changes {
        if !seen.insert(item.id){return Err("Duplicate item in pending changes".into());}
        validate(ram,item,*total)?;
    }
    let mut prepared=ram.to_vec();
    for (item,total) in changes {
        let mut target=item.clone();
        if !records(&prepared)?.iter().any(|e|e.1==target.id){
            target.offset=empty_item(&prepared,target.id).ok_or("Not enough free inventory slots; nothing was applied")?;
        }
        write(&mut prepared,&target,*total)?;
    }
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(offset:usize,id:u16,total:u8,equipped:u8)->menu::OwnedItem{
        menu::OwnedItem{offset,id,name:"Test".into(),kind:"Hat".into(),category:2,total,equipped,editable:true,teaches:Vec::new()}
    }
    #[test]
    fn item_editor_finds_moved_records_and_preserves_equipped_counts_and_other_items(){
        let mut ram=vec![0;262144];let cap=START+12;ram[cap..cap+4].copy_from_slice(&[0x15,1,3,1]);
        ram[START..START+4].copy_from_slice(&[1,0,2,2]);
        let expected=item(cap,277,3,1);let before=ram.clone();write(&mut ram,&expected,4).unwrap();
        assert!(ram.iter().zip(&before).enumerate().all(|(i,(a,b))|i==cap+2 || a==b));
        assert_eq!(ram[cap+3],1);assert_eq!(records(&ram).unwrap().len(),2);
        assert!(write(&mut ram,&expected,5).is_err());
        let updated=item(cap,277,4,1);assert!(write(&mut ram,&updated,0).is_err());assert!(write(&mut ram,&updated,100).is_err());
        let sword=item(START,1,2,2);assert!(write(&mut ram,&sword,1).is_err());
        ram[START+8..START+12].copy_from_slice(&[1,0,1,0]);assert!(write(&mut ram,&updated,5).is_err());
    }
    #[test]
    fn item_editor_reads_both_verified_text_encodings(){
        assert_eq!(glyph(0xb0,false),Some('A'));assert_eq!(glyph(0xb1,true),Some('A'));
        assert_eq!(glyph(0x9f,false),Some(' '));assert_eq!(glyph(0xa0,true),Some(' '));
    }
    #[test]
    fn item_editor_name_table_and_consumable_record_use_verified_offsets(){
        for single in [false,true]{
            let mut rom=vec![0;0x730000];let pos=0x7233b7;
            rom[ITEM_TABLE+277*32..ITEM_TABLE+277*32+2].copy_from_slice(&400u16.to_le_bytes());
            rom[NAME_TABLE+400*4..NAME_TABLE+400*4+4].copy_from_slice(&(0x08000000u32+pos as u32).to_le_bytes());
            let mut encoded=Vec::new();if single {encoded.push(1);}
            for ch in "Feather Cap".chars(){
                let value=if ch==' ' {0x9f} else if ch.is_ascii_uppercase(){0xb0+(ch as u8-b'A')}else{0xca+(ch as u8-b'a')};
                if !single {encoded.push(0x80);}encoded.push(value+u8::from(single));
            }
            encoded.push(0);rom[pos..pos+encoded.len()].copy_from_slice(&encoded);
            assert_eq!(name(&rom,277).unwrap(),"Feather Cap");
        }
        let mut ram=vec![0;262144];let offset=0x1ee8;
        ram[offset..offset+4].copy_from_slice(&[0x6f,1,4,0]);
        let mut expected=item(offset,367,4,0);expected.category=4;expected.kind="Item".into();
        write(&mut ram,&expected,5).unwrap();assert_eq!(&ram[offset..offset+4],&[0x6f,1,5,0]);
    }

    #[test]
    fn item_editor_adds_all_mythril_weapons_without_changing_existing_equipment_or_gil(){
        let mut ram=vec![0;262144];ram[START..START+4].copy_from_slice(&[1,0,2,2]);
        ram[EQUIPMENT_END..EQUIPMENT_END+4].copy_from_slice(&[106,1,10,0]);
        ram[0x1f64..0x1f68].copy_from_slice(&4300u32.to_le_bytes());
        let original=ram.clone();
        for &(id,name,_) in MYTHRIL {
            let offset=empty_equipment(&ram).unwrap();let mut e=item(offset,id,0,0);e.category=1;e.name=name.into();
            write(&mut ram,&e,1).unwrap();assert_eq!(&ram[offset..offset+4],&[id as u8,(id>>8)as u8,1,0]);
            let added=ram.clone();assert!(write(&mut ram,&e,2).is_err());assert_eq!(ram,added);
        }
        assert_eq!(records(&ram).unwrap().len(),19);
        assert_eq!(&ram[START..START+4],&original[START..START+4]);
        assert_eq!(&ram[START+18*4..],&original[START+18*4..]);
        assert_eq!(MYTHRIL.len(),17);
    }
    #[test]
    fn item_editor_addition_rejects_stale_slots_unknown_items_and_quantity_limits(){
        let mut ram=vec![0;262144];let mut e=item(START,9,0,0);e.category=1;
        for total in [0,100,255]{assert!(write(&mut ram,&e,total).is_err());}
        e.id=253;assert!(write(&mut ram,&e,1).is_err());e.id=9;
        e.offset=START+4;assert!(write(&mut ram,&e,1).is_err());e.offset=START;
        e.equipped=1;assert!(write(&mut ram,&e,1).is_err());e.equipped=0;
        e.offset=EQUIPMENT_END;assert!(write(&mut ram,&e,1).is_err());e.offset=START;
        ram[START..START+4].copy_from_slice(&[9,0,0,0]);write(&mut ram,&e,1).unwrap();
        let before=ram.clone();assert!(write(&mut ram,&e,2).is_err());assert_eq!(ram,before);
    }

    #[test]
    fn item_editor_batch_allocates_distinct_slots_and_rejects_every_change_if_one_is_stale(){
        let mut ram=vec![0;262144];ram[START..START+4].copy_from_slice(&[1,0,2,2]);
        let mut a=item(START+4,9,0,0);a.category=1;
        let mut b=item(START+4,27,0,0);b.category=1;
        let existing=item(START,1,2,2);let before=ram.clone();
        let changes=vec![(a.clone(),1),(b.clone(),2),(existing.clone(),3)];
        let result=prepare_batch(&ram,&changes).unwrap();assert_eq!(ram,before);
        assert_eq!(&result[START..START+12],&[1,0,3,2,9,0,1,0,27,0,2,0]);
        assert_eq!(&result[START+12..],&before[START+12..]);
        ram[START+2]=4;let stale=ram.clone();assert!(prepare_batch(&ram,&changes).is_err());assert_eq!(ram,stale);
        assert!(prepare_batch(&before,&[(a.clone(),1),(a,2)]).is_err());
        assert!(prepare_batch(&before,&[(b,1),(existing,1)]).is_err());
    }

    #[test]
    fn item_editor_teaching_groups_three_spells_and_deduplicates_racial_jobs(){
        let mut rom=vec![0;0x730000];
        fn put_text(rom:&mut[u8],table:usize,id:usize,at:usize,text:&str){
            rom[table+id*4..table+id*4+4].copy_from_slice(&(0x08000000u32+at as u32).to_le_bytes());
            let mut bytes=vec![1];
            for c in text.bytes(){bytes.push(if c==b' '{0xa0}else if c.is_ascii_uppercase(){0xb1+c-b'A'}else{0xcb+c-b'a'});}
            bytes.push(0);rom[at..at+bytes.len()].copy_from_slice(&bytes);
        }
        put_text(&mut rom,NAME_TABLE,400,0x728000,"Black Mage");
        for (k,(id,label)) in [(398,"Fire"),(285,"Thunder"),(419,"Blizzard")].iter().enumerate(){
            put_text(&mut rom,ABILITY_NAMES,*id,0x728100+k*64,label);
        }
        for &(job,race) in &[(8usize,1usize),(21,3),(42,5)]{
            rom[JOBS+job*52..JOBS+job*52+2].copy_from_slice(&400u16.to_le_bytes());rom[JOBS+job*52+4]=race as u8;
            let base=0x6f0000+race*0x1000;
            rom[RACE_ABILITIES+race*4..RACE_ABILITIES+race*4+4].copy_from_slice(&(0x08000000u32+base as u32).to_le_bytes());
            for (index,id) in [(0,398u16),(1,285),(2,419)]{rom[base+index*8..base+index*8+2].copy_from_slice(&id.to_le_bytes());}
        }
        rom[ITEM_TABLE+135*32+29]=7;let base=LEARNING+7*20;rom[base]=9;
        for k in 0..9 {rom[base+2+k*2]=[8,21,42][k%3];rom[base+3+k*2]=(k/3)as u8;}
        assert_eq!(teachings(&rom,135).unwrap(),vec![("Fire".into(),"Black Mage".into()),("Thunder".into(),"Black Mage".into()),("Blizzard".into(),"Black Mage".into())]);
        assert!(teachings(&rom,277).unwrap().is_empty());
        rom[base]=1;rom[base+2]=255;assert!(teachings(&rom,135).unwrap().is_empty());
        rom[base]=10;assert!(teachings(&rom,135).is_err());
    }

    #[test]
    fn item_editor_unowned_weapon_batch_accepts_catalog_boundaries_and_rejects_armour(){
        let ram=vec![0;262144];let mut changes=Vec::new();
        for id in [1u16,2,135,252]{let mut e=item(START,id,0,0);e.category=1;changes.push((e,1));}
        let prepared=prepare_batch(&ram,&changes).unwrap();assert_eq!(records(&prepared).unwrap().len(),4);
        for (i,id) in [1u16,2,135,252].iter().enumerate(){assert_eq!(half(&prepared,START+i*4).unwrap(),*id);}
        for id in [0u16,253,375,376]{let mut e=item(START,id,0,0);e.category=1;assert!(prepare_batch(&ram,&[(e,1)]).is_err());}
        assert!(ram.iter().all(|&b|b==0));
    }

    #[test]
    fn item_editor_extended_name_punctuation_uses_the_original_text_format(){
        let mut rom=vec![0;512];rom[..4].copy_from_slice(&0x08000080u32.to_le_bytes());
        for (code,ch) in [(1,'='),(2,'<'),(3,'>'),(7,'%'),(8,'&'),(9,'*'),(12,'|')]{
            rom[128..135].copy_from_slice(&[0x80,0xb0,0x81,code,0x80,0xb1,0]);
            assert_eq!(text_at(&rom,0,0).unwrap(),format!("A{ch}B"));
        }
        rom[131]=255;assert!(text_at(&rom,0,0).unwrap_err().contains("81 ff"));
    }
    #[test]
    fn item_editor_all_catalog_names_and_abilities_from_local_rom(){
        let Ok(path)=std::env::var("FFTA_TEST_ROM")else{return;};
        let rom=std::fs::read(path).expect("Read local ROM for the read-only catalog check");
        assert_eq!(gba_core::ffta_rom::version(&rom),Some(gba_core::ffta_rom::Version::Europe));
        verify_teaching_tables(&rom).unwrap();
        for id in 1..=375 {
            let label=name(&rom,id).unwrap_or_else(|e|panic!("Weapon {id}: {e}"));
            assert!(!label.is_empty(),"Weapon {id} has no name");
            let rows=teachings(&rom,id).unwrap_or_else(|e|panic!("Weapon {id} ({label}): {e}"));
            assert!(rows.len()<=3);
        }
        println!("All 375 item names and teaching details decoded. No ROM, save or state files written.");
    }

    #[test]
    fn item_editor_mixed_batch_places_consumables_in_their_separate_storage(){
        let mut ram=vec![0;262144];ram[EQUIPMENT_END..EQUIPMENT_END+4].copy_from_slice(&[106,1,10,0]);
        ram[0x1f64..0x1f68].copy_from_slice(&5000u32.to_le_bytes());let original=ram.clone();
        let mut armour=item(START,288,0,0);armour.category=2;
        let mut accessory=item(START,339,0,0);accessory.category=3;
        let mut ether=item(EQUIPMENT_END+4,365,0,0);ether.category=4;
        let changes=vec![(ether.clone(),5),(armour,1),(accessory,2)];
        let prepared=prepare_batch(&ram,&changes).unwrap();assert_eq!(ram,original);
        assert_eq!(&prepared[START..START+8],&[32,1,1,0,83,1,2,0]);
        assert_eq!(&prepared[EQUIPMENT_END..EQUIPMENT_END+8],&[106,1,10,0,109,1,5,0]);
        assert_eq!(&prepared[END..],&original[END..]);
        ether.offset=START;assert!(prepare_batch(&ram,&[(ether.clone(),5)]).is_err());
        ether.offset=EQUIPMENT_END+4;ether.category=1;assert!(prepare_batch(&ram,&[(ether,5)]).is_err());
    }

}
