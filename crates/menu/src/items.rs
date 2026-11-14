//! Fixed-size inventory browser; no game memory or files are written here.
use super::{canvas::Canvas,MenuInput,GilAction};
#[derive(Clone,Debug)]
pub struct OwnedItem {
    pub offset:usize,pub id:u16,pub name:String,pub kind:String,pub category:u8,
    pub total:u8,pub equipped:u8,pub editable:bool,pub teaches:Vec<(String,String)>,
}
const CATEGORIES:[&str;5]=["All","Weapons","Armour","Accessories","Consumables"];
const ROWS:usize=8;
#[derive(Clone,Copy,PartialEq,Eq,Debug)]
enum Area { Category,Search,List,Quantity,Apply,Undo,Back,Pager,Unowned,BulkQuantity,BulkAdd }
impl Default for Area {fn default()->Self{Self::List}}
#[derive(Default)]
pub struct ItemEditor {
    pub current:u8,pub equipped:u8,pub proposed:u8,pub enabled:bool,pub status:String,
    rows:Vec<OwnedItem>,visible:Vec<usize>,selected:usize,first:usize,category:usize,
    query:String,area:Area,editing:bool,confirm:Option<GilAction>,accept:bool,
    pub undo_available:bool,
    include_unowned:bool,
    bulk_quantity:u8,
    pending:std::collections::BTreeMap<u16,u8>,
}
pub(super) enum Event { None,Back,Action(GilAction) }
#[derive(Clone,Copy)]
struct Rect(f32,f32,f32,f32);
impl Rect {fn hit(self,x:f32,y:f32)->bool{x>=self.0&&x<self.0+self.2&&y>=self.1&&y<self.1+self.3}}
const UNOWNED:Rect=Rect(530.,38.,326.,30.);
const BULK_ADD:Rect=Rect(156.,38.,220.,30.);
const BULK_MINUS:Rect=Rect(384.,38.,32.,30.);
const BULK_PLUS:Rect=Rect(480.,38.,32.,30.);
const SEARCH:Rect=Rect(44.,128.,812.,34.);
const MINUS:Rect=Rect(530.,316.,44.,40.);
const PLUS:Rect=Rect(812.,316.,44.,40.);
const APPLY:Rect=Rect(530.,374.,326.,38.);
const UNDO:Rect=Rect(530.,422.,326.,38.);
const BACK:Rect=Rect(530.,470.,326.,36.);
const PREV:Rect=Rect(44.,470.,126.,36.);
const NEXT:Rect=Rect(378.,470.,126.,36.);
const CANCEL:Rect=Rect(210.,316.,220.,44.);
const ACCEPT:Rect=Rect(470.,316.,220.,44.);
impl ItemEditor {
    pub fn selected_item(&self)->Option<&OwnedItem>{self.visible.get(self.selected).and_then(|&i|self.rows.get(i))}
    fn selected_id(&self)->Option<u16>{self.selected_item().map(|i|i.id)}
    pub fn open_rows(&mut self,rows:Result<Vec<OwnedItem>,String>){
        self.pending.clear();self.include_unowned=false;self.bulk_quantity=1;
        let id=self.selected_id().or(Some(277));
        self.category=0;self.query.clear();self.area=Area::List;self.editing=false;self.confirm=None;
        self.replace_rows(rows,id);
    }
    pub fn refresh_rows(&mut self,rows:Result<Vec<OwnedItem>,String>){let id=self.selected_id();self.pending.clear();self.replace_rows(rows,id);}
    pub fn pending_changes(&self)->Vec<(OwnedItem,u8)>{
        self.rows.iter().filter_map(|r|self.pending.get(&r.id).map(|&n|(r.clone(),n))).collect()
    }
    fn replace_rows(&mut self,rows:Result<Vec<OwnedItem>,String>,id:Option<u16>){
        match rows {Ok(rows)=>{self.rows=rows;self.enabled=true;self.status="Include unowned to browse the catalog. Equipped quantities protected.".into();}
            Err(e)=>{self.rows.clear();self.enabled=false;self.status=e;}}
        self.filter(id);
    }
    fn filter(&mut self,id:Option<u16>){
        let query=self.query.to_ascii_lowercase();
        self.visible=self.rows.iter().enumerate().filter_map(|(i,row)|
            ((row.total>0 || self.include_unowned) && (self.category==0||row.category as usize==self.category) && row.name.to_ascii_lowercase().contains(&query)).then_some(i)).collect();
        self.selected=id.and_then(|id|self.visible.iter().position(|&i|self.rows[i].id==id)).unwrap_or(0);
        self.first=0;self.sync_selected();
    }
    fn toggle_unowned(&mut self){let id=self.selected_id();self.include_unowned=!self.include_unowned;self.filter(id);}
    fn adjust_bulk(&mut self,right:bool){
        self.bulk_quantity=if right{self.bulk_quantity.saturating_add(1).clamp(1,10)}else{self.bulk_quantity.saturating_sub(1).clamp(1,10)};
    }
    fn add_all_unowned(&mut self){
        if !self.enabled{return;}
        let quantity=self.bulk_quantity.clamp(1,10);let mut count=0;
        // Deliberately use the full catalog, independently of search/category.
        // Owned and equipped items, protected records and other edits survive.
        for row in &self.rows {if row.editable && row.total==0 && row.equipped==0 {
            self.pending.insert(row.id,quantity);count+=1;
        }}
        self.status=if count==0{"No unowned items to add.".into()}else{format!("Staged {count} unowned items at {quantity} each. Apply changes to save.")};
        self.sync_selected();
    }
    fn sync_selected(&mut self){
        self.selected=self.selected.min(self.visible.len().saturating_sub(1));
        let counts=self.selected_item().map(|r|(r.total,r.equipped));
        if let Some((total,equipped))=counts {self.current=total;self.equipped=equipped;self.proposed=self.selected_id().and_then(|id|self.pending.get(&id).copied()).unwrap_or(total);}
        else {self.current=0;self.equipped=0;self.proposed=0;}
        if self.selected<self.first {self.first=self.selected;}
        if self.selected>=self.first+ROWS {self.first=self.selected+1-ROWS;}
        self.first=self.first.min(self.visible.len().saturating_sub(1));
    }
    fn move_item(&mut self,delta:isize){
        self.selected=self.selected.saturating_add_signed(delta).min(self.visible.len().saturating_sub(1));self.sync_selected();
    }
    fn page(&mut self,next:bool){
        let last=self.visible.len().saturating_sub(1)/ROWS*ROWS;
        self.first=if next {(self.first+ROWS).min(last)} else {self.first.saturating_sub(ROWS)};
        self.selected=self.first;self.sync_selected();
    }
    fn category(&mut self,next:bool){
        self.category=if next {(self.category+1)%CATEGORIES.len()}else{(self.category+CATEGORIES.len()-1)%CATEGORIES.len()};self.filter(None);
    }
    pub(super) fn adjust(&mut self,right:bool){
        if !self.enabled||!self.selected_item().is_some_and(|r|r.editable){return;}
        self.proposed=if right {self.proposed.saturating_add(1).min(99)}else{self.proposed.saturating_sub(1).max(self.equipped.max(u8::from(self.current>0)))};
        if let Some(id)=self.selected_id(){if self.proposed==self.current{self.pending.remove(&id);}else{self.pending.insert(id,self.proposed);}}
    }
    pub fn search_active(&self)->bool{self.editing}
    pub fn finish_search(&mut self){self.editing=false;self.area=Area::List;}
    pub fn search_text(&mut self,text:&str,backspace:bool){
        if !self.editing{return;}
        if backspace {self.query.pop();}else{for c in text.chars().filter(|c|!c.is_control()){if self.query.chars().count()<40{self.query.push(c);}}}
        self.filter(None);
    }
    pub fn result(&mut self,status:String){self.confirm=None;self.status=status;}
    fn request(&mut self,action:GilAction)->Event{
        if action==GilAction::ItemApply {
            if !self.enabled||self.pending.is_empty(){self.status="No pending changes to apply.".into();return Event::None;}
        }else if !self.undo_available{self.status="No recovery state available.".into();return Event::None;}
        self.confirm=Some(action);self.accept=false;self.editing=false;Event::None
    }
    pub(super) fn handle(&mut self,input:MenuInput)->Event{
        if let Some(action)=self.confirm {
            match input {MenuInput::Cancel=>self.confirm=None,
                MenuInput::Left|MenuInput::Right|MenuInput::Up|MenuInput::Down=>self.accept=!self.accept,
                MenuInput::Confirm=>{self.confirm=None;if self.accept{return Event::Action(action);}}}
            return Event::None;
        }
        if self.editing {if matches!(input,MenuInput::Confirm|MenuInput::Cancel){self.finish_search();}return Event::None;}
        if input==MenuInput::Cancel{return Event::Back;}
        match (self.area,input){
            (Area::Category,MenuInput::Left|MenuInput::Right)=>self.category(input==MenuInput::Right),
            (Area::Category,MenuInput::Down|MenuInput::Confirm)=>self.area=Area::Search,
            (Area::Category,MenuInput::Up)=>self.area=Area::Unowned,
            (Area::Unowned,MenuInput::Confirm|MenuInput::Left|MenuInput::Right)=>self.toggle_unowned(),
            (Area::Unowned,MenuInput::Down)=>self.area=Area::Category,
            (Area::Unowned,MenuInput::Up)=>self.area=Area::BulkQuantity,
            (Area::BulkQuantity,MenuInput::Left|MenuInput::Right)=>self.adjust_bulk(input==MenuInput::Right),
            (Area::BulkQuantity,MenuInput::Up|MenuInput::Confirm)=>self.area=Area::BulkAdd,
            (Area::BulkQuantity,MenuInput::Down)=>self.area=Area::Unowned,
            (Area::BulkAdd,MenuInput::Confirm)=>self.add_all_unowned(),
            (Area::BulkAdd,MenuInput::Right|MenuInput::Down)=>self.area=Area::BulkQuantity,
            (Area::BulkAdd,MenuInput::Left)=>self.area=Area::Unowned,
            (Area::BulkAdd,MenuInput::Up)=>self.area=Area::Back,
            (Area::Search,MenuInput::Confirm)=>self.editing=true,
            (Area::Search,MenuInput::Up)=>self.area=Area::Category,
            (Area::Search,MenuInput::Down)=>self.area=Area::List,
            (Area::List,MenuInput::Up)=>{if self.selected==0{self.area=Area::Search;}else{self.move_item(-1);}}
            (Area::List,MenuInput::Down)=>{if self.selected+1>=self.visible.len(){self.area=Area::Pager;}else{self.move_item(1);}}
            (Area::List,MenuInput::Right|MenuInput::Confirm)=>self.area=Area::Quantity,
            (Area::List,MenuInput::Left)=>self.area=Area::Category,
            (Area::Quantity,MenuInput::Left|MenuInput::Right)=>self.adjust(input==MenuInput::Right),
            (Area::Quantity,MenuInput::Up)=>self.area=Area::List,
            (Area::Quantity,MenuInput::Down|MenuInput::Confirm)=>self.area=Area::Apply,
            (Area::Apply,MenuInput::Confirm)=>return self.request(GilAction::ItemApply),
            (Area::Undo,MenuInput::Confirm)=>return self.request(GilAction::Undo),
            (Area::Back,MenuInput::Confirm)=>return Event::Back,
            (Area::Apply,MenuInput::Up)=>self.area=Area::Quantity,
            (Area::Apply,MenuInput::Down)=>self.area=Area::Undo,
            (Area::Undo,MenuInput::Up)=>self.area=Area::Apply,
            (Area::Undo,MenuInput::Down)=>self.area=Area::Back,
            (Area::Back,MenuInput::Up)=>self.area=Area::Undo,
            (Area::Back,MenuInput::Down)=>self.area=Area::Unowned,
            (Area::Apply|Area::Undo|Area::Back,MenuInput::Left)=>self.area=Area::List,
            (Area::Pager,MenuInput::Left)=>self.page(false),
            (Area::Pager,MenuInput::Right|MenuInput::Confirm)=>self.page(true),
            (Area::Pager,MenuInput::Up)=>self.area=Area::List,
            (Area::Pager,MenuInput::Down)=>self.area=Area::Category,
            _=>{}
        }
        Event::None
    }
    fn point(x:usize,y:usize,w:usize,h:usize)->(f32,f32){
        let s=(w as f32/900.).min(h as f32/600.);
        ((x as f32-(w as f32-900.*s)/2.)/s,(y as f32-(h as f32-600.*s)/2.)/s)
    }
    pub(super) fn hover(&mut self,x:usize,y:usize,w:usize,h:usize){
        if self.confirm.is_some(){let(x,y)=Self::point(x,y,w,h);if CANCEL.hit(x,y){self.accept=false;}if ACCEPT.hit(x,y){self.accept=true;}}
    }
    pub(super) fn scroll(&mut self,up:bool)->Event{if self.confirm.is_none(){self.editing=false;self.area=Area::List;self.move_item(if up{-3}else{3});}Event::None}
    pub(super) fn click(&mut self,x:usize,y:usize,w:usize,h:usize,left:bool)->Event{
        if !left{return self.handle(MenuInput::Cancel);}
        let(x,y)=Self::point(x,y,w,h);
        if self.confirm.is_some(){if CANCEL.hit(x,y){self.accept=false;return self.handle(MenuInput::Confirm);}if ACCEPT.hit(x,y){self.accept=true;return self.handle(MenuInput::Confirm);}return Event::None;}
        if BULK_ADD.hit(x,y){self.editing=false;self.area=Area::BulkAdd;self.add_all_unowned();return Event::None;}
        if BULK_MINUS.hit(x,y)||BULK_PLUS.hit(x,y){self.editing=false;self.area=Area::BulkQuantity;self.adjust_bulk(BULK_PLUS.hit(x,y));return Event::None;}
        if UNOWNED.hit(x,y){self.editing=false;self.area=Area::Unowned;self.toggle_unowned();return Event::None;}
        for i in 0..CATEGORIES.len(){if Rect(44.+i as f32*(812./CATEGORIES.len()as f32),82.,812./CATEGORIES.len()as f32-4.,34.).hit(x,y){self.editing=false;self.area=Area::Category;self.category=i;self.filter(None);return Event::None;}}
        if SEARCH.hit(x,y){self.area=Area::Search;self.editing=true;return Event::None;}
        self.editing=false;
        for i in 0..ROWS{if Rect(44.,176.+i as f32*36.,460.,34.).hit(x,y){
            let selected=self.first+i;if selected<self.visible.len(){self.selected=selected;self.sync_selected();self.area=Area::List;}return Event::None;}}
        if MINUS.hit(x,y){self.area=Area::Quantity;self.adjust(false);}
        else if PLUS.hit(x,y){self.area=Area::Quantity;self.adjust(true);}
        else if APPLY.hit(x,y){self.area=Area::Apply;return self.request(GilAction::ItemApply);}
        else if UNDO.hit(x,y){self.area=Area::Undo;return self.request(GilAction::Undo);}
        else if BACK.hit(x,y){return Event::Back;}
        else if PREV.hit(x,y){self.area=Area::Pager;self.page(false);}
        else if NEXT.hit(x,y){self.area=Area::Pager;self.page(true);}
        Event::None
    }
    pub(super) fn draw(&self,px:&mut [[u8;4]],w:usize,h:usize){
        if w==0||h==0||px.len()<w*h{return;}
        for p in px.iter_mut().take(w*h){
            p[0]=(p[0]as u16*5/16+5)as u8;
            p[1]=(p[1]as u16*4/16+3)as u8;
            p[2]=(p[2]as u16*3/16+1)as u8;
        }
        let mut canvas=Canvas::new(px,w,h);
        let s=(w as f32/900.).min(h as f32/600.);let ox=(w as f32-900.*s)/2.;let oy=(h as f32-600.*s)/2.;
        let mut ui=Painter{c:&mut canvas,s,ox,oy};
        ui.panel(Rect(20.,20.,860.,560.),false);
        ui.text(44.,40.,"ITEMS",24.,true,812.);
        ui.panel(BULK_ADD,self.area==Area::BulkAdd);
        ui.text(168.,46.,"Add all unowned",15.,false,196.);
        ui.panel(Rect(384.,38.,128.,30.),self.area==Area::BulkQuantity);
        ui.text(423.,46.,&format!("{}",self.bulk_quantity.clamp(1,10)),15.,false,50.);
        ui.panel(BULK_MINUS,false);ui.text(395.,45.,"-",17.,false,18.);
        ui.panel(BULK_PLUS,false);ui.text(489.,45.,"+",17.,false,18.);
        ui.panel(UNOWNED,self.include_unowned||self.area==Area::Unowned);
        ui.text(542.,46.,if self.include_unowned{"[x] Include unowned"}else{"[ ] Include unowned"},15.,false,302.);
        for (i,label) in CATEGORIES.iter().enumerate(){let r=Rect(44.+i as f32*(812./CATEGORIES.len()as f32),82.,812./CATEGORIES.len()as f32-4.,34.);ui.panel(r,self.category==i);ui.text(r.0+10.,r.1+9.,label,15.,false,r.2-20.);}
        ui.panel(SEARCH,self.area==Area::Search||self.editing);
        let query=if self.query.is_empty(){"Search item names...".into()}else{format!("Search: {}{}",self.query,if self.editing{"_"}else{""})};
        ui.text(56.,137.,&query,15.,false,788.);
        ui.text(44.,164.,"ITEM",14.,false,320.);ui.text(390.,164.,"EQ / OWNED",14.,false,114.);
        for i in 0..ROWS{
            let index=self.first+i;let r=Rect(44.,176.+i as f32*36.,460.,34.);ui.panel(r,index==self.selected&&!self.visible.is_empty());
            if let Some(&entry)=self.visible.get(index){let item=&self.rows[entry];ui.text(54.,r.1+9.,&item.name,15.,false,322.);ui.text(390.,r.1+9.,&format!("{} / {}{}",item.equipped,self.pending.get(&item.id).copied().unwrap_or(item.total),if self.pending.contains_key(&item.id){" *"}else{""}),14.,false,104.);}
        }
        if self.visible.is_empty(){
            if !self.enabled && self.status=="Return to the world map to edit inventory." {
                ui.text(56.,190.,"Return to the world map",18.,true,430.);
                ui.text(56.,222.,"to edit inventory.",17.,false,430.);
            }else if !self.enabled{
                ui.text(56.,190.,"Inventory editor unavailable",17.,true,430.);
                ui.text(56.,222.,"See the message below.",15.,false,430.);
            }else{ui.text(56.,190.,"No matching items",16.,false,430.);}
        }
        ui.panel(PREV,self.area==Area::Pager);ui.text(56.,481.,"< Previous",14.,false,108.);
        ui.panel(NEXT,self.area==Area::Pager);ui.text(393.,481.,"Next >",14.,false,102.);
        let end=(self.first+ROWS).min(self.visible.len());
        ui.text(180.,481.,&format!("{}-{} of {}",if self.visible.is_empty(){0}else{self.first+1},end,self.visible.len()),14.,false,186.);
        if let Some(item)=self.selected_item(){
            // Keep the original readable type sizes. The unused right-hand
            // space holds a separate ability/job column, above the quantity row.
            let title=wrap_details(&item.name,19.,160.,true);
            for (i,line) in title.iter().take(2).enumerate(){ui.text(530.,176.+i as f32*21.,line,19.,true,160.);}
            ui.text(530.,224.,&item.kind,15.,false,160.);
            ui.text(530.,252.,&format!("Total owned: {}",item.total),17.,false,160.);
            ui.text(530.,280.,&format!("Equipped: {}",item.equipped),17.,false,160.);
            ui.text(700.,176.,"TEACHES",15.,true,156.);
            if item.teaches.is_empty(){
                for (i,line) in wrap_details("No abilities taught.",17.,156.,false).iter().enumerate(){
                    ui.text(700.,202.+i as f32*19.,line,17.,false,156.);
                }
            }else{
                let pitch=if item.teaches.len()==3{38.}else if item.teaches.len()==2{58.}else{114.};
                for (i,(ability,jobs)) in item.teaches.iter().take(3).enumerate(){
                    let y=198.+i as f32*pitch;
                    ui.text(700.,y,ability,17.,false,156.);
                    for (j,line) in wrap_details(jobs,17.,156.,false).iter().enumerate(){
                        ui.text(700.,y+19.+j as f32*19.,line,17.,false,156.);
                    }
                }
            }
        }else{ui.text(530.,176.,if self.enabled {"Select an item"} else {"Editing unavailable"},19.,true,326.);}
        ui.panel(Rect(530.,316.,326.,40.),self.area==Area::Quantity);
        ui.text(590.,328.,&format!("New total: {}",self.proposed),17.,false,206.);
        ui.panel(MINUS,false);ui.text(544.,327.,"-",19.,false,20.);
        ui.panel(PLUS,false);ui.text(825.,327.,"+",19.,false,20.);
        for (r,label,active) in [(APPLY,"Apply changes",self.area==Area::Apply),(UNDO,"Undo Last Edit",self.area==Area::Undo),(BACK,"Back",self.area==Area::Back)]{
            ui.panel(r,active);ui.text(r.0+14.,r.1+11.,label,16.,false,r.2-28.);
        }
        let status=if self.pending.is_empty(){self.status.clone()}else{format!("{} pending item changes. Apply changes to save; Back discards on exit.",self.pending.len())};
        ui.text(44.,526.,&status,14.,false,812.);
        ui.text(44.,552.,"Up/Down: list   Right: edit   Left/Right: quantity   Confirm: select   Back: return",12.,false,812.);
        if let Some(action)=self.confirm{
            ui.panel(Rect(180.,206.,540.,174.),false);
            ui.text(210.,226.,if action==GilAction::ItemApply{"APPLY ALL CHANGES?"}else{"UNDO LAST EDIT?"},21.,true,480.);
            let message=if action==GilAction::ItemApply{format!("Update {} items. Equipped quantities stay unchanged.",self.pending.len())}else{"Restore the full game state before the last edit.".into()};
            ui.text(210.,267.,&message,15.,false,480.);
            ui.panel(CANCEL,!self.accept);ui.text(230.,330.,"Cancel",16.,false,180.);
            ui.panel(ACCEPT,self.accept);ui.text(490.,330.,"Confirm",16.,false,180.);
        }
        drop(ui);
        canvas.finish(px);
    }
}
fn wrap_details(text:&str,size:f32,width:f32,title:bool)->Vec<String>{
    use ab_glyph::{Font,FontRef,ScaleFont,VariableFont};
    let bytes:&[u8]=if title {include_bytes!("../assets/fonts/Cinzel.ttf")}else{include_bytes!("../assets/fonts/Inter.ttf")};
    let mut font=FontRef::try_from_slice(bytes).expect("Bundled menu font");
    font.set_variation(b"wght",if title{700.}else{600.});let scaled=font.as_scaled(size);
    let measure=|s:&str|{let mut width=0.;let mut last=None;for c in s.chars(){let id=font.glyph_id(c);
        if let Some(prev)=last{width+=scaled.kern(prev,id);}width+=scaled.h_advance(id);last=Some(id);}width};
    let mut result=Vec::new();let mut line=String::new();
    for word in text.split_whitespace(){
        let next=if line.is_empty(){word.into()}else{format!("{line} {word}")};
        if !line.is_empty() && measure(&next)>width{result.push(line);line=word.into();}else{line=next;}
    }
    if !line.is_empty(){result.push(line);}result
}
struct Painter<'a>{c:&'a mut Canvas,s:f32,ox:f32,oy:f32}
impl Painter<'_>{
    fn panel(&mut self,r:Rect,active:bool){self.c.rect(self.ox+r.0*self.s,self.oy+r.1*self.s,r.2*self.s,r.3*self.s,4.*self.s,
        if active{[255,231,158]}else{[251,244,226]},if active{[242,208,123]}else{[235,220,185]},[184,146,65]);}
    fn text(&mut self,x:f32,y:f32,text:&str,size:f32,title:bool,width:f32){self.c.text(self.ox+x*self.s,self.oy+y*self.s,text,[35,24,16],size.max(15.)*self.s,title,width*self.s);}
}
#[cfg(test)]
mod item_editor_tests{
    use super::*;
    fn rows()->Vec<OwnedItem>{(1..=24).map(|id|OwnedItem{offset:id as usize*4,id,name:format!("Item {id:02}"),kind:"Sword".into(),category:if id%2==0{1}else{4},total:3,equipped:1,editable:true,teaches:Vec::new()}).collect()}
    #[test]
    fn item_editor_filters_scrolls_and_protects_equipped_items(){
        let mut e=ItemEditor::default();e.open_rows(Ok(rows()));
        for _ in 0..12{e.move_item(1);}assert!(e.first>0);assert_eq!(e.selected_item().unwrap().id,13);
        e.category=4;e.filter(None);assert_eq!(e.visible.len(),12);
        e.editing=true;e.search_text("Item 03",false);assert_eq!(e.visible.len(),1);assert_eq!(e.selected_item().unwrap().id,3);
        e.finish_search();for _ in 0..6{e.adjust(false);}assert_eq!(e.proposed,1);assert_eq!(e.current,3);
        e.adjust(true);assert_eq!(e.proposed,2);
    }
    #[test]
    fn item_editor_confirm_is_cancel_by_default_and_mouse_arrows_match(){
        for (w,h) in [(900,600),(854,480),(1920,1080)]{
            let mut e=ItemEditor::default();e.open_rows(Ok(rows()));
            let s=(w as f32/900.).min(h as f32/600.);let ox=(w as f32-900.*s)/2.;let oy=(h as f32-600.*s)/2.;
            let(x,y)=((ox+834.*s).round() as usize,(oy+336.*s).round() as usize);
            e.click(x,y,w,h,true);assert_eq!(e.proposed,4);
            assert!(matches!(e.request(GilAction::ItemApply),Event::None));
            assert!(matches!(e.handle(MenuInput::Confirm),Event::None));
            assert_eq!(e.current,3);
            e.request(GilAction::ItemApply);e.handle(MenuInput::Right);
            assert!(matches!(e.handle(MenuInput::Confirm),Event::Action(GilAction::ItemApply)));
            let mut pixels=vec![[0,0,0,255];w*h];e.draw(&mut pixels,w,h);
        }
    }
    #[test]
    fn item_editor_page_buttons_and_search_keep_selections_in_bounds(){
        let mut e=ItemEditor::default();e.open_rows(Ok(rows()));
        e.page(true);assert_eq!(e.first,8);assert_eq!(e.selected,8);
        e.page(true);assert_eq!(e.first,16);
        e.page(true);assert_eq!(e.first,16);
        e.page(false);assert_eq!(e.first,8);
        e.editing=true;e.search_text("missing",false);assert!(e.visible.is_empty());
        assert!(matches!(e.request(GilAction::ItemApply),Event::None));
        e.page(true);assert!(e.selected_item().is_none());
        e.open_rows(Err("Return to the map".into()));assert!(!e.enabled);
    }

