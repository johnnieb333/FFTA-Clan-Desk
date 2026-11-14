//! Independent reward preferences. Never edits guest memory or existing results.
use super::{canvas::Canvas,MenuInput};
const FACTORS:[u8;5]=[1,2,3,5,10];
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum RewardAction {Open,Changed}
pub(super) enum Event {None,Back,Changed}
pub struct RewardMenu {pub gil:u8,pub ap:u8,pub exp:u8,pub jp:u8,pub status:String,enabled:bool,selected:usize}
impl Default for RewardMenu {
    fn default()->Self{Self{gil:1,ap:1,exp:1,jp:1,status:String::new(),enabled:false,selected:0}}
}
#[derive(Clone,Copy)]struct Rect(f32,f32,f32,f32);
impl Rect {fn hit(self,x:f32,y:f32)->bool{x>=self.0&&x<self.0+self.2&&y>=self.1&&y<self.1+self.3}}
fn transform(w:usize,h:usize)->(f32,f32,f32){let s=(w as f32/760.).min(h as f32/540.);(s,(w as f32-760.*s)/2.,(h as f32-540.*s)/2.)}
fn choice(row:usize,i:usize)->Rect{Rect(300.+i as f32*84.,144.+row as f32*64.,76.,44.)}
const DEFAULTS:Rect=Rect(36.,480.,190.,36.);
const BACK:Rect=Rect(580.,480.,140.,36.);
impl RewardMenu {
    pub fn sync(&mut self,gil:u8,ap:u8,exp:u8,jp:u8,enabled:bool){
        self.gil=if FACTORS.contains(&gil){gil}else{1};self.ap=if FACTORS.contains(&ap){ap}else{1};
        self.exp=if FACTORS.contains(&exp){exp}else{1};self.jp=if FACTORS.contains(&jp){jp}else{1};
        self.enabled=enabled;self.selected=0;
        self.status=if enabled{"Settings are remembered between launches."}else{"Reward multipliers are unavailable for this ROM."}.into();
    }
    fn set(&mut self,row:usize,value:u8)->Event {
        if !self.enabled{return Event::None;}
        let setting=match row{0=>&mut self.gil,1=>&mut self.ap,2=>&mut self.exp,_=>&mut self.jp};
        if *setting==value{return Event::None;}*setting=value;Event::Changed
    }
    fn cycle(&mut self,right:bool)->Event {
        if self.selected>3{return Event::None;}
        let value=match self.selected{0=>self.gil,1=>self.ap,2=>self.exp,_=>self.jp};
        let i=FACTORS.iter().position(|&n|n==value).unwrap_or(0);
        self.set(self.selected,FACTORS[if right{(i+1)%5}else{(i+4)%5}])
    }
    pub(super) fn handle(&mut self,input:MenuInput)->Event {
        match input {
            MenuInput::Cancel=>Event::Back,
            MenuInput::Up=>{self.selected=(self.selected+5)%6;Event::None},
            MenuInput::Down=>{self.selected=(self.selected+1)%6;Event::None},
            MenuInput::Left=>self.cycle(false),MenuInput::Right=>self.cycle(true),
            MenuInput::Confirm=>match self.selected {
                0..=3=>self.cycle(true),
                4 if self.enabled && (self.gil!=1||self.ap!=1||self.exp!=1||self.jp!=1)=>{self.gil=1;self.ap=1;self.exp=1;self.jp=1;Event::Changed},
                5=>Event::Back,_=>Event::None,
            },
        }
    }
    pub(super) fn hover(&mut self,x:usize,y:usize,w:usize,h:usize){
        let(s,ox,oy)=transform(w,h);let(x,y)=((x as f32-ox)/s,(y as f32-oy)/s);
        for row in 0..4 {if Rect(36.,144.+row as f32*64.,684.,44.).hit(x,y){self.selected=row;return;}}
        if DEFAULTS.hit(x,y){self.selected=4;}else if BACK.hit(x,y){self.selected=5;}
    }
    pub(super) fn click(&mut self,x:usize,y:usize,w:usize,h:usize,left:bool)->Event {
        if !left{return Event::Back;}
        let(s,ox,oy)=transform(w,h);let(x,y)=((x as f32-ox)/s,(y as f32-oy)/s);
        for row in 0..4 {for (i,&factor) in FACTORS.iter().enumerate(){
            if choice(row,i).hit(x,y){self.selected=row;return self.set(row,factor);}
        }}
        if DEFAULTS.hit(x,y){self.selected=4;return self.handle(MenuInput::Confirm);}
        if BACK.hit(x,y){self.selected=5;return Event::Back;}Event::None
    }
    pub(super) fn draw(&self,px:&mut [[u8;4]],w:usize,h:usize){
        if w<32||h<32||px.len()<w*h{return;}
        for p in px.iter_mut().take(w*h){
            p[0]=(p[0]as u16*5/16+5)as u8;
            p[1]=(p[1]as u16*4/16+3)as u8;
            p[2]=(p[2]as u16*3/16+1)as u8;
        }
        let(s,ox,oy)=transform(w,h);let mut c=Canvas::new(px,w,h);
        let mut panel=|r:Rect,active:bool|c.rect(ox+r.0*s,oy+r.1*s,r.2*s,r.3*s,4.*s,
            if active{[255,231,158]}else{[251,244,226]},if active{[242,208,123]}else{[235,220,185]},[184,146,65]);
        panel(Rect(12.,12.,736.,516.),false);
        for row in 0..4 {for (i,&factor) in FACTORS.iter().enumerate(){panel(choice(row,i),factor==match row{0=>self.gil,1=>self.ap,2=>self.exp,_=>self.jp});}}
        panel(DEFAULTS,self.selected==4);panel(BACK,self.selected==5);drop(panel);
        let mut text=|x:f32,y:f32,t:&str,size:f32,title:bool,width:f32|c.text(ox+x*s,oy+y*s,t,[35,24,16],size*s,title,width*s);
        text(36.,36.,"REWARD MULTIPLIERS",23.,true,684.);
        text(36.,78.,"Applies to rewards calculated after changing these settings.",15.,false,684.);
        text(36.,102.,"Existing results and loaded reward amounts stay unchanged.",15.,false,684.);
        for (row,label) in ["Gil received","AP received","EXP received","JP received"].iter().enumerate(){
            text(36.,156.+row as f32*64.,&format!("{}{}",if self.selected==row{"> "}else{""},label),19.,false,252.);
            for (i,factor) in FACTORS.iter().enumerate(){let r=choice(row,i);text(r.0+20.,r.1+12.,&format!("{factor}x"),18.,false,54.);}
        }
        text(36.,404.,"JP cap: 10. EXP keeps the game's normal level-up rules.",16.,false,684.);
        text(36.,426.,"Left/Right: choose   Enter: next   Back: return",16.,false,684.);
        text(36.,452.,&self.status,16.,false,684.);
        text(52.,490.,"Reset all to 1x",15.,false,164.);text(624.,490.,"Back",15.,false,86.);
        drop(text);c.finish(px);
    }
}
#[cfg(test)]mod reward_menu_tests {
    use super::*;
    #[test]fn reward_menu_independent_factors_and_defaults(){
        let mut m=RewardMenu::default();m.sync(1,1,1,1,true);
        assert!(matches!(m.handle(MenuInput::Right),Event::Changed));assert_eq!((m.gil,m.ap),(2,1));
        m.handle(MenuInput::Down);m.handle(MenuInput::Left);assert_eq!((m.gil,m.ap),(2,10));
        m.handle(MenuInput::Down);m.handle(MenuInput::Right);assert_eq!(m.exp,2);
        m.handle(MenuInput::Down);m.handle(MenuInput::Right);assert_eq!(m.jp,2);
        m.handle(MenuInput::Down);assert!(matches!(m.handle(MenuInput::Confirm),Event::Changed));assert_eq!((m.gil,m.ap),(1,1));
        m.handle(MenuInput::Down);assert!(matches!(m.handle(MenuInput::Confirm),Event::Back));
        m.sync(1,1,1,1,false);assert!(matches!(m.handle(MenuInput::Right),Event::None));assert_eq!((m.gil,m.ap),(1,1));
    }
    #[test]fn reward_menu_mouse_hits_exact_factor_at_each_window_size(){
        for (w,h) in [(854,480),(1920,1080),(640,360)] {
            let mut m=RewardMenu::default();m.sync(1,1,1,1,true);let(s,ox,oy)=transform(w,h);
            for row in 0..4 {for (i,&factor) in FACTORS.iter().enumerate(){
                let r=choice(row,i);let(x,y)=((ox+(r.0+r.2/2.)*s)as usize,(oy+(r.1+r.3/2.)*s)as usize);
                m.click(x,y,w,h,true);assert_eq!(match row{0=>m.gil,1=>m.ap,2=>m.exp,_=>m.jp},factor);
            }}
            let mut px=vec![[0,0,0,255];w*h];m.draw(&mut px,w,h);
            assert!(px.iter().any(|p|p[0]>0));
        }
    }
}
