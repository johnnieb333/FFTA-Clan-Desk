//! A new name, one confirmation, and full-state Undo.
use super::{canvas::Canvas,CharacterInfo,GilAction,MenuInput};
#[derive(Default)]pub struct CloneEditor{pub member:Option<CharacterInfo>,pub destination:usize,pub name:String,pub status:String,pub editing:bool,pub undo_available:bool,replace:bool,area:u8,confirm:Option<bool>,accept:bool,caret_started:Option<std::time::Instant>,caret_visible:bool}
pub(super)enum Event{None,Back,Action(GilAction)}
#[derive(Clone,Copy)]struct Rect(f32,f32,f32,f32);
impl Rect{fn hit(self,x:f32,y:f32)->bool{x>=self.0&&x<self.0+self.2&&y>=self.1&&y<self.1+self.3}}
const NAME:Rect=Rect(44.,180.,812.,46.);const APPLY:Rect=Rect(44.,440.,245.,44.);const UNDO:Rect=Rect(310.,440.,245.,44.);const BACK:Rect=Rect(580.,440.,276.,44.);
const CANCEL:Rect=Rect(210.,350.,220.,44.);const ACCEPT:Rect=Rect(470.,350.,220.,44.);
impl CloneEditor{
 pub fn open(&mut self,result:Result<(CharacterInfo,usize),String>,undo:bool){*self=Self::default();self.undo_available=undo;match result{Ok((r,slot))=>{self.member=Some(r);self.destination=slot;self.status="Choose a new name, then Apply clone.".into();},Err(e)=>self.status=e}}
 pub fn result(&mut self,status:String,success:bool,undo:bool){self.confirm=None;self.accept=false;self.editing=false;self.status=status;self.undo_available=undo;if success{self.member=None;self.name.clear();self.area=3;}}
 fn begin_text(&mut self){self.editing=true;self.replace=true;self.reset_caret();}
 fn reset_caret(&mut self){self.caret_started=Some(std::time::Instant::now());self.caret_visible=true;}
 fn update_caret(&mut self,millis:u128)->bool{let visible=(millis/500)%2==0;let changed=self.caret_visible!=visible;self.caret_visible=visible;changed}
 pub fn tick(&mut self)->bool{if !self.editing{return false;}let millis=self.caret_started.map(|t|t.elapsed().as_millis()).unwrap_or(0);self.update_caret(millis)}
 fn field_text(&self)->String{if self.editing{format!("{}{}",self.name,if self.caret_visible{"|"}else{" "})}else if self.name.is_empty(){"Click here and type a new name".into()}else{self.name.clone()}}
 pub fn finish_text(&mut self){self.editing=false;}
 pub fn text(&mut self,text:&str,backspace:bool){if !self.editing{return;}self.reset_caret();if self.replace{self.name.clear();self.replace=false;if backspace{return;}}if backspace{self.name.pop();return;}for c in text.chars(){if self.name.len()<9&&(c.is_ascii_alphanumeric()||matches!(c,' '|'\''|'-')){self.name.push(c);}}}
 fn request(&mut self,apply:bool){self.editing=false;if apply{let Some(r)=&self.member else{return;};if self.name.trim().is_empty()||self.name.trim().eq_ignore_ascii_case(&r.name){self.status="Enter a new name for the clone first.".into();self.area=0;return;}}else if !self.undo_available{return;}self.confirm=Some(apply);self.accept=false;}
 pub(super)fn handle(&mut self,input:MenuInput)->Event{
  if self.editing{if matches!(input,MenuInput::Confirm|MenuInput::Cancel){self.editing=false;}return Event::None;}
  if let Some(apply)=self.confirm{match input{MenuInput::Cancel=>self.confirm=None,MenuInput::Left|MenuInput::Right|MenuInput::Up|MenuInput::Down=>self.accept=!self.accept,MenuInput::Confirm=>{self.confirm=None;if self.accept{return Event::Action(if apply{GilAction::CloneApply}else{GilAction::Undo});}}}return Event::None;}
  match input{MenuInput::Cancel=>return Event::Back,MenuInput::Up=>self.area=self.area.saturating_sub(1),MenuInput::Down=>self.area=(self.area+1).min(3),MenuInput::Left|MenuInput::Right=>{},MenuInput::Confirm=>match self.area{0=>{if self.member.is_some(){self.begin_text();}},1=>self.request(true),2=>self.request(false),3=>return Event::Back,_=>{}}}Event::None
 }
 fn point(x:usize,y:usize,w:usize,h:usize)->(f32,f32){let s=(w as f32/900.).min(h as f32/600.);((x as f32-(w as f32-900.*s)/2.)/s,(y as f32-(h as f32-600.*s)/2.)/s)}
 pub(super)fn hover(&mut self,x:usize,y:usize,w:usize,h:usize){if self.confirm.is_some(){let(x,y)=Self::point(x,y,w,h);if CANCEL.hit(x,y){self.accept=false;}if ACCEPT.hit(x,y){self.accept=true;}}}
 pub(super)fn scroll(&mut self,up:bool){if self.confirm.is_none()&&!self.editing{self.area=if up{self.area.saturating_sub(1)}else{(self.area+1).min(3)};}}
 pub(super)fn click(&mut self,x:usize,y:usize,w:usize,h:usize,left:bool)->Event{if !left{return self.handle(MenuInput::Cancel);}let(x,y)=Self::point(x,y,w,h);if self.confirm.is_some(){if CANCEL.hit(x,y)||ACCEPT.hit(x,y){self.accept=ACCEPT.hit(x,y);return self.handle(MenuInput::Confirm);}return Event::None;}for(r,area)in[(NAME,0),(APPLY,1),(UNDO,2),(BACK,3)]{if r.hit(x,y){self.editing=false;self.area=area;return self.handle(MenuInput::Confirm);}}Event::None}
 pub(super)fn draw(&self,px:&mut[[u8;4]],w:usize,h:usize){
  if w<32||h<32||px.len()<w*h{return;}for p in px.iter_mut().take(w*h){p[0]=(p[0]as u16*5/16+5)as u8;p[1]=(p[1]as u16*4/16+3)as u8;p[2]=(p[2]as u16*3/16+1)as u8;}
  let mut c=Canvas::new(px,w,h);let s=(w as f32/900.).min(h as f32/600.);let mut ui=Painter{c:&mut c,s,ox:(w as f32-900.*s)/2.,oy:(h as f32-600.*s)/2.};
  ui.panel(Rect(20.,20.,860.,560.),false);ui.text(44.,42.,"CLONE MEMBER",24.,true,812.);
  let source=self.member.as_ref().map(|r|format!("Source: {}   {} / {}   Level {}",r.name,r.race,r.job,r.level)).unwrap_or("Reopen Clone member to select a source.".into());ui.text(44.,100.,&source,19.,false,812.);
  ui.text(44.,143.,&format!("New name (1-9 characters)   /   Clan slot {}",self.destination),17.,false,812.);ui.panel(NAME,self.area==0||self.editing);
  let name=self.field_text();ui.text(60.,194.,&name,19.,false,780.);
  for(i,text)in["Copies race, job, level, base stats, EXP, JP and ability progress.","The new member starts healthy and unequipped.","Equipment and inventory stay unchanged.","Marche, Montblanc and dispatched members cannot be cloned.","Apply saves a full recovery state. Undo restores that state."].iter().enumerate(){ui.text(44.,257.+i as f32*31.,text,17.,false,812.);}
  for(r,label,area)in[(APPLY,"Apply clone",1),(UNDO,"Undo Last Edit",2),(BACK,"Back",3)]{ui.panel(r,self.area==area);ui.text(r.0+14.,r.1+13.,label,17.,false,r.2-28.);}
  ui.text(44.,514.,&self.status,17.,false,812.);ui.text(44.,552.,if self.editing{"Type the new name. Enter: finish   Backspace: delete"}else{"Arrows: navigate   Confirm: select   Back: return"},17.,false,812.);
  if let Some(apply)=self.confirm{ui.panel(Rect(180.,210.,540.,206.),false);ui.text(210.,230.,if apply{"CREATE CLONE?"}else{"UNDO LAST EDIT?"},23.,true,480.);
   let text=if apply{format!("Add {} to clan slot {}?",self.name.trim(),self.destination)}else{"Restore the full state before the last edit?".into()};ui.text(210.,276.,&text,17.,false,480.);ui.text(210.,309.,if apply{"The source member stays unchanged."}else{"Later gameplay changes will also be undone."},17.,false,480.);
   for(r,label,active)in[(CANCEL,"Cancel",!self.accept),(ACCEPT,"Confirm",self.accept)]{ui.panel(r,active);ui.text(r.0+20.,r.1+14.,label,17.,false,r.2-40.);}
  }drop(ui);c.finish(px);
 }
}
struct Painter<'a>{c:&'a mut Canvas,s:f32,ox:f32,oy:f32}
impl Painter<'_>{
 fn panel(&mut self,r:Rect,active:bool){self.c.rect(self.ox+r.0*self.s,self.oy+r.1*self.s,r.2*self.s,r.3*self.s,4.*self.s,if active{[255,231,158]}else{[251,244,226]},if active{[242,208,123]}else{[235,220,185]},[184,146,65]);}
 fn text(&mut self,x:f32,y:f32,text:&str,size:f32,title:bool,width:f32){self.c.text(self.ox+x*self.s,self.oy+y*self.s,text,[35,24,16],size.max(15.)*self.s,title,width*self.s);}
}
#[cfg(test)]mod clone_caret_tests{
 use super::*;
 #[test]fn clone_caret_is_visible_on_focus_blinks_and_resets_on_typing(){
  let mut e=CloneEditor::default();assert!(e.field_text().starts_with("Click here"));
  e.begin_text();assert_eq!(e.field_text(),"|");assert!(!e.update_caret(499));
  assert!(e.update_caret(500));assert_eq!(e.field_text()," ");assert!(e.update_caret(1000));assert_eq!(e.field_text(),"|");
  e.update_caret(1500);e.text("aB3",false);assert_eq!(e.field_text(),"aB3|");
  e.finish_text();assert_eq!(e.field_text(),"aB3");assert!(!e.tick());
 }
}
#[cfg(test)]mod clone_ui_tests{
 use super::*;
 fn editor()->CloneEditor{let r=CharacterInfo{slot:3,identity:1,record:vec![0;0x108],name:"Cooper".into(),race:"Human".into(),job:"Soldier".into(),level:3,exp:18,jp:2};let mut e=CloneEditor::default();e.open(Ok((r,7)),true);e}
 #[test]fn clone_name_staging_and_confirmation_default_to_cancel(){
  let mut e=editor();e.request(true);assert!(e.confirm.is_none());e.handle(MenuInput::Confirm);assert!(e.editing);e.text("aB3-!",false);assert_eq!(e.name,"aB3-");e.finish_text();assert_eq!(e.member.as_ref().unwrap().name,"Cooper");
  e.request(true);assert_eq!(e.confirm,Some(true));assert!(!e.accept);assert!(matches!(e.handle(MenuInput::Confirm),Event::None));assert!(e.confirm.is_none());
  e.request(true);e.handle(MenuInput::Right);assert!(matches!(e.handle(MenuInput::Confirm),Event::Action(GilAction::CloneApply)));e.result("Added".into(),true,true);e.request(true);assert!(e.confirm.is_none());e.request(false);e.handle(MenuInput::Right);assert!(matches!(e.handle(MenuInput::Confirm),Event::Action(GilAction::Undo)));
 }
 #[test]fn clone_scaled_mouse_text_entry_back_and_overlay(){
  for(w,h)in[(852,480),(1920,1080)]{let mut e=editor();let s=(w as f32/900.).min(h as f32/600.);let ox=(w as f32-900.*s)/2.;let oy=(h as f32-600.*s)/2.;let point=|x:f32,y:f32|((ox+x*s)as usize,(oy+y*s)as usize);
   let(x,y)=point(100.,200.);e.click(x,y,w,h,true);assert!(e.editing);e.text("NewUnit",false);e.finish_text();let(x,y)=point(100.,460.);e.click(x,y,w,h,true);assert_eq!(e.confirm,Some(true));
   let mut pixels=vec![[128,128,128,255];w*h];e.draw(&mut pixels,w,h);assert_ne!(pixels[0],[128,128,128,255]);let(x,y)=point(550.,370.);assert!(matches!(e.click(x,y,w,h,true),Event::Action(GilAction::CloneApply)));assert!(matches!(e.handle(MenuInput::Cancel),Event::Back));
  }
 }
}