    #[test]
    fn item_editor_mythril_search_lists_unowned_weapons_and_addition_requires_confirmation(){
        let mut r=rows();r.push(OwnedItem{offset:1000,id:250,name:"Mythril Gun".into(),kind:"Gun".into(),category:1,total:0,equipped:0,editable:true,teaches:Vec::new()});
        let mut e=ItemEditor::default();e.open_rows(Ok(r));
        assert!(!e.visible.iter().any(|&i|e.rows[i].id==250));
        e.click(250,98,900,600,true);assert_eq!(e.category,1);
        e.toggle_unowned();e.editing=true;e.search_text("Mythril",false);e.finish_search();assert_eq!(e.visible.len(),1);
        assert_eq!(e.selected_item().unwrap().id,250);assert_eq!(e.proposed,0);
        e.adjust(false);assert_eq!(e.proposed,0);e.adjust(true);assert_eq!(e.proposed,1);
        assert!(matches!(e.request(GilAction::ItemApply),Event::None));
        assert!(matches!(e.handle(MenuInput::Confirm),Event::None)); // default Cancel
        e.request(GilAction::ItemApply);e.handle(MenuInput::Right);
        assert!(matches!(e.handle(MenuInput::Confirm),Event::Action(GilAction::ItemApply)));
        e.category=4;e.category(true);assert_eq!(e.category,0);e.category(false);assert_eq!(e.category,4);
        e.category=1;e.query.clear();
        e.editing=true;e.search_text("gun",false);assert_eq!(e.visible.len(),1);
    }

