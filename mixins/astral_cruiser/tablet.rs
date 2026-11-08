// Engine-independent tablet geometry and hit testing. All rectangles use GUI units.
use super::astral_lang::LangSetting;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Weapons,
    Map,
    Navigation,
    Status,
}

impl Tab {
    pub const ALL: [Self; 4] = [Self::Weapons, Self::Map, Self::Navigation, Self::Status];
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    /// Half-open bounds ensure adjacent controls never both own an edge.
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    pub fn center(self) -> [f32; 2] {
        [self.x + self.w * 0.5, self.y + self.h * 0.5]
    }

    pub fn array(self) -> [f32; 4] {
        [self.x, self.y, self.w, self.h]
    }
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub scale: f32,
    pub panel: Rect,
    pub tabs: [Rect; 4],
    pub body: Rect,
    pub close: Rect,
    pub weapon_list: Rect,
    pub weapon_card: Rect,
    pub weapon_fields: [Rect; 2],
    pub fire: Rect,
    pub map: Rect,
    /// Zoom out, zoom in.
    pub map_zoom: [Rect; 2],
    pub target_mode: Rect,
    pub map_fire: Rect,
    pub map_dash: Rect,
    pub nav_fields: [Rect; 3],
    pub nav_dash: Rect,
    pub waypoint_save: Rect,
    pub waypoint_rows: [Rect; 8],
    pub waypoint_fly: [Rect; 8],
    pub waypoint_delete: [Rect; 8],
    pub portals: [Rect; 3],
    pub sliders: [Rect; 2],
    pub status_call: Rect,
    pub status_board: Rect,
    pub status_seat: Rect,
    pub status_leave: Rect,
    pub status_reset_onboarding: Rect,
    pub status_lang: [Rect; 3],
}

impl Layout {
    pub fn row_height(&self) -> f32 {
        30.0 * self.scale
    }

    /// `scroll` is in canonical GUI units, independent of window size.
    pub fn weapon_row(&self, index: usize, scroll: f32) -> Rect {
        Rect {
            y: self.weapon_list.y + (index as f32 * 30.0 - clean_scroll(scroll)) * self.scale,
            h: self.row_height(),
            ..self.weapon_list
        }
    }
}

