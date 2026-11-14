//! Dedicated staged funds UI. Runtime owns validation, backup and persistence.
use super::{canvas::Canvas,Menu,MenuInput,AvConfig,MenuAction,Page};
const STEPS:[u32;6]=[1,10,100,1000,10000,100000];
fn grouped(n:u32)->String{let s=n.to_string();let mut out=String::new();for(i,c)in s.chars().enumerate(){if i>0&&(s.len()-i)%3==0{out.push(',');}out.push(c);}out}
fn point(x:usize,y:usize,w:usize,h:usize)->(f32,f32){let(s,ox,oy)=transform(w,h);((x as f32-ox)/s,(y as f32-oy)/s)}
fn transform(w:usize,h:usize)->(f32,f32,f32){let s=(w as f32/900.).min(h as f32/600.);(s,(w as f32-900.*s)/2.,(h as f32-600.*s)/2.)}
fn hit(x:f32,y:f32,l:f32,t:f32,w:f32,h:f32)->bool{x>=l&&x<l+w&&y>=t&&y<t+h}
pub(super)fn active(page:Page)->bool{matches!(page,Page::GilEditor|Page::GilConfirm|Page::GilUndoConfirm)}
pub(super)fn repeat_target(menu:&Menu,x:usize,y:usize,w:usize,h:usize)->Option<(usize,bool)>{
 if menu.page!=Page::GilEditor||!menu.gil.enabled||w==0||h==0{return None;}let(x,y)=point(x,y,w,h);
 if hit(x,y,44.,210.,72.,64.){Some((7,false))}else if hit(x,y,784.,210.,72.,64.){Some((7,true))}else{None}
}
pub(super)fn hover(menu:&mut Menu,x:usize,y:usize,w:usize,h:usize){
 if menu.page==Page::GilEditor{return;}let(x,y)=point(x,y,w,h);for(i,l)in[(0,180.),(1,470.)]{if hit(x,y,l,354.,250.,46.){menu.selected=i;}}
}
pub(super)fn click(menu:&mut Menu,x:usize,y:usize,w:usize,h:usize,av:&mut AvConfig)->Option<MenuAction>{
 let(x,y)=point(x,y,w,h);
 if menu.page!=Page::GilEditor{for(i,l)in[(0,180.),(1,470.)]{if hit(x,y,l,354.,250.,46.){menu.selected=i;return menu.handle(MenuInput::Confirm,av);}}return None;}
 if hit(x,y,44.,210.,72.,64.)||hit(x,y,784.,210.,72.,64.){menu.selected=1;menu.gil.adjust(x>=784.);return None;}
 for(i,_)in STEPS.iter().enumerate(){if hit(x,y,44.+i as f32*136.,365.,132.,40.){menu.selected=2;menu.gil.set_step(i);return None;}}
 if hit(x,y,620.,290.,236.,34.){menu.selected=0;menu.gil.reset_preview();return None;}
 for(i,l,width)in[(3,44.,245.),(4,310.,245.),(5,580.,276.)]{if hit(x,y,l,486.,width,38.){menu.selected=i;return menu.handle(MenuInput::Confirm,av);}}
 None
}
struct Painter<'a>{c:&'a mut Canvas,s:f32,ox:f32,oy:f32}
impl Painter<'_>{
 fn panel(&mut self,x:f32,y:f32,w:f32,h:f32,on:bool){self.c.rect(self.ox+x*self.s,self.oy+y*self.s,w*self.s,h*self.s,5.*self.s,if on{[38,129,119]}else{[251,244,226]},if on{[27,106,100]}else{[239,226,202]},[99,59,80]);}
 fn text(&mut self,x:f32,y:f32,t:&str,size:f32,title:bool,width:f32){self.c.text(self.ox+x*self.s,self.oy+y*self.s,t,[62,34,53],size*self.s,title,width*self.s);}
}
pub(super)fn draw(menu:&Menu,px:&mut[[u8;4]],w:usize,h:usize){
 if w<32||h<32||px.len()<w*h{return;}for p in px.iter_mut().take(w*h){for c in &mut p[..3]{*c/=4;}}
 let mut c=Canvas::new(px,w,h);let(s,ox,oy)=transform(w,h);let mut u=Painter{c:&mut c,s,ox,oy};u.panel(20.,20.,860.,560.,false);
 if menu.page==Page::GilEditor{
  u.text(44.,40.,"CLAN FUNDS",26.,true,600.);u.text(44.,78.,"Choose the new total. Your funds change only after Apply and confirmation.",17.,false,812.);
  u.panel(44.,108.,812.,70.,false);u.text(62.,130.,"Current funds",20.,true,350.);u.text(530.,121.,&format!("{} gil",grouped(menu.gil.current)),30.,false,306.);
  u.text(44.,186.,"NEW TOTAL",17.,true,812.);u.panel(44.,210.,72.,64.,menu.selected==1);u.panel(128.,210.,644.,64.,false);u.panel(784.,210.,72.,64.,menu.selected==1);
  u.text(68.,222.,"−",36.,false,40.);u.text(808.,222.,"+",36.,false,40.);u.text(295.,221.,&format!("{} gil",grouped(menu.gil.proposed)),36.,false,460.);
  let d=menu.gil.proposed as i64-menu.gil.current as i64;u.text(44.,297.,&if d==0{"No pending change".into()}else{format!("Change: {}{} gil",if d>0{"+"}else{"−"},grouped(d.unsigned_abs()as u32))},20.,false,560.);
  u.panel(620.,290.,236.,34.,menu.selected==0);u.text(640.,299.,"Reset preview",17.,false,196.);
  u.text(44.,335.,"CHANGE EACH CLICK BY",17.,true,812.);for(i,&step)in STEPS.iter().enumerate(){let x=44.+i as f32*136.;u.panel(x,365.,132.,40.,menu.gil.step()==step);u.text(x+12.,377.,&grouped(step),18.,false,108.);}
  u.text(44.,420.,"Click − or +, or hold it to repeat. Range: 0–999,999 gil.",17.,false,812.);u.text(44.,450.,&menu.gil.status,16.,false,812.);
  for(i,x,width,label)in[(3,44.,245.,"Apply changes"),(4,310.,245.,"Undo Last Edit"),(5,580.,276.,"Back")]{u.panel(x,486.,width,38.,menu.selected==i);u.text(x+14.,497.,label,17.,false,width-28.);}
  u.text(44.,542.,"Left/Right: adjust   Up/Down: focus   Enter: select   Esc: back",16.,false,812.);
 }else{
  let apply=menu.page==Page::GilConfirm;u.text(44.,40.,if apply{"APPLY NEW CLAN FUNDS?"}else{"UNDO LAST EDIT?"},25.,true,812.);
  if apply{u.text(100.,150.,"CURRENT",19.,true,300.);u.text(500.,150.,"NEW TOTAL",19.,true,300.);u.text(100.,194.,&grouped(menu.gil.current),36.,false,300.);u.text(500.,194.,&grouped(menu.gil.proposed),36.,false,300.);u.text(100.,268.,"A recovery snapshot is saved before applying this change.",18.,false,700.);}
  else{u.text(100.,160.,"Restore the full game state before the most recent editor change.",20.,false,700.);u.text(100.,220.,"This also restores other changes made since that snapshot.",18.,false,700.);}
  for(i,x,label)in[(0,180.,"Cancel"),(1,470.,if apply{"Confirm change"}else{"Restore snapshot"})]{u.panel(x,354.,250.,46.,menu.selected==i);u.text(x+18.,369.,label,18.,false,214.);}
  u.text(100.,450.,"Cancel keeps your preview. Enter confirms the selected button.",17.,false,700.);
 }
 drop(u);c.finish(px);
}
#[cfg(test)]mod gil_desk_tests{
 use super::*;
 #[test]fn gil_desk_grouped_values_and_scaled_minus_plus_repeat(){assert_eq!(grouped(999999),"999,999");assert_eq!(grouped(0),"0");let mut m=Menu::new(true,true);m.page=Page::GilEditor;m.gil.open(Ok(4600),true);let mut av=AvConfig::default();for(w,h)in[(852,480),(1920,1080)]{let(s,ox,oy)=transform(w,h);let(x,y)=((ox+820.*s)as usize,(oy+240.*s)as usize);assert_eq!(repeat_target(&m,x,y,w,h),Some((7,true)));let old=m.gil.proposed;click(&mut m,x,y,w,h,&mut av);assert_eq!(m.gil.proposed,old+1000);assert_eq!(m.gil.current,4600);let mut px=vec![[128,128,128,255];w*h];draw(&m,&mut px,w,h);}}
 #[test]fn gil_desk_steps_reset_and_apply_still_requires_confirmation(){let mut m=Menu::new(true,true);m.open(Page::GilEditor);m.gil.open(Ok(4600),true);let mut av=AvConfig::default();click(&mut m,610,385,900,600,&mut av);assert_eq!(m.gil.step(),10000);click(&mut m,820,240,900,600,&mut av);assert_eq!(m.gil.proposed,14600);click(&mut m,650,305,900,600,&mut av);assert_eq!(m.gil.proposed,4600);click(&mut m,820,240,900,600,&mut av);assert_eq!(click(&mut m,100,505,900,600,&mut av),None);assert_eq!(m.page,Page::GilConfirm);assert_eq!(m.handle(MenuInput::Confirm,&mut av),None);assert_eq!(m.page,Page::GilEditor);assert_eq!(m.gil.proposed,14600);}
}
