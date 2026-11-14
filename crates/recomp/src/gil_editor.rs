//! EUR Gil location verified against the user's purchase and edited-state test.
const GIL: usize = 0x1f64;
// Deliberately limited test range; this is not a claim about the game's maximum.
pub const TEST_MAX: u32 = 999_999;
pub fn read(ram: &[u8]) -> Result<u32,String> {
    let bytes=ram.get(GIL..GIL+4).ok_or("Gil memory unavailable")?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}
pub fn write(ram: &mut [u8], expected: u32, value: u32) -> Result<(),String> {
    if value>TEST_MAX { return Err("Test editor range: 0 to 999999 Gil".into()); }
    if read(ram)?!=expected { return Err("Gil changed; reopen the editor before applying".into()); }
    ram[GIL..GIL+4].copy_from_slice(&value.to_le_bytes());
    Ok(())
}
pub fn ready(machine: &gba_core::Machine) -> Result<u32,String> {
    if gba_core::ffta_rom::version(&machine.bus.rom)!=Some(gba_core::ffta_rom::Version::Europe) {
        return Err("Gil editing is verified for the original European ROM only".into());
    }
    if !crate::autosave::editor_world_map(&machine.bus) {
        return Err("Return to the unobstructed world map to edit Gil".into());
    }
    let value=read(&machine.bus.ewram)?;
    if value>TEST_MAX {return Err("Current Gil exceeds this test editor's range".into());}
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gil_edit_changes_only_verified_bytes_and_rejects_stale_or_out_of_range_edits() {
        let mut ram=vec![0xa5;262144];
        ram[GIL..GIL+4].copy_from_slice(&4650u32.to_le_bytes());
        let before=ram.clone();
        write(&mut ram,4650,5650).unwrap();
        assert_eq!(read(&ram).unwrap(),5650);
        assert_eq!(&ram[..GIL],&before[..GIL]);
        assert_eq!(&ram[GIL+4..],&before[GIL+4..]);
        let edited=ram.clone();
        assert!(write(&mut ram,4650,6000).is_err());
        assert!(write(&mut ram,5650,TEST_MAX+1).is_err());
        assert_eq!(ram,edited);
        write(&mut ram,5650,0).unwrap();assert_eq!(read(&ram).unwrap(),0);
        write(&mut ram,0,TEST_MAX).unwrap();assert_eq!(read(&ram).unwrap(),TEST_MAX);
        assert!(write(&mut [],0,10).is_err());
    }
}
