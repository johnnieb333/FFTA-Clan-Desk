//! Opt-in EUR Gil/AP/EXP/JP rewards. Scale freshly calculated result fields once,
//! before the original results constructor displays and eventually awards them.
//! Ordinary money changes, unit AP arrays and cartridge files are not edited.
use crate::{ffta_rom,Machine};
pub const FACTORS:[u8;5]=[1,2,3,5,10];
const GIL:usize=0x1f510;
const AP:usize=0x1f514;
const MAX_GIL:u32=99_999_999; // Original EUR result-credit saturation constant.
const MAX_AP_UNITS:u16=127; // 10 AP per unit; keep low-seven-bit progress safe.

#[derive(Clone,Copy,Debug)]
pub struct State {verified:bool,pub gil:u8,pub ap:u8,pub exp:u8,pub jp:u8}
impl Default for State {fn default()->Self{Self{verified:false,gil:1,ap:1,exp:1,jp:1}}}
pub fn trace_enabled()->bool {std::env::var("FFTA_REWARD_TRACE").as_deref()==Ok("1")}
fn factor(value:u8)->u8 {if FACTORS.contains(&value){value}else{1}}
impl State {
    pub fn configured(rom:&[u8],gil:u8,ap:u8)->Self {
        Self::configured_all(rom,gil,ap,1,1)
    }
    pub fn configured_all(rom:&[u8],gil:u8,ap:u8,exp:u8,jp:u8)->Self {
        let(gil,ap,exp,jp)=(factor(gil),factor(ap),factor(exp),factor(jp));
        let verified=ffta_rom::version(rom)==Some(ffta_rom::Version::Europe)
            && rom.get(0x498f8..0x49900)==Some(&[0x0a,0x48,0x00,0x68,0x0a,0x49,0x09,0x68][..])
            && rom.get(0x49b0e..0x49b16)==Some(&[0x0b,0x48,0x00,0x68,0x0b,0x49,0x09,0x68][..])
            && rom.get(0xaa806..0xaa80a)==Some(&[0x00,0x04,0x00,0x14][..])
            && rom.get(0xbf094..0xbf098)==Some(&[0x31,0x1c,0x78,0x31][..])
            && rom.get(0xbf3d8..0xbf3dc)==Some(&[0x00,0x24,0x30,0x1c][..]);
        if verified {Self{verified,gil,ap,exp,jp}}else{Self::default()}
    }
    pub fn from_env(rom:&[u8])->Self {
        let read=|name|std::env::var(name).ok().and_then(|s|s.parse::<u8>().ok()).map(factor).unwrap_or(1);
        let gil=read("FFTA_GIL_MULTIPLIER");let ap=read("FFTA_AP_MULTIPLIER");
        let exp=read("FFTA_EXP_MULTIPLIER");let jp=read("FFTA_JP_MULTIPLIER");
        let state=Self::configured_all(rom,gil,ap,exp,jp);
        if gil!=1 || ap!=1 || exp!=1 || jp!=1 {
            eprintln!("FFTA_REWARDS supported={} Gil={}x AP={}x EXP={}x JP={}x",state.verified,state.gil,state.ap,state.exp,state.jp);
        }
        state
    }
    pub fn interprets(self,key:u32)->bool {
        self.verified && ((self.gil!=1 || self.ap!=1) && matches!(key,0x080498f9|0x08049b0f)
            || self.exp!=1 && key==0x080aa807
            || self.jp!=1 && matches!(key,0x080bf095|0x080bf3d9))
    }
}
fn scaled(gil:u32,ap:u16,settings:State)->(u32,u16) {
    (if settings.gil==1{gil}else{(u64::from(gil)*u64::from(settings.gil)).min(u64::from(MAX_GIL))as u32},
     if settings.ap==1{ap}else{(u32::from(ap)*u32::from(settings.ap)).min(u32::from(MAX_AP_UNITS))as u16})
}
pub fn before_instruction(m:&mut Machine) {
    let settings=m.bus.ffta_rewards;
    if !settings.interprets(m.cpu.regs[15]|m.cpu.thumb()as u32){return;}
    let pc=m.cpu.regs[15];
    if pc==0x080aa806 {
        let raw=m.cpu.regs[0]as u16 as i16;
        if raw>0 {m.cpu.regs[0]=(raw as u32*u32::from(settings.exp)).min(100);}
        return;
    }
    if matches!(pc,0x080bf094|0x080bf3d8) {
        scale_jp_queue(m,settings.jp,pc==0x080bf3d8);
        return;
    }
    let gil=u32::from_le_bytes(m.bus.ewram[GIL..GIL+4].try_into().unwrap());
    let ap=u16::from_le_bytes(m.bus.ewram[AP..AP+2].try_into().unwrap());
    let(new_gil,new_ap)=scaled(gil,ap,settings);
    m.bus.ewram[GIL..GIL+4].copy_from_slice(&new_gil.to_le_bytes());
    m.bus.ewram[AP..AP+2].copy_from_slice(&new_ap.to_le_bytes());
    eprintln!("FFTA_REWARD_APPLIED pc={:08x} Gil={gil}->{new_gil} AP={}->{}",m.cpu.regs[15],u32::from(ap)*10,u32::from(new_ap)*10);
}
/// Presentation starts only after all original KO/law contributions have
/// been combined. Scale here, once, so popup and final credit share an amount.
/// Only clan units are affected; enemy and guest reward groups stay native.
fn scale_jp_queue(m:&mut Machine,multiplier:u8,fast:bool) {
    let Some(o)=m.cpu.regs[6].checked_sub(0x02000000).map(|o|o as usize)
        .filter(|&o|o+0x80<=m.bus.ewram.len()) else{return;};
    let phase=u16::from_le_bytes(m.bus.ewram[o+0x78..o+0x7a].try_into().unwrap());
    let count=m.bus.ewram[o+0x6e]as usize;
    let expected_phase=if fast{12}else{0};
    if trace_enabled(){eprintln!("FFTA_JP_QUEUE frame={} pc={:08x} multiplier={} root={:08x} phase={} expected_phase={} entries={}",m.bus.frames,m.cpu.regs[15],multiplier,m.cpu.regs[6],phase,expected_phase,count);}
    if phase!=expected_phase || count>13{return;}
    for i in 0..count {
        let slot=o+i*8;
        let wrapper=u32::from_le_bytes(m.bus.ewram[slot..slot+4].try_into().unwrap());
        let unit=wrapper.checked_sub(0x02000000).map(|p|p as usize)
            .and_then(|p|m.bus.ewram.get(p..p+4))
            .map(|p|u32::from_le_bytes(p.try_into().unwrap()));
        if unit.is_some_and(|p|(0x02000080..0x02001940).contains(&p) && (p-0x02000080)%0x108==0) {
            let before=m.bus.ewram[slot+4];
            m.bus.ewram[slot+4]=(u32::from(before)*u32::from(multiplier)).min(10)as u8;
            if trace_enabled(){eprintln!("FFTA_JP_SCALE frame={} root={:08x} entry={} unit={:08x?} factor={} before={} after={}",m.bus.frames,m.cpu.regs[6],i,unit,multiplier,before,m.bus.ewram[slot+4]);}
        }
    }
}
/// Read the live reward queue through the EUR task's literal pointer. Keeping
/// this derived from guest RAM also covers states saved midway through a popup.
/// No presentation metadata is written into guest RAM or serialized states.
pub fn jp_popup_digit(mem:&crate::MemMap)->Option<u8> {
    if !mem.ffta_rewards.verified{return None;}
    let h=u16::from_le_bytes(mem.rom.get(0xc018a..0xc018c)?.try_into().ok()?);
    if h&0xff00!=0x4800{return None;} // ldr r0, [pc, #imm]
    let literal=((0xc018ausize+4)&!3)+(h as usize&255)*4;
    let root=u32::from_le_bytes(mem.rom.get(literal..literal+4)?.try_into().ok()?);
    let root=root.checked_sub(0x02000000)? as usize;
    let q=mem.ewram.get(root..root.checked_add(0x80)?)?;
    let phase=u16::from_le_bytes([q[0x78],q[0x79]]);
    let index=match phase {6=>q[0x6d]as usize,4|7=>(q[0x6d]as usize).checked_sub(1)?,_=>return None};
    let count=q[0x6e]as usize;
    if count==0 || count>13 || index>=count{return None;}
    let entry=&q[index*8..index*8+8];let amount=entry[4];
    if !(2..=9).contains(&amount){return None;} // Original 1 and 10 stay intact.
    let wrapper=u32::from_le_bytes(entry[..4].try_into().ok()?).checked_sub(0x02000000)? as usize;
    let unit=u32::from_le_bytes(mem.ewram.get(wrapper..wrapper.checked_add(4)?)?.try_into().ok()?);
    if !(0x02000080..0x02001940).contains(&unit) || (unit-0x02000080)%0x108!=0{return None;}
    // The captured EUR named-effect allocation and resources identify 10 JP,
    // independently of screen position, animation age or widescreen ownership.
    let active=mem.ewram.get(0xfa3c..0xffbc)?.chunks_exact(0x2c).filter(|e|
        e[0x1d]==1 && e[0x1e]==27
        && e[..4]==0x08419c5cu32.to_le_bytes()
        && e[4..8]==0x08412310u32.to_le_bytes()).count();
    if active!=1{return None;}
    // The JP 1/0 tiles duplicate the numeric font exactly. Refuse substitution
    // if a different scene has reused this OBJ allocation.
    let tile=|id:usize|mem.vram.get(0x10000+id*32..0x10000+(id+1)*32);
    if tile(0x1ec)!=tile(0x1e1) || tile(0x1ed)!=tile(0x1e0){return None;}
    Some(amount)
}
/// Substitute only local render attributes; native OAM, animation and payouts
/// remain untouched. Hide the leading 1 and use the original 0's animated slot.
pub(crate) fn jp_popup_attrs(digit:Option<u8>,attrs:[u16;3])->Option<[u16;3]> {
    let Some(digit)=digit.filter(|n|(2..=9).contains(n)) else{return Some(attrs);};
    let [a,b,c]=attrs;
    if a&0xff00!=0 || b&0xfe00!=0{return Some(attrs);}
    match c {0x21ec=>None,0x21ed=>Some([a,b,0x21e0+u16::from(digit)]),_=>Some(attrs)}
}

