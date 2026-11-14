//! In-game pause/settings overlay for the play runtime.
//!
//! This crate owns the overlay's state machine, embedded scalable
//! fonts, and rgba compositing â€” everything the play loop needs to draw a
//! settings panel over the last composed frame and steer it with the same
//! keyboard/gamepad poll it already runs. It deliberately knows nothing
//! about windows, audio rings, or the emulator: the play loop feeds it
//! edge-triggered input and the live [`AvConfig`], and it returns the
//! actions to take (resume, quit, a setting changed) and draws the panel.
//!
//! The settings vocabulary mirrors the launcher's A/V surface, parsed
//! through the `screen` crate's enums so the two frontends never drift.

mod canvas;
mod controller_icons;
mod menu_icons;
mod theme;
mod states;
mod auto_states;
mod gil;
mod gil_desk;
mod items;
mod rewards;
mod characters;
mod roster;
mod home;
pub use roster::RosterSprite;
mod cloning;
pub use cloning::CloneEditor;
mod levels;
pub use levels::{GrowthJob,LevelPlan,LevelEditor};
mod abilities;
pub use abilities::{AbilityInfo,AbilityJob,AbilityEditor};
pub use characters::{CharacterInfo,CharacterPreview};
pub use rewards::{RewardAction,RewardMenu};
pub use items::OwnedItem;
pub use gil::{GilAction,GilEditor,ItemEditor};
pub use states::{StateAction, StateCard, StateMenu, StateNoticeImage, state_notice_image};

use input_config::{AvConfig, Button, Device, GamepadLayout, InputConfig};
use screen::{ResponseMode, ScreenKind};
use std::ops::{Deref, DerefMut};

/// One edge-triggered navigation event. The play loop debounces its own
/// keyboard/gamepad poll into these (a held direction yields one event on
/// the press edge, never a stream), so the menu logic stays stateless
/// about input timing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuInput {
    Up,
    Down,
    Left,
    Right,
    /// Activate the selected row (Enter / South).
    Confirm,
    /// Back out / close the menu (Escape / East).
    Cancel,
}

/// What the play loop should do after feeding input to the menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    Rewards(RewardAction),
    GilEditor(GilAction),
    ManualState(StateAction),
    DebugVictory,
    /// Close the menu and continue the game.
    Resume,
    /// Close the game (the .sav is flushed by the play loop's teardown).
    Quit,
    /// Close only the game process so the launcher can select another ROM.
    SwitchRom,
    /// A persisted setting changed; the loop reapplies live state and
    /// writes `av.cfg`. The variant says which surface moved so the loop
    /// only rebuilds what it must.
    Changed(Changed),
    InputDevice(Device),
    ResetGamepad,
}

/// Which A/V surface a [`MenuAction::Changed`] touched, so the play loop
/// rebuilds only the affected objects.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Changed {
    /// `audio_enhanced` flipped.
    Audio,
    /// A video setting that rebuilds the color LUT / temporal / grid in
    /// place (screen model, darken, response, grid).
    Video,
    /// Persist-only / session-only change (fast-forward speed): nothing to
    /// rebuild live. The name is historical.
    Gamut,
    /// `video_vsync` flipped; the loop retunes the presenter in place.
    Vsync,
    WindowMode,
}

/// What a pending "press a key" capture will bind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bind {
    /// A GBA pad button.
    Button(Button),
    Gamepad(Button),
    /// The hold-to-fast-forward key.
    FastForward,
    ZoomIn,
    FastForwardToggle,
    ZoomOut,
    GamepadZoomIn,
    GamepadZoomOut,
    GamepadFastForward,
    GamepadFastForwardToggle,
    MenuKey,
    GamepadMenu,
}

/// Which page of the menu is showing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Page {
    Abilities,
    CloneMember,
    Levels,
    CharacterRename,CharacterRenameConfirm,
    Laws,
    ShopStock,
    Characters,
    Editor,
    RewardMultipliers,
    ItemEditor,
    ItemConfirm,
    GilEditor,
    GilConfirm,
    GilUndoConfirm,
    SaveStates,
    Debug,
    Main,
    Exit,
    SwitchRom,
    FastForward,
    Audio,
    Video,
    TextBubbles,
    Inputs,
    Gamepad,
    MouseKeyboard,
}

/// The rows of the menu. Action rows (`Resume`, `Quit`, the submenu
/// entries, `Back`, `Key`) have no value to cycle; setting rows cycle on
/// left/right/confirm. `Key(b)` starts a "press a key" capture.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Row {
    AbilitiesMenu,
    CloneMenu,
    LevelsMenu,
    RenameMenu,RenameCurrent,RenameValue,RenameApply,RenameYes,
    LawMenu,LawValue,
    ShopMenu,ShopValue,ShopReset,
    CharactersMenu,CharacterMember,CharacterRace,CharacterJob,CharacterLevel,CharacterExp,CharacterJp,
    GilMenu,
    RewardsMenu,
    ItemsMenu,
    ItemTotal,
    ItemEquipped,
    ItemValue,
    ItemApply,
    ItemConfirmYes,
    GilEditorMenu,
    GilCurrent,
    GilValue,
    GilStep,
    GilApply,
    GilUndo,
    GilConfirmYes,
    GilUndoYes,
    GilCancel,
    SaveStatesMenu,
    DebugEnabled,
    DebugVictory,
    DebugCharacters,
    DebugMission,
    DebugLaunch,
    DebugStatus,
    MouseMode,
    Resume,
    FastForwardMenu,
    FastForward,
    AudioMenu,
    FastForwardAudio,
    VideoMenu,
    TextBubblesMenu,
    BubbleSize,
    BubbleLength,
    InputsMenu,
    SwitchRomMenu,
    ResetGamepad,
    Quit,
    QuitYes,
    QuitNo,
    ReturnToGame,
    ChangeRom,
    Volume,
    Audio,
    Screen,
    Brightness,
    Darken,
    Response,
    Grid,
    Scaling,
    Aspect,
    Vsync,
    Key(Button),
    FfKey,
    FfToggleKey,
    ZoomIn,
    ZoomOut,
    MenuKey,
    WindowMode,
    Back,
}

const MAIN_ROWS: [Row; 10] = [
    Row::Resume,
    Row::SaveStatesMenu,
    Row::GilEditorMenu,
    Row::FastForwardMenu,
    Row::TextBubblesMenu,
    Row::AudioMenu,
    Row::VideoMenu,
    Row::InputsMenu,
    Row::SwitchRomMenu,
    Row::Quit,
];

const AUDIO_ROWS: [Row; 3] = [Row::Volume, Row::Audio, Row::Back];
const EXIT_ROWS: [Row; 2] = [Row::QuitYes, Row::QuitNo];
const SWITCH_ROM_ROWS: [Row; 2] = [Row::ReturnToGame, Row::ChangeRom];
const FAST_FORWARD_ROWS: [Row; 3] = [Row::FastForward, Row::FastForwardAudio, Row::Back];

/// Scaling modes offered by the menu (`av.scaling`): "off" keeps the
/// original exact area-average scaler.
const SCALING_MODES: [&str; 4] = ["off", "nearest", "sharp", "epx"];

/// Aspect ratios offered by the menu (`av.video_aspect`). Applied on resume.
const ASPECT_MODES: [&str; 7] = ["auto", "3:2", "16:10", "16:9", "20:9", "21:9", "32:9"];

const VIDEO_ROWS: [Row; 10] = [
    Row::WindowMode,
    Row::Aspect,
    Row::Screen,
    Row::Brightness,
    Row::Darken,
    Row::Response,
    Row::Grid,
    Row::Scaling,
    Row::Vsync,
    Row::Back,
];

const INPUT_ROWS: [Row; 16] = [
    Row::Key(Button::A),
    Row::Key(Button::B),
    Row::Key(Button::L),
    Row::Key(Button::R),
    Row::Key(Button::Start),
    Row::Key(Button::Select),
    Row::Key(Button::Up),
    Row::Key(Button::Down),
    Row::Key(Button::Left),
    Row::Key(Button::Right),
    Row::FfKey,
    Row::FfToggleKey,
    Row::ZoomIn,
    Row::ZoomOut,
    Row::MenuKey,
    Row::Back,
];
const MOUSE_ROWS: [Row; 17] = [
    Row::MouseMode,
    Row::Key(Button::A),
    Row::Key(Button::B),
    Row::Key(Button::L),
    Row::Key(Button::R),
    Row::Key(Button::Start),
    Row::Key(Button::Select),
    Row::Key(Button::Up),
    Row::Key(Button::Down),
    Row::Key(Button::Left),
    Row::Key(Button::Right),
    Row::FfKey,
    Row::FfToggleKey,
    Row::ZoomIn,
    Row::ZoomOut,
    Row::MenuKey,
    Row::Back,
];
const GAMEPAD_ROWS: [Row; 17] = [
    Row::Key(Button::A),
    Row::Key(Button::B),
    Row::Key(Button::L),
    Row::Key(Button::R),
    Row::Key(Button::Start),
    Row::Key(Button::Select),
    Row::Key(Button::Up),
    Row::Key(Button::Down),
    Row::Key(Button::Left),
    Row::Key(Button::Right),
    Row::FfKey,
    Row::FfToggleKey,
    Row::ZoomIn,
    Row::ZoomOut,
    Row::MenuKey,
    Row::ResetGamepad,
    Row::Back,
];

fn rows_of(page: Page) -> &'static [Row] {
    match page {
        Page::Debug => &[
            Row::DebugEnabled,
            Row::DebugVictory,
            Row::DebugCharacters,
            Row::DebugMission,
            Row::DebugLaunch,

            Row::DebugStatus,
            Row::Resume,
        ],

        Page::RewardMultipliers => &[Row::Back],
        Page::Editor => &[Row::GilMenu,Row::RewardsMenu,Row::ItemsMenu,Row::CharactersMenu,Row::ShopMenu,Row::LawMenu,Row::Back],
        Page::Laws=>&[Row::LawValue,Row::Back],
        Page::ShopStock=>&[Row::ShopValue,Row::ShopReset,Row::Back],
        Page::Characters => &[Row::CharacterMember,Row::CharacterRace,Row::CharacterJob,Row::CharacterLevel,Row::CharacterExp,Row::CharacterJp,Row::RenameMenu,Row::AbilitiesMenu,Row::LevelsMenu,Row::CloneMenu,Row::Back],
        Page::Abilities=>&[Row::Back],
        Page::CloneMember=>&[Row::Back],
        Page::Levels=>&[Row::Back],
        Page::CharacterRename => &[Row::RenameCurrent,Row::RenameValue,Row::RenameApply,Row::GilUndo,Row::Back],
        Page::CharacterRenameConfirm => &[Row::GilCancel,Row::RenameYes],
        Page::GilEditor => &[Row::GilCurrent,Row::GilValue,Row::GilStep,Row::GilApply,Row::GilUndo,Row::Back],
        Page::ItemEditor => &[Row::ItemTotal,Row::ItemEquipped,Row::ItemValue,Row::ItemApply,Row::GilUndo,Row::Back],
        Page::ItemConfirm => &[Row::GilCancel,Row::ItemConfirmYes],
        Page::GilConfirm => &[Row::GilCancel,Row::GilConfirmYes],
        Page::GilUndoConfirm => &[Row::GilCancel,Row::GilUndoYes],
        Page::SaveStates => &[Row::Back],
        Page::Main => &MAIN_ROWS,
        Page::Exit => &EXIT_ROWS,
        Page::SwitchRom => &SWITCH_ROM_ROWS,
        Page::FastForward => &FAST_FORWARD_ROWS,
        Page::Audio => &AUDIO_ROWS,
        Page::Video => &VIDEO_ROWS,
        Page::TextBubbles => &[Row::BubbleSize, Row::BubbleLength, Row::Back],
        Page::Inputs => &INPUT_ROWS,
        Page::MouseKeyboard => &MOUSE_ROWS,
        Page::Gamepad => &GAMEPAD_ROWS,
    }
}

/// Fast-forward multipliers offered by the menu.
const FF_SPEEDS: [f32; 4] = [1.5, 2.0, 3.0, 4.0];

