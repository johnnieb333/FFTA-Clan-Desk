//! Timed autosave browser; writes and recovery belong to the runtime.
use super::{canvas::Canvas,states::{Event,StateAction,StateCard},MenuInput};
#[derive(Default)]pub struct AutoStateMenu{pub rows:Vec<(usize,String)>,pub selected:usize,pub card:StateCard,pub minutes:u16,pub status:String,legacy:bool,area:usize,confirm:Option<StateAction>,accept:bool}
impl AutoStateMenu{
 pub fn id(&self)->usize{if self.legacy{0}else{self.rows.get(self.selected).map(|r|r.0).unwrap_or(1)}}
 pub fn open(&mut self){self.legacy=false;self.area=0;self.selected=0;self.confirm=None;self.accept=false;}
 fn timer(&mut self,right:bool)->Event{let values=[0,1,2,5,10];let at=values.iter().position(|&n|n==self.minutes).unwrap_or(3);self.minutes=values[if right{(at+1)%5}else{(at+4)%5}];Event::Action(StateAction::TimerChanged)}
 pub(super)fn handle(&mut self,input:MenuInput)->Event{
  if let Some(action)=self.confirm{match input{MenuInput::Cancel=>self.confirm=None,MenuInput::Left|MenuInput::Right|MenuInput::Up|MenuInput::Down=>self.accept=!self.accept,MenuInput::Confirm=>{self.confirm=None;if self.accept{return Event::Action(action);}}}return Event::None;}
  match input{
   MenuInput::Cancel=>return Event::Back,
   MenuInput::Up|MenuInput::Down=>{let down=input==MenuInput::Down;if self.area==0{self.legacy=false;let n=self.rows.len();if n>0{self.selected=if down{(self.selected+1).min(n-1)}else{self.selected.saturating_sub(1)};}return Event::Action(StateAction::AutoRefresh);}self.area=if down{(self.area+1).min(6)}else{self.area.saturating_sub(1)};if self.area==2{self.legacy=true;}if self.area==0{self.legacy=false;}return Event::Action(StateAction::AutoRefresh);},
   MenuInput::Left|MenuInput::Right=>{if self.area==1{return self.timer(input==MenuInput::Right);}if self.area==0{self.area=1;}else{self.area=0;self.legacy=false;}return Event::Action(StateAction::AutoRefresh);},
   MenuInput::Confirm=>match self.area{0|3=>{if self.card.occupied{self.confirm=Some(StateAction::AutoLoad);self.accept=false;}},1=>return self.timer(true),2=>{if self.card.occupied{self.confirm=Some(StateAction::AutoLoad);self.accept=false;}},4=>{if self.card.undo_load{self.confirm=Some(StateAction::AutoUndoLoad);self.accept=false;}},5=>return Event::Action(StateAction::ManualTab),_=>return Event::Back}
  }Event::None
 }
 pub(super)fn hover(&mut self,x:usize,y:usize,w:usize,h:usize){if self.confirm.is_some(){let(x,y)=point(x,y,w,h);if hit(x,y,150.,278.,240.,44.){self.accept=false;}if hit(x,y,410.,278.,240.,44.){self.accept=true;}}}
 pub(super)fn click(&mut self,x:usize,y:usize,w:usize,h:usize,left:bool)->Event{
  if !left{return self.handle(MenuInput::Cancel);}self.hover(x,y,w,h);let(x,y)=point(x,y,w,h);
  if self.confirm.is_some(){if hit(x,y,150.,278.,240.,44.)||hit(x,y,410.,278.,240.,44.){return self.handle(MenuInput::Confirm);}return Event::None;}
  for i in 0..10{if i<self.rows.len()&&hit(x,y,430.,106.+i as f32*30.,314.,28.){self.area=0;self.legacy=false;self.selected=i;return Event::Action(StateAction::AutoRefresh);}}
  if hit(x,y,56.,370.,348.,38.){self.area=1;return self.timer(x>=230.);}
  if hit(x,y,56.,416.,348.,34.){self.area=2;self.legacy=true;return Event::Action(StateAction::AutoRefresh);}
  for(area,l,width)in[(6,56.,140.),(3,430.,150.),(4,594.,150.)]{if hit(x,y,l,458.,width,34.){self.area=area;return self.handle(MenuInput::Confirm);}}
  Event::None
 }
 pub(super)fn draw(&self,px:&mut[[u8;4]],w:usize,h:usize){
  if w<32||h<32||px.len()<w*h{return;}for p in px.iter_mut().take(w*h){for c in &mut p[..3]{*c/=4;}}
  let(s,ox,oy)=transform(w,h);let mut c=Canvas::new(px,w,h);
  let mut panel=|x:f32,y:f32,ww:f32,hh:f32,a:bool|c.rect(ox+x*s,oy+y*s,ww*s,hh*s,4.*s,if a{[38,129,119]}else{[251,244,226]},if a{[27,106,100]}else{[239,226,202]},[99,59,80]);
  panel(28.,24.,744.,492.,false);panel(56.,108.,348.,232.,false);panel(430.,46.,150.,36.,self.area==5);panel(590.,46.,154.,36.,true);
  for i in 0..10{panel(430.,106.+i as f32*30.,314.,28.,self.area==0&&self.selected==i);}
  panel(56.,370.,348.,38.,self.area==1);panel(56.,416.,348.,34.,self.area==2);for(area,x,width)in[(6,56.,140.),(3,430.,150.),(4,594.,150.)]{panel(x,458.,width,34.,self.area==area);}drop(panel);
  let mut text=|x:f32,y:f32,t:&str,size:f32,title:bool,width:f32|c.text(ox+x*s,oy+y*s,t,[62,34,53],size*s,title,width*s);
  text(56.,46.,"SAVES",24.,true,348.);text(453.,56.,"Manual / 30",16.,false,126.);text(609.,56.,"Autosaves / 10",16.,false,130.);text(56.,82.,"Ten rotating snapshots. Latest entries appear first.",15.,false,688.);
  if !self.card.occupied{text(82.,208.,"No autosave in this slot yet",17.,false,296.);}
  text(56.,350.,&self.card.saved,14.,false,348.);
  let timer=if self.minutes==0{"Off".into()}else{format!("{} min",self.minutes)};text(68.,380.,&format!("<  Autosave timer: {timer}  >"),17.,false,324.);text(68.,424.,"Existing QuickSave (select to preview)",15.,false,324.);
  for(i,(_,label))in self.rows.iter().take(10).enumerate(){text(440.,113.+i as f32*30.,label,15.,false,294.);}
  text(68.,468.,"Back",16.,false,116.);text(446.,468.,"Load selected",16.,false,124.);text(610.,468.,"Undo Load",16.,false,124.);
  text(56.,498.,if self.status.is_empty(){"Right: actions  /  Up-Down: select  /  Timer counts active play"}else{&self.status},14.,false,688.);drop(text);
  if let Some(action)=self.confirm{
   c.rect(ox+120.*s,oy+180.*s,560.*s,164.*s,6.*s,[251,244,226],[239,226,202],[99,59,80]);
   c.text(ox+150.*s,oy+208.*s,if action==StateAction::AutoLoad{"LOAD THIS AUTOSAVE?"}else{"UNDO THE LAST LOAD?"},[62,34,53],21.*s,true,500.*s);
   c.text(ox+150.*s,oy+248.*s,"A recovery snapshot is kept before loading.",[62,34,53],15.*s,false,500.*s);
   for(x,label,on)in[(150.,"Cancel",!self.accept),(410.,"Confirm",self.accept)]{c.rect(ox+x*s,oy+278.*s,240.*s,44.*s,4.*s,if on{[38,129,119]}else{[251,244,226]},[239,226,202],[99,59,80]);c.text(ox+(x+18.)*s,oy+290.*s,label,[62,34,53],17.*s,false,204.*s);}
  }
  c.finish(px);
  if self.confirm.is_none()&&self.card.image.len()==38400{let x0=(ox+56.*s)as usize;let y0=(oy+108.*s)as usize;let ww=(348.*s).max(1.)as usize;let hh=(232.*s).max(1.)as usize;for y in 0..hh.min(h.saturating_sub(y0)){for x in 0..ww.min(w.saturating_sub(x0)){let p=self.card.image[y*160/hh*240+x*240/ww];px[(y0+y)*w+x0+x]=[((p&31)*255/31)as u8,(((p>>5)&31)*255/31)as u8,(((p>>10)&31)*255/31)as u8,255];}}}
 }
}
fn transform(w:usize,h:usize)->(f32,f32,f32){let s=(w as f32/800.).min(h as f32/520.);(s,(w as f32-800.*s)/2.,(h as f32-520.*s)/2.)}
fn point(x:usize,y:usize,w:usize,h:usize)->(f32,f32){let(s,ox,oy)=transform(w,h);((x as f32-ox)/s,(y as f32-oy)/s)}
fn hit(x:f32,y:f32,l:f32,t:f32,w:f32,h:f32)->bool{x>=l&&x<l+w&&y>=t&&y<t+h}
#[cfg(test)]mod save_browser_menu_tests{
 use super::*;
 #[test]fn save_browser_load_is_confirmed_and_timer_choices_are_bounded(){let mut m=AutoStateMenu::default();m.open();m.minutes=5;m.card.occupied=true;assert!(matches!(m.handle(MenuInput::Confirm),Event::None));assert!(matches!(m.handle(MenuInput::Confirm),Event::None));m.handle(MenuInput::Confirm);m.handle(MenuInput::Right);assert!(matches!(m.handle(MenuInput::Confirm),Event::Action(StateAction::AutoLoad)));m.area=1;for _ in 0..10{m.handle(MenuInput::Right);assert!([0,1,2,5,10].contains(&m.minutes));}}
}

#[cfg(test)]mod save_browser_legacy_tests{
 use super::*;
 #[test]fn save_browser_quicksave_selection_survives_load_button_focus(){let mut m=AutoStateMenu::default();m.rows=vec![(7,"Auto 7".into())];m.open();m.card.occupied=true;m.click(80,430,800,520,true);assert_eq!(m.id(),0);m.click(470,470,800,520,true);assert_eq!(m.id(),0);m.handle(MenuInput::Right);assert!(matches!(m.handle(MenuInput::Confirm),Event::Action(StateAction::AutoLoad)));assert_eq!(m.id(),0);m.click(450,116,800,520,true);assert_eq!(m.id(),7);}
}