/// A centered 440x270 panel, proportionally reduced only when it cannot fit.
pub fn layout(screen_w: f32, screen_h: f32) -> Layout {
    let finite = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
    let (screen_w, screen_h) = (finite(screen_w), finite(screen_h));
    let scale = ((screen_w - 8.0).max(0.0) / 440.0)
        .min((screen_h - 8.0).max(0.0) / 270.0).min(1.0);
    let (left, top) = ((screen_w - 440.0 * scale) * 0.5, (screen_h - 270.0 * scale) * 0.5);
    let rect = |x: f32, y: f32, w: f32, h: f32| Rect {
        x: left + x * scale, y: top + y * scale, w: w * scale, h: h * scale,
    };
    Layout {
        scale,
        panel: rect(0.0, 0.0, 440.0, 270.0),
        tabs: std::array::from_fn(|i| rect(8.0, 43.0 + i as f32 * 43.0, 88.0, 36.0)),
        body: rect(104.0, 42.0, 326.0, 218.0),
        close: rect(418.0, 8.0, 14.0, 16.0),
        weapon_list: rect(104.0, 44.0, 158.0, 210.0),
        weapon_card: rect(270.0, 44.0, 160.0, 210.0),
        weapon_fields: [rect(274.0, 178.0, 72.0, 20.0), rect(354.0, 178.0, 72.0, 20.0)],
        fire: rect(274.0, 218.0, 152.0, 25.0),
        map: rect(104.0, 44.0, 210.0, 210.0),
        map_zoom: [rect(322.0, 44.0, 48.0, 20.0), rect(378.0, 44.0, 48.0, 20.0)],
        target_mode: rect(322.0, 78.0, 104.0, 24.0),
        map_fire: rect(322.0, 192.0, 104.0, 25.0),
        map_dash: rect(322.0, 225.0, 104.0, 25.0),
        nav_fields: std::array::from_fn(|i| rect(104.0 + i as f32 * 77.0, 55.0, 71.0, 20.0)),
        nav_dash: rect(339.0, 55.0, 87.0, 20.0),
        waypoint_save: rect(104.0, 82.0, 322.0, 18.0),
        waypoint_rows: std::array::from_fn(|i| rect(104.0, 104.0 + i as f32 * 15.0, 322.0, 14.0)),
        waypoint_fly: std::array::from_fn(|i| rect(326.0, 104.0 + i as f32 * 15.0, 54.0, 14.0)),
        waypoint_delete: std::array::from_fn(|i| rect(384.0, 104.0 + i as f32 * 15.0, 42.0, 14.0)),
        portals: std::array::from_fn(|i| rect(104.0 + i as f32 * 110.0, 239.0, 102.0, 18.0)),
        sliders: [rect(114.0, 214.0, 302.0, 12.0), rect(114.0, 243.0, 302.0, 12.0)],
        status_call: rect(104.0, 144.0, 156.0, 18.0),
        status_board: rect(268.0, 144.0, 156.0, 18.0),
        status_seat: rect(104.0, 166.0, 156.0, 18.0),
        status_leave: rect(268.0, 166.0, 156.0, 18.0),
        status_reset_onboarding: rect(104.0, 188.0, 114.0, 16.0),
        status_lang: [
            rect(274.0, 188.0, 46.0, 16.0),
            rect(323.0, 188.0, 50.0, 16.0),
            rect(376.0, 188.0, 48.0, 16.0),
        ],
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Hit {
    Tab(Tab),
    WeaponRow(usize),
    Fire,
    Field(usize),
    MapPoint { dx: f64, dz: f64 },
    MapZoom(i32),
    TargetMode,
    Dash,
    WaypointSave,
    WaypointFly(usize),
    WaypointDelete(usize),
    Portal(usize),
    Slider(usize, f32),
    StatusCall,
    StatusBoard,
    StatusSeat,
    StatusLeave,
    StatusResetOnboarding,
    StatusLang(usize),
    Close,
}

#[derive(Clone, Debug)]
pub struct TabletState {
    pub tab: Tab,
    /// Weapon list offset in canonical, unscaled GUI units.
    pub scroll: f32,
    pub selected: usize,
    /// Blocks per map pixel: 1, 2 or 4.
    pub zoom: u8,
    /// Strike X/Z followed by navigation X/Y/Z.
    pub fields: [String; 5],
    pub focus: Option<usize>,
    pub target_dash: bool,
}

impl Default for TabletState {
    fn default() -> Self {
        Self {
            tab: Tab::Weapons, scroll: 0.0, selected: 0, zoom: 1,
            fields: std::array::from_fn(|_| String::new()), focus: None, target_dash: false,
        }
    }
}

fn clean_scroll(scroll: f32) -> f32 {
    if scroll.is_finite() { scroll.max(0.0) } else { 0.0 }
}

/// Only geometry is interpreted here; the caller checks row/waypoint availability.
pub fn hit(l: &Layout, state: &TabletState, x: f32, y: f32) -> Option<Hit> {
    if !l.panel.contains(x, y) { return None; }
    if l.close.contains(x, y) { return Some(Hit::Close); }
    for (i, r) in l.tabs.iter().enumerate() {
        if r.contains(x, y) { return Some(Hit::Tab(Tab::ALL[i])); }
    }
    match state.tab {
        Tab::Weapons => {
            for (i, r) in l.weapon_fields.iter().enumerate() {
                if r.contains(x, y) { return Some(Hit::Field(i)); }
            }
            if l.fire.contains(x, y) { return Some(Hit::Fire); }
            if l.weapon_list.contains(x, y) && l.scale > 0.0 {
                let row = (((y - l.weapon_list.y) / l.scale + clean_scroll(state.scroll)) / 30.0).floor();
                return Some(Hit::WeaponRow(row as usize));
            }
        }
        Tab::Map => {
            if l.map.contains(x, y) {
                let zoom = match state.zoom { 2 => 2.0, 4 => 4.0, _ => 1.0 };
                return Some(Hit::MapPoint {
                    dx: ((x - l.map.center()[0]) / l.map.w) as f64 * 256.0 * zoom,
                    dz: ((y - l.map.center()[1]) / l.map.h) as f64 * 256.0 * zoom,
                });
            }
            for (i, r) in l.map_zoom.iter().enumerate() {
                if r.contains(x, y) { return Some(Hit::MapZoom(if i == 0 { 1 } else { -1 })); }
            }
            if l.target_mode.contains(x, y) { return Some(Hit::TargetMode); }
            if l.map_fire.contains(x, y) { return Some(Hit::Fire); }
            if l.map_dash.contains(x, y) { return Some(Hit::Dash); }
        }
        Tab::Navigation => {
            for (i, r) in l.nav_fields.iter().enumerate() {
                if r.contains(x, y) { return Some(Hit::Field(i + 2)); }
            }
            if l.nav_dash.contains(x, y) { return Some(Hit::Dash); }
            if l.waypoint_save.contains(x, y) { return Some(Hit::WaypointSave); }
            for i in 0..8 {
                if l.waypoint_fly[i].contains(x, y) { return Some(Hit::WaypointFly(i)); }
                if l.waypoint_delete[i].contains(x, y) { return Some(Hit::WaypointDelete(i)); }
            }
            for (i, r) in l.portals.iter().enumerate() {
                if r.contains(x, y) { return Some(Hit::Portal(i)); }
            }
        }
        Tab::Status => {
            if l.status_call.contains(x, y) { return Some(Hit::StatusCall); }
            if l.status_board.contains(x, y) { return Some(Hit::StatusBoard); }
            if l.status_seat.contains(x, y) { return Some(Hit::StatusSeat); }
            if l.status_leave.contains(x, y) { return Some(Hit::StatusLeave); }
            if l.status_reset_onboarding.contains(x, y) { return Some(Hit::StatusResetOnboarding); }
            for (i, r) in l.status_lang.iter().enumerate() {
                if r.contains(x, y) { return Some(Hit::StatusLang(i)); }
            }
            for (i, r) in l.sliders.iter().enumerate() {
                if r.contains(x, y) { return Some(Hit::Slider(i, ((x - r.x) / r.w * 2.0).clamp(0.0, 2.0))); }
            }
        }
    }
    None
}

#[derive(Clone, Debug, PartialEq)]
pub enum TabletAction {
    Select(usize),
    Fire { weapon: usize, x: f64, z: f64 },
    Dash { x: f64, y: f64, z: f64 },
    SaveWaypoint,
    FlyTo(usize),
    DeleteWaypoint(usize),
    OpenPortal(usize),
    SetShake(f32),
    SetVolume(f32),
    SetLang(LangSetting),
    CallShip,
    Board,
    Seat,
    Leave,
    ResetOnboarding,
    Close,
}