impl Row {
    fn label(self) -> &'static str {
        match self {

            Row::DebugEnabled => "Debug tools",
            Row::DebugVictory => "Win current battle",
            Row::DebugCharacters => "Unlock characters",
            Row::DebugMission => "Mission ID",
            Row::DebugLaunch => "Load selected mission",
            Row::DebugStatus => "Status",
            Row::MouseMode => "Mouse & Keyboard",
            Row::Resume => "Resume",
            Row::SaveStatesMenu => "Save States",
            Row::RewardsMenu => "Reward Multipliers",
            Row::ItemsMenu => "Items",
            Row::ItemTotal => "Total owned",
            Row::ItemEquipped => "Equipped",
            Row::ItemValue => "New total",
            Row::ItemApply => "Apply quantity",
            Row::ItemConfirmYes => "Yes, apply quantity",
            Row::GilEditorMenu => "Editor",
            Row::AbilitiesMenu=>"Jobs & Abilities",
            Row::CloneMenu=>"Clone member",
            Row::LevelsMenu=>"Level & Stats",
            Row::RenameMenu=>"Rename",Row::RenameCurrent=>"Current name",Row::RenameValue=>"New name",Row::RenameApply=>"Apply change",Row::RenameYes=>"Yes, rename member",
            Row::LawMenu=>"Laws",Row::LawValue=>"Law penalties",
            Row::ShopMenu=>"Shop stock",Row::ShopValue=>"Stock tier",Row::ShopReset=>"Restore game default",
            Row::CharactersMenu=>"Characters",
            Row::CharacterMember=>"Clan member",
            Row::CharacterRace=>"Race",
            Row::CharacterJob=>"Job",
            Row::CharacterLevel=>"Level",
            Row::CharacterExp=>"EXP",
            Row::CharacterJp=>"JP",
            Row::GilMenu => "Gil",
            Row::GilCurrent => "Current Gil",
            Row::GilValue => "New Gil",
            Row::GilStep => "Change by",
            Row::GilApply => "Apply change",
            Row::GilUndo => "Undo last edit",
            Row::GilConfirmYes => "Yes, apply change",
            Row::GilUndoYes => "Yes, restore previous state",
            Row::GilCancel => "No, go back",
            Row::FastForwardMenu => "Fast forward",
            Row::FastForward => "Multiplier",
            Row::FastForwardAudio => "Audio mode",
            Row::AudioMenu => "Audio",
            Row::VideoMenu => "Video",
            Row::TextBubblesMenu => "Text Bubbles",
            Row::BubbleSize => "Size",
            Row::BubbleLength => "Length",
            Row::InputsMenu => "Input",
            Row::SwitchRomMenu => "Switch ROM",
            Row::ResetGamepad => "Default layout",
            Row::Quit => "Exit",
            Row::QuitYes => "Yes, exit",
            Row::QuitNo => "No, go back",
            Row::ReturnToGame => "Return to game",
            Row::ChangeRom => "Choose ROM...",
            Row::Volume => "Volume",
            Row::Audio => "Enhanced Audio",
            Row::Screen => "Screen",
            Row::Brightness => "Brightness",
            Row::Darken => "Darken",
            Row::Response => "Response",
            Row::Grid => "Grid",
            Row::Scaling => "Scaling",
            Row::Aspect => "Aspect",
            Row::Vsync => "Vsync",
            Row::Key(Button::A) => "A",
            Row::Key(Button::B) => "B",
            Row::Key(Button::L) => "L",
            Row::Key(Button::R) => "R",
            Row::Key(Button::Start) => "Start",
            Row::Key(Button::Select) => "Select",
            Row::Key(Button::Up) => "Up",
            Row::Key(Button::Down) => "Down",
            Row::Key(Button::Left) => "Left",
            Row::Key(Button::Right) => "Right",
            Row::FfKey => "Fast Fwd",
            Row::FfToggleKey => "Toggle Fast Fwd",
            Row::ZoomIn => "Zoom In",
            Row::ZoomOut => "Zoom Out",
            Row::MenuKey => "Mod Menu",
            Row::WindowMode => "Window mode",
            Row::Back => "Back",
        }
    }

    /// Is this an action row (no value to cycle) rather than a setting row?
    fn is_action(self) -> bool {
        matches!(
            self,
            Row::CloneMenu|Row::LevelsMenu|Row::AbilitiesMenu|Row::RenameMenu|Row::RenameCurrent|Row::RenameValue|Row::RenameApply|Row::RenameYes|Row::LawMenu|Row::LawValue|Row::ShopMenu|Row::ShopValue|Row::ShopReset|Row::CharactersMenu|Row::CharacterMember|Row::CharacterRace|Row::CharacterJob|Row::CharacterLevel|Row::CharacterExp|Row::CharacterJp|Row::RewardsMenu|Row::ItemsMenu|Row::ItemTotal|Row::ItemEquipped|Row::ItemApply|Row::ItemConfirmYes
                | Row::GilMenu | Row::GilEditorMenu | Row::GilCurrent | Row::GilApply | Row::GilUndo
                | Row::GilConfirmYes | Row::GilUndoYes | Row::GilCancel
                | Row::DebugEnabled
                | Row::DebugVictory
                | Row::DebugCharacters
                | Row::DebugLaunch
                | Row::DebugStatus
                | Row::SaveStatesMenu
                | Row::Resume
                | Row::Quit
                | Row::QuitYes
                | Row::QuitNo
                | Row::ReturnToGame
                | Row::ChangeRom
                | Row::FastForwardMenu
                | Row::AudioMenu
                | Row::TextBubblesMenu
                | Row::VideoMenu
                | Row::InputsMenu
                | Row::SwitchRomMenu
                | Row::ResetGamepad
                | Row::Key(_)
                | Row::FfKey
                | Row::FfToggleKey
                | Row::ZoomIn
                | Row::ZoomOut
                | Row::MenuKey
                | Row::Back
        )
    }

    /// The submenu this row opens, if any.
    fn opens(self) -> Option<Page> {
        match self {

            Row::Quit => Some(Page::Exit),
            Row::SwitchRomMenu => Some(Page::SwitchRom),
            Row::FastForwardMenu => Some(Page::FastForward),
            Row::AudioMenu => Some(Page::Audio),
            Row::VideoMenu => Some(Page::Video),
            Row::TextBubblesMenu => Some(Page::TextBubbles),
            Row::InputsMenu => Some(Page::Inputs),
            _ => None,
        }
    }
}

/// Panel geometry in frame pixels, shared by drawing and mouse hit-testing
/// so the two can never disagree.
#[derive(Clone, Copy)]
struct Layout {
    panel_x: usize,
    panel_y: usize,
    panel_w: usize,
    panel_h: usize,
    pad: usize,
    line_h: usize,
    /// Text-top y of the first row (its highlight bar starts 2 px above).
    rows_y: usize,
    visible_rows: usize,
}

/// Responsive panel geometry in UI pixels, independent of the game resolution.
fn layout(n: usize, w: usize, h: usize) -> Layout {
    layout_for_rows(n,w,h,n==MAIN_ROWS.len(),false)
}
fn page_layout(page: Page,w: usize,h: usize) -> Layout {
    layout_for_rows(rows_of(page).len(),w,h,page==Page::Main,matches!(page,Page::GilEditor|Page::ItemEditor|Page::Characters|Page::CharacterRename|Page::ShopStock|Page::Laws))
}
fn layout_for_rows(n: usize, w: usize, h: usize,main: bool,editor: bool) -> Layout {
    let inputs = n == INPUT_ROWS.len() || n == GAMEPAD_ROWS.len();
    let scale = (h as f32 / 480.).min(w as f32 / 560.).max(0.28);
    let pad = (20. * scale).round() as usize;
    let panel_w = w
        .saturating_sub(pad * 2)
        .min(((if main { 320. } else { 760. }) * scale) as usize);
    let panel_x = w.saturating_sub(panel_w) / 2;
    let avail = h
        .saturating_sub(pad * 2)
        .min(if inputs { (460. * scale) as usize } else { h });
    let line_h = ((if main {40.} else {44.}) * scale).round().max(12.) as usize;
    let header = if main {
        0
    } else if editor {
        (100. * scale) as usize
    } else if inputs {
        (82. * scale) as usize
    } else if n == 2 {
        (90. * scale) as usize
    } else {
        (50. * scale) as usize
    };
    let footer = if inputs { (50. * scale) as usize } else { 0 };
    let visible_rows = ((avail.saturating_sub(pad * 2 + header + footer)) / line_h)
        .max(1)
        .min(if inputs { n - 1 } else { n });
    let panel_h = (pad * 2 + header + visible_rows * line_h + footer).min(avail);
    let panel_y = h.saturating_sub(panel_h) / 2;
    let rows_y = panel_y + pad + header;
    Layout {
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        pad,
        line_h,
        rows_y,
        visible_rows,
    }
}

/// One navigation point saved on the submenu stack.
#[derive(Clone, Copy)]
struct PageFrame {
    page: Page,
    selected: usize,
}

/// Navigation model. Page history and the selected row live together so
/// opening, switching tabs, and returning cannot leave a stale cursor behind.
#[doc(hidden)]
pub struct Navigation {
    page: Page,
    selected: usize,
    parents: Vec<PageFrame>,
}

impl Navigation {
    fn new() -> Self {
        Self {
            page: Page::Main,
            selected: 0,
            parents: Vec::new(),
        }
    }

    fn open(&mut self, page: Page) {
        if self.page != page {
            self.parents.push(PageFrame {
                page: self.page,
                selected: self.selected,
            });
        }
        self.page = page;
        self.selected = if page == Page::Exit { 1 } else { 0 };
    }

    fn switch_input_tab(&mut self, page: Page) {
        debug_assert!(matches!(
            page,
            Page::Inputs | Page::Gamepad | Page::MouseKeyboard
        ));
        if matches!(
            self.page,
            Page::Inputs | Page::Gamepad | Page::MouseKeyboard
        ) {
            self.page = page;
        } else {
            self.open(page);
        }
    }

    fn back_to_main(&mut self) {
        while let Some(parent) = self.parents.pop() {
            self.page = parent.page;
            self.selected = parent.selected;
            if self.page == Page::Main {
                return;
            }
        }
        self.page = Page::Main;
        self.selected = 0;
    }

    fn reset(&mut self) {
        self.page = Page::Main;
        self.selected = 0;
        self.parents.clear();
    }
}

/// The pause-overlay model and its public input/draw boundary.
pub struct Menu {
    wheel_mode:bool,
    wheel_pointer:Option<(usize,usize)>,
    last_pointer:Option<(usize,usize)>,
    pub abilities:AbilityEditor,
    pub cloning:CloneEditor,
    pub levels:LevelEditor,
    pub characters:CharacterPreview,
    pub law_mode:u8,pub law_status:String,
    pub shop_tier:u8,
    pub shop_status:String,
    pub rewards: RewardMenu,
    pub gil: GilEditor,
    pub items: ItemEditor,
    pub states: StateMenu,
    debug_tools: bool,
    debug_mission: u16,
    debug_status: String,
    navigation: Navigation,
    /// Whether the enhanced-audio toggle applies live or on next launch.
    /// The play loop reports this once at construction; the menu only
    /// surfaces it as a footnote on that row.
    audio_live: bool,
    /// `Some(button)` while waiting for the player to press a key for it.
    /// The play loop owns the keyboard: it reads [`Menu::capturing`], and
    /// calls [`Menu::end_capture`] once it has taken (or cancelled) the key.
    capture: Option<Bind>,
    scroll: std::cell::Cell<usize>,
    dropdown: Option<Dropdown>,
    input_gamepad: std::cell::Cell<u8>,
    mouse_mode: std::cell::Cell<bool>,
    menu_binding: std::cell::RefCell<String>,
    controller_layout: std::cell::Cell<GamepadLayout>,
}

// Keep field access concise inside the renderer/tests while making
// Navigation the sole owner of page/cursor state.
impl Deref for Menu {
    type Target = Navigation;
    fn deref(&self) -> &Self::Target {
        &self.navigation
    }
}

impl DerefMut for Menu {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.navigation
    }
}
struct Dropdown {
    row: Row,
    index: usize,
    first: std::cell::Cell<usize>,
}

impl Menu {
    /// Open a fresh menu with the first row selected. `audio_live` tells
    /// the menu whether enhanced audio takes effect immediately. The second
    /// argument is kept for source compatibility (the gamut row now lives
    /// in the launcher) and is ignored.
    pub fn new(audio_live: bool, _gamut_live: bool) -> Menu {
        Menu {
            states: StateMenu::default(),
            gil: GilEditor::default(),
            rewards: RewardMenu::default(),
            characters:CharacterPreview::default(),
            shop_tier:0,shop_status:String::new(),
            law_mode:0,law_status:String::new(),
            abilities:AbilityEditor::default(),
            cloning:CloneEditor::default(),
            levels:LevelEditor::default(),
            wheel_mode:false,wheel_pointer:None,last_pointer:None,
            items: ItemEditor::default(),
            debug_tools: true,
            debug_mission: 1,
            debug_status: "Experimental".into(),
            navigation: Navigation::new(),
            audio_live,
            capture: None,
            scroll: std::cell::Cell::new(0),
            dropdown: None,
            input_gamepad: std::cell::Cell::new(0),
            mouse_mode: std::cell::Cell::new(false),
            menu_binding: std::cell::RefCell::new("Escape".into()),
            controller_layout: std::cell::Cell::new(GamepadLayout::Xbox),
        }
    }

    /// Reset to the top of the main page (called on each open so the cursor
    /// doesn't persist a stale position across pauses).
    pub fn reset(&mut self) {
        self.navigation.reset();
        self.wheel_mode=false;self.wheel_pointer=None;self.last_pointer=None;
        self.states.clear_confirmation();
        self.capture = None;
        self.scroll.set(0);
        self.dropdown = None;
    }

    /// Developer entry point; the runtime checks the ROM and opt-in first.
    fn close_gil_confirmation(&mut self) {
        self.scroll.set(0);
        self.dropdown=None;
        if let Some(parent)=self.navigation.parents.pop() {
            self.navigation.page=parent.page;
            self.navigation.selected=parent.selected;
        } else {
            self.navigation.page=Page::GilEditor;
            self.navigation.selected=3;
        }
    }
    fn reward_event(&mut self,event:rewards::Event)->Option<MenuAction>{
        match event {rewards::Event::None=>None,rewards::Event::Back=>{self.close_gil_confirmation();None},
            rewards::Event::Changed=>Some(MenuAction::Rewards(RewardAction::Changed))}
    }
    pub fn clone_page_active(&self)->bool{self.page==Page::CloneMember}
    fn clone_event(&mut self,event:cloning::Event)->Option<MenuAction>{match event{
        cloning::Event::None=>None,cloning::Event::Back=>{self.cloning.finish_text();self.close_gil_confirmation();None},
        cloning::Event::Action(action)=>Some(MenuAction::GilEditor(action)),
    }}
    pub fn level_page_active(&self)->bool{self.page==Page::Levels}
    pub fn stat_repeat_target(&self,x:usize,y:usize,w:usize,h:usize)->Option<(usize,bool)>{
        if self.level_page_active(){self.levels.repeat_target(x,y,w,h)}else{gil_desk::repeat_target(self,x,y,w,h)}
    }
    fn level_event(&mut self,event:levels::Event)->Option<MenuAction>{match event{
        levels::Event::None=>None,levels::Event::Back=>{self.close_gil_confirmation();None},
        levels::Event::Action(action)=>Some(MenuAction::GilEditor(action)),
    }}
    pub fn ability_page_active(&self)->bool{self.page==Page::Abilities}
    fn ability_event(&mut self,event:abilities::Event)->Option<MenuAction>{match event{
        abilities::Event::None=>None,abilities::Event::Back=>{self.close_gil_confirmation();None},
        abilities::Event::Action(action)=>Some(MenuAction::GilEditor(action)),
    }}
    pub fn item_page_active(&self)->bool {self.page==Page::ItemEditor}
    pub fn item_search_active(&self)->bool {if self.clone_page_active(){return self.cloning.editing;}(self.item_page_active()&&self.items.search_active()) || (self.page==Page::CharacterRename && self.characters.editing)}
    pub fn finish_item_search(&mut self){if self.clone_page_active(){self.cloning.finish_text();return;}if self.page==Page::CharacterRename{self.characters.editing=false;}else{self.items.finish_search();}}
    pub fn item_search_text(&mut self,text:&str,backspace:bool){if self.clone_page_active(){self.cloning.text(text,backspace);return;}if self.page==Page::CharacterRename{self.characters.text(text,backspace);}else{self.items.search_text(text,backspace);}}
    fn item_event(&mut self,event:items::Event)->Option<MenuAction>{
        match event {items::Event::None=>None,items::Event::Back=>{self.close_gil_confirmation();None},
            items::Event::Action(action)=>Some(MenuAction::GilEditor(action))}
    }
    pub fn item_result(&mut self, status: String) {self.items.result(status);}
    pub fn rename_result(&mut self,status:String){if self.page==Page::CharacterRenameConfirm{self.close_gil_confirmation();}self.characters.rename_status=status;}
    pub fn gil_result(&mut self, status: String) {
        if matches!(self.page,Page::GilConfirm|Page::GilUndoConfirm|Page::ItemConfirm) {self.close_gil_confirmation();}
        self.gil.status=status;
    }
    pub fn open_states(&mut self, slot: usize) {
        self.reset();
        self.open(Page::SaveStates);
        self.states.slot = slot.clamp(1,30);
        self.states.open();
    }
    pub fn confirm_state_save(&mut self, slot: usize) {
        self.open_states(slot);
        self.states.handle(MenuInput::Down);
        self.states.handle(MenuInput::Confirm);
    }
    pub fn open_debug(&mut self) {
        self.reset();
        self.open(Page::Debug);
    }
    pub fn debug_status(&mut self, status: &str) {
        self.debug_status = status.into();
    }

