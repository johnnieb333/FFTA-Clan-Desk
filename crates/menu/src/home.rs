//! Large pause-menu tiles using the same identity as the roster hub.
use super::{canvas::Canvas,Menu,MAIN_ROWS,Row};
fn transform(w:usize,h:usize)->(f32,f32,f32){let s=(w as f32/900.).min(h as f32/600.);(s,(w as f32-900.*s)/2.,(h as f32-600.*s)/2.)}
pub(super)fn center(i:usize,w:usize,h:usize)->(usize,usize){let(s,x,y)=transform(w,h);((x+(44.+(i/5)as f32*418.+197.)*s)as usize,(y+(116.+(i%5)as f32*82.+34.)*s)as usize)}
pub(super)fn hit(x:usize,y:usize,w:usize,h:usize)->Option<usize>{if w==0||h==0{return None;}let(s,ox,oy)=transform(w,h);let(x,y)=((x as f32-ox)/s,(y as f32-oy)/s);(0..10).find(|i|{let xx=44.+(i/5)as f32*418.;let yy=116.+(i%5)as f32*82.;x>=xx&&x<xx+394.&&y>=yy&&y<yy+68.})}
pub(super)fn draw(menu:&Menu,px:&mut[[u8;4]],w:usize,h:usize){
 if w<32||h<32||px.len()<w*h{return;}for p in px.iter_mut().take(w*h){p[0]=(p[0]as u16/4+5)as u8;p[1]=(p[1]as u16/4+3)as u8;p[2]=(p[2]as u16/4+5)as u8;}
 let mut c=Canvas::new(px,w,h);let(s,ox,oy)=transform(w,h);
 c.rect(ox+20.*s,oy+20.*s,860.*s,560.*s,5.*s,[251,244,226],[239,226,202],[99,59,80]);
 c.text(ox+44.*s,oy+40.*s,"CLAN DESK",[62,34,53],28.*s,true,600.*s);c.text(ox+746.*s,oy+47.*s,"PAUSED",[62,34,53],17.*s,true,110.*s);
 c.text(ox+44.*s,oy+85.*s,"Your game, saves and clan tools",[62,34,53],17.*s,false,812.*s);
 for(i,row)in MAIN_ROWS.iter().enumerate(){let x=ox+(44.+(i/5)as f32*418.)*s;let y=oy+(116.+(i%5)as f32*82.)*s;let active=menu.selected==i;
  c.rect(x,y,394.*s,68.*s,5.*s,if active{[38,129,119]}else{[251,244,226]},if active{[27,106,100]}else{[239,226,202]},[99,59,80]);
  let(label,hint)=match row{Row::Resume=>("Return to game","Resume your paused game"),Row::SaveStatesMenu=>("Saves","Manual states and timed autosaves"),Row::GilEditorMenu=>("Clan & Editor","Characters, inventory and clan tools"),Row::FastForwardMenu=>("Fast forward","Speed and audio behaviour"),Row::TextBubblesMenu=>("Text windows","Dialogue size and layout"),Row::AudioMenu=>("Audio","Volume and enhanced audio"),Row::VideoMenu=>("Video & Display","Window, scaling and picture settings"),Row::InputsMenu=>("Controls","Keyboard, gamepad and mouse"),Row::SwitchRomMenu=>("Switch game","Choose another ROM"),_=>("Exit","Close the application")};let color=if active{[255,251,238]}else{[62,34,53]};
  c.text(x+18.*s,y+11.*s,label,color,20.*s,true,358.*s);c.text(x+18.*s,y+40.*s,hint,color,15.*s,false,358.*s);
 }
 c.text(ox+44.*s,oy+550.*s,"Up/Down: select   Left/Right: column   Confirm: open   Esc: resume",[62,34,53],15.*s,false,812.*s);c.finish(px);
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn clan_theme_home_hit_targets_scale_and_gaps_do_not_activate(){for(w,h)in[(852,480),(1920,1080),(372,160)]{for i in 0..10{let(x,y)=center(i,w,h);assert_eq!(hit(x,y,w,h),Some(i));}}assert_eq!(hit(450,150,900,600),None);}
}
