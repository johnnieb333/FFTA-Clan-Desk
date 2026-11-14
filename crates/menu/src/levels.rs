//! Level targets and immutable stat previews; memory edits belong to the runtime.
use super::{canvas::Canvas,CharacterInfo,GilAction,MenuInput};
#[derive(Clone,Debug)]pub struct GrowthJob{pub id:u8,pub name:String,pub growth:[u8;7]}
#[derive(Clone,Debug,PartialEq,Eq)]pub struct LevelPlan{pub target:u8,pub job:u8,pub record:Vec<u8>,pub before:[u16;7],pub after:[u16;7]}
#[derive(Default)]pub struct LevelEditor{
 pub member:Option<CharacterInfo>,pub jobs:Vec<GrowthJob>,pub target:u8,pub plan:Option<LevelPlan>,
 pub status:String,pub undo_available:bool,pub mode:u8,pub overrides:[Option<u16>;7],pub limits:Option<[u16;7]>,step:u16,job:usize,area:u8,confirm:Option<bool>,accept:bool,
}
pub(super)enum Event{None,Back,Action(GilAction)}
#[derive(Clone,Copy)]struct Rect(f32,f32,f32,f32);
impl Rect{fn hit(self,x:f32,y:f32)->bool{x>=self.0&&x<self.0+self.2&&y>=self.1&&y<self.1+self.3}}
const JOB_PREV:Rect=Rect(44.,82.,44.,36.);const JOB_NEXT:Rect=Rect(812.,82.,44.,36.);
const MINUS:Rect=Rect(530.,142.,44.,38.);const PLUS:Rect=Rect(812.,142.,44.,38.);
const APPLY:Rect=Rect(44.,486.,245.,38.);const UNDO:Rect=Rect(310.,486.,245.,38.);const BACK:Rect=Rect(580.,486.,276.,38.);
const CANCEL:Rect=Rect(210.,316.,220.,44.);const ACCEPT:Rect=Rect(470.,316.,220.,44.);
impl LevelEditor{
 pub fn open(&mut self,result:Result<(CharacterInfo,Vec<GrowthJob>,[u16;7]),String>,undo:bool){
  self.step=1;self.limits=None;self.mode=0;self.overrides=[None;7];self.member=None;self.jobs.clear();self.plan=None;self.target=0;self.job=0;self.area=1;self.confirm=None;self.accept=false;self.undo_available=undo;
  match result{Ok((r,jobs,before))=>{
   if jobs.is_empty()||r.record.len()!=0x108{self.status="Character growth data unavailable".into();return;}
   self.target=r.level;self.job=jobs.iter().position(|j|j.id==r.record[7]).unwrap_or(0);
   self.plan=Some(LevelPlan{target:r.level,job:jobs[self.job].id,record:r.record.clone(),before,after:before});
   self.member=Some(r);self.jobs=jobs;self.status="Choose a level or edit base stats. Job and equipment stay unchanged.".into();
  },Err(e)=>self.status=e}
 }
 pub fn job_id(&self)->Option<u8>{self.jobs.get(self.job).map(|j|j.id)}
 pub fn set_preview(&mut self,result:Result<LevelPlan,String>){
  match result{Ok(p)=>{self.plan=Some(p);self.status=if self.mode&2!=0{"Game-possible preview: caps applied. Review every change before Apply.".into()}else if self.mode&1==0 && self.member.as_ref().is_some_and(|r|self.target<r.level){"Estimated reduction: past jobs and random growth rolls are not recorded.".into()}else{"Preview ready. Level changes reset EXP; stat-only edits keep EXP.".into()};},Err(e)=>{self.plan=None;self.status=e}}
 }
 fn change_job(&mut self,right:bool)->Event{
  let n=self.jobs.len();if n==0{return Event::None;}self.job=if right{(self.job+1)%n}else{(self.job+n-1)%n};self.plan=None;Event::Action(GilAction::LevelPreview)
 }
 fn adjust(&mut self,right:bool)->Event{
  if self.member.is_none(){return Event::None;}let next=if right{self.target.saturating_add(1).min(50)}else{self.target.saturating_sub(1).max(1)};
  if next==self.target{return Event::None;}self.target=next;self.plan=None;Event::Action(GilAction::LevelPreview)
 }
 fn change_mode(&mut self)->Event{
  self.mode^=1;self.plan=None;Event::Action(GilAction::LevelPreview)
 }
 fn change_limits(&mut self)->Event{
  self.mode^=2;if self.mode&2!=0{self.step=1;}
  // Keep explicit edits at their visible values; leave growth-driven fields automatic.
  if let Some(p)=&self.plan{for i in 0..7{if self.overrides[i].is_some(){self.overrides[i]=Some(p.after[i]);}}}
  self.plan=None;Event::Action(GilAction::LevelPreview)
 }
 fn change_step(&mut self,right:bool)->Event{
  if self.mode&2!=0{return Event::None;}let values=[1,2,5];let at=values.iter().position(|&n|n==self.step).unwrap_or(0);self.step=values[if right{(at+1)%3}else{(at+2)%3}];Event::None
 }
 fn adjust_stat(&mut self,index:usize,right:bool)->Event{
  let Some(plan)=self.plan.as_ref()else{return Event::None;};
  let value=plan.after[index];let min=if index==1{0}else{1};
  let ceiling=if self.mode&2!=0{self.limits.map(|v|v[index]).unwrap_or(999)}else{999};
  let step=if self.mode&2!=0{1}else{self.step.clamp(1,5)};
  let next=if right{value.saturating_add(step).min(ceiling)}else{value.saturating_sub(step).max(min)};
  if value==next{return Event::None;}
  self.overrides[index]=Some(next);self.plan=None;Event::Action(GilAction::LevelPreview)
 }
 fn can_apply(&self)->bool{self.member.as_ref().zip(self.plan.as_ref()).is_some_and(|(r,p)|p.record!=r.record && p.target==self.target && Some(p.job)==self.job_id())}
 fn request(&mut self,apply:bool){if (apply&&self.can_apply())||(!apply&&self.undo_available){self.confirm=Some(apply);self.accept=false;}}
 pub(super)fn handle(&mut self,input:MenuInput)->Event{
  if let Some(apply)=self.confirm{
   match input{MenuInput::Cancel=>self.confirm=None,MenuInput::Left|MenuInput::Right|MenuInput::Up|MenuInput::Down=>self.accept=!self.accept,
    MenuInput::Confirm=>{self.confirm=None;if self.accept{return Event::Action(if apply{GilAction::LevelApply}else{GilAction::Undo});}}}
   return Event::None;
  }
  match input{
   MenuInput::Cancel=>return Event::Back,
   MenuInput::Up=>self.area=self.area.saturating_sub(1),MenuInput::Down=>self.area=(self.area+1).min(14),
   MenuInput::Left|MenuInput::Right=>{let right=input==MenuInput::Right;match self.area{0=>return self.change_job(right),1=>return self.adjust(right),2=>return self.change_mode(),3..=9=>return self.adjust_stat((self.area-3)as usize,right),13=>return self.change_limits(),14=>return self.change_step(right),_=>{}}},
   MenuInput::Confirm=>match self.area{0=>return self.change_job(true),1=>self.area=2,2=>return self.change_mode(),3..=9=>return self.adjust_stat((self.area-3)as usize,true),10=>self.request(true),11=>self.request(false),12=>return Event::Back,13=>return self.change_limits(),14=>return self.change_step(true),_=>{}}
  }Event::None
 }
 pub(super)fn repeat_target(&self,x:usize,y:usize,w:usize,h:usize)->Option<(usize,bool)>{
  if self.confirm.is_some()||self.member.is_none()||w==0||h==0{return None;}
  let(x,y)=Self::point(x,y,w,h);
  for i in 0..7{let yy=258.+i as f32*28.;for(right,xx)in[(false,574.),(true,718.)]{if Rect(xx,yy-3.,32.,26.).hit(x,y){return Some((i,right));}}}None
 }
 fn point(x:usize,y:usize,w:usize,h:usize)->(f32,f32){let s=(w as f32/900.).min(h as f32/600.);((x as f32-(w as f32-900.*s)/2.)/s,(y as f32-(h as f32-600.*s)/2.)/s)}
 pub(super)fn hover(&mut self,x:usize,y:usize,w:usize,h:usize){if self.confirm.is_some(){let(x,y)=Self::point(x,y,w,h);if CANCEL.hit(x,y){self.accept=false;}if ACCEPT.hit(x,y){self.accept=true;}}}
 pub(super)fn scroll(&mut self,up:bool)->Event{if self.confirm.is_some(){return Event::None;}if (3..=9).contains(&self.area){return self.adjust_stat((self.area-3)as usize,up);}self.area=1;self.adjust(up)}
 pub(super)fn click(&mut self,x:usize,y:usize,w:usize,h:usize,left:bool)->Event{
  if !left{return self.handle(MenuInput::Cancel);}let(x,y)=Self::point(x,y,w,h);
  if self.confirm.is_some(){if CANCEL.hit(x,y)||ACCEPT.hit(x,y){self.accept=ACCEPT.hit(x,y);return self.handle(MenuInput::Confirm);}return Event::None;}
  if self.mode&2==0{for(i,step)in[1,2,5].into_iter().enumerate(){if Rect(300.+i as f32*72.,142.,66.,38.).hit(x,y){self.step=step;self.area=14;return Event::None;}}}
  if Rect(350.,36.,220.,32.).hit(x,y){self.area=13;return self.change_limits();}
  if JOB_PREV.hit(x,y)||JOB_NEXT.hit(x,y){self.area=0;return self.change_job(JOB_NEXT.hit(x,y));}
  if Rect(92.,82.,716.,36.).hit(x,y){self.area=0;return self.change_job(true);}
  if MINUS.hit(x,y)||PLUS.hit(x,y){self.area=1;return self.adjust(PLUS.hit(x,y));}
  if Rect(44.,190.,812.,32.).hit(x,y){self.area=2;return self.change_mode();}
  for i in 0..7{let yy=258.+i as f32*28.;
   if Rect(574.,yy-3.,32.,26.).hit(x,y)||Rect(718.,yy-3.,32.,26.).hit(x,y){self.area=3+i as u8;return self.adjust_stat(i,x>=718.);}
  }
  if APPLY.hit(x,y){self.area=10;self.request(true);}else if UNDO.hit(x,y){self.area=11;self.request(false);}else if BACK.hit(x,y){return Event::Back;}
  Event::None
 }
 pub(super)fn draw(&self,px:&mut[[u8;4]],w:usize,h:usize){
  if w<32||h<32||px.len()<w*h{return;}for p in px.iter_mut().take(w*h){p[0]=(p[0]as u16*5/16+5)as u8;p[1]=(p[1]as u16*4/16+3)as u8;p[2]=(p[2]as u16*3/16+1)as u8;}
  let mut c=Canvas::new(px,w,h);let s=(w as f32/900.).min(h as f32/600.);let mut ui=Painter{c:&mut c,s,ox:(w as f32-900.*s)/2.,oy:(h as f32-600.*s)/2.};
  ui.panel(Rect(20.,20.,860.,560.),false);ui.text(44.,40.,"LEVEL & STATS",24.,true,300.);
  ui.panel(Rect(350.,36.,220.,32.),self.area==13);ui.text(359.,45.,if self.mode&2!=0{"Limits: Game-possible"}else{"Limits: Unrestricted"},16.,false,202.);
  ui.text(590.,44.,self.member.as_ref().map(|m|m.name.as_str()).unwrap_or("Unavailable"),19.,true,266.);
  ui.panel(Rect(92.,82.,716.,36.),self.area==0);ui.text(106.,92.,&self.jobs.get(self.job).map(|j|format!("Growth job: {} ({}/{})",j.name,self.job+1,self.jobs.len())).unwrap_or("No growth jobs available".into()),17.,false,680.);
  for(r,label)in[(JOB_PREV,"<"),(JOB_NEXT,">")]{ui.panel(r,false);ui.text(r.0+15.,r.1+8.,label,19.,false,25.);}
  ui.text(44.,151.,&self.member.as_ref().map(|r|format!("Current level: {}   EXP: {}",r.level,r.exp)).unwrap_or("Current level: --".into()),17.,false,242.);
  if self.mode&2==0{for(i,step)in[1,2,5].into_iter().enumerate(){let x=300.+i as f32*72.;ui.panel(Rect(x,142.,66.,38.),self.step==step);ui.text(x+18.,153.,&format!("×{step}"),17.,false,40.);}}
  ui.panel(Rect(530.,142.,326.,38.),self.area==1);ui.text(594.,153.,&format!("Target level: {}",self.target),17.,false,204.);
  for(r,label)in[(MINUS,"-"),(PLUS,"+")]{ui.panel(r,false);ui.text(r.0+15.,r.1+9.,label,19.,false,25.);}
  ui.panel(Rect(44.,190.,812.,32.),self.area==2);
  ui.text(56.,198.,if self.mode&1==0{"Level mode: Job growth / estimated reduction   (click to change)"}else{"Level mode: Keep current stats   (click to change)"},17.,false,788.);
  ui.text(44.,230.,"BASE STAT",17.,true,360.);ui.text(444.,230.,"CURRENT",17.,true,120.);ui.text(612.,230.,"PREVIEW",17.,true,140.);ui.text(778.,230.,"CHANGE",15.,true,80.);
  for(i,label)in["Maximum HP","Maximum MP","Speed","Weapon Attack","Weapon Defense","Magic Power","Magic Resistance"].iter().enumerate(){
   let y=258.+i as f32*28.;ui.text(44.,y,label,17.,false,380.);
   ui.panel(Rect(574.,y-3.,32.,26.),self.area==3+i as u8);ui.text(584.,y-1.,"-",17.,false,20.);
   ui.panel(Rect(718.,y-3.,32.,26.),self.area==3+i as u8);ui.text(727.,y-1.,"+",17.,false,20.);
   if let Some(p)=&self.plan{ui.text(444.,y,&p.before[i].to_string(),17.,false,150.);ui.text(612.,y,&if self.mode&2!=0{format!("{}/{}",p.after[i],self.limits.map(|v|v[i]).unwrap_or(999))}else{p.after[i].to_string()},16.,false,104.);ui.text(786.,y,&format!("{:+}",p.after[i]as i32-p.before[i]as i32),17.,false,70.);}
   else{for x in[444.,620.,786.]{ui.text(x,y,"--",17.,false,70.);}}
  }
  ui.text(44.,461.,if self.mode&2!=0{"Per-stat recruit + growth caps; preview/max. Excludes boosts and level-down cycles."}else{"Base stats before equipment. Range 1-999 (MP: 0-999). Level: 1-50."},15.,false,812.);
  for(r,label,area)in[(APPLY,"Apply changes",10),(UNDO,"Undo Last Edit",11),(BACK,"Back",12)]{ui.panel(r,self.area==area);ui.text(r.0+14.,r.1+11.,label,16.,false,r.2-28.);}
  ui.text(44.,534.,&self.status,17.,false,812.);ui.text(44.,558.,"Arrows: navigate/change   Confirm: select   Back: return",17.,false,812.);
  if let Some(apply)=self.confirm{ui.panel(Rect(180.,206.,540.,174.),false);ui.text(210.,226.,if apply{"APPLY CHARACTER EDIT?"}else{"UNDO LAST EDIT?"},21.,true,480.);
   let text=if apply{format!("Level {} with previewed stats. Backup saved first.",self.target)}else{"Restore the full game state before the last edit.".into()};
   ui.text(210.,267.,&text,17.,false,480.);
   for(r,label,active)in[(CANCEL,"Cancel",!self.accept),(ACCEPT,"Confirm",self.accept)]{ui.panel(r,active);ui.text(r.0+20.,r.1+14.,label,16.,false,r.2-40.);}
  }
  drop(ui);c.finish(px);
 }
}
struct Painter<'a>{c:&'a mut Canvas,s:f32,ox:f32,oy:f32}
impl Painter<'_>{
 fn panel(&mut self,r:Rect,active:bool){self.c.rect(self.ox+r.0*self.s,self.oy+r.1*self.s,r.2*self.s,r.3*self.s,4.*self.s,if active{[255,231,158]}else{[251,244,226]},if active{[242,208,123]}else{[235,220,185]},[184,146,65]);}
 fn text(&mut self,x:f32,y:f32,text:&str,size:f32,title:bool,width:f32){self.c.text(self.ox+x*self.s,self.oy+y*self.s,text,[35,24,16],size.max(15.)*self.s,title,width*self.s);}
}
#[cfg(test)]mod levels_tests{
 use super::*;
 fn editor()->LevelEditor{
  let mut record=vec![0;0x108];record[7]=2;let r=CharacterInfo{slot:3,identity:1,record,name:"Cooper".into(),race:"Human".into(),job:"Soldier".into(),level:3,exp:18,jp:2};
  let mut e=LevelEditor::default();e.open(Ok((r,vec![GrowthJob{id:2,name:"Soldier".into(),growth:[10;7]},GrowthJob{id:6,name:"Ninja".into(),growth:[20;7]}],[50;7])),true);e
 }
 fn ready(e:&mut LevelEditor){let mut r=e.member.as_ref().unwrap().record.clone();r[9]=e.target;r[10]=0;e.set_preview(Ok(LevelPlan{target:e.target,job:e.job_id().unwrap(),record:r,before:[50;7],after:[60;7]}));}
 #[test]fn levels_stage_preview_confirm_cancel_and_back(){
  let mut e=editor();let original=e.member.as_ref().unwrap().record.clone();e.request(true);assert!(e.confirm.is_none());
  assert!(matches!(e.handle(MenuInput::Right),Event::Action(GilAction::LevelPreview)));assert_eq!(e.target,4);assert!(e.plan.is_none());e.request(true);assert!(e.confirm.is_none());
  ready(&mut e);assert_eq!(e.member.as_ref().unwrap().record,original);e.request(true);assert_eq!(e.confirm,Some(true));assert!(!e.accept);
  assert!(matches!(e.handle(MenuInput::Confirm),Event::None));assert!(e.confirm.is_none());
  e.request(true);e.handle(MenuInput::Right);assert!(matches!(e.handle(MenuInput::Confirm),Event::Action(GilAction::LevelApply)));
  e.request(false);assert!(matches!(e.handle(MenuInput::Cancel),Event::None));assert!(e.confirm.is_none());assert!(matches!(e.handle(MenuInput::Cancel),Event::Back));
 }
 #[test]fn levels_limits_and_job_change_invalidate_old_preview(){
  let mut e=editor();assert!(matches!(e.adjust(false),Event::Action(GilAction::LevelPreview)));assert_eq!(e.target,2);e.target=50;assert!(matches!(e.adjust(true),Event::None));
  ready(&mut e);assert!(e.can_apply());assert!(matches!(e.change_job(true),Event::Action(GilAction::LevelPreview)));assert_eq!(e.job_id(),Some(6));assert!(!e.can_apply());assert_eq!(e.member.as_ref().unwrap().record[7],2);
  e.set_preview(Err("Cannot preview".into()));e.request(true);assert!(e.confirm.is_none());assert_eq!(e.status,"Cannot preview");
 }
 #[test]fn levels_scaled_mouse_and_draw_use_same_controls(){
  for(w,h)in[(852,480),(1920,1080)]{
   let mut e=editor();let s=(w as f32/900.).min(h as f32/600.);let ox=(w as f32-900.*s)/2.;let oy=(h as f32-600.*s)/2.;let point=|x:f32,y:f32|((ox+x*s)as usize,(oy+y*s)as usize);
   let(x,y)=point(830.,160.);assert!(matches!(e.click(x,y,w,h,true),Event::Action(GilAction::LevelPreview)));assert_eq!(e.target,4);ready(&mut e);
   let mut pixels=vec![[128,128,128,255];w*h];e.draw(&mut pixels,w,h);assert_ne!(pixels[0],[128,128,128,255]);
   let(x,y)=point(100.,500.);e.click(x,y,w,h,true);assert_eq!(e.confirm,Some(true));assert!(!e.accept);
   let(x,y)=point(550.,335.);assert!(matches!(e.click(x,y,w,h,true),Event::Action(GilAction::LevelApply)));
  }
 }
}