    /// The button currently waiting for a key press, if any.
    pub fn capturing(&self) -> Option<Bind> {
        self.capture
    }

    /// Stop waiting for a key (bound or cancelled).
    pub fn end_capture(&mut self) {
        self.capture = None;
    }

    fn open(&mut self, page: Page) {
        self.navigation.open(page);
        if page==Page::GilEditor{self.selected=1;}
        self.scroll.set(0);
        self.dropdown = None;
    }

    fn back_to_main(&mut self) {
        self.scroll.set(0);
        self.dropdown = None;
        self.navigation.back_to_main();
    }
    fn first_visible(&self, l: Layout) -> usize {
        let len = rows_of(self.page).len()
            - usize::from(matches!(
                self.page,
                Page::Inputs | Page::Gamepad | Page::MouseKeyboard
            ));
        let mut first = self.scroll.get().min(len.saturating_sub(l.visible_rows));
        if self.selected < len {
            if self.selected < first {
                first = self.selected;
            }
            if self.selected >= first + l.visible_rows {
                first = self.selected + 1 - l.visible_rows;
            }
        }
        self.scroll.set(first);
        first
    }
    fn input_tab(&mut self, tab: u8) -> Option<MenuAction> {
        if tab == 0 && self.mouse_mode.get() {
            return None;
        }
        self.input_gamepad.set(tab);
        self.navigation.switch_input_tab(match tab {
            1 => Page::Gamepad,
            2 => Page::MouseKeyboard,
            _ => Page::Inputs,
        });
        self.scroll.set(0);
        self.dropdown = None;
        if tab == 2 && !self.mouse_mode.get() {
            return None;
        }
        Some(MenuAction::InputDevice(match tab {
            1 => Device::Gamepad,
            2 => Device::MouseKeyboard,
            _ => Device::Keyboard,
        }))
    }
    fn toggle_mouse_mode(&mut self) -> Option<MenuAction> {
        let enabled = !self.mouse_mode.get();
        self.mouse_mode.set(enabled);
        self.input_gamepad.set(if enabled { 2 } else { 0 });
        Some(MenuAction::InputDevice(if enabled {
            Device::MouseKeyboard
        } else {
            Device::Keyboard
        }))
    }
    fn open_dropdown(&mut self, row: Row, av: &AvConfig) {
        let opts = setting_options(row);
        if opts.is_empty() {
            return;
        }
        let index = if row == Row::DebugMission {
            self.debug_mission as usize - 1
        } else {
            opts.iter()
                .position(|s| *s == setting_value(row, av))
                .unwrap_or(0)
        };
        self.dropdown = Some(Dropdown {
            row,
            index,
            first: std::cell::Cell::new(0),
        });
    }

    /// Feed one navigation event. Mutates `av` for value changes and
    /// returns the action the play loop must take, if any. `Up`/`Down`
    /// move the cursor (no action). `Left`/`Right` cycle the selected
    /// setting. `Confirm` activates an action row (and does nothing on a
    /// value setting).
    /// `Cancel` goes back from a submenu, or resumes from the main page.
    /// While a key capture is pending every event is ignored â€” the play
    /// loop decides what ends it.
    fn state_event(&mut self, event: states::Event) -> Option<MenuAction> {
        match event {
            states::Event::None => None,
            states::Event::Back => { self.back_to_main(); None }
            states::Event::Action(a) => Some(MenuAction::ManualState(a)),
        }
    }
    pub fn handle(&mut self, input: MenuInput, av: &mut AvConfig) -> Option<MenuAction> {
        if self.page==Page::Main&&matches!(input,MenuInput::Left|MenuInput::Right){self.selected=(self.selected+5)%MAIN_ROWS.len();return None;}
        if self.page==Page::Characters{
            if self.selected==0{
                match input{
                    MenuInput::Up|MenuInput::Down=>{self.characters.cycle(input==MenuInput::Down);return None;},
                    MenuInput::Left=>{self.characters.cycle(false);return None;},
                    MenuInput::Right|MenuInput::Confirm=>{self.selected=rows_of(Page::Characters).iter().position(|r|*r==Row::LevelsMenu).unwrap();return None;},
                    MenuInput::Cancel=>{}
                }
            }else{
                match input{
                    MenuInput::Left=>{self.selected=0;return None;},
                    MenuInput::Up|MenuInput::Down=>{let current=rows_of(Page::Characters).get(self.selected);let at=roster::ACTIONS.iter().position(|r|Some(r)==current).unwrap_or(0);let next=if input==MenuInput::Down{(at+1)%5}else{(at+4)%5};self.selected=rows_of(Page::Characters).iter().position(|r|*r==roster::ACTIONS[next]).unwrap();return None;},
                    MenuInput::Right=>return None,_=>{}
                }
            }
        }

        if self.page==Page::RewardMultipliers{let event=self.rewards.handle(input);return self.reward_event(event);}
        if self.clone_page_active(){let event=self.cloning.handle(input);return self.clone_event(event);}
        if self.level_page_active(){let event=self.levels.handle(input);return self.level_event(event);}
        if self.ability_page_active(){let event=self.abilities.handle(input);return self.ability_event(event);}
        if self.item_page_active(){let event=self.items.handle(input);return self.item_event(event);}

        if self.page == Page::SaveStates {
            let event = self.states.handle(input);
            return self.state_event(event);
        }


        if self.capture.is_some() {
            return None;
        }
        if let Some(mut dd) = self.dropdown.take() {
            let opts = setting_options(dd.row);
            match input {
                MenuInput::Cancel => return None,
                MenuInput::Confirm => {
                    if dd.row == Row::DebugMission {
                        self.debug_mission = dd.index as u16 + 1;
                        return None;
                    }
                    return choose_setting(dd.row, &opts[dd.index], av).map(MenuAction::Changed);
                }
                MenuInput::Up | MenuInput::Left => {
                    dd.index = (dd.index + opts.len() - 1) % opts.len()
                }
                MenuInput::Down | MenuInput::Right => dd.index = (dd.index + 1) % opts.len(),
            }
            self.dropdown = Some(dd);
            return None;
        }
        if matches!(
            self.page,
            Page::Inputs | Page::Gamepad | Page::MouseKeyboard
        ) && matches!(input, MenuInput::Left | MenuInput::Right)
        {
            let tab = match self.page {
                Page::Gamepad => 1,
                Page::MouseKeyboard => 2,
                _ => 0,
            };
            if self.page == Page::MouseKeyboard
                && rows_of(self.page)[self.selected] == Row::MouseMode
            {
                return self.toggle_mouse_mode();
            }
            let mut next = if input == MenuInput::Right {
                (tab + 1) % 3
            } else {
                (tab + 2) % 3
            };
            if next == 0 && self.mouse_mode.get() {
                next = if input == MenuInput::Right { 1 } else { 2 };
            }
            return self.input_tab(next);
        }
        if matches!(self.page,Page::GilConfirm|Page::GilUndoConfirm|Page::ItemConfirm|Page::CharacterRenameConfirm) && input==MenuInput::Cancel {
            self.close_gil_confirmation();
            return None;
        }
        let rows = rows_of(self.page);
        let row = rows[self.selected];
        if self.page==Page::CharacterRename {
            if input==MenuInput::Cancel || (row==Row::Back && input==MenuInput::Confirm){self.characters.editing=false;self.close_gil_confirmation();return None;}
            if input==MenuInput::Confirm {match row{
                Row::RenameValue=>{self.characters.editing=true;self.characters.replace_text=true;return None;},
                Row::RenameApply=>{if self.characters.expected.as_ref().is_some_and(|r|r.name!=self.characters.proposed) && !self.characters.proposed.trim().is_empty(){self.open(Page::CharacterRenameConfirm);}return None;},
                Row::GilUndo=>{if self.gil.undo_available{self.open(Page::GilUndoConfirm);}return None;},
                Row::RenameCurrent=>return None,_=>{}}
            }
        }
        if self.page==Page::Laws {
            if input==MenuInput::Cancel||(row==Row::Back&&input==MenuInput::Confirm){self.close_gil_confirmation();return None;}
            if row==Row::LawValue&&matches!(input,MenuInput::Left|MenuInput::Right|MenuInput::Confirm){self.law_mode=1-self.law_mode.min(1);return Some(MenuAction::GilEditor(GilAction::LawChanged));}
        }
        if self.page==Page::ShopStock {
            if input==MenuInput::Cancel||(row==Row::Back&&input==MenuInput::Confirm){self.close_gil_confirmation();return None;}
            if row==Row::ShopValue&&matches!(input,MenuInput::Left|MenuInput::Right|MenuInput::Confirm){
                self.shop_tier=if input==MenuInput::Left{(self.shop_tier+3)%4}else{(self.shop_tier+1)%4};
                return Some(MenuAction::GilEditor(GilAction::ShopChanged));
            }
            if row==Row::ShopReset&&input==MenuInput::Confirm{self.shop_tier=0;return Some(MenuAction::GilEditor(GilAction::ShopChanged));}
        }
        if self.page==Page::Characters {
            if input==MenuInput::Cancel || (row==Row::Back && input==MenuInput::Confirm){self.close_gil_confirmation();return None;}
            if row==Row::CharacterMember && matches!(input,MenuInput::Left|MenuInput::Right|MenuInput::Confirm){self.characters.cycle(input!=MenuInput::Left);return None;}
            if row!=Row::Back && row!=Row::RenameMenu && row!=Row::AbilitiesMenu && row!=Row::LevelsMenu && row!=Row::CloneMenu && input==MenuInput::Confirm{return None;}
        }
        if self.page==Page::GilEditor && (input==MenuInput::Cancel || (input==MenuInput::Confirm && row==Row::Back)) {
            self.close_gil_confirmation();return None;
        }
        if self.page == Page::GilEditor {
            match (row,input) {
                (Row::GilValue,MenuInput::Left|MenuInput::Right)=>{self.gil.adjust(input==MenuInput::Right);return None;}
                (Row::GilStep,MenuInput::Left|MenuInput::Right|MenuInput::Confirm)=>{self.gil.cycle_step(input!=MenuInput::Left);return None;}
                (Row::GilCurrent,MenuInput::Confirm)=>{self.gil.reset_preview();return None;},
                (Row::GilValue,MenuInput::Confirm)=>return None,
                (Row::GilApply,MenuInput::Confirm)=>{
                    if self.gil.enabled && self.gil.proposed!=self.gil.current {self.open(Page::GilConfirm);}
                    else if self.gil.enabled {self.gil.status="No change to apply.".into();}
                    return None;
                }
                (Row::GilUndo,MenuInput::Confirm)=>{
                    if self.gil.enabled && self.gil.undo_available {self.open(Page::GilUndoConfirm);}
                    else {self.gil.status="No recovery state available, or editing is locked.".into();}
                    return None;
                }
                _=>{}
            }
        }
        if self.page == Page::Debug {
            match (row, input) {
                (Row::DebugEnabled, MenuInput::Confirm | MenuInput::Left | MenuInput::Right) => {
                    self.debug_tools = !self.debug_tools;
                    return None;
                }
                (Row::DebugMission, MenuInput::Confirm) => {
                    self.open_dropdown(row, av);
                    return None;
                }
                (Row::DebugMission, MenuInput::Left | MenuInput::Right) => {
                    self.debug_mission = if input == MenuInput::Left {
                        if self.debug_mission == 1 {
                            405
                        } else {
                            self.debug_mission - 1
                        }
                    } else {
                        self.debug_mission % 405 + 1
                    };
                    return None;
                }
                (Row::DebugVictory, MenuInput::Confirm) if self.debug_tools => {
                    return Some(MenuAction::DebugVictory)
                }
                (Row::DebugCharacters | Row::DebugLaunch, MenuInput::Confirm) => {
                    self.debug_status = "Not validated - locked".into();
                    return None;
                }
                (Row::DebugVictory, MenuInput::Confirm) => {
                    self.debug_status = "Enable debug tools first".into();
                    return None;
                }
                _ => {}
            }
        }
        match input {
            MenuInput::Up => {
                self.selected = (self.selected + rows.len() - 1) % rows.len();
                None
            }
            MenuInput::Down => {
                self.selected = (self.selected + 1) % rows.len();
                None
            }
            MenuInput::Cancel => {
                if self.page == Page::Main {
                    Some(MenuAction::Resume)
                } else {
                    self.back_to_main();
                    None
                }
            }
            MenuInput::Confirm => match row {
                Row::MouseMode => self.toggle_mouse_mode(),
                Row::Resume => Some(MenuAction::Resume),
                Row::GilEditorMenu=>{self.open(Page::Editor);None}
                Row::CloneMenu=>{self.open(Page::CloneMember);Some(MenuAction::GilEditor(GilAction::CloneOpen))}
                Row::LevelsMenu=>{self.open(Page::Levels);Some(MenuAction::GilEditor(GilAction::LevelOpen))}
                Row::AbilitiesMenu=>{self.open(Page::Abilities);Some(MenuAction::GilEditor(GilAction::AbilitiesOpen))}
                Row::RenameMenu=>{if self.characters.begin_rename(){self.open(Page::CharacterRename);}None}
                Row::RenameYes=>Some(MenuAction::GilEditor(GilAction::RenameApply)),
                Row::LawMenu=>{self.open(Page::Laws);Some(MenuAction::GilEditor(GilAction::LawOpen))}
                Row::ShopMenu=>{self.open(Page::ShopStock);Some(MenuAction::GilEditor(GilAction::ShopOpen))}
                Row::CharactersMenu=>{self.open(Page::Characters);Some(MenuAction::GilEditor(GilAction::CharactersOpen))}
                Row::GilMenu=>{self.open(Page::GilEditor);Some(MenuAction::GilEditor(GilAction::Open))}
                Row::RewardsMenu=>{self.open(Page::RewardMultipliers);Some(MenuAction::Rewards(RewardAction::Open))}
                Row::ItemsMenu=>{self.open(Page::ItemEditor);Some(MenuAction::GilEditor(GilAction::ItemsOpen))}
                Row::ItemConfirmYes=>Some(MenuAction::GilEditor(GilAction::ItemApply)),
                Row::GilConfirmYes=>Some(MenuAction::GilEditor(GilAction::Apply)),
                Row::GilUndoYes=>Some(MenuAction::GilEditor(GilAction::Undo)),
                Row::GilCancel=>{self.close_gil_confirmation();None}

                Row::SaveStatesMenu => {
                    self.open(Page::SaveStates);
                    self.states.open();
                    Some(MenuAction::ManualState(StateAction::Refresh))
                }
                Row::Quit => {
                    self.open(Page::Exit);
                    None
                }
                Row::QuitYes => Some(MenuAction::Quit),
                Row::ReturnToGame => Some(MenuAction::Resume),
                Row::ChangeRom => Some(MenuAction::SwitchRom),
                Row::QuitNo => {
                    self.back_to_main();
                    None
                }
                Row::Back => {
                    self.back_to_main();
                    None
                }
                Row::Key(b) => {
                    self.capture = Some(if self.page == Page::Gamepad {
                        Bind::Gamepad(b)
                    } else {
                        Bind::Button(b)
                    });
                    None
                }
                Row::InputsMenu => self.input_tab(self.input_gamepad.get()),
                Row::ResetGamepad => Some(MenuAction::ResetGamepad),
                Row::FfKey => {
                    self.capture = Some(if self.page == Page::Gamepad {
                        Bind::GamepadFastForward
                    } else {
                        Bind::FastForward
                    });
                    None
                }
                Row::FfToggleKey => {
                    self.capture = Some(if self.page == Page::Gamepad {
                        Bind::GamepadFastForwardToggle
                    } else {
                        Bind::FastForwardToggle
                    });
                    None
                }
                Row::ZoomIn => {
                    self.capture = Some(if self.page == Page::Gamepad {
                        Bind::GamepadZoomIn
                    } else {
                        Bind::ZoomIn
                    });
                    None
                }
                Row::MenuKey => {
                    self.capture = Some(if self.page == Page::Gamepad {
                        Bind::GamepadMenu
                    } else {
                        Bind::MenuKey
                    });
                    None
                }
                Row::ZoomOut => {
                    self.capture = Some(if self.page == Page::Gamepad {
                        Bind::GamepadZoomOut
                    } else {
                        Bind::ZoomOut
                    });
                    None
                }
                r if r.opens().is_some() => {
                    self.open(r.opens().unwrap());
                    None
                }
                // Confirm does nothing on value settings: they change with
                // Left/Right only (so Enter can't flip an ON/OFF by accident).
                _ => {
                    self.open_dropdown(row, av);
                    None
                }
            },
            MenuInput::Left => {
                if row.is_action() {
                    None
                } else {
                    cycle(row, av, false).map(MenuAction::Changed)
                }
            }
            MenuInput::Right => {
                if matches!(row, Row::InputsMenu | Row::SaveStatesMenu | Row::CloneMenu | Row::LevelsMenu | Row::AbilitiesMenu | Row::RenameMenu | Row::LawMenu | Row::ShopMenu | Row::CharactersMenu | Row::GilMenu | Row::GilEditorMenu | Row::ItemsMenu | Row::RewardsMenu) {
                    self.handle(MenuInput::Confirm, av)
                } else if let Some(p) = row.opens() {
                    self.open(p);
                    None
                } else if row.is_action() {
                    None
                } else {
                    cycle(row, av, true).map(MenuAction::Changed)
                }
            }
        }
    }

