//! Independent manual snapshots. Never writes the game's normal save slots.
use std::{fs, io::Write, path::{Path, PathBuf}};

pub struct ManualStates {
    dir: PathBuf,
    pub slot: usize,
}
fn replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    let mut file = fs::File::create(&tmp).map_err(|e| e.to_string())?;
    file.write_all(bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
    drop(file);
    if path.is_file() {
        gba_core::state::preview(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Existing state is damaged; preserve or recover it before overwriting: {e}"))?;
        fs::copy(path, path.with_extension("bak")).map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}
fn read_state(path: &Path) -> Result<Vec<u8>, String> {
    let size = fs::metadata(path).map_err(|e| format!("{}: {e}",path.display()))?.len();
    if size > 8 * 1024 * 1024 { return Err("State file is too large".into()); }
    fs::read(path).map_err(|e| e.to_string())
}
impl ManualStates {
    pub fn new(save_dir: &Path) -> Self {
        Self { dir: save_dir.join("ManualStates"), slot: 1 }
    }
    pub fn select(&mut self, next: bool) {
        self.slot = if next { self.slot % 30 + 1 } else { (self.slot + 28) % 30 + 1 };
    }
    fn path(&self) -> PathBuf {
        self.dir.join(format!("slot-{:02}.state", self.slot))
    }
    pub fn card(&self) -> menu::StateCard {
        let path = self.path();
        let mut card = menu::StateCard::default();
        card.occupied = path.is_file();
        card.undo_load = self.dir.join("before-load.state").is_file();
        if let Ok(bytes) = read_state(&path) {
            if let Ok((_, image)) = gba_core::state::preview(&bytes) { card.image = image; }
            if let Ok(previous) = read_state(&path.with_extension("bak")) {
                card.undo_save = previous != bytes && gba_core::state::preview(&previous).is_ok();
            }
        }
        card.saved = fs::metadata(&path).and_then(|m| m.modified()).ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| {
                let c = gba_core::hostclock::from_linear(d.as_secs() as i64);
                format!("Saved {:04}-{:02}-{:02} {:02}:{:02} UTC", c.year,c.month,c.day,c.hour,c.min)
            }).unwrap_or_else(|| "Save time unavailable".into());
        card
    }
    pub fn undo_save(&self) -> Result<(), String> {
        let path = self.path();
        let previous = read_state(&path.with_extension("bak"))?;
        gba_core::state::preview(&previous)?;
        let current = read_state(&path)?;
        if current == previous { return Err("This save has already been undone".into()); }
        // Keep the overwritten version recoverable, but leave .bak as the
        // one-level undo target. Never replace a good backup with a bad slot.
        let recovery = path.with_extension("undone");
        let mut file = fs::File::create(&recovery).map_err(|e| e.to_string())?;
        file.write_all(&current).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
        drop(file);
        let tmp = path.with_extension("tmp");
        let mut file = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        file.write_all(&previous).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(tmp,path).map_err(|e| e.to_string())
    }
    pub fn save(&self, machine: &gba_core::Machine) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        replace(&self.path(), &machine.save_state())
    }
    pub fn backup_editor(&self, machine: &gba_core::Machine) -> Result<(),String> {
        fs::create_dir_all(&self.dir).map_err(|e|e.to_string())?;
        replace(&self.dir.join("before-editor.state"),&machine.save_state())
    }
    pub fn has_editor_backup(&self) -> bool { self.dir.join("before-editor.state").is_file() }
    pub fn undo_editor(&self, machine: &mut gba_core::Machine) -> Result<(),String> {
        self.load_path(machine,self.dir.join("before-editor.state"),true)
    }
    pub fn load(&self, machine: &mut gba_core::Machine, undo: bool) -> Result<(), String> {
        let path = if undo { self.dir.join("before-load.state") } else { self.path() };
        self.load_path(machine,path,!undo)
    }
    pub fn load_external(&self,machine:&mut gba_core::Machine,bytes:&[u8])->Result<(),String>{
        gba_core::state::preview(bytes)?;let current=machine.save_state();
        if bytes.get(8..72)!=current.get(8..72){return Err("Autosave belongs to another ROM or BIOS".into());}
        fs::create_dir_all(&self.dir).map_err(|e|e.to_string())?;
        replace(&self.dir.join("before-load.state"),&current)?;machine.load_state(bytes)
    }
    fn load_path(&self, machine: &mut gba_core::Machine, path: PathBuf, make_undo: bool) -> Result<(),String> {
        // Bound the read before allocating. Machine::load_state validates the
        // ROM, BIOS, checksum and complete payload before changing the machine.
        let bytes = read_state(&path)?;
        gba_core::state::preview(&bytes)?;
        let current = machine.save_state();
        if bytes.get(8..40) != current.get(8..40) {
            return Err("State belongs to another ROM".into());
        }
        if bytes.get(40..72) != current.get(40..72) {
            return Err("State uses another BIOS".into());
        }
        if make_undo {
            // Abort the load if the recovery snapshot cannot be durably saved.
            replace(&self.dir.join("before-load.state"), &current)?;
        }
        machine.load_state(&bytes)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slots_are_separate_and_bad_load_preserves_machine() {
        let dir = std::env::temp_dir().join(format!("ffta-manual-states-{}", std::process::id()));
        let mut states = ManualStates::new(&dir);
        let mut m = gba_core::Machine::new(vec![0xfe, 0xff, 0xff, 0xea]);
        m.cpu.cpsr = 0x1f;
        m.bus.ewram[0x89] = 3;
        states.save(&m).unwrap();
        states.select(true);
        m.bus.ewram[0x89] = 9;
        states.save(&m).unwrap();
        states.select(false);
        states.load(&mut m, false).unwrap();
        assert_eq!(m.bus.ewram[0x89], 3);
        states.load(&mut m, true).unwrap();
        assert_eq!(m.bus.ewram[0x89], 9);
        m.bus.ewram[0x89] = 12;
        states.save(&m).unwrap();
        assert!(states.path().with_extension("bak").is_file());
        let current_game = m.save_state();
        assert!(states.card().undo_save);
        states.undo_save().unwrap();
        assert_eq!(m.save_state(),current_game,"Undo Save must only edit disk");
        assert!(!states.card().undo_save);
        assert!(states.undo_save().is_err());
        states.load(&mut m,false).unwrap();
        assert_eq!(m.bus.ewram[0x89],3);
        let good = m.save_state();
        fs::write(states.path(), b"bad state").unwrap();
        assert!(states.load(&mut m, false).is_err());
        assert_eq!(good, m.save_state());
        assert!(!dir.join("QuickSave").exists());
        assert!(!dir.join("Save1").exists());
        for _ in 0..30 { states.select(true); }
        assert_eq!(states.slot, 1);
        states.select(false);
        assert_eq!(states.slot, 30);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn gil_editor_backup_restores_pre_edit_machine_without_touching_slots() {
        let dir=std::env::temp_dir().join(format!("ffta-gil-editor-backup-{}",std::process::id()));
        let states=ManualStates::new(&dir);
        let mut machine=gba_core::Machine::new(vec![0xfe,0xff,0xff,0xea]);
        machine.bus.ewram[0x1f64..0x1f68].copy_from_slice(&4650u32.to_le_bytes());
        states.backup_editor(&machine).unwrap();
        machine.bus.ewram[0x1f64..0x1f68].copy_from_slice(&5650u32.to_le_bytes());
        states.undo_editor(&mut machine).unwrap();
        assert_eq!(crate::gil_editor::read(&machine.bus.ewram).unwrap(),4650);
        assert!(states.has_editor_backup());
        assert!(!states.path().exists());
        assert!(!dir.join("QuickSave").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn character_names_undo_then_restart_load_accepts_automatic_recovery_snapshot(){
        let dir=std::env::temp_dir().join(format!("ffta-name-recovery-{}",std::process::id()));
        let states=ManualStates::new(&dir);let rom=vec![0xfe,0xff,0xff,0xea];
        let mut m=gba_core::Machine::new(rom.clone());
        states.save(&m).unwrap(); // The user only saved BEFORE renaming.
        states.backup_editor(&m).unwrap();
        m.bus.ffta_names.set(2,"aBc123").unwrap();
        states.undo_editor(&mut m).unwrap(); // Automatically saves the named recovery state.
        assert_eq!(m.bus.ffta_names,gba_core::ffta_names::NameBank::default());
        let recovery=std::fs::read(states.dir.join("before-load.state")).unwrap();
        assert!(gba_core::state::preview(&recovery).is_ok());
        let mut fresh=gba_core::Machine::new(rom);states.load(&mut fresh,false).unwrap();
        assert_eq!(fresh.bus.ffta_names,gba_core::ffta_names::NameBank::default());
        // Also check overwriting and recovering a genuinely named slot.
        fresh.bus.ffta_names.set(2,"NewName").unwrap();states.save(&fresh).unwrap();
        fresh.bus.ffta_names.set(2,"Other").unwrap();states.save(&fresh).unwrap();states.undo_save().unwrap();
        states.load(&mut fresh,false).unwrap();assert_eq!(fresh.bus.ffta_names.text(gba_core::ffta_names::BASE+32).as_deref(),Some("NewName"));
        std::fs::remove_dir_all(dir).unwrap();
    }

}

#[cfg(test)]mod save_browser_manual_tests{
 use super::*;
 #[test]fn save_browser_slot_thirty_loads_without_changing_existing_slot_one(){
  let dir=std::env::temp_dir().join(format!("ffta-manual-thirty-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));let mut states=ManualStates::new(&dir);let mut m=gba_core::Machine::new(vec![0;256]);states.save(&m).unwrap();let first=fs::read(states.path()).unwrap();states.slot=30;m.bus.ewram[0x90]=30;states.save(&m).unwrap();m.bus.ewram[0x90]=0;states.load(&mut m,false).unwrap();assert_eq!(m.bus.ewram[0x90],30);assert_eq!(fs::read(dir.join("ManualStates/slot-01.state")).unwrap(),first);states.select(true);assert_eq!(states.slot,1);fs::remove_dir_all(dir).unwrap();
 }
}