#[cfg(test)]mod stat_editor_menu_tests{
 use super::*;
 fn editor()->LevelEditor{
  let mut record=vec![0;0x108];record[7]=2;record[9]=3;record[10]=18;
  let r=CharacterInfo{slot:3,identity:1,record,name:"Billy".into(),race:"Human".into(),job:"Soldier".into(),level:3,exp:18,jp:0};
  let mut e=LevelEditor::default();e.open(Ok((r,vec![GrowthJob{id:2,name:"Soldier".into(),growth:[10;7]}],[100;7])),true);e
 }
 #[test]fn stat_editor_menu_stages_each_stat_and_mode_without_changing_member(){
  let mut e=editor();let original=e.member.as_ref().unwrap().record.clone();
  for i in 0..7{
   let mut plan=e.plan.clone().unwrap();plan.after[i]=100;e.plan=Some(plan.clone());
   assert!(matches!(e.adjust_stat(i,true),Event::Action(GilAction::LevelPreview)));assert_eq!(e.overrides[i],Some(101));assert!(e.plan.is_none());e.plan=Some(plan);
  }
  let edits=e.overrides;assert!(matches!(e.change_mode(),Event::Action(GilAction::LevelPreview)));assert_eq!(e.mode,1);assert_eq!(e.overrides,edits);
  e.target=1;assert!(matches!(e.adjust(false),Event::None));e.target=50;assert!(matches!(e.adjust(true),Event::None));assert_eq!(e.member.as_ref().unwrap().record,original);
 }
 #[test]fn stat_editor_menu_mouse_stats_invalidate_preview_and_do_not_apply(){
  for(w,h)in[(900,600),(852,480),(1920,1080)]{
   let mut e=editor();let scale=(w as f32/900.).min(h as f32/600.);let ox=(w as f32-900.*scale)/2.;let oy=(h as f32-600.*scale)/2.;
   let(x,y)=((ox+734.*scale).round()as usize,(oy+(258.+2.*28.+8.)*scale).round()as usize);
   assert!(matches!(e.click(x,y,w,h,true),Event::Action(GilAction::LevelPreview)));assert_eq!(e.overrides[2],Some(101));assert!(e.plan.is_none());
   e.request(true);assert!(e.confirm.is_none());
  }
 }
}