    /// Row index under frame-pixel `(x, y)`, if any.
    fn row_at(&self, x: usize, y: usize, w: usize, h: usize) -> Option<usize> {
        let rows = rows_of(self.page);
        let l = page_layout(self.page, w, h);
        if matches!(
            self.page,
            Page::Inputs | Page::Gamepad | Page::MouseKeyboard
        ) && y >= l.panel_y + l.panel_h - l.pad - l.line_h + 6
            && y < l.panel_y + l.panel_h - l.pad
            && x >= l.panel_x + l.panel_w - l.pad - l.line_h * 3
            && x < l.panel_x + l.panel_w - l.pad
        {
            return Some(rows.len() - 1);
        }
        if x < l.panel_x + 2 || x >= l.panel_x + l.panel_w - 2 {
            return None;
        }
        let top = l.rows_y.checked_sub(2)?;
        if y < top {
            return None;
        }
        let i = (y - top) / l.line_h;
        if i >= l.visible_rows {
            return None;
        }
        let index = i + self.first_visible(l);
        if rows[index] == Row::Back && x < l.panel_x + l.panel_w - l.pad - l.line_h * 3 {
            return None;
        }
        Some(index)
    }

    /// Pointer moved: highlight the row under it (ignored while a key
    /// capture is pending). `(x, y)` are frame pixels, `w`/`h` the frame size.
    pub fn hover(&mut self, x: usize, y: usize, w: usize, h: usize) {
        // Wheel-driven focus belongs to the menu until deliberate pointer
        // movement. Stationary events and wheel-induced hand jitter must not
        // retarget a row as the viewport moves beneath the cursor.
        if self.wheel_mode {
            if let Some((px,py))=self.wheel_pointer {
                if x.abs_diff(px)<=6 && y.abs_diff(py)<=6{return;}
            }else{
                self.wheel_pointer=Some((x,y));self.last_pointer=Some((x,y));return;
            }
            self.wheel_mode=false;self.wheel_pointer=None;
        }
        self.last_pointer=Some((x,y));
        if gil_desk::active(self.page){gil_desk::hover(self,x,y,w,h);return;}
        if self.page==Page::Characters{return;}
        if self.page==Page::Main{if let Some(i)=home::hit(x,y,w,h){self.selected=i;}return;}
        if self.page==Page::RewardMultipliers{self.rewards.hover(x,y,w,h);return;}
        if self.clone_page_active(){self.cloning.hover(x,y,w,h);return;}
        if self.level_page_active(){self.levels.hover(x,y,w,h);return;}
        if self.ability_page_active(){self.abilities.hover(x,y,w,h);return;}
        if self.item_page_active(){self.items.hover(x,y,w,h);return;}

        if self.page == Page::SaveStates {
            self.states.hover(x,y,w,h);
            return;
        }

        if self.capture.is_some() {
            return;
        }
        if let Some(dd) = self.dropdown.as_mut() {
            if let Some(i) = theme::dropdown_hit(dd, x, y, w, h) {
                dd.index = i;
            }
            return;
        }
        if let Some(i) = self.row_at(x, y, w, h) {
            self.selected = i;
        }
    }

    /// Wheel scrolling stays within the list rather than wrapping at its ends.
    pub fn scroll(&mut self, up: bool, av: &mut AvConfig) -> Option<MenuAction> {
        self.wheel_mode=true;self.wheel_pointer=self.last_pointer;
        if self.page==Page::Characters{let n=self.characters.rows.len();if n>0{self.characters.selected=if up{self.characters.selected.saturating_sub(1)}else{(self.characters.selected+1).min(n-1)};}self.selected=0;return None;}
        if self.page==Page::RewardMultipliers{return self.handle(if up{MenuInput::Up}else{MenuInput::Down},av);}
        if self.clone_page_active(){self.cloning.scroll(up);return None;}
        if self.level_page_active(){let event=self.levels.scroll(up);return self.level_event(event);}
        if self.ability_page_active(){let event=self.abilities.scroll(up);return self.ability_event(event);}
        if self.item_page_active(){let event=self.items.scroll(up);return self.item_event(event);}

        if let Some(dd) = self.dropdown.as_mut() {
            let last = setting_options(dd.row).len().saturating_sub(1);
            dd.index = if up {
                dd.index.saturating_sub(3)
            } else {
                (dd.index + 3).min(last)
            };
            dd.first.set(if up {
                dd.first.get().saturating_sub(3)
            } else {
                (dd.first.get() + 3).min(last)
            });
            return None;
        }
        if self.page==Page::SaveStates{return self.handle(if up{MenuInput::Up}else{MenuInput::Down},av);}
        let last=rows_of(self.page).len().saturating_sub(1);
        self.selected=if up{self.selected.saturating_sub(1)}else{(self.selected+1).min(last)};
        None
    }

    /// Mouse click at frame-pixel `(x, y)`. Left on an action/submenu row =
    /// Confirm; left on a value row = next value; right on a value row =
    /// previous value; right on an action row or outside any row = Cancel
    /// (back). A left click outside any row does nothing.
    pub fn click(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        left: bool,
        av: &mut AvConfig,
    ) -> Option<MenuAction> {
        if !left{return None;} // Accidental right clicks must never discard an editor page.
        self.wheel_mode=false;self.wheel_pointer=None;self.last_pointer=Some((x,y));
        if gil_desk::active(self.page){return gil_desk::click(self,x,y,w,h,av);}
        if self.page==Page::Main{if !left{return self.handle(MenuInput::Cancel,av);}if let Some(i)=home::hit(x,y,w,h){self.selected=i;return self.handle(MenuInput::Confirm,av);}return None;}
        if self.page==Page::Characters{
            if !left{return self.handle(MenuInput::Cancel,av);}
            match roster::hit(self,x,y,w,h){
                Some(roster::Hit::Member(index))=>{self.characters.selected=index;self.selected=0;},
                Some(roster::Hit::Action(row))=>{self.selected=rows_of(Page::Characters).iter().position(|r|*r==row).unwrap();return self.handle(MenuInput::Confirm,av);},
                Some(roster::Hit::Page(right))=>{let n=self.characters.rows.len();if n>0{let first=self.characters.selected/6*6;self.characters.selected=if right{(first+6).min((n-1)/6*6)}else{first.saturating_sub(6)};self.selected=0;}},None=>{}
            }return None;
        }

        if self.page==Page::RewardMultipliers{let event=self.rewards.click(x,y,w,h,left);return self.reward_event(event);}
        if self.clone_page_active(){let event=self.cloning.click(x,y,w,h,left);return self.clone_event(event);}
        if self.level_page_active(){let event=self.levels.click(x,y,w,h,left);return self.level_event(event);}
        if self.ability_page_active(){let event=self.abilities.click(x,y,w,h,left);return self.ability_event(event);}
        if self.item_page_active(){let event=self.items.click(x,y,w,h,left);return self.item_event(event);}
        if self.page == Page::SaveStates {
            let event = self.states.click(x,y,w,h,left);
            return self.state_event(event);
        }
        if self.capture.is_some()
        {
            return None;
        }
        if let Some(dd) = self.dropdown.as_ref() {
            if left {
                if let Some(index) = theme::dropdown_scroll_hit(dd, x, y, w, h) {
                    self.dropdown.as_mut().unwrap().index = index;
                    return None;
                }
            }
            let hit = theme::dropdown_hit(dd, x, y, w, h);
            if left {
                if let Some(i) = hit {
                    self.dropdown.as_mut().unwrap().index = i;
                    return self.handle(MenuInput::Confirm, av);
                }
            }
            self.dropdown = None;
            return None;
        }
        if matches!(
            self.page,
            Page::Inputs | Page::Gamepad | Page::MouseKeyboard
        ) {
            let l = page_layout(self.page,w,h);
            if y >= l.panel_y + l.pad + l.line_h
                && y < l.rows_y - 2
                && x >= l.panel_x + l.pad
                && x < l.panel_x + l.panel_w - l.pad
            {
                return self.input_tab(
                    (((x - l.panel_x - l.pad) * 3) / (l.panel_w - 2 * l.pad)).min(2) as u8,
                );
            }
        }
        match self.row_at(x, y, w, h) {
            Some(i) => {
                self.selected = i;
                let row = rows_of(self.page)[i];
                if self.page==Page::Laws&&row==Row::LawValue{return self.handle(MenuInput::Confirm,av);}
                if self.page==Page::ShopStock&&row==Row::ShopValue{return self.handle(if left{MenuInput::Right}else{MenuInput::Left},av);}
                if self.page==Page::Characters && row==Row::CharacterMember{self.characters.cycle(left);return None;}
                if left {

                    if let Some(choice) =
                        theme::segment_hit(row, page_layout(self.page,w,h), x)
                    {
                        match row {
                            Row::FastForward => av.ff_speed = FF_SPEEDS[choice],
                            Row::FastForwardAudio => {
                                av.ff_audio = ["normal", "silent", "fast"][choice].into()
                            }
                            _ => unreachable!(),
                        }
                        return Some(MenuAction::Changed(Changed::Gamut));
                    }
                    if !setting_options(row).is_empty() {
                        self.open_dropdown(row, av);
                        return None;
                    }
                }
                let input = if row.is_action() {
                    if left {
                        MenuInput::Confirm
                    } else {
                        MenuInput::Cancel
                    }
                } else if left {
                    MenuInput::Right
                } else {
                    MenuInput::Left
                };
                self.handle(input, av)
            }
            None if left => None,
            None => self.handle(MenuInput::Cancel, av),
        }
    }

    /// Composite the overlay onto a composed `rgba` frame (`w*h`,
    /// row-major, RGBA bytes â€” the alpha channel is left at 255 and
    /// ignored by both present paths). `keys` are the current keyboard
    /// bindings (indexed by `Button::index`), shown on the Inputs page.
    /// The play loop supplies a high-resolution pause canvas independent
    /// of the GBA frame. Both GPU and CPU presenters display that canvas.
    /// Its parchment and brass palette follows the supplied HTML reference.
    pub fn draw(
        &self,
        av: &AvConfig,
        keys: &[String; 10],
        ff_key: &str,
        ff_toggle_key: &str,
        zoom_in: &str,
        zoom_out: &str,
        rgba: &mut [[u8; 4]],
        w: usize,
        h: usize,
    ) {
        if gil_desk::active(self.page){gil_desk::draw(self,rgba,w,h);return;}
        if self.page==Page::Main{home::draw(self,rgba,w,h);return;}
        if self.page==Page::Characters{roster::draw(self,rgba,w,h);return;}
        if self.page==Page::RewardMultipliers{self.rewards.draw(rgba,w,h);return;}
        if self.clone_page_active(){self.cloning.draw(rgba,w,h);return;}
        if self.level_page_active(){self.levels.draw(rgba,w,h);return;}
        if self.ability_page_active(){self.abilities.draw(rgba,w,h);return;}
        if self.item_page_active(){self.items.draw(rgba,w,h);return;}
        if self.page == Page::SaveStates {
            self.states.draw(rgba,w,h);
            return;
        }
        theme::draw(
            self,
            av,
            keys,
            ff_key,
            ff_toggle_key,
            zoom_in,
            zoom_out,
            rgba,
            w,
            h,
        );
    }

