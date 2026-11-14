//! Staged per-member ability AP; no game memory or file access.
use super::{canvas::Canvas,CharacterInfo,GilAction,MenuInput};
#[derive(Clone,Debug)]pub struct AbilityInfo{pub index:u8,pub name:String,pub kind:String,pub required:u8,pub current:u8}
#[derive(Clone,Debug)]pub struct AbilityJob{pub id:u8,pub name:String,pub skills:Vec<AbilityInfo>}
#[derive(Default)]pub struct AbilityEditor{
    pub member:Option<CharacterInfo>,pub jobs:Vec<AbilityJob>,pub pending:std::collections::BTreeMap<u8,u8>,
    pub status:String,pub undo_available:bool,job:usize,row:usize,first:usize,area:u8,confirm:Option<bool>,accept:bool,job_change:bool,
}
pub(super)enum Event{None,Back,Action(GilAction)}
#[derive(Clone,Copy)]struct Rect(f32,f32,f32,f32);
impl Rect{fn hit(self,x:f32,y:f32)->bool{x>=self.0&&x<self.0+self.2&&y>=self.1&&y<self.1+self.3}}
const CHANGE_JOB:Rect=Rect(350.,36.,220.,32.);
const MINUS:Rect=Rect(530.,296.,44.,38.);const PLUS:Rect=Rect(812.,296.,44.,38.);
const MASTER:Rect=Rect(530.,346.,326.,28.);const MASTER_JOB:Rect=Rect(530.,378.,326.,28.);
const MASTER_ALL:Rect=Rect(530.,410.,326.,28.);
const APPLY:Rect=Rect(530.,442.,326.,28.);const UNDO:Rect=Rect(530.,474.,326.,28.);const BACK:Rect=Rect(530.,506.,326.,28.);
const PREV:Rect=Rect(44.,470.,126.,36.);const NEXT:Rect=Rect(378.,470.,126.,36.);
const JOB_PREV:Rect=Rect(44.,82.,44.,34.);const JOB_NEXT:Rect=Rect(812.,82.,44.,34.);
const CANCEL:Rect=Rect(210.,316.,220.,44.);const ACCEPT:Rect=Rect(470.,316.,220.,44.);
impl AbilityEditor{
    pub fn open(&mut self,result:Result<(CharacterInfo,Vec<AbilityJob>),String>,undo:bool){
        self.job_change=false;self.member=None;self.jobs.clear();self.pending.clear();self.job=0;self.row=0;self.first=0;self.area=0;self.confirm=None;self.undo_available=undo;
        match result{Ok((member,jobs))=>{self.job=jobs.iter().position(|j|j.id==member.record.get(7).copied().unwrap_or(0)).unwrap_or(0);self.member=Some(member);self.jobs=jobs;
            self.status="Add AP or stage mastery, then Apply once. Shared skills update together.".into();},Err(error)=>self.status=error}
    }
    pub fn job_id(&self)->Option<u8>{self.jobs.get(self.job).map(|j|j.id)}
    pub fn skill_id(&self)->Option<u8>{self.skill().map(|s|s.index)}
    pub fn restore_selection(&mut self,job:Option<u8>,skill:Option<u8>){
        self.job=job.and_then(|id|self.jobs.iter().position(|j|j.id==id)).unwrap_or(self.job);
        self.row=skill.and_then(|id|self.jobs.get(self.job)?.skills.iter().position(|s|s.index==id)).unwrap_or(0);self.first=self.row/8*8;
    }
    fn skill(&self)->Option<&AbilityInfo>{self.jobs.get(self.job)?.skills.get(self.row)}
    fn value(&self,s:&AbilityInfo)->u8{self.pending.get(&s.index).copied().unwrap_or(s.current)}
    fn stage(&mut self,index:u8,target:u8){
        let linked=self.member.as_ref().is_some_and(|m|m.race=="Viera") && matches!(index,26|33);
        let ids=if linked{vec![26,33]}else{vec![index]};
        let floor=self.jobs.iter().flat_map(|j|&j.skills).filter(|s|ids.contains(&s.index)).map(|s|s.current).max().unwrap_or(0);
        for id in ids{if let Some(s)=self.jobs.iter().flat_map(|j|&j.skills).find(|s|s.index==id){let v=target.max(floor).min(s.required);
            if v>s.current{self.pending.insert(id,v);}else{self.pending.remove(&id);}}}
    }
    fn adjust(&mut self,right:bool){if let Some(s)=self.skill().cloned(){let old=self.value(&s);let target=if right{old.saturating_add(1).min(s.required)}else{old.saturating_sub(1).max(s.current)};self.stage(s.index,target);}}
    fn master_skill(&mut self){if let Some(s)=self.skill().cloned(){self.stage(s.index,s.required);}}
    fn master_job(&mut self){if let Some(j)=self.jobs.get(self.job).cloned(){for s in j.skills{self.stage(s.index,s.required);}}}
    fn master_all_jobs(&mut self){
        let skills:Vec<_>=self.jobs.iter().flat_map(|j|j.skills.clone()).collect();
        for skill in skills{self.stage(skill.index,skill.required);}
        self.status=if self.pending.is_empty(){"All jobs for this race are already mastered.".into()}else{"All race jobs staged. Apply changes to confirm; Undo remains available.".into()};
    }
    fn change_job(&mut self,right:bool){let n=self.jobs.len();if n==0{return;}self.job=if right{(self.job+1)%n}else{(self.job+n-1)%n};self.row=0;self.first=0;}
    fn move_row(&mut self,right:bool){let n=self.jobs.get(self.job).map(|j|j.skills.len()).unwrap_or(0);self.row=if right{(self.row+1).min(n.saturating_sub(1))}else{self.row.saturating_sub(1)};if self.row<self.first{self.first=self.row;}if self.row>=self.first+8{self.first=self.row+1-8;}}
    fn page(&mut self,next:bool){let last=self.jobs.get(self.job).map(|j|j.skills.len().saturating_sub(1)/8*8).unwrap_or(0);self.first=if next{(self.first+8).min(last)}else{self.first.saturating_sub(8)};self.row=self.first;}
    fn request(&mut self,apply:bool){self.job_change=false;if (apply&&!self.pending.is_empty())||(!apply&&self.undo_available){self.confirm=Some(apply);self.accept=false;}}
    fn request_job(&mut self){
        if !self.pending.is_empty(){self.status="Apply pending AP changes or reopen this page before changing jobs.".into();return;}
        let Some(member)=self.member.as_ref()else{return;};
        if self.job_id()==Some(member.record[7]){self.status="This job is already equipped.".into();return;}
        if self.job_id().is_none(){return;}
        self.job_change=true;self.confirm=Some(true);self.accept=false;
    }
    pub(super)fn handle(&mut self,input:MenuInput)->Event{
        if let Some(apply)=self.confirm{match input{MenuInput::Cancel=>self.confirm=None,MenuInput::Left|MenuInput::Right|MenuInput::Up|MenuInput::Down=>self.accept=!self.accept,
            MenuInput::Confirm=>{self.confirm=None;if self.accept{return Event::Action(if apply{if self.job_change{GilAction::JobChangeApply}else{GilAction::AbilitiesApply}}else{GilAction::Undo});}}}return Event::None;}
        match input{
            MenuInput::Cancel=>return Event::Back,
            MenuInput::Up=>{if self.area==0{self.move_row(false);}else{self.area-=1;}},
            MenuInput::Down=>{if self.area==0{self.move_row(true);}else{self.area=(self.area+1).min(10);}},
            MenuInput::Left|MenuInput::Right=>{let right=input==MenuInput::Right;match self.area{0=>{self.area=if right{2}else{1};},1=>self.change_job(right),2=>self.adjust(right),8=>self.page(right),_=>self.area=0}},
            MenuInput::Confirm=>match self.area{0=>self.area=2,1=>self.change_job(true),2=>self.area=3,3=>self.master_skill(),4=>self.master_job(),5=>self.request(true),6=>self.request(false),7=>return Event::Back,8=>self.page(true),9=>self.request_job(),10=>self.master_all_jobs(),_=>{}},
        }Event::None
    }
    fn point(x:usize,y:usize,w:usize,h:usize)->(f32,f32){let s=(w as f32/900.).min(h as f32/600.);((x as f32-(w as f32-900.*s)/2.)/s,(y as f32-(h as f32-600.*s)/2.)/s)}
    pub(super)fn hover(&mut self,x:usize,y:usize,w:usize,h:usize){if self.confirm.is_some(){let(x,y)=Self::point(x,y,w,h);if CANCEL.hit(x,y){self.accept=false;}if ACCEPT.hit(x,y){self.accept=true;}}}
    pub(super)fn scroll(&mut self,up:bool)->Event{if self.confirm.is_none(){self.area=0;self.move_row(!up);}Event::None}
    pub(super)fn click(&mut self,x:usize,y:usize,w:usize,h:usize,left:bool)->Event{
        if !left{return self.handle(MenuInput::Cancel);}let(x,y)=Self::point(x,y,w,h);
        if self.confirm.is_some(){if CANCEL.hit(x,y)||ACCEPT.hit(x,y){self.accept=ACCEPT.hit(x,y);return self.handle(MenuInput::Confirm);}return Event::None;}
        if MASTER_ALL.hit(x,y){self.area=10;self.master_all_jobs();return Event::None;}
        if CHANGE_JOB.hit(x,y){self.area=9;self.request_job();return Event::None;}
        if JOB_PREV.hit(x,y)||JOB_NEXT.hit(x,y){self.area=1;self.change_job(JOB_NEXT.hit(x,y));}
        else if Rect(92.,82.,716.,34.).hit(x,y){self.area=1;self.change_job(true);}
        else if MINUS.hit(x,y)||PLUS.hit(x,y){self.area=2;self.adjust(PLUS.hit(x,y));}
        else if MASTER.hit(x,y){self.area=3;self.master_skill();}else if MASTER_JOB.hit(x,y){self.area=4;self.master_job();}
        else if APPLY.hit(x,y){self.area=5;self.request(true);}else if UNDO.hit(x,y){self.area=6;self.request(false);}else if BACK.hit(x,y){return Event::Back;}
        else if PREV.hit(x,y)||NEXT.hit(x,y){self.area=8;self.page(NEXT.hit(x,y));}
        else{for i in 0..8{if Rect(44.,176.+i as f32*36.,460.,34.).hit(x,y){let index=self.first+i;
            if self.jobs.get(self.job).is_some_and(|j|index<j.skills.len()){self.row=index;self.area=0;}break;}}}Event::None
    }
    pub(super)fn draw(&self,px:&mut[[u8;4]],w:usize,h:usize){
        if w<32||h<32||px.len()<w*h{return;}for p in px.iter_mut().take(w*h){p[0]=(p[0]as u16*5/16+5)as u8;p[1]=(p[1]as u16*4/16+3)as u8;p[2]=(p[2]as u16*3/16+1)as u8;}
        let mut canvas=Canvas::new(px,w,h);let s=(w as f32/900.).min(h as f32/600.);let mut ui=Painter{c:&mut canvas,s,ox:(w as f32-900.*s)/2.,oy:(h as f32-600.*s)/2.};
        ui.panel(Rect(20.,20.,860.,560.),false);ui.text(44.,40.,"JOBS & ABILITIES",24.,true,300.);
        ui.text(590.,44.,self.member.as_ref().map(|m|m.name.as_str()).unwrap_or("Unavailable"),19.,true,266.);
        ui.panel(Rect(92.,82.,716.,34.),self.area==1);ui.text(106.,91.,&self.jobs.get(self.job).map(|j|format!("Job: {} ({}/{})",j.name,self.job+1,self.jobs.len())).unwrap_or("No jobs available".into()),17.,false,680.);
        for(r,label)in[(JOB_PREV,"<"),(JOB_NEXT,">")]{ui.panel(r,false);ui.text(r.0+15.,r.1+7.,label,19.,false,25.);}
        ui.text(44.,137.,"Browse jobs without changing the equipped job. AP changes apply only after confirmation.",15.,false,812.);
        ui.text(44.,164.,"ABILITY",14.,false,300.);ui.text(390.,164.,"AP / NEED",14.,false,114.);
        for i in 0..8{let at=self.first+i;let r=Rect(44.,176.+i as f32*36.,460.,34.);ui.panel(r,at==self.row&&self.jobs.get(self.job).is_some());
            if let Some(skill)=self.jobs.get(self.job).and_then(|j|j.skills.get(at)){ui.text(54.,r.1+9.,&skill.name,17.,false,322.);
                ui.text(390.,r.1+9.,&format!("{}/{}{}",self.value(skill)as u16*10,skill.required as u16*10,if self.pending.contains_key(&skill.index){"*"}else{""}),14.,false,104.);}}
        if let Some(skill)=self.skill(){ui.text(530.,176.,&skill.name,19.,true,326.);ui.text(530.,208.,&skill.kind,15.,false,326.);
            ui.text(530.,236.,&format!("Current AP: {}",skill.current as u16*10),17.,false,326.);ui.text(530.,263.,&format!("Required AP: {}",skill.required as u16*10),17.,false,326.);}
        ui.panel(Rect(530.,296.,326.,38.),self.area==2);ui.text(590.,307.,&format!("New AP: {}",self.skill().map(|s|self.value(s)as u16*10).unwrap_or(0)),17.,false,206.);
        for(r,label)in[(MINUS,"-"),(PLUS,"+")]{ui.panel(r,false);ui.text(r.0+15.,r.1+9.,label,19.,false,25.);}
        for(r,label,area)in[(MASTER,"Master selected ability",3),(MASTER_JOB,"Master this job",4),(MASTER_ALL,"Master all race jobs",10),(APPLY,"Apply changes",5),(UNDO,"Undo Last Edit",6),(BACK,"Back",7)]{ui.panel(r,self.area==area);ui.text(r.0+14.,r.1+8.,label,16.,false,r.2-28.);}
        for(r,label)in[(PREV,"< Previous"),(NEXT,"Next >")]{ui.panel(r,self.area==8);ui.text(r.0+12.,r.1+11.,label,14.,false,r.2-24.);}
        let n=self.jobs.get(self.job).map(|j|j.skills.len()).unwrap_or(0);ui.text(180.,481.,&format!("{}-{} of {}",if n==0{0}else{self.first+1},(self.first+8).min(n),n),14.,false,186.);
        ui.panel(CHANGE_JOB,self.area==9);ui.text(362.,44.,"Change to this job...",16.,false,196.);
        let status=if self.pending.is_empty(){self.status.clone()}else{format!("{} pending ability changes. Apply once; Undo restores the whole edit.",self.pending.len())};ui.text(44.,539.,&status,14.,false,812.);
        ui.text(44.,560.,"Up/Down: list   Left: jobs   Right: AP   Confirm: select   Back: return",12.,false,812.);
        if let Some(apply)=self.confirm{ui.panel(Rect(180.,206.,540.,174.),false);ui.text(210.,226.,if apply{if self.job_change{"CHANGE EQUIPPED JOB?"}else{"APPLY ABILITY CHANGES?"}}else{"UNDO LAST EDIT?"},21.,true,480.);
            let message=if apply&&self.job_change{format!("{} -> {}. All five gear slots will be unequipped.",self.member.as_ref().map(|r|r.job.as_str()).unwrap_or("--"),self.jobs.get(self.job).map(|j|j.name.as_str()).unwrap_or("--"))}else if apply{"Only this member's ability progress changes. Backup saved first.".into()}else{"Restore the full game state before the most recent edit.".into()};
            ui.text(210.,263.,&message,14.,false,480.);
            if apply&&self.job_change{ui.text(210.,286.,"Stats kept. Items retained. Job prerequisites unchanged.",14.,false,480.);}
            for(r,label,active)in[(CANCEL,"Cancel",!self.accept),(ACCEPT,"Confirm",self.accept)]{ui.panel(r,active);ui.text(r.0+20.,r.1+14.,label,16.,false,r.2-40.);}}
        drop(ui);canvas.finish(px);
    }
}
struct Painter<'a>{c:&'a mut Canvas,s:f32,ox:f32,oy:f32}
impl Painter<'_>{
    fn panel(&mut self,r:Rect,active:bool){self.c.rect(self.ox+r.0*self.s,self.oy+r.1*self.s,r.2*self.s,r.3*self.s,4.*self.s,if active{[255,231,158]}else{[251,244,226]},if active{[242,208,123]}else{[235,220,185]},[184,146,65]);}
    fn text(&mut self,x:f32,y:f32,text:&str,size:f32,title:bool,width:f32){self.c.text(self.ox+x*self.s,self.oy+y*self.s,text,[35,24,16],size.max(15.)*self.s,title,width*self.s);}
}