    #[test]
    fn item_editor_batch_retains_quantities_across_selection_search_and_confirmation_cancel(){
        let mut e=ItemEditor::default();e.open_rows(Ok(rows()));
        e.adjust(true);e.move_item(1);e.adjust(true);e.adjust(true);
        assert_eq!(e.pending_changes().len(),2);e.move_item(-1);assert_eq!(e.proposed,4);
        e.editing=true;e.search_text("Item 02",false);assert_eq!(e.proposed,5);e.finish_search();
        e.request(GilAction::ItemApply);assert!(matches!(e.handle(MenuInput::Confirm),Event::None));
        assert_eq!(e.pending_changes().len(),2);
        e.query.clear();e.filter(Some(1));e.adjust(false);assert_eq!(e.pending_changes().len(),1);
        e.open_rows(Ok(rows()));assert!(e.pending_changes().is_empty());
    }

    #[test]
    fn item_editor_unowned_toggle_preserves_pending_changes_and_filters_categories(){
        let mut r=rows();
        r.push(OwnedItem{offset:1000,id:100,name:"Unowned Rapier".into(),kind:"Rapier".into(),category:1,total:0,equipped:0,editable:true,teaches:Vec::new()});
        r.push(OwnedItem{offset:1000,id:300,name:"Unowned Armour".into(),kind:"Armour".into(),category:2,total:0,equipped:0,editable:true,teaches:Vec::new()});
        let mut e=ItemEditor::default();e.open_rows(Ok(r));assert!(!e.include_unowned);assert_eq!(e.visible.len(),24);
        e.click(650,52,900,600,true);assert!(e.include_unowned);assert_eq!(e.visible.len(),26);
        e.filter(Some(100));assert_eq!(e.selected_id(),Some(100));e.adjust(true);
        e.toggle_unowned();assert_eq!(e.pending_changes().len(),1);assert!(!e.visible.iter().any(|&i|e.rows[i].id==100));
        e.toggle_unowned();e.filter(Some(100));assert_eq!(e.proposed,1);
        e.category=2;e.filter(None);assert!(e.visible.iter().any(|&i|e.rows[i].id==300));
        e.area=Area::Unowned;e.handle(MenuInput::Confirm);assert!(!e.include_unowned);
    }

}