    pub fn draw_inputs(
        &self,
        av: &AvConfig,
        cfg: &InputConfig,
        rgba: &mut [[u8; 4]],
        w: usize,
        h: usize,
    ) {
        self.mouse_mode.set(cfg.mouse_keyboard);
        self.menu_binding.replace(if self.page == Page::Gamepad {
            cfg.menu_pad.clone()
        } else {
            cfg.menu_key.clone()
        });
        self.controller_layout.set(cfg.gamepad_layout);
        if !matches!(
            self.page,
            Page::Inputs | Page::Gamepad | Page::MouseKeyboard
        ) {
            self.input_gamepad.set(if cfg.device == Device::Gamepad {
                1
            } else if cfg.mouse_keyboard {
                2
            } else {
                0
            });
        }
        self.draw(
            av,
            if self.page == Page::Gamepad {
                &cfg.pads
            } else {
                if self.page == Page::MouseKeyboard {
                    &cfg.mouse_keys
                } else {
                    &cfg.keys
                }
            },
            if self.page == Page::Gamepad {
                &cfg.ff_pad
            } else {
                &cfg.ff_key
            },
            if self.page == Page::Gamepad {
                &cfg.ff_toggle_pad
            } else {
                &cfg.ff_toggle_key
            },
            if self.page == Page::Gamepad {
                &cfg.zoom_in_pad
            } else {
                &cfg.zoom_in_key
            },
            if self.page == Page::Gamepad {
                &cfg.zoom_out_pad
            } else {
                &cfg.zoom_out_key
            },
            rgba,
            w,
            h,
        );
    }
}

/// A 0..1-ish value in whole tenths (so stepping never accumulates float
/// error), and back.
fn tenths(v: f32) -> i32 {
    (v * 10.0).round() as i32
}

fn percent(v: f32) -> String {
    format!("{}%", tenths(v) * 10)
}

/// The display value for a row (`None` for rows without one).
fn value_of(
    row: Row,
    av: &AvConfig,
    keys: &[String; 10],
    ff_key: &str,
    capture: Option<Bind>,
) -> Option<String> {
    Some(match row {

        Row::DebugEnabled
        | Row::DebugVictory
        | Row::DebugCharacters
        | Row::DebugMission
        | Row::DebugLaunch
        | Row::DebugStatus => return None,
        Row::ShopReset=>return None,
        Row::CloneMenu|Row::LevelsMenu|Row::AbilitiesMenu|Row::RenameMenu|Row::LawMenu|Row::LawValue|Row::ShopMenu|Row::ShopValue|Row::CharactersMenu|Row::RewardsMenu|Row::ItemsMenu=>return Some(">".into()),
        Row::RenameCurrent|Row::RenameValue|Row::RenameApply|Row::RenameYes=>return None,
        Row::CharacterMember|Row::CharacterRace|Row::CharacterJob|Row::CharacterLevel|Row::CharacterExp|Row::CharacterJp=>return None,
        Row::ItemTotal|Row::ItemEquipped|Row::ItemValue|Row::ItemApply|Row::ItemConfirmYes=>return None,
        Row::GilMenu|Row::GilEditorMenu => return Some(">".into()),
        Row::GilCurrent|Row::GilValue|Row::GilStep|Row::GilApply|Row::GilUndo
        |Row::GilConfirmYes|Row::GilUndoYes|Row::GilCancel => return None,
        Row::MouseMode => return None,
        Row::SaveStatesMenu | Row::FastForwardMenu => ">".to_string(),
        Row::Resume
        | Row::Quit
        | Row::QuitYes
        | Row::QuitNo
        | Row::ReturnToGame
        | Row::ChangeRom
        | Row::Back
        | Row::ResetGamepad => return None,
        Row::AudioMenu
        | Row::TextBubblesMenu
        | Row::VideoMenu
        | Row::InputsMenu
        | Row::SwitchRomMenu => ">".to_string(),
        Row::FastForward => {
            if av.ff_speed < 1.0 {
                "OFF".to_string()
            } else {
                format!("{}X", av.ff_speed)
            }
        }
        Row::Volume => percent(av.audio_volume),
        Row::FastForwardAudio => match av.ff_audio.as_str() {
            "fast" => "FAST AUDIO",
            "normal" => "NORMAL",
            _ => "SILENT",
        }
        .to_string(),
        Row::Audio => if av.audio_enhanced { "ON" } else { "OFF" }.to_string(),
        Row::Screen => screen_kind(av).name().to_uppercase(),
        Row::BubbleSize => percent(av.bubble_size),
        Row::BubbleLength => percent(av.bubble_length),
        Row::Brightness => percent(av.video_brightness),
        Row::Darken => av.screen_darken.to_uppercase(),
        Row::Response => response_mode(av).name().to_uppercase(),
        Row::Grid => av.grid.to_uppercase(),
        Row::Scaling => scaling_name(av).to_uppercase(),
        Row::Aspect => aspect_name(av).to_uppercase(),
        Row::Vsync => if av.video_vsync { "ON" } else { "OFF" }.to_string(),
        Row::Key(b) => {
            if capture == Some(Bind::Gamepad(b)) {
                "PRESS BUTTON".to_string()
            } else if capture == Some(Bind::Button(b)) {
                "PRESS KEY".to_string()
            } else {
                keys[b.index()].to_uppercase()
            }
        }
        Row::FfKey => {
            if capture == Some(Bind::FastForward) {
                "PRESS KEY".to_string()
            } else {
                ff_key.to_uppercase()
            }
        }
        Row::MenuKey => "PRESS KEY".to_string(),
        Row::WindowMode => if av.video_fullscreen {
            "FULLSCREEN"
        } else {
            "WINDOWED"
        }
        .into(),
        Row::ZoomIn | Row::ZoomOut => "PRESS KEY".to_string(),
        Row::FfToggleKey => "PRESS KEY".to_string(),
    })
}

fn gamepad_label(token: &str, layout: GamepadLayout) -> String {
    token
        .strip_prefix("DInputButton")
        .map(|n| format!("BUTTON {n}"))
        .unwrap_or_else(|| layout.label(token))
}

/// Step a 0..1 style slider by one tenth within `[lo, hi]` tenths (clamped,
/// no wrap). Returns whether the value moved.
fn step_slider(value: &mut f32, forward: bool, lo: i32, hi: i32) -> bool {
    let cur = tenths(*value).clamp(lo, hi);
    let next = (cur + if forward { 1 } else { -1 }).clamp(lo, hi);
    let changed = next != tenths(*value);
    *value = next as f32 / 10.0;
    changed
}

/// Cycle the selected setting forward (`forward`) or backward and report
/// which surface moved. Returns `None` for action rows (and for sliders
/// already at their end).
fn setting_options(row: Row) -> Vec<String> {
    match row {
        Row::WindowMode => vec!["WINDOWED".into(), "FULLSCREEN".into()],
        Row::DebugMission => (1..=405).map(|id| format!("Mission ID {id:03}")).collect(),
        Row::Screen => ScreenKind::ALL
            .iter()
            .map(|s| s.name().to_uppercase())
            .collect(),
        Row::Response => ResponseMode::ALL
            .iter()
            .map(|s| s.name().to_uppercase())
            .collect(),
        Row::Scaling => SCALING_MODES.iter().map(|s| s.to_uppercase()).collect(),
        Row::Aspect => ASPECT_MODES.iter().map(|s| s.to_uppercase()).collect(),
        Row::Grid | Row::Darken => std::iter::once("AUTO".into())
            .chain((0..=10).map(|i| format!("{:.1}", i as f32 / 10.0)))
            .collect(),
        _ => Vec::new(),
    }
}
fn setting_value(row: Row, av: &AvConfig) -> String {
    value_of(row, av, &std::array::from_fn(|_| String::new()), "", None).unwrap_or_default()
}
fn choose_setting(row: Row, value: &str, av: &mut AvConfig) -> Option<Changed> {
    let mut change = None;
    for _ in 0..=setting_options(row).len() {
        if setting_value(row, av) == value {
            break;
        }
        change = cycle(row, av, true);
    }
    change
}
fn cycle(row: Row, av: &mut AvConfig, forward: bool) -> Option<Changed> {
    match row {

        Row::DebugEnabled
        | Row::DebugVictory
        | Row::DebugCharacters
        | Row::DebugMission
        | Row::DebugLaunch
        | Row::DebugStatus => None,
        Row::CloneMenu|Row::LevelsMenu|Row::AbilitiesMenu|Row::RenameMenu|Row::RenameCurrent|Row::RenameValue|Row::RenameApply|Row::RenameYes|Row::LawMenu|Row::LawValue|Row::ShopMenu|Row::ShopValue|Row::ShopReset|Row::CharactersMenu|Row::CharacterMember|Row::CharacterRace|Row::CharacterJob|Row::CharacterLevel|Row::CharacterExp|Row::CharacterJp|Row::RewardsMenu|Row::ItemsMenu|Row::ItemTotal|Row::ItemEquipped|Row::ItemValue|Row::ItemApply|Row::ItemConfirmYes=>None,
        Row::GilMenu|Row::GilEditorMenu|Row::GilCurrent|Row::GilValue|Row::GilStep|Row::GilApply|Row::GilUndo
        |Row::GilConfirmYes|Row::GilUndoYes|Row::GilCancel => None,
        Row::MouseMode => None,
        Row::Resume
        | Row::Quit
        | Row::QuitYes
        | Row::QuitNo
        | Row::ReturnToGame
        | Row::ChangeRom
        | Row::SaveStatesMenu
        | Row::FastForwardMenu
        | Row::AudioMenu
        | Row::TextBubblesMenu
        | Row::VideoMenu
        | Row::InputsMenu
        | Row::SwitchRomMenu
        | Row::ResetGamepad
        | Row::Key(_)
        | Row::FfKey
        | Row::FfToggleKey
        | Row::ZoomIn
        | Row::ZoomOut
        | Row::MenuKey
        | Row::Back => None,
        Row::WindowMode => {
            av.video_fullscreen = !av.video_fullscreen;
            Some(Changed::WindowMode)
        }
        Row::FastForward => {
            let cur = FF_SPEEDS
                .iter()
                .copied()
                .find(|s| *s == av.ff_speed)
                .unwrap_or(4.0);
            av.ff_speed = step_enum(&FF_SPEEDS, cur, forward);
            Some(Changed::Gamut)
        }
        Row::Volume => step_slider(&mut av.audio_volume, forward, 0, 10).then_some(Changed::Audio),
        Row::FastForwardAudio => {
            let modes = ["silent", "fast", "normal"];
            av.ff_audio = step_enum(
                &modes,
                modes
                    .iter()
                    .copied()
                    .find(|m| *m == av.ff_audio)
                    .unwrap_or("silent"),
                forward,
            )
            .into();
            Some(Changed::Gamut)
        }
        Row::Audio => {
            av.audio_enhanced = !av.audio_enhanced;
            Some(Changed::Audio)
        }
        Row::Screen => {
            let cur = screen_kind(av);
            let next = step_enum(&ScreenKind::ALL, cur, forward);
            av.screen = next.name().to_string();
            Some(Changed::Video)
        }
        Row::BubbleSize => {
            step_slider(&mut av.bubble_size, forward, 5, 20).then_some(Changed::Video)
        }
        Row::BubbleLength => {
            step_slider(&mut av.bubble_length, forward, 10, 20).then_some(Changed::Video)
        }
        Row::Brightness => {
            step_slider(&mut av.video_brightness, forward, 5, 15).then_some(Changed::Video)
        }
        Row::Darken => {
            av.screen_darken = step_knob(&av.screen_darken, forward);
            Some(Changed::Video)
        }
        Row::Response => {
            let cur = response_mode(av);
            let next = step_enum(&ResponseMode::ALL, cur, forward);
            av.response = next.name().to_string();
            Some(Changed::Video)
        }
        Row::Grid => {
            av.grid = step_knob(&av.grid, forward);
            Some(Changed::Video)
        }
        Row::Scaling => {
            let next = step_enum(&SCALING_MODES, scaling_name(av), forward);
            av.scaling = next.to_string();
            Some(Changed::Video)
        }
        Row::Aspect => {
            let next = step_enum(&ASPECT_MODES, aspect_name(av), forward);
            av.video_aspect = next.to_string();
            Some(Changed::Video)
        }
        Row::Vsync => {
            av.video_vsync = !av.video_vsync;
            Some(Changed::Vsync)
        }
    }
}

/// The configured scaling mode, normalized (unknown values read as "off").
fn scaling_name(av: &AvConfig) -> &'static str {
    SCALING_MODES
        .iter()
        .copied()
        .find(|m| *m == av.scaling.trim().to_lowercase())
        .unwrap_or("off")
}

/// The configured aspect, normalized (unknown values read as "16:9").
fn aspect_name(av: &AvConfig) -> &'static str {
    ASPECT_MODES
        .iter()
        .copied()
        .find(|m| *m == av.video_aspect.trim().to_lowercase())
        .unwrap_or("16:9")
}

/// A 0..1 knob string cycled through `auto` and discrete tenths. The
/// launcher exposes these as continuous sliders; the overlay steps them
/// so a d-pad can reach every useful value.
fn step_knob(cur: &str, forward: bool) -> String {
    // Ordered ring: auto, 0.0, 0.1, ... 1.0.
    let mut steps: Vec<String> = vec!["auto".to_string()];
    for i in 0..=10 {
        steps.push(format!("{:.1}", i as f32 / 10.0));
    }
    // Find the closest current entry (parse, else "auto").
    let idx = match AvConfig::knob(cur) {
        None => 0,
        Some(v) => {
            let q = (v.clamp(0.0, 1.0) * 10.0).round() as usize;
            (q + 1).min(steps.len() - 1)
        }
    };
    let next = if forward {
        (idx + 1) % steps.len()
    } else {
        (idx + steps.len() - 1) % steps.len()
    };
    steps[next].clone()
}