#[cfg(test)]mod abilities_tests {
    use super::*;
    fn editor()->AbilityEditor{
        let mut record=vec![0;0x108];record[7]=2;
        let member=CharacterInfo{slot:3,identity:1,record,name:"Cooper".into(),race:"Human".into(),job:"Soldier".into(),level:3,exp:10,jp:2};
        let skill=|index|AbilityInfo{index,name:format!("Skill {index}"),kind:"Action".into(),required:10,current:0};
        let mut e=AbilityEditor::default();e.open(Ok((member,vec![AbilityJob{id:2,name:"Soldier".into(),skills:(1..=10).map(skill).collect()},AbilityJob{id:3,name:"Paladin".into(),skills:vec![skill(11)]} ])),true);e
    }
    #[test]fn abilities_stage_across_jobs_cancel_and_one_apply(){
        let mut e=editor();let original=e.member.as_ref().unwrap().record.clone();
        e.adjust(true);e.change_job(true);e.master_job();assert_eq!(e.pending.len(),2);
        e.change_job(false);assert_eq!(e.value(e.skill().unwrap()),1);e.adjust(false);assert_eq!(e.pending.len(),1);
        e.master_job();assert_eq!(e.pending.len(),11);assert_eq!(e.member.as_ref().unwrap().record,original);
        e.request(true);assert!(matches!(e.handle(MenuInput::Confirm),Event::None));assert!(e.confirm.is_none());assert_eq!(e.pending.len(),11);
        e.request(true);e.handle(MenuInput::Right);assert!(matches!(e.handle(MenuInput::Confirm),Event::Action(GilAction::AbilitiesApply)));
        e.request(false);assert!(matches!(e.handle(MenuInput::Cancel),Event::None));assert!(e.confirm.is_none());
    }
    #[test]fn abilities_mouse_scaled_paging_and_complete_draw(){
        for(w,h)in[(852,480),(1920,1080)]{
            let mut e=editor();let s=(w as f32/900.).min(h as f32/600.);let ox=(w as f32-900.*s)/2.;let oy=(h as f32-600.*s)/2.;
            let point=|x:f32,y:f32|((ox+x*s)as usize,(oy+y*s)as usize);
            let(x,y)=point(820.,310.);e.click(x,y,w,h,true);assert_eq!(e.pending.get(&1),Some(&1));
            let(x,y)=point(400.,482.);e.click(x,y,w,h,true);assert_eq!(e.first,8);assert_eq!(e.skill_id(),Some(9));
            let(x,y)=point(600.,395.);e.click(x,y,w,h,true);assert_eq!(e.pending.len(),10);
            let mut pixels=vec![[128,128,128,255];w*h];e.draw(&mut pixels,w,h);assert_ne!(pixels[0],[128,128,128,255]);
            let(x,y)=point(600.,454.);e.click(x,y,w,h,true);assert_eq!(e.confirm,Some(true));assert!(!e.accept);
        }
    }
    #[test]fn abilities_shared_cure_and_baseline_floor(){
        let mut e=editor();e.member.as_mut().unwrap().race="Viera".into();
        e.jobs[0].skills=vec![AbilityInfo{index:26,name:"Cure".into(),kind:"Action".into(),current:2,required:10}];
        e.jobs[1].skills[0].index=33;e.stage(26,5);assert_eq!(e.pending.get(&33),Some(&5));
        e.stage(33,0);assert!(!e.pending.contains_key(&26));assert_eq!(e.pending.get(&33),Some(&2));
    }
    #[test]fn job_change_menu_requires_explicit_confirmation_and_blocks_pending_ap(){
        let mut e=editor();e.request_job();assert!(e.confirm.is_none());e.change_job(true);e.request_job();assert!(e.job_change);assert_eq!(e.confirm,Some(true));assert!(!e.accept);
        assert!(matches!(e.handle(MenuInput::Confirm),Event::None));
        e.request_job();e.handle(MenuInput::Right);assert!(matches!(e.handle(MenuInput::Confirm),Event::Action(GilAction::JobChangeApply)));
        e.pending.insert(11,10);e.request_job();assert!(e.confirm.is_none());assert_eq!(e.pending.len(),1);
    }

}

#[cfg(test)]mod editor_comfort_master_tests{
 use super::*;
 #[test]fn editor_comfort_master_all_jobs_stages_every_job_without_applying(){
  let mut e=AbilityEditor::default();let skill=|i|AbilityInfo{index:i,name:format!("Skill {i}"),kind:"Action".into(),required:10,current:0};
  e.jobs=vec![AbilityJob{id:2,name:"Soldier".into(),skills:vec![skill(1),skill(2)]},AbilityJob{id:6,name:"Ninja".into(),skills:vec![skill(3)]}];
  e.master_all_jobs();assert_eq!(e.pending.len(),3);assert!(e.jobs.iter().flat_map(|j|&j.skills).all(|s|s.current==0));assert!(e.confirm.is_none());e.request(true);assert!(matches!(e.handle(MenuInput::Confirm),Event::None));assert_eq!(e.pending.len(),3);
 }
}
