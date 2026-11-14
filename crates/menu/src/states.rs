//! Native fullscreen-safe manual-state controls.
use super::{canvas::Canvas, MenuInput};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateAction { Refresh, Save, Load, UndoSave, UndoLoad, AutoRefresh, AutoLoad, AutoUndoLoad, TimerChanged, ManualTab }
#[derive(Default)]
pub struct StateCard {
    pub occupied: bool,
    pub undo_save: bool,
    pub undo_load: bool,
    pub saved: String,
    pub image: Vec<u16>,
}
#[derive(Default)]
pub struct StateMenu {
    pub automatic:bool, pub autos:super::auto_states::AutoStateMenu, pub auto_load_id:usize,
    pub slot: usize,
    pub card: StateCard,
    pub message: String,
    selected: usize,
    confirm: Option<StateAction>,
    accept: bool,
}
pub(super) enum Event { None, Back, Action(StateAction) }
// Shared by rendering and hit testing, in the menu's 800 x 520 logical canvas.
const SLOT_ARROWS: [(MenuInput,[f32;4]);2] = [
    (MenuInput::Left,[430.,100.,44.,40.]),
    (MenuInput::Right,[700.,100.,44.,40.]),
];
fn slot_arrow(x: f32, y: f32) -> Option<MenuInput> {
    SLOT_ARROWS.iter().find_map(|(input,[rx,ry,w,h])| {
        (x>=*rx && x<*rx+*w && y>=*ry && y<*ry+*h).then_some(*input)
    })
}
impl StateMenu {
    pub fn open(&mut self) {
        self.automatic=false;self.autos.open();
        self.slot = self.slot.clamp(1, 30);
        self.selected = 0;
        self.confirm = None;
        self.message.clear();
    }
    pub fn clear_confirmation(&mut self) { self.confirm = None; }
    pub(super) fn handle(&mut self, input: MenuInput) -> Event {
        if self.automatic{let event=self.autos.handle(input);if matches!(event,Event::Action(StateAction::AutoLoad)){self.auto_load_id=self.autos.id();}return event;}
        if self.confirm.is_none()&&self.selected==6&&matches!(input,MenuInput::Left|MenuInput::Right|MenuInput::Confirm){self.automatic=true;self.autos.open();return Event::Action(StateAction::AutoRefresh);}
        if let Some(action) = self.confirm {
            match input {
                MenuInput::Cancel => self.confirm = None,
                MenuInput::Up | MenuInput::Down | MenuInput::Left | MenuInput::Right => self.accept = !self.accept,
                MenuInput::Confirm => {
                    self.confirm = None;
                    if self.accept { return Event::Action(action); }
                }
            }
            return Event::None;
        }
        match input {
            MenuInput::Cancel => return Event::Back,
            MenuInput::Up => self.selected = (self.selected + 6) % 7,
            MenuInput::Down => self.selected = (self.selected + 1) % 7,
            MenuInput::Left | MenuInput::Right if self.selected == 0 => {
                self.slot = if input == MenuInput::Right { self.slot % 30 + 1 } else { (self.slot + 28) % 30 + 1 };
                self.message.clear();
                return Event::Action(StateAction::Refresh);
            }
            MenuInput::Confirm => {
                let action = match self.selected {
                    0 => {
                        self.slot = self.slot % 30 + 1;
                        self.message.clear();
                        return Event::Action(StateAction::Refresh);
                    }
                    1 => StateAction::Save,
                    2 => StateAction::Load,
                    3 => StateAction::UndoSave,
                    4 => StateAction::UndoLoad,
                    6 => {self.automatic=true;self.autos.open();return Event::Action(StateAction::AutoRefresh);},
                    _ => return Event::Back,
                };
                if (action == StateAction::Load && !self.card.occupied)
                    || (action == StateAction::UndoSave && !self.card.undo_save)
                    || (action == StateAction::UndoLoad && !self.card.undo_load)
                {
                    self.message = "Nothing available for this action.".into();
                    return Event::None;
                }
                if (action == StateAction::Save && self.card.occupied) || action == StateAction::UndoSave {
                    self.confirm = Some(action);
                    self.accept = false;
                    return Event::None;
                }
                return Event::Action(action);
            }
            _ => {}
        }
        Event::None
    }
    fn point(x: usize, y: usize, w: usize, h: usize) -> (f32, f32) {
        let s = (w as f32 / 800.).min(h as f32 / 520.);
        ((x as f32 - (w as f32 - 800. * s) / 2.) / s,
         (y as f32 - (h as f32 - 520. * s) / 2.) / s)
    }
    pub(super) fn hover(&mut self, x: usize, y: usize, w: usize, h: usize) {
        if self.automatic{self.autos.hover(x,y,w,h);return;}
        let (x, y) = Self::point(x,y,w,h);
        if self.confirm.is_some() {
            if (278. ..322.).contains(&y) {
                if (150. ..390.).contains(&x) { self.accept = false; }
                if (410. ..650.).contains(&x) { self.accept = true; }
            }
        } else if (430. ..744.).contains(&x) && (100. ..364.).contains(&y) {
            self.selected = ((y - 100.) / 44.) as usize;
        }
    }
    pub(super) fn click(&mut self, x: usize, y: usize, w: usize, h: usize, left: bool) -> Event {
        if !left { return self.handle(MenuInput::Cancel); }
        let(tx,ty)=Self::point(x,y,w,h);
        if self.confirm.is_none()&&(46. ..82.).contains(&ty){
            if (430. ..580.).contains(&tx){self.automatic=false;return Event::Action(StateAction::Refresh);}
            if (590. ..744.).contains(&tx){self.automatic=true;self.autos.open();return Event::Action(StateAction::AutoRefresh);}
        }
        if self.automatic{let event=self.autos.click(x,y,w,h,left);if matches!(event,Event::Action(StateAction::AutoLoad)){self.auto_load_id=self.autos.id();}return event;}
        self.hover(x,y,w,h);
        let (x,y) = Self::point(x,y,w,h);
        if self.confirm.is_some() {
            if (278. ..322.).contains(&y) && ((150. ..390.).contains(&x) || (410. ..650.).contains(&x)) {
                return self.handle(MenuInput::Confirm);
            }
        } else if (430. ..744.).contains(&x) && (100. ..364.).contains(&y) {
            if self.selected == 0 {
                return slot_arrow(x,y).map_or(Event::None,|input|self.handle(input));
            }
            return self.handle(MenuInput::Confirm);
        }
        Event::None
    }
    pub(super) fn draw(&self, px: &mut [[u8;4]], w: usize, h: usize) {
        if self.automatic{self.autos.draw(px,w,h);return;}
        if w == 0 || h == 0 || px.len() < w*h { return; }
        let s = (w as f32/800.).min(h as f32/520.);
        let ox = (w as f32-800.*s)/2.; let oy = (h as f32-520.*s)/2.;
        for p in px.iter_mut() { for ch in &mut p[..3] { *ch /= 4; } }
        let mut c = Canvas::new(px,w,h);
        let paper=[235,220,185]; let ink=[35,24,16]; let gold=[184,146,65];
        c.rect(ox+28.*s,oy+24.*s,744.*s,472.*s,8.*s,[251,244,226],paper,gold);
        c.text(ox+56.*s,oy+46.*s,"SAVES",ink,24.*s,true,348.*s);
        for(x,label,active)in[(430.,"Manual / 30",self.selected!=6),(590.,"Autosaves / 10",self.selected==6)]{c.rect(ox+x*s,oy+46.*s,if active{150.*s}else{154.*s},36.*s,4.*s,if active{[38,129,119]}else{[251,244,226]},paper,gold);c.text(ox+(x+16.)*s,oy+56.*s,label,ink,16.*s,false,138.*s);}
        c.text(ox+56.*s,oy+80.*s,"Manual snapshots - separate from normal saves",ink,15.*s,false,688.*s);
        c.rect(ox+56.*s,oy+108.*s,348.*s,232.*s,0.,[26,26,26],[26,26,26],gold);
        let labels=[format!("State {} / 30",self.slot),"Save current game".into(),"Load selected state".into(),"Undo Save".into(),"Undo Load".into(),"Back".into()];
        for (i,label) in labels.iter().enumerate() {
            let active = self.selected==i;
            let disabled = (i==2&&!self.card.occupied)||(i==3&&!self.card.undo_save)||(i==4&&!self.card.undo_load);
            let color=if disabled {[128,117,98]} else {ink};
            c.rect(ox+430.*s,oy+(100.+44.*i as f32)*s,314.*s,40.*s,4.*s,
                if active {[255,231,158]} else {paper},if active {[242,208,123]} else {paper},if active {gold} else {paper});
            if i == 0 {
                c.text(ox+505.*s,oy+110.*s,label,color,17.*s,false,180.*s);
                for (input,[x,y,w,h]) in SLOT_ARROWS {
                    c.rect(ox+x*s,oy+y*s,w*s,h*s,4.*s,[251,244,226],paper,gold);
                    c.text(ox+(x+15.)*s,oy+(y+10.)*s,
                        if input==MenuInput::Left { "<" } else { ">" },ink,17.*s,false,(w-20.)*s);
                }
            } else {
                c.text(ox+444.*s,oy+(110.+44.*i as f32)*s,label,color,17.*s,false,284.*s);
            }
        }
        c.text(ox+56.*s,oy+350.*s,if self.card.occupied {&self.card.saved} else {"Empty slot"},ink,15.*s,false,350.*s);
        c.text(ox+56.*s,oy+382.*s,"Undo Save restores the slot. Your game keeps its place.",ink,15.*s,false,688.*s);
        c.text(ox+56.*s,oy+404.*s,"Undo Load returns your game to before the last load.",ink,15.*s,false,688.*s);
        c.text(ox+56.*s,oy+440.*s,&self.message,ink,15.*s,false,688.*s);
        c.text(ox+56.*s,oy+470.*s,"Up/Down: action   Left/Right: slot   Confirm: select   Back: return",ink,15.*s,false,688.*s);
        if let Some(action)=self.confirm {
            c.rect(ox+120.*s,oy+180.*s,560.*s,164.*s,8.*s,[251,244,226],paper,gold);
            let question=if action==StateAction::Save { format!("Overwrite State {}?",self.slot) } else { format!("Restore State {}'s previous save?",self.slot) };
            c.text(ox+150.*s,oy+208.*s,&question,ink,21.*s,true,500.*s);
            c.text(ox+150.*s,oy+248.*s,"Your current game will stay where it is.",ink,15.*s,false,500.*s);
            for (x,label,selected) in [(150.,"Cancel",!self.accept),(410.,"Confirm",self.accept)] {
                c.rect(ox+x*s,oy+278.*s,240.*s,44.*s,4.*s,if selected {[255,231,158]} else {paper},paper,gold);
                c.text(ox+(x+18.)*s,oy+290.*s,label,ink,17.*s,false,204.*s);
            }
        }
        c.finish(px);
        if self.confirm.is_none() && self.card.image.len()==240*160 {
            let x0=(ox+57.*s).round() as usize; let y0=(oy+109.*s).round() as usize;
            let pw=(346.*s).round().max(1.) as usize; let ph=(230.*s).round().max(1.) as usize;
            for y in 0..ph.min(h.saturating_sub(y0)) { for x in 0..pw.min(w.saturating_sub(x0)) {
                let p=self.card.image[(y*160/ph)*240+x*240/pw];
                px[(y0+y)*w+x0+x]=[((p&31)*255/31) as u8,(((p>>5)&31)*255/31) as u8,(((p>>10)&31)*255/31) as u8,255];
            }}
        }
    }
}
/// Compact, fully rasterized physical-pixel notification.
pub struct StateNoticeImage {
    pub pixels: Vec<[u8;4]>,
    pub size: (u32,u32),
    pub origin: (u32,u32),
}
pub fn state_notice_image(message: &str, window: (u32,u32)) -> StateNoticeImage {
    let (w,h)=(window.0.max(1),window.1.max(1));
    let s=(w as f32/960.).min(h as f32/720.).clamp(0.65,2.);
    let margin=(14.*s).round() as u32;
    let available=w.saturating_sub(margin*2).max(1);
    let width=(((message.chars().count() as f32*11.+32.).clamp(240.,640.)*s).ceil() as u32).min(available).max(1);
    let height=((48.*s).round() as u32).min(h).max(1);
    let mut pixels=vec![[235,220,185,255];width as usize*height as usize];
    let mut c=Canvas::new(&pixels,width as usize,height as usize);
    c.rect(0.,0.,width as f32,height as f32,0.,[251,244,226],[235,220,185],[184,146,65]);
    c.text(14.*s,13.*s,message,[35,24,16],20.*s,false,(width as f32-28.*s).max(1.));
    c.finish(&mut pixels);
    StateNoticeImage { pixels,size:(width,height),origin:((w-width)/2,margin.min(h-height)) }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn occupied_save_requires_explicit_confirmation_and_cancel_is_default() {
        let mut m=StateMenu::default();m.open();m.card.occupied=true;
        assert!(matches!(m.handle(MenuInput::Down),Event::None));
        assert!(matches!(m.handle(MenuInput::Confirm),Event::None));
        assert!(matches!(m.handle(MenuInput::Confirm),Event::None));
        assert!(matches!(m.handle(MenuInput::Confirm),Event::None));
        m.handle(MenuInput::Right);
        assert!(matches!(m.handle(MenuInput::Confirm),Event::Action(StateAction::Save)));
    }
    #[test]
    fn slot_selection_wraps_and_empty_load_is_blocked() {
        let mut m=StateMenu::default();m.open();
        m.handle(MenuInput::Left);assert_eq!(m.slot,30);
        m.handle(MenuInput::Right);assert_eq!(m.slot,1);
        m.handle(MenuInput::Down);m.handle(MenuInput::Down);
        assert!(matches!(m.handle(MenuInput::Confirm),Event::None));
        assert!(!m.message.is_empty());
    }
    #[test]
    fn preview_and_confirmation_render_at_window_and_fullscreen_sizes() {
        let mut menu=StateMenu::default();menu.open();
        menu.card.occupied=true;
        menu.card.image=vec![31;240*160];
        for (w,h) in [(320,240),(854,480),(1920,1080)] {
            let mut pixels=vec![[0,0,0,255];w*h];
            menu.draw(&mut pixels,w,h);
            assert!(pixels.iter().any(|p| *p==[255,0,0,255]));
            menu.selected=1;
            menu.handle(MenuInput::Confirm);
            menu.draw(&mut pixels,w,h);
            menu.handle(MenuInput::Cancel);
            menu.selected=0;
        }
    }

    #[test]
    fn notification_uses_physical_pixels_and_fits_resized_windows() {
        for window in [(1,1),(320,240),(854,480),(1920,1080),(3840,2160)] {
            let image=state_notice_image("Loaded State 1",window);
            assert_eq!(image.pixels.len(),image.size.0 as usize*image.size.1 as usize);
            assert!(image.origin.0+image.size.0<=window.0);
            assert!(image.origin.1+image.size.1<=window.1);
            assert!(image.pixels.iter().all(|p|p[3]==255));
        }
        let small=state_notice_image("Loaded State 1",(854,480));
        let large=state_notice_image("Loaded State 1",(1920,1080));
        assert!(large.size.0>small.size.0);
        assert!(large.size.1>small.size.1);
        assert!(large.size.0<1920/2,"brief messages stay compact");
    }

    #[test]
    fn visible_arrow_buttons_select_the_matching_direction_at_each_resolution() {
        for (w,h) in [(800,520),(854,480),(1920,1080),(2560,1080)] {
            let s=(w as f32/800.).min(h as f32/520.);
            let ox=(w as f32-800.*s)/2.; let oy=(h as f32-520.*s)/2.;
            let point=|x:f32,y:f32|((ox+x*s).round() as usize,(oy+y*s).round() as usize);
            let mut menu=StateMenu::default();menu.open();
            let (x,y)=point(722.,120.);
            assert!(matches!(menu.click(x,y,w,h,true),Event::Action(StateAction::Refresh)));
            assert_eq!(menu.slot,2,"right arrow must increase the slot");
            let (x,y)=point(452.,120.);
            menu.click(x,y,w,h,true);assert_eq!(menu.slot,1);
            menu.click(x,y,w,h,true);assert_eq!(menu.slot,30);
            let (x,y)=point(587.,120.);
            assert!(matches!(menu.click(x,y,w,h,true),Event::None));
            assert_eq!(menu.slot,30,"the label is not an arrow button");
        }
        assert_eq!(slot_arrow(430.,100.),Some(MenuInput::Left));
        assert_eq!(slot_arrow(473.9,139.9),Some(MenuInput::Left));
        assert_eq!(slot_arrow(474.,120.),None);
        assert_eq!(slot_arrow(700.,100.),Some(MenuInput::Right));
        assert_eq!(slot_arrow(744.,120.),None);
        assert_eq!(slot_arrow(722.,140.),None);
    }

}