/// Step through a fixed enum table, wrapping.
fn step_enum<T: Copy + PartialEq>(all: &[T], cur: T, forward: bool) -> T {
    let i = all.iter().position(|&x| x == cur).unwrap_or(0);
    let n = all.len();
    if forward {
        all[(i + 1) % n]
    } else {
        all[(i + n - 1) % n]
    }
}

fn screen_kind(av: &AvConfig) -> ScreenKind {
    ScreenKind::by_name(&av.screen).unwrap_or(ScreenKind::Frontlit)
}

fn response_mode(av: &AvConfig) -> ResponseMode {
    ResponseMode::by_name(&av.response).unwrap_or(ResponseMode::Smart)
}

// â”€â”€ rgba drawing primitives â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_mode_and_mod_menu_binding_are_available() {
        let mut av = AvConfig::default();
        let mut m = Menu::new(true, true);
        m.open(Page::Video);
        at(&mut m, Row::WindowMode);
        assert_eq!(
            m.handle(MenuInput::Right, &mut av),
            Some(MenuAction::Changed(Changed::WindowMode))
        );
        assert!(!av.video_fullscreen);
        m.open(Page::Inputs);
        at(&mut m, Row::MenuKey);
        m.handle(MenuInput::Confirm, &mut av);
        assert_eq!(m.capturing(), Some(Bind::MenuKey));
        m.end_capture();
        m.open(Page::Gamepad);
        at(&mut m, Row::MenuKey);
        m.handle(MenuInput::Confirm, &mut av);
        assert_eq!(m.capturing(), Some(Bind::GamepadMenu));
    }

    #[test]
    fn debug_actions_require_enable_and_unverified_tools_stay_locked() {
        let mut menu = Menu::new(true, false);
        let mut av = AvConfig::default();
        menu.open_debug();
        assert!(menu.debug_tools);
        menu.selected = 0;
        menu.handle(MenuInput::Confirm, &mut av);
        menu.selected = 1;
        assert_eq!(menu.handle(MenuInput::Confirm, &mut av), None);
        menu.selected = 0;
        assert_eq!(menu.handle(MenuInput::Confirm, &mut av), None);
        menu.selected = 1;
        assert_eq!(
            menu.handle(MenuInput::Confirm, &mut av),
            Some(MenuAction::DebugVictory)
        );
        for row in [2, 4] {
            menu.selected = row;
            assert_eq!(menu.handle(MenuInput::Confirm, &mut av), None);
            assert_eq!(menu.debug_status, "Not validated - locked");
        }
        menu.selected = 0;
        menu.handle(MenuInput::Confirm, &mut av);
        menu.selected = 1;
        assert_eq!(menu.handle(MenuInput::Confirm, &mut av), None);
    }

    #[test]
    fn debug_mission_selector_wraps_without_changing_persisted_settings() {
        let mut menu = Menu::new(true, false);
        let mut av = AvConfig::default();
        let before = format!("{av:?}");
        menu.open_debug();
        menu.selected = 3;
        menu.handle(MenuInput::Left, &mut av);
        assert_eq!(menu.debug_mission, 405);
        menu.handle(MenuInput::Right, &mut av);
        assert_eq!(menu.debug_mission, 1);
        for _ in 0..404 {
            menu.handle(MenuInput::Right, &mut av);
        }
        assert_eq!(menu.debug_mission, 405);
        menu.handle(MenuInput::Confirm, &mut av);
        assert!(menu.dropdown.is_some());
        menu.handle(MenuInput::Up, &mut av);
        menu.handle(MenuInput::Confirm, &mut av);
        assert_eq!(menu.debug_mission, 404);
        assert_eq!(format!("{av:?}"), before);
        menu.reset();
        assert_eq!(menu.page, Page::Main);
        assert!(menu.capturing().is_none());
    }
    #[test]
    fn mission_wheel_and_scrollbar_reach_both_ends_without_committing() {
        let mut m = Menu::new(true, false);
        let mut av = AvConfig::default();
        m.open_debug();
        m.selected = 3;
        m.handle(MenuInput::Confirm, &mut av);
        for _ in 0..200 {
            m.scroll(false, &mut av);
        }
        assert_eq!(m.dropdown.as_ref().unwrap().index, 404);
        let dd = m.dropdown.as_ref().unwrap();
        assert!(theme::dropdown_scroll_hit(dd, 790, 690, 1280, 720).is_some());
        for _ in 0..200 {
            m.scroll(true, &mut av);
        }
        assert_eq!(m.dropdown.as_ref().unwrap().index, 0);
        assert_eq!(m.debug_mission, 1);
    }

    #[test]
    fn physical_pixel_layout_scales_proportionally_from_1080p_to_4k() {
        for n in [MAIN_ROWS.len(), INPUT_ROWS.len(), GAMEPAD_ROWS.len()] {
            let hd = layout(n, 1920, 1080);
            let uhd = layout(n, 3840, 2160);
            assert!(uhd.panel_w.abs_diff(2 * hd.panel_w) <= 2);
            assert!(uhd.line_h.abs_diff(2 * hd.line_h) <= 1);
            assert!(uhd.panel_x + uhd.panel_w <= 3840);
            assert!(uhd.panel_y + uhd.panel_h <= 2160);
            assert_eq!(hd.visible_rows, uhd.visible_rows);
        }
    }
    #[test]
    fn dropdown_cancel_and_commit_use_real_video_settings() {
        let mut menu = Menu::new(true, true);
        let mut av = AvConfig::default();
        menu.open(Page::Video);
        at(&mut menu, Row::Screen);
        let original = av.screen.clone();
        menu.handle(MenuInput::Confirm, &mut av);
        assert!(menu.dropdown.is_some());
        menu.handle(MenuInput::Down, &mut av);
        assert_eq!(av.screen, original);
        menu.handle(MenuInput::Cancel, &mut av);
        assert_eq!(menu.page, Page::Video);
        assert!(menu.dropdown.is_none());
        menu.handle(MenuInput::Confirm, &mut av);
        menu.handle(MenuInput::Down, &mut av);
        let index = menu.dropdown.as_ref().unwrap().index;
        let expected = setting_options(Row::Screen)[index].clone();
        assert_eq!(
            menu.handle(MenuInput::Confirm, &mut av),
            Some(MenuAction::Changed(Changed::Video))
        );
        assert_eq!(av.screen.to_uppercase(), expected);
    }
    #[test]
    fn scrolled_input_hit_test_and_tabs_keep_binding_targets() {
        let mut menu = Menu::new(true, true);
        let mut av = AvConfig::default();
        menu.open(Page::Gamepad);
        at(&mut menu, Row::FfToggleKey);
        let l = layout(GAMEPAD_ROWS.len(), 240, 160);
        let first = menu.first_visible(l);
        assert!(first > 0);
        let y = l.rows_y + (menu.selected - first) * l.line_h + 2;
        assert_eq!(menu.click(l.panel_x + 30, y, 240, 160, true, &mut av), None);
        assert_eq!(menu.capturing(), Some(Bind::GamepadFastForwardToggle));
        menu.end_capture();
        assert_eq!(
            menu.handle(MenuInput::Left, &mut av),
            Some(MenuAction::InputDevice(Device::Keyboard))
        );
        assert_eq!(menu.page, Page::Inputs);
        assert_eq!(
            menu.handle(MenuInput::Right, &mut av),
            Some(MenuAction::InputDevice(Device::Gamepad))
        );
        menu.handle(MenuInput::Cancel, &mut av);
        at(&mut menu, Row::InputsMenu);
        assert_eq!(
            menu.handle(MenuInput::Confirm, &mut av),
            Some(MenuAction::InputDevice(Device::Gamepad))
        );
    }
    #[test]
    fn third_input_tab_selects_independent_mouse_keyboard_profile() {
        let mut menu = Menu::new(true, true);
        let mut av = AvConfig::default();
        menu.open(Page::Gamepad);
        assert_eq!(menu.handle(MenuInput::Right, &mut av), None);
        assert_eq!(menu.page, Page::MouseKeyboard);
        assert_eq!(
            menu.handle(MenuInput::Confirm, &mut av),
            Some(MenuAction::InputDevice(Device::MouseKeyboard))
        );
        assert!(menu.mouse_mode.get());
        assert_eq!(menu.input_tab(0), None);
        assert_eq!(menu.page, Page::MouseKeyboard);
        at(&mut menu, Row::Key(Button::Up));
        menu.handle(MenuInput::Confirm, &mut av);
        assert_eq!(menu.capturing(), Some(Bind::Button(Button::Up)));
        menu.end_capture();
        assert_eq!(
            menu.handle(MenuInput::Right, &mut av),
            Some(MenuAction::InputDevice(Device::Gamepad))
        );
        menu.input_tab(2);
        at(&mut menu, Row::MouseMode);
        assert_eq!(
            menu.handle(MenuInput::Confirm, &mut av),
            Some(MenuAction::InputDevice(Device::Keyboard))
        );
        assert!(!menu.mouse_mode.get());
        assert_eq!(
            menu.input_tab(0),
            Some(MenuAction::InputDevice(Device::Keyboard))
        );
    }
    #[test]
    fn segmented_controls_set_exact_values_by_mouse() {
        let mut menu = Menu::new(true, true);
        let mut av = AvConfig::default();
        menu.open(Page::FastForward);
        let l = layout(FAST_FORWARD_ROWS.len(), 568, 160);
        let (x, bw) = theme::segment_geometry_for(l, FF_SPEEDS.len());
        assert_eq!(
            menu.click(x + (bw + 2) + 3, l.rows_y + 3, 568, 160, true, &mut av),
            Some(MenuAction::Changed(Changed::Gamut))
        );
        assert_eq!(av.ff_speed, 2.0);
        let (audio_x, _) = theme::segment_geometry_for(l, 3);
        menu.click(
            audio_x + 3,
            l.rows_y + l.line_h + 3,
            568,
            160,
            true,
            &mut av,
        );
        assert_eq!(av.ff_audio, "normal");
    }

    #[test]
    fn input_submenus_keep_keyboard_and_gamepad_captures_separate() {
        let mut menu = Menu::new(true, false);
        let mut av = AvConfig::default();
        at(&mut menu, Row::InputsMenu);
        menu.handle(MenuInput::Confirm, &mut av);
        assert_eq!(
            menu.handle(MenuInput::Right, &mut av),
            Some(MenuAction::InputDevice(Device::Gamepad))
        );
        at(&mut menu, Row::Key(Button::A));
        menu.handle(MenuInput::Confirm, &mut av);
        assert_eq!(menu.capturing(), Some(Bind::Gamepad(Button::A)));
        assert_eq!(
            value_of(Row::Key(Button::A), &av, &keys(), "Tab", menu.capturing()),
            Some("PRESS BUTTON".into())
        );
        menu.end_capture();
        for (row, binding) in [
            (Row::FfKey, Bind::GamepadFastForward),
            (Row::FfToggleKey, Bind::GamepadFastForwardToggle),
        ] {
            at(&mut menu, row);
            menu.handle(MenuInput::Confirm, &mut av);
            assert_eq!(menu.capturing(), Some(binding));
            menu.end_capture();
        }
        menu.handle(MenuInput::Cancel, &mut av);
        assert_eq!(menu.page, Page::Main);
        menu.open(Page::Gamepad);
        assert_eq!(
            menu.handle(MenuInput::Left, &mut av),
            Some(MenuAction::InputDevice(Device::Keyboard))
        );
        at(&mut menu, Row::Key(Button::A));
        menu.handle(MenuInput::Confirm, &mut av);
        assert_eq!(menu.capturing(), Some(Bind::Button(Button::A)));
    }

    fn keys() -> [String; 10] {
        input_config::InputConfig::default().keys
    }

    fn at(menu: &mut Menu, row: Row) {
        menu.selected = rows_of(menu.page)
            .iter()
            .position(|r| *r == row)
            .expect("row on this page");
    }

    #[test]
    fn enhanced_audio_lives_in_audio_submenu() {
        let mut av = AvConfig::default();
        let mut menu = Menu::new(true, true);
        at(&mut menu, Row::AudioMenu);
        assert_eq!(menu.handle(MenuInput::Confirm, &mut av), None);
        assert_eq!(menu.page, Page::Audio);
        at(&mut menu, Row::Audio);
        let act = menu.handle(MenuInput::Right, &mut av);
        assert_eq!(act, Some(MenuAction::Changed(Changed::Audio)));
        assert!(av.audio_enhanced);
    }

    #[test]
    fn volume_steps_clamps_and_reports_audio() {
        let mut av = AvConfig::default();
        let mut menu = Menu::new(true, true);
        menu.open(Page::Audio);
        at(&mut menu, Row::Volume);
        assert_eq!(menu.handle(MenuInput::Right, &mut av), None);
        assert_eq!(
            menu.handle(MenuInput::Left, &mut av),
            Some(MenuAction::Changed(Changed::Audio))
        );
        assert!((av.audio_volume - 0.9).abs() < 1e-6);
        for _ in 0..20 {
            menu.handle(MenuInput::Left, &mut av);
        }
        assert_eq!(av.audio_volume, 0.0);
        assert_eq!(menu.handle(MenuInput::Left, &mut av), None);
    }

    #[test]
    fn fast_forward_multiplier_cycles_without_enabling() {
        let mut av = AvConfig::default();
        assert_eq!(av.ff_speed, 4.0);
        let mut menu = Menu::new(true, true);
        menu.open(Page::FastForward);
        at(&mut menu, Row::FastForward);
        let seq: Vec<f32> = (0..4)
            .map(|_| {
                menu.handle(MenuInput::Right, &mut av);
                av.ff_speed
            })
            .collect();
        assert_eq!(seq, vec![1.5, 2.0, 3.0, 4.0]);
        menu.handle(MenuInput::Left, &mut av);
        assert_eq!(av.ff_speed, 3.0);
        assert_eq!(
            value_of(Row::FastForward, &av, &keys(), "Tab", None).unwrap(),
            "3X"
        );
    }

    #[test]
    fn bubbles_are_main_menu_sibling_and_all_rows_fit() {
        assert_eq!(MAIN_ROWS[4], Row::TextBubblesMenu);
        assert!(!VIDEO_ROWS.contains(&Row::TextBubblesMenu));
        assert_eq!(
            layout(MAIN_ROWS.len(), 1280, 720).visible_rows,
            MAIN_ROWS.len()
        );
    }
    #[test]
    fn submenus_open_and_cancel_goes_back() {
        let mut av = AvConfig::default();
        let mut menu = Menu::new(true, true);
        for (row, page) in [
            (Row::FastForwardMenu, Page::FastForward),
            (Row::TextBubblesMenu, Page::TextBubbles),
            (Row::AudioMenu, Page::Audio),
            (Row::VideoMenu, Page::Video),
            (Row::InputsMenu, Page::Inputs),
        ] {
            menu.reset();
            at(&mut menu, row);
            let expected = if row == Row::InputsMenu {
                Some(MenuAction::InputDevice(Device::Keyboard))
            } else {
                None
            };
            assert_eq!(menu.handle(MenuInput::Confirm, &mut av), expected);
            assert_eq!(menu.page, page);
            // Cancel leaves the submenu (cursor returns to its entry row).
            assert_eq!(menu.handle(MenuInput::Cancel, &mut av), None);
            assert_eq!(menu.page, Page::Main);
            assert_eq!(rows_of(Page::Main)[menu.selected], row);
        }
        assert_eq!(
            menu.handle(MenuInput::Cancel, &mut av),
            Some(MenuAction::Resume)
        );
    }

    #[test]
    fn screen_cycles_through_known_models() {
        let mut av = AvConfig::default();
        av.screen = "frontlit".into();
        let mut menu = Menu::new(true, true);
        menu.open(Page::Video);
        at(&mut menu, Row::Screen);
        menu.handle(MenuInput::Right, &mut av);
        assert!(ScreenKind::by_name(&av.screen).is_some());
        for _ in 0..ScreenKind::ALL.len() {
            menu.handle(MenuInput::Right, &mut av);
        }
        assert!(ScreenKind::by_name(&av.screen).is_some());
    }

    #[test]
    fn brightness_steps_within_range_and_rebuilds_video() {
        let mut av = AvConfig::default();
        let mut menu = Menu::new(true, true);
        menu.open(Page::Video);
        at(&mut menu, Row::Brightness);
        assert_eq!(
            menu.handle(MenuInput::Right, &mut av),
            Some(MenuAction::Changed(Changed::Video))
        );
        assert!((av.video_brightness - 1.1).abs() < 1e-6);
        for _ in 0..30 {
            menu.handle(MenuInput::Right, &mut av);
        }
        assert!((av.video_brightness - 1.5).abs() < 1e-6);
    }

    #[test]
    fn key_row_starts_capture_and_freezes_menu() {
        let mut av = AvConfig::default();
        let mut menu = Menu::new(true, true);
        menu.open(Page::Inputs);
        at(&mut menu, Row::Key(Button::Start));
        assert_eq!(menu.capturing(), None);
        assert_eq!(menu.handle(MenuInput::Confirm, &mut av), None);
        assert_eq!(menu.capturing(), Some(Bind::Button(Button::Start)));
        // Everything is ignored while waiting for a key.
        let before = menu.selected;
        assert_eq!(menu.handle(MenuInput::Down, &mut av), None);
        assert_eq!(menu.handle(MenuInput::Cancel, &mut av), None);
        assert_eq!(menu.selected, before);
        assert_eq!(menu.page, Page::Inputs);
        let v = value_of(
            Row::Key(Button::Start),
            &av,
            &keys(),
            "Tab",
            menu.capturing(),
        );
        assert_eq!(v.unwrap(), "PRESS KEY");
        menu.end_capture();
        assert_eq!(menu.capturing(), None);
        let v = value_of(Row::Key(Button::Start), &av, &keys(), "Tab", None);
        assert_eq!(v.unwrap(), "ENTER");
    }

    #[test]
    fn enter_does_nothing_on_value_settings_but_arrows_change_them() {
        let mut av = AvConfig::default();
        let mut menu = Menu::new(true, true);
        menu.open(Page::Audio);
        at(&mut menu, Row::Audio);
        assert_eq!(menu.handle(MenuInput::Confirm, &mut av), None);
        assert!(!av.audio_enhanced);
        assert_eq!(
            menu.handle(MenuInput::Right, &mut av),
            Some(MenuAction::Changed(Changed::Audio))
        );
        assert!(av.audio_enhanced);
        // Same for the fast-forward submenu multiplier.
        menu.reset();
        menu.open(Page::FastForward);
        at(&mut menu, Row::FastForward);
        assert_eq!(menu.handle(MenuInput::Confirm, &mut av), None);
        assert_eq!(av.ff_speed, 4.0);
    }

    #[test]
    fn ff_key_row_captures_the_fast_forward_binding() {
        let mut av = AvConfig::default();
        let mut menu = Menu::new(true, true);
        menu.open(Page::Inputs);
        at(&mut menu, Row::FfKey);
        menu.handle(MenuInput::Confirm, &mut av);
        assert_eq!(menu.capturing(), Some(Bind::FastForward));
        let v = value_of(Row::FfKey, &av, &keys(), "Tab", menu.capturing());
        assert_eq!(v.unwrap(), "PRESS KEY");
        menu.end_capture();
        let v = value_of(Row::FfKey, &av, &keys(), "Tab", None);
        assert_eq!(v.unwrap(), "TAB");
    }

    #[test]
    fn toggle_key_and_fast_forward_audio_are_configurable() {
        let mut av = AvConfig::default();
        let mut menu = Menu::new(true, true);
        menu.open(Page::FastForward);
        at(&mut menu, Row::FastForwardAudio);
        menu.handle(MenuInput::Right, &mut av);
        assert_eq!(av.ff_audio, "fast");
        menu.handle(MenuInput::Right, &mut av);
        assert_eq!(av.ff_audio, "normal");
        menu.handle(MenuInput::Right, &mut av);
        assert_eq!(av.ff_audio, "silent");
        menu.open(Page::Inputs);
        at(&mut menu, Row::FfToggleKey);
        menu.handle(MenuInput::Confirm, &mut av);
        assert_eq!(menu.capturing(), Some(Bind::FastForwardToggle));
    }

    #[test]
    fn resume_and_quit_actions() {
        let mut av = AvConfig::default();
        let mut menu = Menu::new(true, true);
        at(&mut menu, Row::Resume);
        assert_eq!(
            menu.handle(MenuInput::Confirm, &mut av),
            Some(MenuAction::Resume)
        );
        assert_eq!(
            menu.handle(MenuInput::Cancel, &mut av),
            Some(MenuAction::Resume)
        );
        at(&mut menu, Row::Quit);
        assert_eq!(menu.handle(MenuInput::Confirm, &mut av), None);
        assert_eq!(menu.page, Page::Exit);
        assert_eq!(rows_of(menu.page)[menu.selected], Row::QuitNo);
        assert_eq!(menu.handle(MenuInput::Confirm, &mut av), None);
        assert_eq!(menu.page, Page::Main);
        at(&mut menu, Row::Quit);
        menu.handle(MenuInput::Confirm, &mut av);
        menu.handle(MenuInput::Up, &mut av);
        assert_eq!(
            menu.handle(MenuInput::Confirm, &mut av),
            Some(MenuAction::Quit)
        );
    }

    #[test]
    fn switch_rom_submenu_can_resume_or_return_to_launcher() {
        let mut av = AvConfig::default();
        let mut menu = Menu::new(true, true);
        at(&mut menu, Row::SwitchRomMenu);
        assert_eq!(menu.handle(MenuInput::Confirm, &mut av), None);
        assert_eq!(menu.page, Page::SwitchRom);
        assert_eq!(rows_of(menu.page)[menu.selected], Row::ReturnToGame);
        assert_eq!(
            menu.handle(MenuInput::Confirm, &mut av),
            Some(MenuAction::Resume)
        );

        menu.reset();
        at(&mut menu, Row::SwitchRomMenu);
        menu.handle(MenuInput::Confirm, &mut av);
        menu.handle(MenuInput::Down, &mut av);
        assert_eq!(
            menu.handle(MenuInput::Confirm, &mut av),
            Some(MenuAction::SwitchRom)
        );
        menu.reset();
        at(&mut menu, Row::SwitchRomMenu);
        menu.handle(MenuInput::Confirm, &mut av);
        assert_eq!(menu.handle(MenuInput::Cancel, &mut av), None);
        assert_eq!(menu.page, Page::Main);
        assert_eq!(rows_of(menu.page)[menu.selected], Row::SwitchRomMenu);
    }

    #[test]
    fn knob_steps_wrap_through_auto() {
        assert_eq!(step_knob("auto", true), "0.0");
        assert_eq!(step_knob("0.0", false), "auto");
        assert_eq!(step_knob("1.0", true), "auto");
        assert_eq!(step_knob("0.5", true), "0.6");
    }

    #[test]
    fn navigation_wraps() {
        let mut av = AvConfig::default();
        let mut menu = Menu::new(true, true);
        menu.selected = 0;
        menu.handle(MenuInput::Up, &mut av);
        assert_eq!(menu.selected, MAIN_ROWS.len() - 1);
        menu.handle(MenuInput::Down, &mut av);
        assert_eq!(menu.selected, 0);
    }

    #[test]
    fn draw_does_not_panic_on_any_page() {
        let av = AvConfig::default();
        let mut menu = Menu::new(false, false);
        for page in [
            Page::Main,
            Page::Exit,
            Page::SwitchRom,
            Page::FastForward,
            Page::Audio,
            Page::Video,
            Page::Inputs,
            Page::Gamepad,
        ] {
            menu.page = page;
            let mut rgba = vec![[10u8, 10, 10, 255]; 240 * 160];
            menu.draw(
                &av,
                &keys(),
                "Tab",
                "T",
                "Equal",
                "Minus",
                &mut rgba,
                240,
                160,
            );
            assert!(rgba.iter().any(|p| p[0] != 10 || p[1] != 10 || p[2] != 10));
        }
    }

    #[test]
    fn mouse_hover_and_click_hit_rows() {
        let mut av = AvConfig::default();
        let mut menu = Menu::new(true, true);
        let (w, h) = (372usize, 160usize);

        // Main page: hover selects, left click on "Audio >" opens it.
        let l = layout(MAIN_ROWS.len(), w, h);
        let x = l.panel_x + l.panel_w / 2;
        let y_main = |i: usize| home::center(i,w,h).1;
        let audio_row = MAIN_ROWS.iter().position(|r| *r == Row::AudioMenu).unwrap();
        let main_x=home::center(audio_row,w,h).0;
        menu.hover(main_x, y_main(audio_row), w, h);
        assert_eq!(menu.selected, audio_row);
        assert_eq!(menu.click(main_x, y_main(audio_row), w, h, true, &mut av), None);
        assert_eq!(menu.page, Page::Audio);

        // Audio page row 1: left changes the value; right clicks are ignored.
        let l = layout(AUDIO_ROWS.len(), w, h);
        let y_audio = |i: usize| l.rows_y + i * l.line_h + 1;
        assert_eq!(
            menu.click(x, y_audio(1), w, h, true, &mut av),
            Some(MenuAction::Changed(Changed::Audio))
        );
        assert!(av.audio_enhanced);
        assert_eq!(
            menu.click(x, y_audio(1), w, h, false, &mut av),
            None
        );
        assert!(av.audio_enhanced);

        // Outside the panel: both buttons leave the page intact.
        assert_eq!(menu.click(0, 0, w, h, true, &mut av), None);
        assert_eq!(menu.page, Page::Audio);
        assert_eq!(menu.click(0, 0, w, h, false, &mut av), None);
        assert_eq!(menu.page, Page::Audio);
    }
}

