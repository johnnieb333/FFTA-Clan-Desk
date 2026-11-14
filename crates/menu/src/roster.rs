//! Compact roster hub; artwork is decoded by the runtime from the loaded ROM.
use super::{canvas::Canvas,Menu,Row};
#[derive(Clone,Debug)]pub struct RosterSprite{pub width:usize,pub height:usize,pub pixels:Vec<[u8;4]>}
pub(super)const ACTIONS:[Row;5]=[Row::LevelsMenu,Row::AbilitiesMenu,Row::RenameMenu,Row::CloneMenu,Row::Back];
pub(super)enum Hit{Member(usize),Action(Row),Page(bool)}
fn point(x:usize,y:usize,w:usize,h:usize)->(f32,f32){let s=(w as f32/900.).min(h as f32/600.);((x as f32-(w as f32-900.*s)/2.)/s,(y as f32-(h as f32-600.*s)/2.)/s)}
fn inside(x:f32,y:f32,l:f32,t:f32,w:f32,h:f32)->bool{x>=l&&x<l+w&&y>=t&&y<t+h}
pub(super)fn hit(menu:&Menu,x:usize,y:usize,w:usize,h:usize)->Option<Hit>{
 if w==0||h==0{return None;}let(x,y)=point(x,y,w,h);let first=menu.characters.selected/6*6;
 for i in 0..6{if first+i<menu.characters.rows.len()&&inside(x,y,44.,116.+i as f32*68.,280.,64.){return Some(Hit::Member(first+i));}}
 for(i,&r)in ACTIONS.iter().enumerate(){if inside(x,y,380.,254.+i as f32*48.,452.,38.){return Some(Hit::Action(r));}}
 if inside(x,y,44.,536.,62.,28.){return Some(Hit::Page(false));}if inside(x,y,262.,536.,62.,28.){return Some(Hit::Page(true));}None
}
struct Painter<'a>{c:&'a mut Canvas,s:f32,ox:f32,oy:f32}
impl Painter<'_>{
 fn panel(&mut self,x:f32,y:f32,w:f32,h:f32,active:bool){self.c.rect(self.ox+x*self.s,self.oy+y*self.s,w*self.s,h*self.s,5.*self.s,if active{[38,129,119]}else{[251,244,226]},if active{[27,106,100]}else{[239,226,202]},[99,59,80]);}
 fn text(&mut self,x:f32,y:f32,text:&str,size:f32,title:bool,width:f32,active:bool){self.c.text(self.ox+x*self.s,self.oy+y*self.s,text,if active{[255,251,238]}else{[62,34,53]},size*self.s,title,width*self.s);}
 fn sprite(&mut self,image:&RosterSprite,x:f32,y:f32,w:f32,h:f32){
  if image.width==0||image.height==0||image.pixels.len()!=image.width*image.height{return;}
  let scale=((w*self.s/image.width as f32).min(h*self.s/image.height as f32)).floor().max(1.)as usize;
  let x=(self.ox+(x+w/2.)*self.s-image.width as f32*scale as f32/2.).round()as i32;
  let y=(self.oy+(y+h)*self.s-image.height as f32*scale as f32).round()as i32;let pw=self.c.pix.width()as usize;let ph=self.c.pix.height()as usize;let data=self.c.pix.data_mut();
  for sy in 0..image.height{for sx in 0..image.width{let color=image.pixels[sy*image.width+sx];if color[3]==0{continue;}for dy in 0..scale{for dx in 0..scale{let xx=x+(sx*scale+dx)as i32;let yy=y+(sy*scale+dy)as i32;if xx>=0&&yy>=0&&(xx as usize)<pw&&(yy as usize)<ph{let at=(yy as usize*pw+xx as usize)*4;data[at..at+4].copy_from_slice(&color);}}}}}
 }
}
pub(super)fn draw(menu:&Menu,px:&mut[[u8;4]],w:usize,h:usize){
 if w<32||h<32||px.len()<w*h{return;}for p in px.iter_mut().take(w*h){p[0]=(p[0]as u16/4+5)as u8;p[1]=(p[1]as u16/4+3)as u8;p[2]=(p[2]as u16/4+5)as u8;}
 let mut c=Canvas::new(px,w,h);let s=(w as f32/900.).min(h as f32/600.);let mut u=Painter{c:&mut c,s,ox:(w as f32-900.*s)/2.,oy:(h as f32-600.*s)/2.};
 u.panel(20.,20.,860.,560.,false);u.text(44.,38.,"CLAN DESK",26.,true,500.,false);u.text(710.,44.,"CHARACTERS",17.,true,140.,false);
 u.text(44.,84.,&format!("CLAN ROSTER  {} / 24",menu.characters.rows.len()),17.,true,280.,false);u.panel(350.,82.,506.,434.,false);
 let first=menu.characters.selected/6*6;
 for i in 0..6{if let Some(row)=menu.characters.rows.get(first+i){let y=116.+i as f32*68.;let selected=first+i==menu.characters.selected;u.panel(44.,y,280.,64.,selected);
  if let Some(image)=menu.characters.sprites.get(&row.slot){u.sprite(image,52.,y+5.,45.,52.);}
  u.text(106.,y+10.,&row.name,18.,true,162.,selected);u.text(106.,y+36.,&row.job,15.,false,155.,selected);u.text(274.,y+13.,&format!("L{}",row.level),14.,false,42.,selected);
 }}
 let row=menu.characters.current();if let Some(row)=row{
  if let Some(image)=menu.characters.sprites.get(&row.slot){u.sprite(image,376.,101.,66.,116.);}
  u.text(464.,106.,&row.name,28.,true,360.,false);u.text(464.,146.,&format!("{} / {}",row.race,row.job),18.,false,360.,false);u.text(464.,177.,&format!("Level {}   EXP {}   JP {}",row.level,row.exp,row.jp),17.,false,360.,false);
 }
 for(i,&r)in ACTIONS.iter().enumerate(){let y=254.+i as f32*48.;let active=menu.selected!=0&&super::rows_of(super::Page::Characters).get(menu.selected)==Some(&r);
  let label=match r{Row::LevelsMenu=>"Stats & Level",Row::AbilitiesMenu=>"Jobs & Abilities",Row::RenameMenu=>if menu.characters.rename_allowed(){"Rename"}else{"Rename (protected character)"},Row::CloneMenu=>"Clone member",_=>"Back to Editor"};u.panel(380.,y,452.,38.,active);u.text(398.,y+10.,label,17.,false,416.,active);
 }
 u.text(380.,226.,"Choose an editor. Equip items and abilities in the game.",14.,false,450.,false);
 for(x,label)in[(44.,"<"),(262.,">")]{u.panel(x,536.,62.,28.,false);u.text(x+24.,541.,label,17.,false,30.,false);}
 u.text(116.,542.,&format!("{}-{} of {}",if menu.characters.rows.is_empty(){0}else{first+1},(first+6).min(menu.characters.rows.len()),menu.characters.rows.len()),14.,false,140.,false);
 u.text(350.,541.,"Up/Down: select   Right: actions   Left: roster   Esc: back",14.,false,506.,false);
 if row.is_none(){u.text(380.,124.,&menu.characters.status,17.,false,450.,false);}
 drop(u);c.finish(px);
}
