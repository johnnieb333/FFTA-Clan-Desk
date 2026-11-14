//! Read-only character preview. No edit actions are exposed.
#[derive(Clone,Debug)]
pub struct CharacterInfo {
    pub slot:usize,pub identity:u8,pub record:Vec<u8>,pub name:String,pub race:String,pub job:String,
    pub level:u8,pub exp:u8,pub jp:u16,
}
#[derive(Default)]
pub struct CharacterPreview {pub sprites:std::collections::BTreeMap<usize,super::RosterSprite>,pub rows:Vec<CharacterInfo>,pub selected:usize,pub status:String,
    pub proposed:String,pub editing:bool,pub replace_text:bool,pub expected:Option<CharacterInfo>,pub rename_status:String}
impl CharacterPreview {
    pub fn open(&mut self,rows:Result<Vec<CharacterInfo>,String>){
        self.selected=0;self.editing=false;self.expected=None;
        match rows {Ok(rows)=>{self.rows=rows;self.status="Choose Rename, Jobs & Abilities, or Level & Stats.".into();},
            Err(error)=>{self.rows.clear();self.status=error;}}
    }
    pub fn current(&self)->Option<&CharacterInfo>{self.rows.get(self.selected)}
    pub fn rename_allowed(&self)->bool{self.current().is_some_and(|r|r.slot>2 && r.identity==1)}
    pub fn begin_rename(&mut self)->bool{
        if !self.rename_allowed(){self.status="Marche, Montblanc and unverified identities are locked.".into();return false;}
        self.expected=self.current().cloned();self.proposed=self.expected.as_ref().unwrap().name.clone();self.editing=false;
        self.rename_status="Use 1-9 letters/numbers, spaces, apostrophe or hyphen.".into();true
    }
    pub fn text(&mut self,text:&str,backspace:bool){
        if !self.editing{return;}
        if self.replace_text {self.proposed.clear();self.replace_text=false;if backspace{return;}}
        if backspace{self.proposed.pop();return;}
        for c in text.chars(){if self.proposed.len()<9 && (c.is_ascii_alphanumeric() || matches!(c,' '|'\''|'-')){self.proposed.push(c);}}
    }
    pub(super) fn cycle(&mut self,right:bool){
        let n=self.rows.len();if n==0{return;}
        self.selected=if right{(self.selected+1)%n}else{(self.selected+n-1)%n};
    }
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn character_editor_preview_cycles_without_modifying_character_values(){
        let row=|name:&str|CharacterInfo{slot:1,identity:2,record:Vec::new(),name:name.into(),race:"Human".into(),job:"Soldier".into(),level:3,exp:18,jp:1};
        let mut p=CharacterPreview::default();p.open(Ok(vec![row("Marche"),row("Cooper")]));
        p.cycle(false);assert_eq!(p.current().unwrap().name,"Cooper");p.cycle(true);assert_eq!(p.selected,0);
        assert_eq!(p.current().unwrap().exp,18);p.open(Err("Unavailable".into()));p.cycle(true);assert!(p.current().is_none());
    }
}