#[cfg(test)]
mod gil_menu_tests {
    use super::*;
    #[test]
    fn gil_apply_requires_explicit_confirmation_and_keeps_video_layout_independent() {
        let mut menu=Menu::new(true,true);let mut av=AvConfig::default();
        menu.open(Page::GilEditor);menu.gil.open(Ok(4650),false);menu.gil.proposed=5650;
        menu.selected=3;
        assert_eq!(menu.handle(MenuInput::Confirm,&mut av),None);
        assert_eq!(menu.page,Page::GilConfirm);
        assert_eq!(menu.handle(MenuInput::Confirm,&mut av),None);
        assert_eq!(menu.page,Page::GilEditor);
        menu.handle(MenuInput::Confirm,&mut av);
        menu.handle(MenuInput::Down,&mut av);
        assert_eq!(menu.handle(MenuInput::Confirm,&mut av),Some(MenuAction::GilEditor(GilAction::Apply)));
        assert!(page_layout(Page::Main,1280,720).panel_w<page_layout(Page::Video,1280,720).panel_w);
    }
    #[test]
    fn gil_confirmation_back_and_completion_return_one_level_without_losing_staged_amount() {
        let mut menu=Menu::new(true,true);let mut av=AvConfig::default();
        menu.open(Page::GilEditor);menu.gil.open(Ok(4650),true);menu.gil.proposed=5650;
        for (row,page) in [(3,Page::GilConfirm),(4,Page::GilUndoConfirm)] {
            menu.selected=row;
            menu.handle(MenuInput::Confirm,&mut av);
            assert_eq!(menu.page,page);
            assert_eq!(menu.handle(MenuInput::Cancel,&mut av),None);
            assert_eq!(menu.page,Page::GilEditor);
            assert_eq!(menu.selected,row);
            assert_eq!(menu.gil.proposed,5650);
            menu.handle(MenuInput::Confirm,&mut av);
            menu.gil_result("Complete".into());
            assert_eq!(menu.page,Page::GilEditor);
            assert_eq!(menu.selected,row);
        }
        assert_eq!(menu.navigation.parents.len(),1);
    }

}

#[cfg(test)]
mod item_editor_menu_tests {
    use super::*;
    #[test]
    fn item_editor_public_input_boundary_keeps_confirmation_cancel_as_default(){
        let mut menu=Menu::new(true,true);let mut av=AvConfig::default();menu.open(Page::ItemEditor);
        menu.items.open_rows(Ok(vec![OwnedItem{offset:0x1968,id:277,name:"Feather Cap".into(),kind:"Hat".into(),category:2,total:3,equipped:1,editable:true,teaches:Vec::new()}]));
        menu.click(834,336,900,600,true,&mut av);assert_eq!(menu.items.proposed,4);
        assert_eq!(menu.click(690,393,900,600,true,&mut av),None);
        assert_eq!(menu.handle(MenuInput::Confirm,&mut av),None);assert_eq!(menu.items.current,3);
        menu.click(690,393,900,600,true,&mut av);menu.handle(MenuInput::Right,&mut av);
        assert_eq!(menu.handle(MenuInput::Confirm,&mut av),Some(MenuAction::GilEditor(GilAction::ItemApply)));
        menu.item_result("Complete".into());assert!(menu.item_page_active());
    }
}