#[cfg(test)]mod stat_limits_menu_tests{
 use super::*;
 #[test]fn stat_limits_toggle_preserves_level_mode_and_stages_visible_values(){
  let mut e=LevelEditor::default();e.mode=1;e.area=13;
  assert!(matches!(e.handle(MenuInput::Confirm),Event::Action(GilAction::LevelPreview)));assert_eq!(e.mode,3);
  e.change_mode();assert_eq!(e.mode,2);e.change_limits();assert_eq!(e.mode,0);
 }
}

#[cfg(test)]mod stat_hold_target_tests{
 use super::*;
 #[test]fn stat_hold_target_only_arrows_and_no_confirmation(){
  let mut e=LevelEditor::default();let r=CharacterInfo{slot:3,identity:1,record:vec![0;0x108],name:"Billy".into(),race:"Human".into(),job:"Soldier".into(),level:3,exp:0,jp:0};e.member=Some(r);
  for(w,h)in[(900,600),(852,480),(1920,1080)]{let s=(w as f32/900.).min(h as f32/600.);let ox=(w as f32-900.*s)/2.;let oy=(h as f32-600.*s)/2.;
   for i in 0..7{for(right,x)in[(false,590.),(true,734.)]{assert_eq!(e.repeat_target((ox+x*s)as usize,(oy+(268.+i as f32*28.)*s)as usize,w,h),Some((i,right)));}}
  }
  assert_eq!(e.repeat_target(100,500,900,600),None);e.confirm=Some(true);assert_eq!(e.repeat_target(734,268,900,600),None);
 }
}

#[cfg(test)]mod editor_comfort_step_tests{
 use super::*;
 fn plan()->LevelPlan{LevelPlan{target:3,job:2,record:vec![0;0x108],before:[100;7],after:[100;7]}}
 #[test]fn editor_comfort_steps_are_staged_and_clamped_in_both_directions(){
  let mut e=LevelEditor::default();e.mode=1;e.step=5;e.plan=Some(plan());assert!(matches!(e.adjust_stat(2,true),Event::Action(GilAction::LevelPreview)));assert_eq!(e.overrides[2],Some(105));
  e.plan=Some(plan());e.step=2;e.adjust_stat(2,false);assert_eq!(e.overrides[2],Some(98));
  let mut p=plan();p.after[2]=998;e.plan=Some(p);e.step=5;e.adjust_stat(2,true);assert_eq!(e.overrides[2],Some(999));
  let mut p=plan();p.after[2]=2;e.plan=Some(p);e.adjust_stat(2,false);assert_eq!(e.overrides[2],Some(1));
  e.mode=3;e.plan=Some(plan());e.limits=Some([101;7]);e.adjust_stat(2,true);assert_eq!(e.overrides[2],Some(101));
 }
}
