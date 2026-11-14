//! Staged Gil edit UI; persistence and game memory are owned by the runtime.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum GilAction { LawOpen,LawChanged, ShopOpen,ShopChanged, CloneOpen,CloneApply, LevelOpen,LevelPreview,LevelApply, AbilitiesOpen,AbilitiesApply,JobChangeApply, RenameApply, CharactersOpen, Open, Apply, Undo, ItemsOpen, ItemApply }
#[derive(Default)]
pub struct GilEditor {
    pub current: u32,
    pub proposed: u32,
    pub enabled: bool,
    pub undo_available: bool,
    pub status: String,
    step: usize,
}
impl GilEditor {
    pub fn open(&mut self, value: Result<u32,String>, undo: bool) {
        self.step=3;
        self.undo_available=undo;
        self.enabled=value.is_ok();
        match value {
            Ok(gil)=>{ self.current=gil;self.proposed=gil;self.status="Choose a new total, then Apply changes.".into(); }
            Err(e)=>{ self.current=0;self.proposed=0;self.status=e; }
        }
    }
    pub fn step(&self) -> u32 { [1,10,100,1000,10000,100000][self.step] }
    pub(super) fn set_step(&mut self,index:usize){self.step=index.min(5);}
    pub(super) fn reset_preview(&mut self){self.proposed=self.current;self.status="Preview reset. Your current funds are unchanged.".into();}
    pub(super) fn cycle_step(&mut self, right: bool) { self.step=if right {(self.step+1)%6} else {(self.step+5)%6}; }
    pub(super) fn adjust(&mut self, right: bool) {
        if !self.enabled {return;}
        self.proposed=if right {self.proposed.saturating_add(self.step()).min(999999)} else {self.proposed.saturating_sub(self.step())};
        self.status=if self.proposed==self.current{"No pending change.".into()}else{"Preview only. Apply changes to update your funds.".into()};
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gil_edits_stay_staged_and_disabled_editor_cannot_change_amount() {
        let mut editor=GilEditor::default();editor.open(Ok(4650),false);
        editor.adjust(true);assert_eq!(editor.proposed,5650);assert_eq!(editor.current,4650);
        editor.proposed=999999;editor.adjust(true);assert_eq!(editor.proposed,999999);
        editor.proposed=0;editor.adjust(false);assert_eq!(editor.proposed,0);
        editor.open(Err("Return to the map".into()),false);
        editor.adjust(true);assert_eq!(editor.proposed,0);assert!(!editor.enabled);
    }
}

pub use super::items::ItemEditor;