#[cfg(test)]mod item_editor_add_all_unowned_tests{
 use super::*;
 fn row(id:u16,total:u8,equipped:u8,editable:bool)->OwnedItem{OwnedItem{offset:id as usize*4,id,name:format!("Item {id}"),kind:"Item".into(),category:1,total,equipped,editable,teaches:Vec::new()}}
 #[test]fn item_editor_bulk_add_ignores_filters_preserves_owned_equipped_and_existing_edits(){
  let mut e=ItemEditor::default();e.open_rows(Ok(vec![row(1,0,0,true),row(2,5,1,true),row(3,0,0,false),row(4,0,1,true),row(5,0,0,true)]));
  e.pending.insert(2,8);e.query="no matches".into();e.category=4;e.filter(None);assert!(e.visible.is_empty());
  e.bulk_quantity=7;e.add_all_unowned();assert_eq!(e.pending.get(&1),Some(&7));assert_eq!(e.pending.get(&5),Some(&7));assert_eq!(e.pending.get(&2),Some(&8));assert!(!e.pending.contains_key(&3));assert!(!e.pending.contains_key(&4));
  assert_eq!(e.pending_changes().len(),3);assert_eq!(e.rows[0].total,0);
  e.request(GilAction::ItemApply);assert!(matches!(e.handle(MenuInput::Confirm),Event::None));assert_eq!(e.pending_changes().len(),3);
  e.request(GilAction::ItemApply);e.handle(MenuInput::Right);assert!(matches!(e.handle(MenuInput::Confirm),Event::Action(GilAction::ItemApply)));
 }
 #[test]fn item_editor_bulk_quantity_mouse_keyboard_and_disabled_state(){
  let mut e=ItemEditor::default();e.open_rows(Ok(vec![row(1,0,0,true)]));
  for _ in 0..20{e.adjust_bulk(false);}assert_eq!(e.bulk_quantity,1);
  for _ in 0..20{e.adjust_bulk(true);}assert_eq!(e.bulk_quantity,10);
  e.area=Area::Unowned;e.handle(MenuInput::Up);assert_eq!(e.area,Area::BulkQuantity);e.handle(MenuInput::Left);assert_eq!(e.bulk_quantity,9);e.handle(MenuInput::Up);e.handle(MenuInput::Confirm);assert_eq!(e.pending.get(&1),Some(&9));
  for (w,h) in [(900,600),(854,480),(1920,1080)]{
   e.open_rows(Ok(vec![row(1,0,0,true)]));let scale=(w as f32/900.).min(h as f32/600.);let ox=(w as f32-900.*scale)/2.;let oy=(h as f32-600.*scale)/2.;
   let point=|x:f32,y:f32|((ox+x*scale).round()as usize,(oy+y*scale).round()as usize);
   let(x,y)=point(495.,53.);e.click(x,y,w,h,true);assert_eq!(e.bulk_quantity,2);
   let(x,y)=point(250.,53.);e.click(x,y,w,h,true);assert_eq!(e.pending.get(&1),Some(&2));assert!(e.confirm.is_none());
  }
  e.open_rows(Err("Unavailable".into()));e.add_all_unowned();assert!(e.pending.is_empty());
 }
}