#[cfg(test)]mod reward_multiplier_tests {
    use super::*;
    fn settings(gil:u8,ap:u8)->State{State{verified:true,gil,ap,exp:1,jp:1}}
    fn machine()->Machine {
        let mut rom=vec![0;0xbf500];
        rom[0x498f8..0x498fc].copy_from_slice(&[0x0a,0x48,0x00,0x68]);
        rom[0xaa806..0xaa80a].copy_from_slice(&[0x00,0x04,0x00,0x14]);
        rom[0xbf094..0xbf098].copy_from_slice(&[0x31,0x1c,0x78,0x31]);
        rom[0xbf3d8..0xbf3dc].copy_from_slice(&[0x00,0x24,0x30,0x1c]);
        let mut m=Machine::new(rom);m.cpu.cpsr=0x3f;m.cpu.regs[15]=0x080498f8;
        m.bus.ewram[GIL..GIL+4].copy_from_slice(&600u32.to_le_bytes());
        m.bus.ewram[AP..AP+2].copy_from_slice(&4u16.to_le_bytes());m
    }
    #[test]fn reward_multiplier_independent_settings_and_caps() {
        for (g,a,gil,ap) in [(1,1,600,4),(2,1,1200,4),(1,2,600,8),(2,2,1200,8),(3,3,1800,12),(5,5,3000,20),(10,10,6000,40)] {
            assert_eq!(scaled(600,4,settings(g,a)),(gil,ap));
        }
        assert_eq!(scaled(u32::MAX,u16::MAX,settings(1,1)),(u32::MAX,u16::MAX));
        assert_eq!(scaled(u32::MAX,u16::MAX,settings(10,10)),(MAX_GIL,MAX_AP_UNITS));
        assert_eq!(scaled(0,0,settings(10,10)),(0,0));
    }
    #[test]fn reward_multiplier_rejects_unverified_rom_and_unrelated_pc() {
        assert!(!State::configured(&[],2,2).interprets(0x080498f9));
        assert_eq!(factor(255),1);assert_eq!(factor(0),1);
        let mut m=machine();m.bus.ffta_rewards=settings(10,10);
        m.cpu.regs[15]=0x0804a178;let before=m.bus.ewram.clone();
        before_instruction(&mut m);assert_eq!(m.bus.ewram,before);
        m.cpu.regs[15]=0x080498f8;m.cpu.cpsr&=!0x20;
        before_instruction(&mut m);assert_eq!(m.bus.ewram,before);
    }
    #[test]fn reward_multiplier_defaults_do_not_change_pending_fields() {
        let mut m=machine();m.bus.ffta_rewards=State::default();
        let before=m.bus.ewram.clone();before_instruction(&mut m);assert_eq!(m.bus.ewram,before);
    }
    #[test]fn reward_multiplier_step_and_snapshot_resume_never_apply_twice() {
        let mut m=machine();m.bus.ffta_rewards=settings(2,2);
        let before=m.save_state();m.step();
        assert_eq!(m.cpu.regs[15],0x080498fa);
        let expected=m.bus.ewram.clone();assert_eq!(u32::from_le_bytes(expected[GIL..GIL+4].try_into().unwrap()),1200);
        assert_eq!(expected[AP],8);
        let after=m.save_state();m.step();assert_eq!(m.bus.ewram,expected);
        m.load_state(&after).unwrap();m.step();assert_eq!(m.bus.ewram,expected);
        m.load_state(&before).unwrap();m.step();assert_eq!(m.bus.ewram,expected);
    }
    #[test]fn reward_exp_keeps_zero_negative_awards_and_caps_a_single_action_at_100(){
        for (raw,factor,expected) in [(0,10,0),(6,2,12),(6,10,60),(99,10,100),(0xffff,10,0xffff)] {
            let mut m=machine();m.bus.ffta_rewards=State{exp:factor,..settings(1,1)};
            m.cpu.regs[15]=0x080aa806;m.cpu.regs[0]=raw;
            before_instruction(&mut m);assert_eq!(m.cpu.regs[0],expected);
        }
    }
    #[test]fn reward_exp_snapshot_resume_does_not_scale_a_second_time(){
        let mut m=machine();m.bus.ffta_rewards=State{exp:2,..settings(1,1)};
        m.cpu.regs[15]=0x080aa806;m.cpu.regs[0]=6;let before=m.save_state();
        m.step();let after=m.save_state();m.step();assert_eq!(m.cpu.regs[0],12);
        m.load_state(&after).unwrap();m.step();assert_eq!(m.cpu.regs[0],12);
        m.load_state(&before).unwrap();m.step();m.step();assert_eq!(m.cpu.regs[0],12);
    }
    fn jp_machine(fast:bool)->Machine{
        let mut m=machine();m.bus.ffta_rewards=State{jp:2,..settings(1,1)};
        m.cpu.regs[15]=if fast{0x080bf3d8}else{0x080bf094};m.cpu.regs[6]=0x02010000;
        m.bus.ewram[0x10000..0x10004].copy_from_slice(&0x02020000u32.to_le_bytes());
        m.bus.ewram[0x20000..0x20004].copy_from_slice(&0x02000398u32.to_le_bytes());
        m.bus.ewram[0x10004]=1;m.bus.ewram[0x1006e]=1;
        m.bus.ewram[0x10078..0x1007a].copy_from_slice(&(if fast{12u16}else{0}).to_le_bytes());m
    }
    #[test]fn reward_jp_scales_the_shared_display_amount_and_preserves_ko_tally(){
        for fast in [false,true] {
            let mut m=jp_machine(fast);m.bus.ewram[0x489]=3;
            let before=m.save_state();m.step();assert_eq!(m.bus.ewram[0x10004],2);
            assert_eq!(m.bus.ewram[0x489],3);let after=m.save_state();
            m.step();assert_eq!(m.bus.ewram[0x10004],2);
            m.load_state(&after).unwrap();m.step();assert_eq!(m.bus.ewram[0x10004],2);
            m.load_state(&before).unwrap();m.step();assert_eq!(m.bus.ewram[0x10004],2);
        }
    }
    #[test]fn reward_jp_does_not_scale_enemies_bad_groups_or_already_started_presentations(){
        let mut m=jp_machine(false);
        m.bus.ewram[0x20000..0x20004].copy_from_slice(&0x02002c10u32.to_le_bytes());
        before_instruction(&mut m);assert_eq!(m.bus.ewram[0x10004],1);
        m.bus.ewram[0x20000..0x20004].copy_from_slice(&0x02000398u32.to_le_bytes());
        m.bus.ewram[0x1006e]=14;before_instruction(&mut m);assert_eq!(m.bus.ewram[0x10004],1);
        m.bus.ewram[0x1006e]=1;m.bus.ewram[0x10078]=1;
        before_instruction(&mut m);assert_eq!(m.bus.ewram[0x10004],1);
        m.bus.ewram[0x10078]=0;m.bus.ewram[0x10004]=10;
        before_instruction(&mut m);assert_eq!(m.bus.ewram[0x10004],10);
    }
    fn popup_machine()->Machine {
        let mut m=jp_machine(false);m.bus.rom.resize(0xc0200,0);
        m.bus.rom[0xc018a..0xc018c].copy_from_slice(&0x4805u16.to_le_bytes());
        m.bus.rom[0xc01a0..0xc01a4].copy_from_slice(&0x02010000u32.to_le_bytes());
        m.bus.ewram[0x10004]=2;m.bus.ewram[0x10078]=6;
        let e=&mut m.bus.ewram[0xfa3c..0xfa68];
        e[..4].copy_from_slice(&0x08419c5cu32.to_le_bytes());
        e[4..8].copy_from_slice(&0x08412310u32.to_le_bytes());e[0x1d]=1;e[0x1e]=27;
        m
    }
    #[test]fn reward_jp_popup_reads_live_amount_and_survives_fresh_state_restore(){
        let mut m=popup_machine();
        for phase in [6,4,7] {m.bus.ewram[0x10078]=phase;m.bus.ewram[0x1006d]=if phase==6{0}else{1};
            for amount in [2,3,5,9] {m.bus.ewram[0x10004]=amount;
                let before=m.save_state();assert_eq!(jp_popup_digit(&m.bus),Some(amount));assert_eq!(before,m.save_state());
                let mut resumed=Machine::new(m.bus.rom.clone());resumed.load_state(&before).unwrap();resumed.bus.ffta_rewards=m.bus.ffta_rewards;
                assert_eq!(jp_popup_digit(&resumed.bus),Some(amount));
            }
        }
        for amount in [0,1,10,255] {m.bus.ewram[0x10004]=amount;assert_eq!(jp_popup_digit(&m.bus),None);}
        m.bus.ewram[0x10004]=2;m.bus.ewram[0x1006d]=0;assert_eq!(jp_popup_digit(&m.bus),None);
        m.bus.ewram[0x1006d]=1;m.bus.ewram[0xfa59]=0;assert_eq!(jp_popup_digit(&m.bus),None);
        m.bus.ewram[0xfa59]=1;m.bus.vram[0x10000+0x1ed*32]=1;assert_eq!(jp_popup_digit(&m.bus),None);
    }
    #[test]fn reward_jp_popup_changes_only_the_captured_native_number_sprites(){
        for amount in [2,3,5,9] {
            assert_eq!(jp_popup_attrs(Some(amount),[0x36,0x69,0x21ec]),None);
            assert_eq!(jp_popup_attrs(Some(amount),[0x36,0x71,0x21ed]),Some([0x36,0x71,0x21e0+amount as u16]));
            for attrs in [[0x36,0x7a,0x1ea],[0x36,0x81,0x1eb],[0x8067,0x1079,0x18c8],
                [0x36,0x71,0x11ed],[0x136,0x71,0x21ed],[0x36,0x4071,0x21ed]] {
                assert_eq!(jp_popup_attrs(Some(amount),attrs),Some(attrs));
            }
        }
        for amount in [None,Some(1),Some(10)] {for c in [0x21ec,0x21ed]{
            assert_eq!(jp_popup_attrs(amount,[0x36,0x71,c]),Some([0x36,0x71,c]));
        }}
    }

}