#[cfg(test)]
mod reward_menu_navigation_tests {
    use super::*;
    #[test]
    fn reward_menu_opens_inside_editor_even_when_inventory_edits_are_locked(){
        let mut m=Menu::new(true,true);let mut av=AvConfig::default();
        m.open(Page::Editor);m.gil.open(Err("Return to the world map".into()),false);
        m.selected=rows_of(Page::Editor).iter().position(|r|*r==Row::RewardsMenu).unwrap();
        let selected=m.selected;
        assert_eq!(m.handle(MenuInput::Confirm,&mut av),Some(MenuAction::Rewards(RewardAction::Open)));
        assert_eq!(m.page,Page::RewardMultipliers);m.rewards.sync(1,1,1,1,true);
        assert_eq!(m.handle(MenuInput::Right,&mut av),Some(MenuAction::Rewards(RewardAction::Changed)));
        assert_eq!((m.rewards.gil,m.rewards.ap),(2,1));
        m.handle(MenuInput::Cancel,&mut av);
        assert_eq!(m.page,Page::Editor);assert_eq!(m.selected,selected);
        assert!(!m.gil.enabled);
    }
}

#[cfg(test)]
mod editor_hub_tests {
    use super::*;
    #[test]
    fn editor_hub_opens_each_tool_and_returns_to_its_selected_entry(){
        let mut m=Menu::new(true,true);let mut av=AvConfig::default();
        m.selected=rows_of(Page::Main).iter().position(|r|*r==Row::GilEditorMenu).unwrap();
        assert_eq!(m.handle(MenuInput::Confirm,&mut av),None);assert_eq!(m.page,Page::Editor);
        assert_eq!(rows_of(Page::Editor),&[Row::GilMenu,Row::RewardsMenu,Row::ItemsMenu,Row::CharactersMenu,Row::ShopMenu,Row::LawMenu,Row::Back]);
        assert_eq!(m.handle(MenuInput::Confirm,&mut av),Some(MenuAction::GilEditor(GilAction::Open)));
        assert_eq!(m.page,Page::GilEditor);assert!(!rows_of(m.page).contains(&Row::RewardsMenu));
        m.selected=rows_of(m.page).len()-1;m.handle(MenuInput::Confirm,&mut av);
        assert_eq!(m.page,Page::Editor);assert_eq!(m.selected,0);
        m.selected=2;assert_eq!(m.handle(MenuInput::Confirm,&mut av),Some(MenuAction::GilEditor(GilAction::ItemsOpen)));
        m.handle(MenuInput::Cancel,&mut av);assert_eq!(m.page,Page::Editor);assert_eq!(m.selected,2);
        m.selected=0;m.handle(MenuInput::Confirm,&mut av);m.handle(MenuInput::Cancel,&mut av);
        assert_eq!(m.page,Page::Editor);m.handle(MenuInput::Cancel,&mut av);assert_eq!(m.page,Page::Main);
    }
}

#[cfg(test)]mod character_editor_navigation_tests {
    use super::*;
    #[test]fn editor_character_preview_is_read_only_and_returns_to_editor(){
        let mut m=Menu::new(true,true);let mut av=AvConfig::default();m.open(Page::Editor);m.selected=3;
        assert_eq!(m.handle(MenuInput::Confirm,&mut av),Some(MenuAction::GilEditor(GilAction::CharactersOpen)));
        assert_eq!(m.page,Page::Characters);
        let row=|name:&str|CharacterInfo{slot:1,identity:2,record:Vec::new(),name:name.into(),race:"Human".into(),job:"Soldier".into(),level:3,exp:18,jp:1};
        m.characters.open(Ok(vec![row("Marche"),row("Cooper")]));m.handle(MenuInput::Down,&mut av);
        assert_eq!(m.characters.current().unwrap().name,"Cooper");
        m.selected=4;assert_eq!(m.handle(MenuInput::Right,&mut av),None);assert_eq!(m.characters.current().unwrap().exp,18);
        m.handle(MenuInput::Cancel,&mut av);assert_eq!(m.page,Page::Editor);assert_eq!(m.selected,3);
    }
}

#[cfg(test)]mod menu_wheel_tests {
    use super::*;
    #[test]fn menu_wheel_keeps_focus_until_deliberate_pointer_motion(){
        let mut m=Menu::new(true,true);let mut av=AvConfig::default();m.open(Page::Video);
        let(w,h)=(854,480);let l=page_layout(m.page,w,h);
        let x=l.panel_x+l.pad+10;let y=l.rows_y+l.line_h/2;
        m.scroll(false,&mut av);assert_eq!(m.selected,1);
        m.hover(x,y,w,h);assert_eq!(m.selected,1); // First move can arrive after opening.
        m.hover(x+20,y,w,h);assert_eq!(m.selected,0);
        m.scroll(false,&mut av);assert_eq!(m.selected,1);
        m.hover(x+20,y,w,h);assert_eq!(m.selected,1);
        m.hover(x+22,y+2,w,h);assert_eq!(m.selected,1);
        m.scroll(false,&mut av);assert_eq!(m.selected,2);
        m.hover(x+40,y,w,h);assert_eq!(m.selected,0);
        for _ in 0..30{m.scroll(false,&mut av);}assert_eq!(m.selected,rows_of(m.page).len()-1);
        for _ in 0..30{m.scroll(true,&mut av);}assert_eq!(m.selected,0);
    }
    #[test]fn menu_wheel_does_not_block_an_explicit_click(){
        let mut m=Menu::new(true,true);let mut av=AvConfig::default();m.open(Page::Editor);
        let(w,h)=(854,480);let l=page_layout(m.page,w,h);let x=l.panel_x+l.pad+10;let y=l.rows_y+l.line_h/2;
        m.hover(x,y,w,h);m.scroll(false,&mut av);
        assert_eq!(m.click(x,y,w,h,true,&mut av),Some(MenuAction::GilEditor(GilAction::Open)));
        assert_eq!(m.page,Page::GilEditor);assert!(m.wheel_pointer.is_none());
    }
}

#[cfg(test)]mod character_rename_menu_tests {
    use super::*;
    #[test]fn character_rename_menu_locks_main_identities_and_stages_regular_names(){
        let mut m=Menu::new(true,true);let mut av=AvConfig::default();m.open(Page::Characters);
        let row=|slot,identity,name:&str|CharacterInfo{slot,identity,record:Vec::new(),name:name.into(),race:"Human".into(),job:"Soldier".into(),level:3,exp:18,jp:1};
        m.characters.open(Ok(vec![row(1,2,"Different"),row(2,8,"Another"),row(3,1,"Cooper")]));
        m.selected=6;m.handle(MenuInput::Confirm,&mut av);assert_eq!(m.page,Page::Characters);
        m.characters.selected=1;m.handle(MenuInput::Confirm,&mut av);assert_eq!(m.page,Page::Characters);
        m.characters.selected=2;m.handle(MenuInput::Confirm,&mut av);assert_eq!(m.page,Page::CharacterRename);
        m.selected=1;m.handle(MenuInput::Confirm,&mut av);assert!(m.item_search_active());
        m.item_search_text("Johnnie",false);m.finish_item_search();assert_eq!(m.characters.proposed,"Johnnie");
        assert_eq!(m.characters.current().unwrap().name,"Cooper");
        m.selected=2;m.handle(MenuInput::Confirm,&mut av);assert_eq!(m.page,Page::CharacterRenameConfirm);
        assert_eq!(m.handle(MenuInput::Confirm,&mut av),None);assert_eq!(m.page,Page::CharacterRename);
        m.handle(MenuInput::Confirm,&mut av);m.selected=1;
        assert_eq!(m.handle(MenuInput::Confirm,&mut av),Some(MenuAction::GilEditor(GilAction::RenameApply)));
    }
}

#[cfg(test)]mod abilities_navigation_tests {
    use super::*;
    #[test]fn abilities_back_keeps_character_menu_selection(){
        let mut m=Menu::new(true,true);let mut av=AvConfig::default();m.open(Page::Characters);
        m.selected=7;
        assert_eq!(m.handle(MenuInput::Confirm,&mut av),Some(MenuAction::GilEditor(GilAction::AbilitiesOpen)));
        assert!(m.ability_page_active());assert_eq!(m.handle(MenuInput::Cancel,&mut av),None);
        assert_eq!(m.page,Page::Characters);assert_eq!(m.selected,7);
    }
}
#[cfg(test)]mod levels_navigation_tests{
 use super::*;
 #[test]fn levels_menu_returns_to_its_character_entry(){
  let mut m=Menu::new(true,true);let mut av=AvConfig::default();m.open(Page::Characters);
  m.selected=rows_of(Page::Characters).iter().position(|r|*r==Row::LevelsMenu).unwrap();let selected=m.selected;
  assert_eq!(m.handle(MenuInput::Confirm,&mut av),Some(MenuAction::GilEditor(GilAction::LevelOpen)));assert!(m.level_page_active());
  assert_eq!(m.handle(MenuInput::Cancel,&mut av),None);assert_eq!(m.page,Page::Characters);assert_eq!(m.selected,selected);
 }
}
#[cfg(test)]mod clone_navigation_tests{
 use super::*;
 #[test]fn clone_menu_routes_text_and_returns_to_selected_member(){
  let mut m=Menu::new(true,true);let mut av=AvConfig::default();m.open(Page::Characters);
  m.selected=rows_of(Page::Characters).iter().position(|r|*r==Row::CloneMenu).unwrap();let selected=m.selected;
  assert_eq!(m.handle(MenuInput::Confirm,&mut av),Some(MenuAction::GilEditor(GilAction::CloneOpen)));assert!(m.clone_page_active());
  m.cloning.editing=true;assert!(m.item_search_active());m.item_search_text("CloneX",false);m.finish_item_search();assert_eq!(m.cloning.name,"CloneX");assert!(!m.item_search_active());
  assert_eq!(m.handle(MenuInput::Cancel,&mut av),None);assert_eq!(m.page,Page::Characters);assert_eq!(m.selected,selected);
 }
}
#[cfg(test)]mod shop_stock_menu_tests{use super::*;
 #[test]fn shop_stock_navigation_cycles_resets_and_returns(){let mut m=Menu::new(true,true);let mut av=AvConfig::default();m.open(Page::Editor);m.selected=rows_of(m.page).iter().position(|r|*r==Row::ShopMenu).unwrap();let selected=m.selected;assert_eq!(m.handle(MenuInput::Confirm,&mut av),Some(MenuAction::GilEditor(GilAction::ShopOpen)));assert_eq!(m.page,Page::ShopStock);assert_eq!(m.handle(MenuInput::Right,&mut av),Some(MenuAction::GilEditor(GilAction::ShopChanged)));assert_eq!(m.shop_tier,1);m.handle(MenuInput::Left,&mut av);assert_eq!(m.shop_tier,0);m.shop_tier=3;m.selected=1;m.handle(MenuInput::Confirm,&mut av);assert_eq!(m.shop_tier,0);m.handle(MenuInput::Cancel,&mut av);assert_eq!(m.page,Page::Editor);assert_eq!(m.selected,selected);}
}
#[cfg(test)]mod law_penalties_menu_tests{use super::*;
 #[test]fn law_penalties_toggle_and_back_preserve_hub_selection(){
  let mut m=Menu::new(true,true);let mut av=AvConfig::default();m.open(Page::Editor);m.selected=rows_of(m.page).iter().position(|r|*r==Row::LawMenu).unwrap();let selected=m.selected;
  assert_eq!(m.handle(MenuInput::Confirm,&mut av),Some(MenuAction::GilEditor(GilAction::LawOpen)));assert_eq!(m.page,Page::Laws);
  assert_eq!(m.handle(MenuInput::Right,&mut av),Some(MenuAction::GilEditor(GilAction::LawChanged)));assert_eq!(m.law_mode,1);
  m.handle(MenuInput::Left,&mut av);assert_eq!(m.law_mode,0);m.handle(MenuInput::Cancel,&mut av);assert_eq!(m.page,Page::Editor);assert_eq!(m.selected,selected);
 }
 #[test]fn law_penalties_mouse_uses_visible_value_row(){for(w,h)in[(852,480),(1920,1080)]{let mut m=Menu::new(true,true);let mut av=AvConfig::default();m.open(Page::Laws);let l=page_layout(Page::Laws,w,h);assert_eq!(m.click(l.panel_x+l.pad+20,l.rows_y+2,w,h,true,&mut av),Some(MenuAction::GilEditor(GilAction::LawChanged)));assert_eq!(m.law_mode,1);}}
}

#[cfg(test)]mod roster_sprite_menu_tests{
 use super::*;
 #[test]fn roster_sprite_menu_keyboard_mouse_and_scroll_keep_selection_stable(){
  let mut m=Menu::new(true,true);let mut av=AvConfig::default();m.open(Page::Characters);
  let rows=(0..24).map(|i|CharacterInfo{slot:i+1,identity:1,record:vec![0;0x108],name:format!("Member {}",i+1),race:"Human".into(),job:"Soldier".into(),level:3,exp:0,jp:0}).collect();m.characters.open(Ok(rows));
  m.handle(MenuInput::Down,&mut av);assert_eq!(m.characters.selected,1);m.handle(MenuInput::Right,&mut av);assert_eq!(rows_of(Page::Characters)[m.selected],Row::LevelsMenu);
  assert_eq!(m.handle(MenuInput::Confirm,&mut av),Some(MenuAction::GilEditor(GilAction::LevelOpen)));m.handle(MenuInput::Cancel,&mut av);m.handle(MenuInput::Left,&mut av);assert_eq!(m.selected,0);
  m.click(140,265,900,600,true,&mut av);assert_eq!(m.characters.selected,2);m.scroll(false,&mut av);assert_eq!(m.characters.selected,3);m.hover(140,265,900,600);assert_eq!(m.characters.selected,3);
  for _ in 0..30{m.scroll(false,&mut av);}assert_eq!(m.characters.selected,23);
  let mut px=vec![[128,128,128,255];900*600];roster::draw(&m,&mut px,900,600);assert_ne!(px[0],[128,128,128,255]);
 }
}

#[cfg(test)]mod editor_comfort_right_click_tests{
 use super::*;
 #[test]fn editor_comfort_right_click_preserves_editor_pages_and_pending_stats(){
  let mut m=Menu::new(true,true);let mut av=AvConfig::default();
  for page in[Page::Levels,Page::Abilities,Page::Characters,Page::Editor,Page::Main]{m.page=page;m.levels.overrides[2]=Some(202);assert_eq!(m.click(0,0,852,480,false,&mut av),None);assert_eq!(m.page,page);assert_eq!(m.levels.overrides[2],Some(202));}
 }
}
