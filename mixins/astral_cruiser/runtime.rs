// The ship in the running game. The server thread owns it: it flies the ship
// against the real world and saves it. The window only sends the pilot's input,
// keeps whoever is aboard on the ship and draws the latest snapshot. The two
// threads meet in the statics below; commands may arrive on either.
use super::astral_guns::{self as guns, Guns};
use super::astral_lang::{self, Lang, LangSetting, resolve, tr};
use super::astral_debris::{self as debris, Debris};
use super::astral_gun_mesh;
use super::astral_effect_events::EffectEvents;
use super::astral_fx_recipe::{self as fx_recipe, Phase};
use super::astral_fx_budget::{distance_share, within_spawn_range};
use horizon_client::gfx::astral_fx_gpu;
use horizon_client::gfx::astral_fx_instance::{FxLight, FxSpawn, Prim};
use super::astral_weapon_wheel as weapon_wheel;
use super::astral_onboarding::{Event as OnboardingEvent, Progress as OnboardingProgress, Step as OnboardingStep};
use super::astral_core::{
    CollisionQuery, Dimension, Flight, HULL_HALF_EXTENTS, NavigationLimits, NavigationTarget,
    ShipInput, ShipPose,
};
use super::astral_dash::{
    Dash, Moment, Stage, areas_in_flight, chunks_of, coordinate, dash_areas, dash_shows_far_terrain,
    far_reach, fog_reach, line_clear, plan, within_world,
};
use super::astral_jump::{Jump, JumpStage, Leap, arrival_area, plan_jump};
use super::astral_tablet::TabletAction;
use super::astral_tablet_draw::{TabletData, WeaponRow};
use super::astral_tutorial::key_name;
use super::astral_passage::{
    ExitKind, Finding, Gate, OPENING, PassStage, Passage, Stride, adoption_ok, chase_behind,
    room_at, scene_radius, through_opening,
};
use super::astral_scene as scene;
use super::astral_pilot::{self as pilot, Arrival, Footing, PilotKeys, ShipIs, Step};
use super::astral_save::{SaveState, Speeds, TabletSave, Waypoint};
use super::astral_weapons::{
    Battery, Body, Cell as ShotCell, Struck, TURRETS, WeaponKind, first_hit, sight_reach,
    within_cone,
};
use super::astral_arsenal::{self, COUNT as WEAPON_COUNT, Group, WaveSound, Weapon};
use super::astral_crater::{self, BATCH, Excavation, bowl, erase, erase_floor, fissures, flat_bowl, grid, rim, shaft, sphere, trench, uplift};
use super::astral_lance::{LANCE_DAMAGE, LANCE_DEPTH, LANCE_RADIUS, LANCE_WIDTH, Lance};
use super::astral_orbit::{Carve, Event as StrikeEvent, Orbit, Refusal, Shown, StrikeView};
use super::astral_scorch::{self, Burn, RingCursor, burn, erode, scan_surface};
use super::astral_shockwave::{self, Wave};
use super::astral_strike_math::{falloff_damage, fire_spots, knockback, surface_y};
use super::astral_clouds::{ease_to, out_of_clouds};
use super::astral_pad::{Pad, pad_box, pad_for, pad_stays};
use horizon_client::gfx::astral_arsenal_fx::{ArsenalFx, arsenal_bolts, scan_bolts};
use horizon_client::gfx::astral_orbit_fx::{StrikeFx, strike_bolts};
use horizon_client::gfx::astral_portal;
use super::lan::LanHost;
use super::perspective::CameraType;
use super::server_travel::LevelTeleport;
use super::{Dim, Game, Screen};
use glam::{DQuat, DVec3, Mat4};
use horizon_client::blocks::Fluid;
use horizon_client::gfx::astral_hull::{Bolt, LightCube, Swirl, arrival_ring, warp_bolts};
use horizon_client::gfx::astral_layout::{RoomKind, ShipLayout};
use horizon_client::gfx::astral_passenger::{Passenger, WalkKeys};
use horizon_client::gfx::astral_portal::Pose as PortalPose;
use horizon_client::gfx::astral_ship::{self, ShipView};
use horizon_client::hud::HudBatch;
use horizon_client::mobs::{Damage, DamageKind};
use horizon_client::world::World;
use horizon_core::geometry::{Aabb, Vec3};
use std::{
    cell::{Cell, RefCell},
    ops::ControlFlow,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicIsize, Ordering},
        Mutex, MutexGuard, OnceLock, PoisonError,
    },
    time::{Duration, Instant},
};
use winit::keyboard::KeyCode;

pub const ID: &str = "astral_cruiser";
// Fixed-size rolling windows: frame counters and server ticks never share a clock.
struct PerfWindow { rows: [[f64; 10]; 120], pending: [f64; 10], next: usize, count: usize }
impl PerfWindow {
    const fn new() -> Self { Self { rows: [[0.0; 10]; 120], pending: [0.0; 10], next: 0, count: 0 } }
    fn finish(&mut self) {
        self.rows[self.next] = self.pending;
        self.pending = [0.0; 10];
        self.next = (self.next + 1) % 120;
        self.count = (self.count + 1).min(120);
    }
    fn report(&self, name: &str, labels: &[&str]) -> String {
        let values: Vec<_> = labels.iter().enumerate().map(|(i, label)| {
            let avg = self.rows.iter().take(self.count).map(|r| r[i]).sum::<f64>() / self.count.max(1) as f64;
            format!("{label}={avg:.3}")
        }).collect();
        format!("Astral {name} last {}: {}", self.count, values.join(" "))
    }
}
static FRAME_PERF: Mutex<PerfWindow> = Mutex::new(PerfWindow::new());
static SERVER_PERF: Mutex<PerfWindow> = Mutex::new(PerfWindow::new());
pub(super) struct PerfTimer { start: Instant, index: usize, server: bool }
impl PerfTimer {
    fn frame(index: usize) -> Self { Self { start: Instant::now(), index, server: false } }
    fn server(index: usize) -> Self { Self { start: Instant::now(), index, server: true } }
}
impl Drop for PerfTimer {
    fn drop(&mut self) {
        let ms = self.start.elapsed().as_secs_f64() * 1000.0;
        lock(if self.server { &SERVER_PERF } else { &FRAME_PERF }).pending[self.index] += ms;
    }
}
pub(super) fn tablet_perf() -> PerfTimer { PerfTimer::frame(5) }
pub const NAME: &str = "Astral Cruiser";
pub const VERSION: &str = "0.2.0";
pub const DESCRIPTION: &str =
    "A huge pilotable cruiser with furnished rooms to walk and a solid hull. N opens the control panel. /astral spawn, board, walk, leave, enter, jump, dash, portal, cruise, remove, status.";
pub const SEAT_KEY: &str = "seat";
pub const SEAT_KEY_DEFAULT: &str = "key.keyboard.v";
pub const PANEL_KEY: &str = "panel";
pub const PANEL_KEY_DEFAULT: &str = "key.keyboard.n";
pub const STOP_KEY: &str = "stop";
pub const STOP_KEY_DEFAULT: &str = "key.keyboard.b";
pub const HELP_KEY: &str = "help";
pub const HELP_KEY_DEFAULT: &str = "key.keyboard.h";
pub const ORBIT_KEY: &str = "orbit";
pub const ORBIT_KEY_DEFAULT: &str = "key.keyboard.o";
pub const HUD_KEY: &str = "hud";
pub const HUD_KEY_DEFAULT: &str = "key.keyboard.j";
pub const WHEEL_KEY: &str = "wheel";
pub const WHEEL_KEY_DEFAULT: &str = "key.keyboard.r";
pub const CALL_KEY: &str = "call";
pub const CALL_KEY_DEFAULT: &str = "key.keyboard.k";
pub const BOARD_KEY: &str = "board";
pub const BOARD_KEY_DEFAULT: &str = "key.keyboard.f";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StrikeHud {
    #[default]
    Full,
    Line,
    Hidden,
}

impl StrikeHud {
    pub fn next(self) -> Self {
        match self {
            Self::Full => Self::Line,
            Self::Line => Self::Hidden,
            Self::Hidden => Self::Full,
        }
    }
}
/// The guns' keys, in `WeaponKind::ALL`'s order: name, labels, default.
pub const GUN_KEYS: [(&str, &str, &str, &str); 4] = [
    ("gun1", "Turrets", "Турели", "key.keyboard.1"),
    ("gun2", "Nose guns", "Носовые орудия", "key.keyboard.2"),
    ("gun3", "Belly beam", "Брюшной луч", "key.keyboard.3"),
    ("gun4", "Main beam", "Главный луч", "key.keyboard.4"),
];

const TICK_SECONDS: f64 = 0.05;
const AUTOSAVE_TICKS: u32 = 100;
/// How far apart the points are that test the hull against the world.
const SURFACE_SPACING: f64 = 2.0;
const FULL_POWER_SPEED: f64 = 64.0;
const STANDING_EYE: f32 = 1.62;
const WORLD_TOP: f64 = 320.0;
/// How close to the pilot's seat counts as being at it.
const SEAT_REACH: f64 = 3.0;
/// How close along the ground to the outer door one must be to come back in.
const DOOR_REACH: f64 = 16.0;
/// How long the server goes on carrying someone who left: their beam may reach
/// it after the order saying so.
const LANDING_TICKS: u32 = 20;
/// The hull's collision around someone outside: how far out, in cells this wide.
const HULL_REACH: f64 = 3.0;
const HULL_CELL: f64 = 0.5;
/// How high over the hull it still carries someone along, as for a jump.
const CARRY_GAP: f64 = 3.0;
/// How far the pilot's camera keeps above the ground.
const CAMERA_CLEARANCE: f64 = 4.0;
/// How far a shot reaches.
const SHOT_RANGE: f64 = 300.0;
/// How far from straight ahead each fixed gun can lay its shot, in radians.
const NOSE_CONE: f64 = 0.35;
const MAIN_CONE: f64 = 0.12;
/// The belly beam hangs under the bow and swings round its forward-down axis.
const BELLY_CONE: f64 = 0.9;
const BELLY_DIP: f64 = 0.6;
/// Radians a turret turns in a tick, and how low its barrels can go.
const TURRET_SPEED: f64 = 0.3;
const TURRET_LOWEST: f64 = -0.1;
/// With blasting on: ticks between the bursts a shot may tear out of the world.
const BLAST_TICKS: u32 = 10;
/// How far the pilot's sight reaches the ground from orbit.
const ORBIT_SIGHT: f64 = 700.0;
/// How fast the ship climbs to orbit, in blocks a tick.
const ORBIT_CLIMB: f64 = 2.0;
/// How much the wheel moves the orbit view, in blocks a step.
const ORBIT_ZOOM_STEP: f64 = 16.0;
/// How far below the ship the orbit view starts.
const ORBIT_START_ZOOM: f64 = 64.0;
/// The game's cloud layer, and how far the pilot's view keeps from it.
const CLOUD_BOTTOM: f64 = 192.33;
const CLOUD_TOP: f64 = 196.33;
const CLOUD_MARGIN: f64 = 3.0;
/// How fast the view leaves the clouds, in blocks a frame.
const CLOUD_EASE: f64 = 1.0;
/// How many fires an ion beam lights round its edge.
const ION_FIRES: usize = 24;
/// How long the window shows a hit, in seconds.
const IMPACT_SECONDS: f32 = 2.0;
/// How much a tick may spend carving craters: CPU time, block writes, and
/// distinct chunk sections (each dirty section is remeshed by the client).
const DIG_MICROS: u64 = 4000;
const DIG_WRITES: usize = 4096;
const DIG_SECTIONS: usize = 32;
/// Writes collected before they go to the level together.
const DIG_RUN: usize = 512;

/// Puts a run of blocks into the level inside one level context, then handles
/// the level's events once. Empties `writes`.
fn set_run(game: &mut Game<'_>, writes: &mut Vec<([i32; 3], horizon_world::registry::BlockStateId)>) {
    if writes.is_empty() {
        return;
    }
    let cells: &[([i32; 3], horizon_world::registry::BlockStateId)] = writes;
    game.with_level(|b, level, ticks| {
        for &([x, y, z], state) in cells {
            b.set_block(level, ticks, horizon_core::position::BlockPos::new(x, y, z), state, DIG_FLAGS);
        }
    });
    if let Err(e) = game.process_level_events() {
        log::error!("astral cruiser: {e}");
    }
    writes.clear();
}

/// Removing blocks: their clients hear, nothing drops, neighbours are not woken.
const DIG_FLAGS: i32 = horizon_world::level::flags::UPDATE_CLIENTS
    | horizon_world::level::flags::UPDATE_KNOWN_SHAPE
    | horizon_world::level::flags::UPDATE_SUPPRESS_DROPS;
/// The speed the lines of a dash start to show at, and how much more they take
/// to show fully.
const RUSH_FROM: f64 = 200.0;
const RUSH_OVER: f64 = 400.0;
/// How much must be loaded around a dash's ends, in chunks: the hull's reach.
const DASH_AREA: i32 = 7;
/// How far apart the points are that test the hull along a whole route.
const ROUTE_SPACING: f64 = 4.0;
/// How far over the top of the world a dash crosses.
const SKY_MARGIN: f64 = 8.0;
/// A dash shorter than this has nowhere to go.
const DASH_LEAST: f64 = 1.0;
/// How far ahead of the ship the way must be clear for a portal to open.
const GATE_GAP: f64 = 30.0;
/// How many ticks a portal takes to open.
const OPENING_TICKS: f32 = 30.0;
/// How near the plane the eye counts as through it.
const THROUGH_MARGIN: f64 = 1.5;
/// How close to the far side's fog a portal's view is drawn, in blocks.
const FAR_SIDE_FOG: f32 = 160.0;
/// How much of the veil goes each tick once the game's own scene has arrived, how
/// many chunks round the player make that so, and how long it may take at most.
const VEIL_FADE: f32 = 0.1;
const SCENE_READY_REACH: i32 = 5;
const VEIL_TICKS: u32 = 200;
/// How fast the pilot's camera closes up or falls back, in blocks a frame.
const CHASE_EASE: f64 = 1.2;
/// How much of each tick the search for a cave may take.
const SEARCH_SLICE: Duration = Duration::from_millis(8);
/// Chunks kept round the ship crossing over the world: none but its own, as the
/// server generates and holds every chunk under the way, about 1 MB a block flown
/// at a width of three.
const DASH_VIEW: i32 = 0;
/// How far behind the middle of the ship its stern is.
const STERN: f64 = 80.0;
/// How fast a jump's light turns, at rest and at full charge, in radians a second.
const WARP_SPIN: (f64, f64) = (0.6, 7.0);
/// Ticks the wave of a ship coming out of a jump lasts, and the white of the flash.
const ARRIVAL_TICKS: f32 = 16.0;
const FLASH_FADE: f32 = 0.09;
const NO_KEYS: PilotKeys = PilotKeys {
    forward: false,
    back: false,
    left: false,
    right: false,
    up: false,
    down: false,
    boost: false,
};

/// What the pilot asks of the ship this tick.
#[derive(Clone, Copy, Debug)]
struct Controls {
    keys: PilotKeys,
    look: f32,
    firing: bool,
    weapon: WeaponKind,
    /// The orbital strike picked, for when the ship is in orbit.
    strike: Weapon,
    /// Where the pilot's line of sight starts, and which way it runs.
    eye: DVec3,
    sight: DVec3,
}

const NO_CONTROLS: Controls = Controls {
    keys: NO_KEYS,
    look: f32::NAN,
    firing: false,
    weapon: WeaponKind::Turrets,
    strike: Weapon::Ion,
    eye: DVec3::ZERO,
    sight: DVec3::Z,
};

/// A shot that left a gun, for the window to show.
#[derive(Clone, Copy, Debug)]
struct ShotEvent {
    kind: WeaponKind,
    mount: usize,
    to: DVec3,
    struck: bool,
}

#[derive(Clone, Copy, Debug)]
enum Order {
    Spawn,
    Board,
    Seat,
    Enter,
    Leave,
    Cruise(Option<(f64, f64)>),
    Go(RoomKind),
    Weapon(WeaponKind),
    NextWeapon(i32),
    Blast(bool),
    /// To these coordinates, or none to call it off.
    Dash(Option<DVec3>),
    /// A portal to there, or none to close it.
    Portal(Option<(Dimension, ExitKind)>),
    PortalGo,
    Jump(DVec3),
    /// From the panel: the speeds as set there, and a jump or a dash.
    Depart { jump: bool, to: DVec3, speeds: Speeds },
    Speeds(Speeds),
    Stop,
    Panel,
    Tutorial(usize),
    ShipEye,
    Remove,
    Status,
    /// Into orbit, or out of it.
    Orbit,
    /// An orbital strike on these X and Z.
    Strike(Weapon, f64, f64, Option<f64>),
}

/// Who the ship carries, as the server knows it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Crew {
    Away,
    /// Outside, on the hull.
    Riding,
    Walking,
    Piloting,
}

impl Crew {
    fn aboard(self) -> bool {
        matches!(self, Self::Walking | Self::Piloting)
    }
}

#[derive(Clone, Copy, Debug)]
enum ServerOrder {
    Spawn(SaveState),
    Remove,
    Crew(Crew),
    Cruise(Option<(f64, f64)>),
    Blast(bool),
    Dash(Option<DVec3>),
    Portal(Option<(Dimension, ExitKind)>),
    PortalGo,
    Jump(DVec3),
    Speeds(Speeds),
    Stop,
    /// The eye has gone through the portal, with the crew member's feet here on
    /// the ship: the game goes over now.
    Cross(DVec3),
    /// Move the player there. A long move is the server's to make: one the
    /// client makes itself is refused if a hill stands in the way.
    Beam([f64; 3]),
    WalkThrough { at: DVec3, to: Dimension },
    Orbit,
    /// A strike on these X and Z, its height found where it lands.
    Strike(Weapon, f64, f64, Option<f64>),
}

/// What the window draws of strikes beyond the one under way: shells in flight
/// and the hits they leave.
#[derive(Clone, Debug)]
enum StrikeShown {
    Launch { id: u64, weapon: Weapon, target: DVec3 },
    Shell { from: DVec3, to: DVec3, seconds: f64, weapon: Weapon },
    Impact { at: DVec3, radius: f64, weapon: Weapon },
    Wave(Wave, WaveSound, Option<(u64, Weapon)>),
    Fx { fx: Shown, at: DVec3, seconds: f64 },
    Chain { points: Vec<DVec3> },
    Seek { from: DVec3, to: DVec3, seconds: f64 },
    Wall { from: DVec3, to: DVec3, seconds: f64 },
}

/// Orbit as the window needs it.
#[derive(Clone, Debug)]
struct OrbitView {
    /// Up at the height it holds; false while climbing.
    up: bool,
    cooldowns: [f64; WEAPON_COUNT],
    aim: Option<DVec3>,
    strikes: Vec<StrikeView>,
    /// Where the lance burns now.
    lance: Option<DVec3>,
}

/// A dash as the pilot sees it. `share` is of the destination loaded, then of
/// the charge gathered.
#[derive(Clone, Copy, Debug)]
struct DashView {
    stage: Stage,
    target: DVec3,
    left: f64,
    share: f64,
}

/// The chunk radius a portal's far side holds: the window's render distance and
/// one more, set by the window each frame.
static SCENE_RADIUS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(14);

/// A portal's area, wide enough that the far side covers the whole view.
fn widened((middle, radius): ((i32, i32), i32)) -> ((i32, i32), i32) {
    (middle, radius.max(SCENE_RADIUS.load(std::sync::atomic::Ordering::Relaxed)))
}

/// The game is changing level (the server's Respawn): the next frame takes the far
/// side over, once the game has made its new, empty world.
static LEVEL_CHANGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(super) fn level_changes() {
    LEVEL_CHANGED.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// The game has just changed level. Through the ship's portal, the far side the
/// window holds becomes the new level at once: its chunks, block entities and
/// meshes, so nothing has to load.
fn adopt(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let Some((dim, area)) = scene::shown() else {
        return;
    };
    let render_distance = game.settings.render_distance as i32;
    if !adoption_ok(dimension_of(dim), dimension_of(game.dim), area, area.0, render_distance, false) {
        log::info!("astral cruiser: the far side does not cover the view; it loads as usual");
        return;
    }
    let meshed = scene::adopt(game);
    CLIENT.with_borrow_mut(|slot| {
        let door = slot.as_mut().and_then(|client| client.doorway.as_mut());
        // The level stands whole already: nothing for the veil to hide.
        if let Some(door) = door.filter(|_| meshed) {
            door.veil = 0.0;
        }
    });
}

/// A portal as the window needs it. `share` is of the far side loaded. A jump
/// gets its arrival ready the same way, `hidden`: no opening stands there.
#[derive(Clone, Copy, Debug)]
struct PassView {
    gate: Gate,
    stage: PassStage,
    area: ((i32, i32), i32),
    near_area: ((i32, i32), i32),
    share: f64,
    hidden: bool,
    /// Which of how many windows the search for a cave is at.
    looking: Option<(usize, usize)>,
}

/// A jump as the pilot sees it. `share` is of the charge, held back by what has
/// still to load at the far end.
#[derive(Clone, Copy, Debug)]
struct JumpView {
    stage: JumpStage,
    target: DVec3,
    share: f64,
}

/// The ship as the server last left it.
#[derive(Clone, Debug)]
struct Published {
    pose: ShipPose,
    velocity: DVec3,
    dimension: Dimension,
    aboard: bool,
    energy: f64,
    heat: f64,
    charge: f64,
    locked: bool,
    dash: Option<DashView>,
    passage: Option<PassView>,
    jump: Option<JumpView>,
    /// How many jumps the ship has made: a change means it has just gone.
    leaps: u32,
    speeds: Speeds,
    /// Crossing over the world: only a little of it is kept loaded.
    light: bool,
    orbit: Option<OrbitView>,
    cooldowns: [f64; WEAPON_COUNT],
    strikes: Vec<StrikeView>,
}

static WAVE_CLOCK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

static ORDERS: Mutex<Vec<Order>> = Mutex::new(Vec::new());
static TABLET_SAVED: Mutex<Option<TabletSave>> = Mutex::new(None);
static TABLET_ORDERS: Mutex<Vec<TabletAction>> = Mutex::new(Vec::new());
static SERVER_ORDERS: Mutex<Vec<ServerOrder>> = Mutex::new(Vec::new());
static CONTROLS: Mutex<Controls> = Mutex::new(NO_CONTROLS);
static SHOTS: Mutex<Vec<ShotEvent>> = Mutex::new(Vec::new());
#[derive(Clone, Copy)]
struct DebrisSample { dim: Dim, cell: [i32; 3], block: u32, center: DVec3, strength: f64, seed: u64 }
static DEBRIS_SHOWN: Mutex<Vec<DebrisSample>> = Mutex::new(Vec::new());
static GUN_READY: Mutex<[bool; 5]> = Mutex::new([false; 5]);
static GUN_MUZZLES: Mutex<[DVec3; 5]> = Mutex::new([DVec3::ZERO; 5]);
static STRIKES_SHOWN: Mutex<Vec<StrikeShown>> = Mutex::new(Vec::new());
static FX_DEBUG: Mutex<Vec<Option<Weapon>>> = Mutex::new(Vec::new());
static FX_PERF_WANTED: AtomicBool = AtomicBool::new(false);
static PUBLISHED: Mutex<Option<Published>> = Mutex::new(None);
/// What the server has to tell the player.
static NOTICES: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// The level a portal keeps running with nobody in it.
static KEPT_LEVEL: Mutex<Option<Dim>> = Mutex::new(None);
static CURRENT_LANG: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn current_lang() -> Lang {
    if CURRENT_LANG.load(Ordering::Relaxed) == 1 {
        Lang::En
    } else {
        Lang::Ru
    }
}

pub fn saved_lang_setting() -> LangSetting {
    lock(&TABLET_SAVED).as_ref().map_or(LangSetting::Auto, |s| s.lang)
}

pub fn update_current_lang(game: &Game<'_>) {
    let setting = saved_lang_setting();
    let resolved = resolve(setting, &game.settings.language);
    CURRENT_LANG.store(if resolved == Lang::En { 1 } else { 0 }, Ordering::Relaxed);
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn notice(text: &str) {
    let tr_text = tr(current_lang(), text);
    lock(&NOTICES).push(format!("Astral Cruiser: {tr_text}"));
}

/// The lowest block of a level. Kept here rather than asked of the game, whose
/// own accessors for this have moved between Nightlies.
fn world_floor(dim: Dim) -> i32 {
    if dim == Dim::Overworld { -64 } else { 0 }
}

/// The first height over a level's blocks.
fn world_top(dim: Dim) -> i32 {
    if dim == Dim::Overworld { 320 } else { 256 }
}

fn layout() -> &'static ShipLayout {
    static LAYOUT: OnceLock<ShipLayout> = OnceLock::new();
    LAYOUT.get_or_init(ShipLayout::cruiser)
}

fn dimension_of(dim: Dim) -> Dimension {
    match dim {
        Dim::Overworld => Dimension::Overworld,
        Dim::Nether => Dimension::Nether,
        Dim::End => Dimension::End,
    }
}

fn dim_of(dimension: Dimension) -> Dim {
    match dimension {
        Dimension::Overworld => Dim::Overworld,
        Dimension::Nether => Dim::Nether,
        Dimension::End => Dim::End,
    }
}

fn stops_things(facts: &horizon_client::blocks::BlockFacts, state: horizon_world::registry::BlockStateId) -> bool {
    let block = facts.get(state);
    !facts.is_air(state) && (block.full_collision || block.legacy_solid)
}

/// The world as the hull meets it. A block stops the ship; so does ground not
/// loaded yet, on the side it is moving toward, as it may hide a mountain.
struct Terrain<'a> {
    world: &'a World,
    facts: &'a horizon_client::blocks::BlockFacts,
    points: &'a [DVec3],
    heading: DVec3,
    /// Nothing stands above this, loaded or not.
    top: f64,
    /// Ground not loaded is in the way on every side.
    strict: bool,
}

impl CollisionQuery for Terrain<'_> {
    fn is_clear(&self, pose: &ShipPose, _half_extents: DVec3) -> bool {
        self.points.iter().all(|&local| {
            let at = pose.local_to_world(local);
            let (x, y, z) = (at.x.floor() as i32, at.y.floor() as i32, at.z.floor() as i32);
            if at.y >= self.top {
                true
            } else if self.world.chunks.contains_key(&(x >> 4, z >> 4)) {
                !stops_things(self.facts, self.world.block(x, y, z))
            } else {
                !self.strict && (at - pose.position).dot(self.heading) <= 0.0
            }
        })
    }
}

/// A jump under way, and the share of what it needs at the far end that has loaded.
struct Jumping {
    jump: Jump,
    area: ((i32, i32), i32),
    loaded: f64,
}

/// A dash under way, the chunk areas it keeps loaded, and the share of them that is.
struct Dashing {
    dash: Dash,
    areas: Vec<((i32, i32), i32)>,
    loaded: f64,
}

struct ServerShip {
    flight: Flight,
    dimension: Dimension,
    crew: Crew,
    /// Autopilot: forward thrust 0 to 1 and a turn in radians a second.
    cruise: Option<(f64, f64)>,
    battery: Battery,
    dash: Option<Dashing>,
    passage: Option<Passage>,
    jump: Option<Jumping>,
    speeds: Speeds,
    leaps: u32,
    orbit: Option<Orbiting>,
    /// Strike cooldowns outlive a stay in orbit.
    strikes: Orbit,
    /// The coordinate target stays loaded through the strike and its digs.
    strike_areas: Vec<(Dimension, ((i32, i32), i32))>,
}

/// The ship in orbit, or climbing to it.
struct Orbiting {
    dimension: Dimension,
    up: bool,
    /// Where the pilot's sight meets the ground.
    aim: Option<DVec3>,
    was_firing: bool,
    lance: Option<Lance>,
    /// A strike asked for by coordinates, waiting for its ground to load.
    queued: Option<(Weapon, f64, f64)>,
    /// How many strikes have gone: each crater's own rim.
    strikes: u64,
}

impl Orbiting {
    fn new(seed: u64, dimension: Dimension) -> Self {
        Self {
            dimension,
            up: false,
            aim: None,
            was_firing: false,
            lance: None,
            queued: None,
            strikes: seed,
        }
    }
}

impl ServerShip {
    fn new(state: SaveState) -> Self {
        Self {
            flight: Flight::new(state.pose),
            dimension: state.dimension,
            crew: if state.aboard { Crew::Walking } else { Crew::Away },
            cruise: None,
            battery: Battery::restored(state.energy, state.heat),
            dash: None,
            passage: None,
            jump: None,
            speeds: state.speeds,
            leaps: 0,
            orbit: None,
            strikes: Orbit::new(0),
            strike_areas: Vec::new(),
        }
    }

    fn input(&self) -> ShipInput {
        if self.crew == Crew::Piloting {
            let controls = *lock(&CONTROLS);
            let rotation = self.flight.snapshot().pose.rotation;
            return pilot::flight_input(&controls.keys, rotation, controls.look);
        }
        let (thrust, yaw_rate) = self.cruise.unwrap_or((0.0, 0.0));
        ShipInput {
            thrust: DVec3::Z * thrust,
            yaw_rate,
            boost: false,
        }
    }
}

struct ServerWave {
    wave: Wave,
    last: f64,
    cursor: RingCursor,
    pending: std::collections::VecDeque<[i32; 2]>,
    dim: Dimension,
}

struct ServerPull {
    at: DVec3,
    radius: f64,
    strength: f64,
    left: f64,
    dim: Dim,
}

struct ScheduledExplosion {
    strike: Option<(u64, Weapon, DVec3)>,
    at: DVec3,
    delay: f64,
    dim: Dim,
}

struct UpliftDig {
    dimension: Dim,
    columns: Vec<([i32; 2], i32)>,
    cursor: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DigKind {
    Air,
    Rim,
    Floor,
}

/// An excavation belongs to one level; unloaded cells get a bounded retry.
struct ClearDig {
    center: [i32; 3],
    radius: f64,
    height: i32,
    columns: std::collections::VecDeque<[i32; 2]>,
    waiting_until: Option<u64>,
    // Snapshot a chunk's upper bounds on first use: removing its first roof
    // block invalidates the engine's entire heightmap (chunk.rs:496).
    // Re-reading it for every column would repeatedly prime all 256 columns.
    tops: std::collections::HashMap<(i32, i32), Box<[i32; 256]>>,
}

struct Dig {
    dimension: Dim,
    excavation: Excavation,
    retry: Vec<([i32; 3], u64)>,
    kind: DigKind,
    strike: Option<(u64, Weapon, DVec3)>,
    rim_surface: std::collections::HashMap<(i32, i32), Option<i32>>,
    clear: Option<ClearDig>,
}

impl Dig {
    fn new(dimension: Dim, cells: Vec<[i32; 3]>) -> Self {
        Self { dimension, excavation: Excavation::sectioned(cells), retry: Vec::new(), kind: DigKind::Air, strike: None, rim_surface: Default::default(), clear: None }
    }

    fn rim(dimension: Dim, cells: Vec<[i32; 3]>) -> Self {
        Self { dimension, excavation: Excavation::sectioned(cells), retry: Vec::new(), kind: DigKind::Rim, strike: None, rim_surface: Default::default(), clear: None }
    }

    fn floor(dimension: Dim, cells: Vec<[i32; 3]>) -> Self {
        Self { dimension, excavation: Excavation::sectioned(cells), retry: Vec::new(), kind: DigKind::Floor, strike: None, rim_surface: Default::default(), clear: None }
    }

    fn clear(dimension: Dim, center: [i32; 3], radius: f64, height: i32) -> Self {
        let mut dig = Self::new(dimension, Vec::new());
        dig.clear = Some(ClearDig {
            center, radius, height,
            columns: astral_crater::clear_columns(center, radius).into(),
            waiting_until: None,
            tops: Default::default(),
        });
        dig
    }

    fn has_columns(&self) -> bool {
        self.clear.as_ref().is_some_and(|c| !c.columns.is_empty())
    }
}

struct Server {
    save_path: PathBuf,
    ship: Option<ServerShip>,
    points: Vec<DVec3>,
    route_points: Vec<DVec3>,
    /// The share of a portal's far side that has loaded.
    far_loaded: f64,
    /// The window says the eye has gone through, with the feet aboard.
    cross_asked: Option<DVec3>,
    /// The connection of the player in the single player game.
    owner: Option<u32>,
    /// What a jump has just moved the crew by.
    leap: Option<DVec3>,
    /// The ship is crossing over the world, and the view distance it left behind.
    light: bool,
    kept_view: Option<i32>,
    since_save: u32,
    landing: u32,
    beam: Option<[f64; 3]>,
    /// Whether shots tear up the world, and how long until the next may.
    blasting: bool,
    blast_wait: u32,
    /// Ground the strikes are taking out, a share each tick.
    digs: Vec<Dig>,
    excavation_cells: usize,
    debris_counts: std::collections::HashMap<u64, usize>,
    uplifts: Vec<UpliftDig>,
    waves: Vec<ServerWave>,
    pulls: Vec<ServerPull>,
    scheduled: Vec<ScheduledExplosion>,
    clock: f64,
}

const EXCAVATION_CAP: usize = 6_000_000;

fn strike_area(kind: Weapon, x: f64, z: f64) -> ((i32, i32), i32) {
    let spec = astral_arsenal::spec(kind);
    let reach = spec.waves.iter().filter(|wave| wave.damaging)
        .fold(spec.reach, |reach, wave| reach.max(wave.radius));
    // Include the crater rim and a chunk for targets near chunk boundaries.
    let radius = (((reach + 2.0) / 16.0).ceil() as i32 + 1).clamp(1, 16);
    (((x.floor() as i32) >> 4, (z.floor() as i32) >> 4), radius)
}

/// Where a gun lays its shot, given the point the pilot is looking at.
fn lay_shot(kind: WeaponKind, from: DVec3, target: DVec3, pose: &ShipPose) -> DVec3 {
    let wanted = (target - from).normalize_or_zero();
    let forward = pose.rotation * DVec3::Z;
    match kind {
        WeaponKind::Turrets => wanted,
        WeaponKind::NosePulse => within_cone(forward, wanted, NOSE_CONE),
        WeaponKind::MainBeam => within_cone(forward, wanted, MAIN_CONE),
        WeaponKind::BellyBeam => {
            let axis = (forward - DVec3::Y * BELLY_DIP).normalize();
            within_cone(axis, wanted, BELLY_CONE)
        }
    }
}

/// The world's edges for a dash, the border apart.
fn dash_limits(dim: Dim) -> NavigationLimits {
    NavigationLimits {
        min_y: f64::from(world_floor(dim)),
        max_y: f64::from(world_top(dim)),
        border: f64::MAX,
        half_extents: HULL_HALF_EXTENTS,
    }
}

/// The height to cross at, over all there is. Under the Nether's roof there is none.
fn open_sky(dim: Dim) -> Option<f64> {
    (dim != Dim::Nether).then(|| f64::from(world_top(dim)) + HULL_HALF_EXTENTS.y + SKY_MARGIN)
}

fn sure_terrain<'a>(
    world: &'a World,
    facts: &'a horizon_client::blocks::BlockFacts,
    points: &'a [DVec3],
    dim: Dim,
) -> Terrain<'a> {
    Terrain {
        world,
        facts,
        points,
        heading: DVec3::ZERO,
        top: f64::from(world_top(dim)),
        strict: true,
    }
}

fn blast_radius(kind: WeaponKind) -> Option<f32> {
    match kind {
        WeaponKind::Turrets => None,
        WeaponKind::NosePulse => Some(2.0),
        WeaponKind::BellyBeam => Some(2.5),
        WeaponKind::MainBeam => Some(5.0),
    }
}

impl Server {
    fn open(world_root: &Path) -> Self {
        let save_path = world_root.join(ID).join("ship.json");
        *lock(&TABLET_SAVED) = Some(TabletSave::load(&save_path.with_file_name("tablet.json"))
            .unwrap_or_else(|e| { log::error!("astral tablet: {e}"); TabletSave::default() }));
        lock(&TABLET_ORDERS).clear();
        let ship = match SaveState::load(&save_path) {
            Ok(saved) => saved.map(ServerShip::new),
            Err(e) => {
                log::error!("astral cruiser: {e}");
                None
            }
        };
        Self {
            save_path,
            ship,
            points: layout().hull_points(SURFACE_SPACING),
            route_points: layout().hull_points(ROUTE_SPACING),
            far_loaded: 0.0,
            cross_asked: None,
            owner: None,
            leap: None,
            light: false,
            kept_view: None,
            since_save: 0,
            landing: 0,
            beam: None,
            blasting: false,
            blast_wait: 0,
            digs: Vec::new(),
            excavation_cells: 0,
            debris_counts: std::collections::HashMap::new(),
            uplifts: Vec::new(),
            waves: Vec::new(),
            pulls: Vec::new(),
            scheduled: Vec::new(),
            clock: 0.0,
        }
    }

    fn carrying(&self) -> bool {
        self.landing > 0 || self.ship.as_ref().is_some_and(|s| s.crew != Crew::Away)
    }

    fn count_excavation(&mut self) {
        self.excavation_cells = self.digs.iter().map(|dig| {
            let columns = dig.clear.as_ref().map_or(0, |clear| {
                clear.columns.len() * (world_top(dig.dimension) - world_floor(dig.dimension)) as usize
            });
            dig.excavation.remaining() + dig.retry.len() + columns
        }).sum::<usize>() + self.uplifts.iter().map(|u| {
            u.columns[u.cursor..].iter().map(|(_, lift)| (*lift).max(0) as usize).sum::<usize>()
        }).sum::<usize>();
    }

    fn save(&mut self) {
        self.since_save = 0;
        let result = match &self.ship {
            Some(ship) => SaveState {
                dimension: ship.dimension,
                pose: match (&ship.passage, &ship.jump) {
                    (Some(passage), _) => passage.safe_pose(),
                    (_, Some(jumping)) => jumping.jump.safe_pose(),
                    _ => ship.flight.snapshot().pose,
                },
                aboard: ship.crew.aboard(),
                energy: ship.battery.energy,
                heat: ship.battery.heat,
                speeds: ship.speeds,
            }
            .save_atomic(&self.save_path),
            None => match std::fs::remove_file(&self.save_path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    Err(super::astral_save::SaveError::Io(e.to_string()))
                }
                _ => Ok(()),
            },
        };
        if let Err(e) = result {
            log::error!("astral cruiser: {e}");
        }
    }

    fn apply(&mut self, order: ServerOrder, game: &mut Game<'_>) {
        match order {
            ServerOrder::Spawn(state) => {
                self.ship = Some(ServerShip::new(state));
                self.save();
                record_onboarding_event(OnboardingEvent::CalledShip);
            }
            ServerOrder::Remove => {
                self.close_passage();
                self.ship = None;
                self.save();
            }
            ServerOrder::Crew(crew) => {
                if let Some(ship) = &mut self.ship {
                    ship.crew = crew;
                }
                if crew == Crew::Away {
                    self.landing = LANDING_TICKS;
                    // An open portal stays: it shuts by itself if the ship drifts off.
                    let waits = self.ship.as_ref().is_some_and(|ship| {
                        ship.passage.as_ref().is_some_and(Passage::can_close)
                    });
                    if !waits {
                        self.stop();
                    }
                }
                self.save();
            }
            ServerOrder::Cruise(cruise) => {
                if let Some(ship) = &mut self.ship {
                    ship.cruise = cruise;
                }
            }
            ServerOrder::Blast(on) => self.blasting = on,
            ServerOrder::Dash(Some(to)) => self.begin_dash(game, to),
            ServerOrder::Dash(None) => {
                if !self.cancel_dash() {
                    notice("no dash under way.");
                }
            }
            ServerOrder::Jump(to) => self.begin_jump(game, to),
            ServerOrder::Speeds(speeds) => {
                if let Some(ship) = &mut self.ship {
                    ship.speeds = speeds;
                }
                self.save();
            }
            ServerOrder::Stop => {
                if !self.stop() {
                    notice("nothing to stop.");
                }
            }
            ServerOrder::Portal(Some((to, kind))) => self.begin_passage(game, to, kind),
            ServerOrder::Portal(None) => {
                let passage = self.ship.as_ref().and_then(|ship| ship.passage.as_ref());
                match passage.map(Passage::can_close) {
                    Some(true) => {
                        let far_side = passage.is_some_and(|p| game.dim == dim_of(p.gate.to));
                        self.close_passage();
                        if far_side {
                            notice("the portal closed behind you.");
                        } else {
                            notice("portal closed.");
                        }
                    }
                    Some(false) => notice("too late: the ship is going through."),
                    None => notice("no portal open."),
                }
            }
            ServerOrder::PortalGo => {
                let passage = self.ship.as_mut().and_then(|ship| ship.passage.as_mut());
                if !passage.is_some_and(Passage::go) {
                    notice("no portal stands open in front of the ship.");
                }
            }
            ServerOrder::Orbit => self.toggle_orbit(game),
            ServerOrder::Strike(kind, x, z, y_opt) => {
                if !x.is_finite() || !z.is_finite() || y_opt.is_some_and(|y| !y.is_finite()) {
                    notice(&refusal_text(Refusal::OutOfRange));
                    return;
                }
                if self.excavation_cells > EXCAVATION_CAP {
                    notice(&refusal_text(Refusal::Busy));
                    return;
                }
                let in_orbit = self.ship.as_ref().is_some_and(|ship| ship.orbit.is_some());
                if in_orbit {
                    if kind == Weapon::Lance {
                        notice("the lance is aimed by hand, from orbit");
                        return;
                    }
                    // Strikes run side by side (Orbit::fire refuses past MAX_ACTIVE);
                    // only one may wait for its area to load at a time.
                    if self.ship.as_ref().is_some_and(|ship| {
                        ship.orbit.as_ref().is_some_and(|orbiting| orbiting.queued.is_some())
                    }) {
                        notice(&refusal_text(Refusal::Busy));
                        return;
                    }
                    if let Some(ship) = &mut self.ship
                        && let Some(orbiting) = &mut ship.orbit
                    {
                        orbiting.queued = Some((kind, x, z));
                        let area = (orbiting.dimension, strike_area(kind, x, z));
                        if !ship.strike_areas.contains(&area) { ship.strike_areas.push(area); }
                    }
                } else {
                    let Some(ship) = &mut self.ship else { return; };
                    let pose = ship.flight.snapshot().pose;
                    let target_y = y_opt.unwrap_or_else(|| {
                        let dim = dim_of(ship.dimension);
                        let (top, bottom) = (world_top(dim), world_floor(dim));
                        ground_at(&game.world, game.res.facts(), x.floor() as i32, z.floor() as i32, top, bottom).map_or(pose.position.y, |y| f64::from(y) + 1.0)
                    });
                    let target = DVec3::new(x, target_y, z);
                    if pose.position.distance(target) > 384.0 {
                        notice(&refusal_text(Refusal::OutOfRange));
                        return;
                    }
                    ship.strikes.select(kind);
                    let b = guns::battery(kind) as usize;
                    let muzzle = pose.local_to_world(lock(&GUN_MUZZLES)[b]);
                    match ship.strikes.fire(target, muzzle, &mut ship.battery.energy) {
                        Ok(()) => {
                            let name = astral_arsenal::display_name(kind, current_lang());
                            let msg = match current_lang() {
                                Lang::Ru => format!("{name} по координатам {x:.0} {z:.0}."),
                                Lang::En => format!("{name} on {x:.0} {z:.0}."),
                            };
                            notice(&msg);
                            let area = (ship.dimension, strike_area(kind, x, z));
                            if !ship.strike_areas.contains(&area) { ship.strike_areas.push(area); }
                            record_onboarding_event(OnboardingEvent::Struck);
                        }
                        Err(why) => notice(&refusal_text(why)),
                    }
                }
            }
            ServerOrder::Cross(feet) => self.cross_asked = Some(feet),
            ServerOrder::Beam(at) => self.beam = Some(at),
            ServerOrder::WalkThrough { at, to } => {
                let can_close = self.ship.as_ref().is_some_and(|ship| {
                    ship.passage.as_ref().is_some_and(Passage::can_close)
                });
                if let Some(conn) = self.owner.filter(|_| can_close) {
                    let passage = self.ship.as_ref().and_then(|ship| ship.passage.as_ref()).unwrap();
                    let dim = dim_of(to);
                    game.level_teleports.push(LevelTeleport {
                        conn,
                        dim,
                        at: at.to_array(),
                        rotation: None,
                    });
                    if to == passage.gate.to {
                        *lock(&KEPT_LEVEL) = Some(dim_of(passage.gate.from));
                    } else {
                        *lock(&KEPT_LEVEL) = Some(dim_of(passage.gate.to));
                    }
                }
            }
        }
    }

    /// Calls off whatever the ship is doing, as far as it still can be.
    fn stop(&mut self) -> bool {
        let Some(ship) = self.ship.as_mut() else {
            return false;
        };
        if let Some(jumping) = &mut ship.jump {
            if jumping.jump.cancel() {
                ship.jump = None;
                notice("jump called off.");
            }
            return true;
        }
        if ship.passage.as_ref().is_some_and(Passage::can_close) {
            let far_side = ship.passage.as_ref().is_some_and(|p| *lock(&KEPT_LEVEL) == Some(dim_of(p.gate.from)));
            self.close_passage();
            if far_side {
                notice("the portal closed behind you.");
            } else {
                notice("portal closed.");
            }
            return true;
        }
        if ship.orbit.take().is_some() {
            ship.strikes.cancel();
            notice("left orbit; the ship holds its height.");
            return true;
        }
        self.cancel_dash()
    }

    /// Starts a jump. While the ship gathers itself the place it goes to loads,
    /// and the window prepares its view of it, so that all of it stands ready
    /// the moment the ship arrives.
    fn begin_jump(&mut self, game: &mut Game<'_>, to: DVec3) {
        let Some(ship) = &mut self.ship else {
            return;
        };
        if ship.dash.is_some() || ship.passage.is_some() || ship.jump.is_some() {
            notice("the ship is busy with a dash, a jump or a portal.");
            return;
        }
        let dim = dim_of(ship.dimension);
        if game.dim != dim {
            notice("the ship is in another dimension.");
            return;
        }
        let pose = ship.flight.snapshot().pose;
        let target = NavigationTarget { dimension: ship.dimension, position: to };
        let [x, z] = game.border.center();
        let middle = DVec3::new(x, 0.0, z);
        let from = ShipPose { position: pose.position - middle, ..pose };
        let bounds = NavigationLimits { border: game.border.size() / 2.0, ..dash_limits(dim) };
        let centred = NavigationTarget { position: to - middle, ..target };
        if let Err(why) = within_world(&from, &centred, &bounds, true) {
            let msg = match current_lang() {
                Lang::Ru => format!("прыжок невозможен: {why}."),
                Lang::En => format!("no jump: {why}."),
            };
            notice(&msg);
            return;
        }
        if pose.position.distance(to) < DASH_LEAST {
            notice("the ship is already there.");
            return;
        }
        ship.cruise = None;
        ship.flight.carry(pose, DVec3::ZERO);
        let jump = Jump::new(pose, target);
        let area = arrival_area(to, jump.rotation);
        ship.jump = Some(Jumping { jump, area, loaded: 0.0 });
    }

    /// A jump's tick. True while it has the ship.
    fn jump(&mut self, game: &mut Game<'_>) -> bool {
        let Some(ship) = &mut self.ship else {
            return false;
        };
        let Some(jumping) = &mut ship.jump else {
            return false;
        };
        let dim = dim_of(ship.dimension);
        if game.dim != dim {
            return true;
        }
        let pose = ship.flight.snapshot().pose;
        let facts = game.res.facts();
        let world = &game.world;
        let mut ended = None;
        if jumping.jump.unprepared() {
            let needed = chunks_of(jumping.area);
            let lit = |chunk: &&(i32, i32)| world.chunks.get(chunk).is_some_and(|c| c.lit);
            let ready = needed.iter().filter(lit).count();
            jumping.loaded = ready as f64 / needed.len().max(1) as f64;
            if ready == needed.len() {
                let terrain = sure_terrain(world, facts, &self.route_points, dim);
                match plan_jump(&pose, &jumping.jump.target, &dash_limits(dim), &terrain) {
                    Ok(exit) => jumping.jump.prepared(exit),
                    Err(why) => {
                        let msg = match current_lang() {
                            Lang::Ru => format!("прыжок невозможен: {why}."),
                            Lang::En => format!("no jump: {why}."),
                        };
                        ended = Some(msg);
                    }
                }
            }
        }
        if ended.is_none() {
            let terrain = sure_terrain(world, facts, &self.points, dim);
            match jumping.jump.tick(TICK_SECONDS) {
                Leap::TimedOut => {
                    ended = Some("jump called off: the destination took too long to load.".to_string());
                }
                Leap::Turned { pose: turned } => {
                    if terrain.is_clear(&turned, HULL_HALF_EXTENTS) {
                        ship.flight.carry(turned, DVec3::ZERO);
                    }
                }
                Leap::Jumped { pose: there, velocity } => {
                    ship.flight.carry(there, velocity);
                    ship.leaps += 1;
                    self.leap = Some(there.position - pose.position);
                }
                Leap::Moved { pose: next, velocity } => ship.flight.carry(next, velocity),
                Leap::Arrived(there) => {
                    ship.flight = Flight::new(there);
                    let [x, y, z] = there.position.to_array();
                    let msg = match current_lang() {
                        Lang::Ru => format!("прибыли в точку {x:.1} {y:.1} {z:.1}."),
                        Lang::En => format!("arrived at {x:.1} {y:.1} {z:.1}."),
                    };
                    ended = Some(msg);
                }
            }
        }
        self.since_save += 1;
        if let Some(message) = ended {
            notice(&message);
            ship.jump = None;
            self.save();
        }
        true
    }

    fn close_passage(&mut self) {
        if let Some(ship) = &mut self.ship {
            ship.passage = None;
        }
        self.far_loaded = 0.0;
        self.cross_asked = None;
        *lock(&KEPT_LEVEL) = None;
    }

    /// Opens a portal ahead of the ship. The far side then has to load before
    /// anything can be seen through it.
    fn begin_passage(&mut self, game: &mut Game<'_>, to: Dimension, kind: ExitKind) {
        let Some(ship) = &mut self.ship else {
            return;
        };
        if ship.dash.is_some() || ship.passage.is_some() || ship.jump.is_some() {
            notice("the ship is busy with a dash, a jump or a portal.");
            return;
        }
        if ship.dimension == to {
            notice("the ship is in that dimension already.");
            return;
        }
        let dim = dim_of(ship.dimension);
        if game.dim != dim {
            notice("the ship is in another dimension.");
            return;
        }
        let pose = ship.flight.snapshot().pose;
        let ahead = pose.position + pose.rotation * DVec3::Z * GATE_GAP;
        let terrain = sure_terrain(&game.world, game.res.facts(), &self.route_points, dim);
        if !line_clear(pose.position, ahead, pose.rotation, HULL_HALF_EXTENTS, &terrain) {
            notice("no room ahead of the nose for a portal.");
            return;
        }
        ship.cruise = None;
        ship.flight.carry(pose, DVec3::ZERO);
        ship.passage = Some(Passage::new(pose, ship.dimension, to, kind));
        self.far_loaded = 0.0;
        self.cross_asked = None;
        *lock(&KEPT_LEVEL) = Some(dim_of(to));
    }

    /// A portal's tick. True while it has the ship.
    fn passage(&mut self, game: &mut Game<'_>) -> bool {
        let Some(ship) = &mut self.ship else {
            return false;
        };
        let Some(passage) = &mut ship.passage else {
            return false;
        };
        let far = dim_of(passage.gate.to);
        let mut ended = None;
        if passage.stage() == PassStage::Preparing {
            let world = if game.dim == far {
                Some(&game.world)
            } else {
                game.levels
                    .parked_mut(far)
                    .map(|parked| &*parked.__horizon_mixins_astral_cruiser_parked_world())
            };
            if let Some(world) = world {
                let needed = chunks_of(passage.exit_area());
                let lit = |chunk: &&(i32, i32)| world.chunks.get(chunk).is_some_and(|c| c.lit);
                let ready = needed.iter().filter(lit).count();
                self.far_loaded = ready as f64 / needed.len().max(1) as f64;
                if ready == needed.len() {
                    let terrain = sure_terrain(world, game.res.facts(), &self.route_points, far);
                    let top = f64::from(world_top(far));
                    let (exit, rotation) = (passage.gate.exit, passage.gate.rotation);
                    if passage.searching().is_some() {
                        // A slice of each tick: a wide look at a whole cave is slow.
                        let started = Instant::now();
                        let more = || started.elapsed() < SEARCH_SLICE;
                        let finding = passage.look_for_cave(&terrain, HULL_HALF_EXTENTS, more);
                        if finding == Finding::NoCave {
                            notice("no room for the hull in the caves here: the portal goes out over the roof.");
                        }
                    } else {
                        match room_at(&terrain, exit, rotation, HULL_HALF_EXTENTS, top) {
                            Some(height) => passage.opened(height),
                            None => ended = Some("no room for the ship on the far side."),
                        }
                    }
                }
            }
        }
        // Until it goes in the ship is free: it flies itself, and may leave.
        if passage.can_close() {
            passage.moved_to(ship.flight.snapshot().pose);
            if passage.too_far() {
                ended = Some(if game.dim == dim_of(passage.gate.to) {
                    "the portal closed behind you."
                } else {
                    "portal closed: the ship went too far from it."
                });
            } else if passage.at_the_mouth() {
                passage.go();
            }
        }
        let asked = self.cross_asked.take();
        let stride = passage.tick(TICK_SECONDS);
        let due = asked.is_some() || stride == Stride::MustCross;
        if due && passage.stage() == PassStage::Going {
            let there = passage.cross();
            let gate = passage.gate;
            ship.dimension = gate.to;
            ship.flight.carry(there, ship.flight.snapshot().velocity);
            if let Some(conn) = self.owner.filter(|_| ship.crew.aboard()) {
                let feet = asked.unwrap_or(layout().pilot_seat());
                game.level_teleports.push(LevelTeleport {
                    conn,
                    dim: far,
                    at: there.local_to_world(feet).to_array(),
                    rotation: None,
                });
            }
        }
        match stride {
            Stride::Held | Stride::MustCross => {}
            Stride::TimedOut => ended = Some("portal closed: the far side took too long to load."),
            Stride::Moved { pose, velocity } => ship.flight.carry(pose, velocity),
            Stride::Done(pose) => {
                ship.flight = Flight::new(pose);
                ended = Some("through the portal.");
            }
        }
        self.since_save += 1;
        let holds = !passage.can_close() && ended.is_none();
        if let Some(message) = ended {
            notice(message);
            self.close_passage();
            self.save();
        }
        holds
    }

    /// Starts a dash, if the place is one the ship can go to at all. It then
    /// waits for the place to load: only then can the way there be judged.
    fn begin_dash(&mut self, game: &mut Game<'_>, to: DVec3) {
        let Some(ship) = &mut self.ship else {
            return;
        };
        if ship.dash.is_some() {
            notice("a dash is already under way. /astral dash cancel stops it.");
            return;
        }
        if ship.passage.is_some() || ship.jump.is_some() {
            notice("the ship is at a portal or gathering a jump.");
            return;
        }
        let dim = dim_of(ship.dimension);
        if game.dim != dim {
            notice("the ship is in another dimension.");
            return;
        }
        let pose = ship.flight.snapshot().pose;
        let target = NavigationTarget { dimension: ship.dimension, position: to };
        // The limits have the world's middle at zero, the border's may not be.
        let [x, z] = game.border.center();
        let middle = DVec3::new(x, 0.0, z);
        let from = ShipPose { position: pose.position - middle, ..pose };
        let bounds = NavigationLimits { border: game.border.size() / 2.0, ..dash_limits(dim) };
        let centred = NavigationTarget { position: to - middle, ..target };
        let sky = open_sky(dim).is_some();
        if let Err(why) = within_world(&from, &centred, &bounds, sky) {
            let msg = match current_lang() {
                Lang::Ru => format!("рывок невозможен: {why}."),
                Lang::En => format!("no dash: {why}."),
            };
            notice(&msg);
            return;
        }
        if pose.position.distance(to) < DASH_LEAST {
            notice("the ship is already there.");
            return;
        }
        let areas = dash_areas(pose.position, to, DASH_AREA, sky);
        ship.cruise = None;
        ship.dash = Some(Dashing { dash: Dash::new(pose, target), areas, loaded: 0.0 });
    }

    /// Calls off a dash, if there is one: at once, or by braking if it is in flight.
    fn cancel_dash(&mut self) -> bool {
        let Some(ship) = self.ship.as_mut() else {
            return false;
        };
        let Some(dashing) = ship.dash.as_mut() else {
            return false;
        };
        if dashing.dash.cancel() {
            ship.dash = None;
            notice("dash called off.");
        } else {
            notice("braking.");
        }
        true
    }

    /// A dash's tick. True while it has the ship, and the pilot only watches.
    fn dash(&mut self, game: &mut Game<'_>) -> bool {
        let Some(ship) = &mut self.ship else {
            return false;
        };
        let Some(dashing) = &mut ship.dash else {
            return false;
        };
        let dim = dim_of(ship.dimension);
        let pose = ship.flight.snapshot().pose;
        let mut ended = None;
        if game.dim != dim {
            ended = Some("dash called off: the ship is in another dimension.".to_string());
        } else {
            let facts = game.res.facts();
            let world = &game.world;
            if dashing.dash.stage() == Stage::Warming {
                let needed: Vec<(i32, i32)> =
                    dashing.areas.iter().flat_map(|&area| chunks_of(area)).collect();
                let ready = needed.iter().filter(|chunk| world.chunks.contains_key(chunk)).count();
                dashing.loaded = ready as f64 / needed.len().max(1) as f64;
                if ready == needed.len() {
                    let terrain = sure_terrain(world, facts, &self.route_points, dim);
                    let target = &dashing.dash.target;
                    match plan(&pose, target, &dash_limits(dim), open_sky(dim), &terrain) {
                        Ok(route) => dashing.dash.warmed(route.at_speed(ship.speeds.dash)),
                        Err(why) => {
                            let msg = match current_lang() {
                                Lang::Ru => format!("рывок невозможен: {why}."),
                                Lang::En => format!("no dash: {why}."),
                            };
                            ended = Some(msg);
                        }
                    }
                }
            }
            if ended.is_none() {
                match dashing.dash.tick(TICK_SECONDS) {
                    Moment::Held => {}
                    Moment::TimedOut => {
                        ended = Some("dash called off: the destination took too long to load.".to_string());
                    }
                    Moment::Moved { pose: next, velocity } => {
                        let terrain = sure_terrain(world, facts, &self.points, dim);
                        if terrain.is_clear(&next, HULL_HALF_EXTENTS) {
                            ship.flight.carry(next, velocity);
                        } else {
                            ship.flight = Flight::new(pose);
                            ended = Some("dash stopped: something is in the way.".to_string());
                        }
                    }
                    Moment::Arrived(there) => {
                        ship.flight = Flight::new(there);
                        let [x, y, z] = there.position.to_array();
                        let msg = match current_lang() {
                            Lang::Ru => format!("прибыли в точку {x:.1} {y:.1} {z:.1}."),
                            Lang::En => format!("arrived at {x:.1} {y:.1} {z:.1}."),
                        };
                        ended = Some(msg);
                    }
                    Moment::Stopped(there) => {
                        ship.flight = Flight::new(there);
                        ended = Some("dash called off.".to_string());
                    }
                }
            }
        }
        self.since_save += 1;
        let high = ship.flight.snapshot().pose.position.y - HULL_HALF_EXTENTS.y;
        let flying = ship.dash.as_ref().is_some_and(|dashing| {
            matches!(dashing.dash.stage(), Stage::Flying | Stage::Braking)
        });
        self.light = flying && high >= f64::from(world_top(dim));
        if let Some(message) = ended {
            notice(&message);
            ship.dash = None;
            self.save();
        }
        true
    }

    /// The guns' tick. Shots go where the world and the pilot's sight say, and
    /// each ends at the first thing in its way, whatever is drawn afterwards.
    fn fire(&mut self, game: &mut Game<'_>) {
        self.blast_wait = self.blast_wait.saturating_sub(1);
        if self.strike(game) {
            return;
        }
        let Some(ship) = &mut self.ship else {
            return;
        };
        let controls = *lock(&CONTROLS);
        let portal_free = ship.passage.as_ref().is_none_or(Passage::can_close);
        let free = ship.dash.is_none() && portal_free && ship.jump.is_none();
        let trigger = ship.crew == Crew::Piloting && controls.firing && free;
        let shots = ship.battery.tick(controls.weapon, trigger, TICK_SECONDS);
        if shots.is_empty() || game.dim != dim_of(ship.dimension) {
            return;
        }
        record_onboarding_event(OnboardingEvent::Fired);
        let pose = ship.flight.snapshot().pose;
        let bodies: Vec<Body> = game
            .mobs
            .iter()
            .filter(|m| m.alive())
            .map(|m| {
                let (bounds, id) = (m.bounds(), m.id);
                let (low, middle) = (bounds.min_position(), bounds.center());
                let min = DVec3::new(low.x, low.y, low.z);
                Body { id, min, max: DVec3::new(middle.x, middle.y, middle.z) * 2.0 - min }
            })
            .collect();
        let facts = game.res.facts();
        let world = &game.world;
        let cell = |x: i32, y: i32, z: i32| {
            if !world.chunks.contains_key(&(x >> 4, z >> 4)) {
                ShotCell::Unloaded
            } else if stops_things(facts, world.block(x, y, z)) {
                ShotCell::Solid
            } else {
                ShotCell::Clear
            }
        };
        let reach = sight_reach(controls.eye, pose.position, SHOT_RANGE);
        let target = first_hit(controls.eye, controls.sight, reach, cell, &bodies).point;
        let mut struck = Vec::new();
        let mut blast = None;
        let mut events = lock(&SHOTS);
        for shot in shots {
            let muzzle = layout().hardpoints[shot.mount].position;
            let from = pose.local_to_world(muzzle);
            let along = lay_shot(shot.kind, from, target, &pose);
            // A turret holds fire rather than shoot through its own ship.
            if shot.kind == WeaponKind::Turrets
                && layout().ray_meets_hull(muzzle, pose.rotation.inverse() * along)
            {
                continue;
            }
            let hit = first_hit(from, along, SHOT_RANGE, cell, &bodies);
            match hit.struck {
                Struck::Body(id) => struck.push((id, shot.damage as f32, from)),
                Struck::Block => blast = blast.or(blast_radius(shot.kind).map(|r| (hit.point, r))),
                Struck::Nothing => {}
            }
            if events.len() < 256 {
                events.push(ShotEvent {
                    kind: shot.kind,
                    mount: shot.mount,
                    to: hit.point,
                    struck: hit.struck != Struck::Nothing,
                });
            }
        }
        drop(events);
        for (id, damage, from) in struck {
            if let Some(mob) = game.mobs.iter_mut().find(|m| m.id == id) {
                let source = Damage {
                    kind: DamageKind::Generic,
                    attacker: None,
                    from: Some(from.to_array()),
                };
                mob.damage(damage, source);
                // A beam hits every tick: no hit may shield a mob from the next.
                mob.invulnerable = 0;
            }
        }
        if let Some((at, radius)) = blast.filter(|_| self.blasting && self.blast_wait == 0) {
            self.blast_wait = BLAST_TICKS;
            let breaks = game.destroy_type("mob_explosion_drop_decay");
            if let Err(e) = game.server_explode(at.to_array(), radius, breaks, false) {
                log::error!("astral cruiser: {e}");
            }
        }
    }

    fn fly(&mut self, game: &mut Game<'_>) {
        self.light = false;
        if self.passage(game) || self.jump(game) || self.dash(game) || self.climb(game) {
            return;
        }
        let Some(ship) = &mut self.ship else {
            return;
        };
        let before = ship.flight.snapshot();
        if super::astral_tablet_draw::holding() {
            ship.flight.carry(before.pose, DVec3::ZERO);
            return;
        }
        let input = ship.input();
        let dim = dim_of(ship.dimension);
        let world = if game.dim == dim {
            Some(&game.world)
        } else {
            game.levels
                .parked_mut(dim)
                .map(|parked| &*parked.__horizon_mixins_astral_cruiser_parked_world())
        };
        let Some(world) = world else {
            return;
        };
        let facts = game.res.facts();
        let standing = Terrain {
            world,
            facts,
            points: &self.points,
            heading: DVec3::ZERO,
            top: f64::from(world_top(dim)),
            strict: false,
        };
        if let Some(raised) =
            pilot::rise_out(&before.pose, HULL_HALF_EXTENTS, WORLD_TOP, &standing)
        {
            ship.flight = Flight::new(raised);
            self.since_save += 1;
            return;
        }
        let idle =
            ship.crew != Crew::Piloting && ship.cruise.is_none() && before.velocity == DVec3::ZERO;
        if idle {
            // Come to rest: this is where the next session finds it.
            if self.since_save > 0 {
                self.save();
            }
            return;
        }
        let terrain = Terrain {
            heading: before.pose.rotation * input.thrust,
            ..standing
        };
        ship.flight.set_cruise(ship.speeds.cruise);
        if let Err(e) = ship.flight.tick(&input, TICK_SECONDS, &terrain) {
            log::error!("astral cruiser: {e}");
        }
        // In orbit the ship flies about at the height it holds.
        if ship.orbit.as_ref().is_some_and(|orbiting| orbiting.up)
            && let Some(height) = orbit_height(dim)
        {
            let now = ship.flight.snapshot();
            let mut pose = now.pose;
            pose.position.y = height;
            ship.flight.carry(pose, DVec3::new(now.velocity.x, 0.0, now.velocity.z));
        }
        self.since_save += 1;
        if self.since_save >= AUTOSAVE_TICKS {
            self.save();
        }
    }

    fn published(&self) -> Option<Published> {
        self.ship.as_ref().map(|ship| {
            let snapshot = ship.flight.snapshot();
            Published {
                pose: snapshot.pose,
                velocity: snapshot.velocity,
                dimension: ship.dimension,
                aboard: ship.crew.aboard(),
                energy: ship.battery.level(),
                heat: ship.battery.heat,
                charge: ship.battery.charge,
                locked: ship.battery.locked,
                dash: ship.dash.as_ref().map(|dashing| {
                    let stage = dashing.dash.stage();
                    DashView {
                        stage,
                        target: dashing.dash.target.position,
                        left: dashing.dash.left(),
                        share: if stage == Stage::Warming { dashing.loaded } else { dashing.dash.charged() },
                    }
                }),
                passage: ship
                    .passage
                    .as_ref()
                    .map(|passage| PassView {
                        gate: passage.gate,
                        stage: passage.stage(),
                        area: widened(passage.exit_area()),
                        near_area: widened(passage.entry_area()),
                        share: self.far_loaded,
                        hidden: false,
                        looking: passage.searching(),
                    })
                    .or(ship.jump.as_ref().map(|jumping| {
                        let at = jumping.jump.target.position;
                        let gate = Gate {
                            from: ship.dimension,
                            to: ship.dimension,
                            entry: at,
                            exit: at,
                            rotation: jumping.jump.rotation,
                        };
                        PassView {
                            gate,
                            stage: PassStage::Preparing,
                            area: jumping.area,
                            near_area: jumping.area,
                            share: jumping.loaded,
                            hidden: true,
                            looking: None,
                        }
                    })),
                jump: ship.jump.as_ref().map(|jumping| JumpView {
                    stage: jumping.jump.stage(),
                    target: jumping.jump.target.position,
                    share: if jumping.jump.unprepared() {
                        jumping.jump.charged().min(jumping.loaded)
                    } else {
                        jumping.jump.charged()
                    },
                }),
                leaps: ship.leaps,
                speeds: ship.speeds,
                light: self.light,
                orbit: ship.orbit.as_ref().map(|orbiting| OrbitView {
                    up: orbiting.up,
                    cooldowns: Weapon::ALL.map(|kind| ship.strikes.cooldown(kind)),
                    aim: orbiting.aim,
                    strikes: ship.strikes.active(),
                    lance: orbiting.lance.map(|lance| lance.at),
                }),
                cooldowns: Weapon::ALL.map(|kind| ship.strikes.cooldown(kind)),
                strikes: ship.strikes.active(),
            }
        })
    }
}

/// The height a ship holds in orbit, over everything; none under the Nether's roof.
fn orbit_height(dim: Dim) -> Option<f64> {
    (dim != Dim::Nether)
        .then(|| f64::from(world_top(dim)) + HULL_HALF_EXTENTS.y + pilot::ORBIT_Y_MARGIN)
}

fn refusal_text(refusal: Refusal) -> String {
    let lang = current_lang();
    match refusal {
        Refusal::Cooling { left } => match lang {
            Lang::Ru => format!("перезарядка: {left:.0} с."),
            Lang::En => format!("still cooling: {left:.0} s."),
        },
        Refusal::NoEnergy => match lang {
            Lang::Ru => "недостаточно энергии.".to_string(),
            Lang::En => "not enough energy.".to_string(),
        },
        Refusal::OutOfRange => match lang {
            Lang::Ru => "слишком далеко: удары действуют на 384 блока от корабля.".to_string(),
            Lang::En => "too far: strikes reach 384 blocks from the ship.".to_string(),
        },
        Refusal::Busy => match lang {
            Lang::Ru => "удар уже наносится.".to_string(),
            Lang::En => "a strike is already under way.".to_string(),
        },
    }
}

/// The top of the ground at a column, if it is loaded.
fn ground_at(world: &World, facts: &horizon_client::blocks::BlockFacts, x: i32, z: i32, top: i32, bottom: i32) -> Option<i32> {
    if !world.chunks.contains_key(&(x >> 4, z >> 4)) {
        return None;
    }
    surface_y(top, bottom, |y| stops_things(facts, world.block(x, y, z)))
}

impl Server {
    /// Into orbit or out of it.
    fn toggle_orbit(&mut self, game: &mut Game<'_>) {
        let Some(ship) = &mut self.ship else {
            notice("no ship.");
            return;
        };
        if ship.orbit.take().is_some() {
            ship.strikes.cancel();
            notice("left orbit; the ship holds its height.");
            return;
        }
        if orbit_height(dim_of(ship.dimension)).is_none() {
            notice("no orbit under the Nether's roof.");
            return;
        }
        if ship.dash.is_some() || ship.jump.is_some() || ship.passage.is_some() {
            notice("not now: a dash, jump or portal is under way.");
            return;
        }
        ship.cruise = None;
        ship.orbit = Some(Orbiting::new(game.ticks ^ u64::from(ship.leaps), ship.dimension));
        notice("climbing to orbit.");
    }

    /// The climb to orbit; true while it lasts, as it moves the ship.
    fn climb(&mut self, game: &mut Game<'_>) -> bool {
        let Some(ship) = &mut self.ship else {
            return false;
        };
        let dim = dim_of(ship.dimension);
        let climbing = ship.orbit.as_ref().is_some_and(|orbiting| !orbiting.up);
        let Some(height) = orbit_height(dim).filter(|_| climbing && game.dim == dim) else {
            return false;
        };
        let mut pose = ship.flight.snapshot().pose;
        pose.position.y = (pose.position.y + ORBIT_CLIMB).min(height);
        let terrain = sure_terrain(&game.world, game.res.facts(), &self.points, dim);
        if !terrain.is_clear(&pose, HULL_HALF_EXTENTS) {
            ship.orbit = None;
            notice("no way up: something stands over the ship.");
            return false;
        }
        ship.flight = Flight::new(pose);
        if pose.position.y >= height
            && let Some(orbiting) = &mut ship.orbit
        {
            orbiting.up = true;
            notice("in orbit. O leaves, 1-4 pick a strike, left mouse fires, the wheel zooms.");
        }
        self.since_save += 1;
        true
    }

    /// The strikes' tick, in place of the guns' while in orbit. False when not in orbit.
    fn strike(&mut self, game: &mut Game<'_>) -> bool {
        let excavation_busy = self.excavation_cells > EXCAVATION_CAP;
        let Some(ship) = &mut self.ship else {
            return false;
        };
        let dim = dim_of(ship.dimension);
        if ship.orbit.as_ref().is_some_and(|orbiting| orbiting.dimension != ship.dimension) {
            ship.strikes.cancel();
            ship.orbit = None;
        }
        if ship.orbit.is_none() {
            if game.dim == dim {
                let pose = ship.flight.snapshot().pose;
                let ship_pos = pose.position - DVec3::Y * 4.0;
                let seed = game.ticks ^ u64::from(ship.leaps);
                let events = ship.strikes.tick(TICK_SECONDS, false, DVec3::ZERO, ship_pos, &mut ship.battery.energy);
                if !events.is_empty() {
                    self.strike_events(game, &events, seed, dim);
                }
            }
            return false;
        }
        let Some(orbiting) = &mut ship.orbit else {
            return false;
        };
        let controls = *lock(&CONTROLS);
        // The guns rest, and their cells charge up again.
        ship.battery.tick(controls.weapon, false, TICK_SECONDS);
        if game.dim != dim {
            return true;
        }
        let pose = ship.flight.snapshot().pose;
        let ship_pos = pose.position - DVec3::Y * 4.0;
        let piloting = ship.crew == Crew::Piloting;
        let facts = game.res.facts();
        let world = &game.world;
        let cell = |x: i32, y: i32, z: i32| {
            if !world.chunks.contains_key(&(x >> 4, z >> 4)) {
                ShotCell::Unloaded
            } else if stops_things(facts, world.block(x, y, z)) {
                ShotCell::Solid
            } else {
                ShotCell::Clear
            }
        };
        orbiting.aim = piloting.then(|| first_hit(controls.eye, controls.sight, ORBIT_SIGHT, cell, &[]))
            .filter(|hit| hit.struck == Struck::Block)
            .map(|hit| hit.point);
        ship.strikes.select(controls.strike);
        let (top, bottom) = (world_top(dim), world_floor(dim));
        if orbiting.up {
            if let Some((kind, x, z)) = orbiting.queued {
                let (bx, bz) = (x.floor() as i32, z.floor() as i32);
                let area = strike_area(kind, x, z);
                let loaded = chunks_of(area).iter().all(|chunk| world.chunks.contains_key(chunk));
                match ground_at(world, facts, bx, bz, top, bottom).filter(|_| loaded && lock(&GUN_READY)[guns::battery(kind) as usize]) {
                    Some(y) => {
                        orbiting.queued = None;
                        ship.strikes.select(kind);
                        let target = DVec3::new(x, f64::from(y) + 1.0, z);
                        let result = if excavation_busy { Err(Refusal::Busy) } else {
                            ship.strikes.fire(target, pose.local_to_world(lock(&GUN_MUZZLES)[guns::battery(ship.strikes.selected()) as usize]), &mut ship.battery.energy)
                        };
                        match result {
                            Ok(()) => {
                                let name = astral_arsenal::display_name(kind, current_lang());
                                let msg = match current_lang() {
                                    Lang::Ru => format!("{name} по координатам {x:.0} {z:.0}."),
                                    Lang::En => format!("{name} on {x:.0} {z:.0}."),
                                };
                                notice(&msg);
                                record_onboarding_event(OnboardingEvent::Struck);
                            }
                            Err(why) => notice(&refusal_text(why)),
                        }
                        ship.strikes.select(controls.strike);
                    }
                    None if (DVec3::new(x, 0.0, z) - DVec3::new(pose.position.x, 0.0, pose.position.z)).length() > orbit_reach() => {
                        orbiting.queued = None;
                        notice(&refusal_text(Refusal::OutOfRange));
                    }
                    None => {}
                }
            } else if piloting && controls.firing && !orbiting.was_firing && lock(&GUN_READY)[guns::battery(controls.strike) as usize] {
                if let Some(target) = orbiting.aim {
                    let result = if excavation_busy { Err(Refusal::Busy) } else {
                        ship.strikes.fire(target, pose.local_to_world(lock(&GUN_MUZZLES)[guns::battery(ship.strikes.selected()) as usize]), &mut ship.battery.energy)
                    };
                    match result {
                        Ok(()) => {
                            let area = (ship.dimension, strike_area(controls.strike, target.x, target.z));
                            if !ship.strike_areas.contains(&area) { ship.strike_areas.push(area); }
                            record_onboarding_event(OnboardingEvent::Struck);
                        }
                        Err(why) => {
                            notice(&refusal_text(why));
                        }
                    }
                }
            }
        }
        orbiting.was_firing = controls.firing && lock(&GUN_READY)[guns::battery(controls.strike) as usize];
        let aim = orbiting.aim.unwrap_or(pose.position);
        let events = ship.strikes.tick(TICK_SECONDS, piloting && controls.firing, aim, ship_pos, &mut ship.battery.energy);
        orbiting.strikes += 1;
        let seed = orbiting.strikes;
        let mut lance_cuts = Vec::new();
        let mut source = None;
        for event in &events {
            if let StrikeEvent::Source { id, weapon, center } = *event { source = Some((id, weapon, center)); }
            if let StrikeEvent::LanceTick { aim, dt } = *event {
                let lance = orbiting.lance.get_or_insert(Lance::start(aim));
                lance_cuts.push((lance.follow(aim, dt), dt, source));
            }
        }
        if !ship.strikes.busy() {
            orbiting.lance = None;
        }
        self.strike_events(game, &events, seed, dim);
        let floor = world_floor(dim);
        for ((from, to), dt, source) in lance_cuts {
            let mut dig = Dig::new(dim, trench(from, to, LANCE_WIDTH, LANCE_DEPTH, floor));
            dig.strike = source;
            self.digs.push(dig);
            if let Some(ship) = &mut self.ship {
                let area = (ship.dimension, strike_area(Weapon::Lance, to.x, to.z));
                if !ship.strike_areas.contains(&area) { ship.strike_areas.push(area); }
            }
            self.strike_events(
                game,
                &[StrikeEvent::Damage { at: to, radius: LANCE_RADIUS, amount: LANCE_DAMAGE * dt, falloff: false }],
                seed,
                dim,
            );
        }
        self.count_excavation();
        true
    }

    /// What a strike does to mobs and ground this tick; what is only seen goes to the window.
    fn strike_events(&mut self, game: &mut Game<'_>, events: &[StrikeEvent], seed: u64, dim: Dim) {
        let floor = world_floor(dim);
        let cells = |at: DVec3| [at.x.floor() as i32, at.y.floor() as i32, at.z.floor() as i32];
        let mut source = None;
        for event in events {
            let first_dig = self.digs.len();
            match *event {
                StrikeEvent::Source { id, weapon, center } => {
                    source = Some((id, weapon, center));
                    lock(&STRIKES_SHOWN).push(StrikeShown::Launch { id, weapon, target: center });
                }
                StrikeEvent::Damage { at, radius, amount, falloff } => {
                    for mob in game.mobs.iter_mut().filter(|m| m.alive()) {
                        let p = mob.position;
                        let distance = DVec3::new(p.x, p.y, p.z).distance(at);
                        let damage = falloff_damage(amount, radius, distance, falloff);
                        if damage > 0.0 {
                            let source = Damage {
                                kind: DamageKind::Generic,
                                attacker: None,
                                from: Some(at.to_array()),
                            };
                            mob.damage(damage as f32, source);
                            mob.invulnerable = 0;
                        }
                    }
                }
                StrikeEvent::Knockback { at, radius, strength } => {
                    for mob in game.mobs.iter_mut().filter(|m| m.alive()) {
                        let p = mob.position;
                        let push = knockback(at, radius, strength, DVec3::new(p.x, p.y, p.z));
                        mob.velocity = Vec3::new(
                            mob.velocity.x + push.x,
                            mob.velocity.y + push.y,
                            mob.velocity.z + push.z,
                        );
                    }
                }
                StrikeEvent::Ignite { at, radius } => {
                    let Some(fire) = game.pack.assets.registry.default_state("minecraft:fire") else {
                        continue;
                    };
                    let air = game.res.facts().air;
                    let top = at.y as i32 + 16;
                    let bottom = at.y as i32 - 32;
                    for (x, z) in fire_spots(at, radius, seed, ION_FIRES) {
                        let ground = ground_at(&game.world, game.res.facts(), x, z, top, bottom);
                        if let Some(y) = ground.filter(|&y| game.world.block(x, y + 1, z) == air)
                            && let Err(e) = game.level_set_with_flags([x, y + 1, z], fire, DIG_FLAGS)
                        {
                            log::error!("astral cruiser: {e}");
                        }
                    }
                }
                StrikeEvent::Carve(Carve::Clear { center, radius, height }) => {
                    self.digs.push(Dig::clear(dim, cells(center), radius, height.round() as i32));
                }
                StrikeEvent::Carve(Carve::Bowl { center, radius, depth }) => {
                    self.digs.push(Dig::new(dim, bowl(cells(center), radius, depth, seed, floor)));
                }
                StrikeEvent::Carve(Carve::Sphere { center, radius }) => {
                    self.digs.push(Dig::new(dim, sphere(cells(center), radius, floor)));
                }
                StrikeEvent::Carve(Carve::FlatBowl { center, radius, depth }) => {
                    self.digs.push(Dig::new(dim, flat_bowl(cells(center), radius, depth, seed, floor)));
                }
                StrikeEvent::Carve(Carve::Shaft { center, radius }) => {
                    let top = center.y.floor() as i32;
                    self.digs.push(Dig::new(dim, shaft(cells(center), radius, top, floor)));
                }
                StrikeEvent::Carve(Carve::Fissures { center, count, length, depth, width }) => {
                    self.digs.push(Dig::new(dim, fissures(cells(center), count, length, depth, width, seed, floor)));
                }
                StrikeEvent::Carve(Carve::Trench { from, to, width, depth }) => {
                    self.digs.push(Dig::new(dim, trench(from, to, width, depth, floor)));
                }
                StrikeEvent::Carve(Carve::Grid { center, half, step, width, depth }) => {
                    self.digs.push(Dig::new(dim, grid(cells(center), half, step, width, depth, floor)));
                }
                StrikeEvent::Carve(Carve::Rim { center, radius, thickness }) => {
                    self.digs.push(Dig::rim(dim, rim(cells(center), radius, thickness, floor)));
                }
                StrikeEvent::Carve(Carve::Uplift { center, rings }) => {
                    let cols = uplift([center.x.floor() as i32, center.z.floor() as i32], &rings, seed);
                    self.uplifts.push(UpliftDig { dimension: dim, columns: cols, cursor: 0 });
                }
                StrikeEvent::Carve(Carve::Erase { center, radius, plane }) => {
                    let center_cell = cells(center);
                    let plane_y = plane.floor() as i32;
                    let top_y = plane_y + 90;
                    self.digs.push(Dig::new(dim, erase(center_cell, radius, plane_y, top_y)));
                    self.digs.push(Dig::floor(dim, erase_floor(center_cell, radius, plane_y)));
                }
                StrikeEvent::Freeze { at, radius } => {
                    let ice = game.pack.assets.registry.default_state("minecraft:ice");
                    let obsidian = game.pack.assets.registry.default_state("minecraft:obsidian");
                    let snow = game.pack.assets.registry.default_state("minecraft:snow");
                    let sphere_cells = sphere(cells(at), radius, floor);
                    for [x, y, z] in sphere_cells {
                        if !game.world.chunks.contains_key(&(x >> 4, z >> 4)) {
                            continue;
                        }
                        let state = game.world.block(x, y, z);
                        let indestructible = game.res.facts().get(state).hardness < 0.0;
                        if indestructible {
                            continue;
                        }
                        let (is_water, is_lava, is_solid) = {
                            let facts = game.res.facts();
                            let block = facts.get(state);
                            let is_water = block.fluid == Fluid::Water || block.name.contains("water");
                            let is_lava = block.fluid == Fluid::Lava || block.name.contains("lava");
                            let is_solid = stops_things(facts, state);
                            (is_water, is_lava, is_solid)
                        };
                        if is_water && let Some(ice_state) = ice {
                            let _ = game.level_set_with_flags([x, y, z], ice_state, DIG_FLAGS);
                        } else if is_lava && let Some(obs_state) = obsidian {
                            let _ = game.level_set_with_flags([x, y, z], obs_state, DIG_FLAGS);
                        } else if is_solid && let Some(snow_state) = snow {
                            let above_air = y + 1 <= world_top(dim) && game.res.facts().is_air(game.world.block(x, y + 1, z));
                            if above_air {
                                let _ = game.level_set_with_flags([x, y + 1, z], snow_state, DIG_FLAGS);
                            }
                        }
                    }
                    for mob in game.mobs.iter_mut().filter(|m| m.alive()) {
                        let p = mob.position;
                        if DVec3::new(p.x, p.y, p.z).distance(at) <= radius {
                            let source = Damage {
                                kind: DamageKind::Generic,
                                attacker: None,
                                from: Some(at.to_array()),
                            };
                            mob.damage(4.0, source);
                            mob.invulnerable = 0;
                            mob.velocity = Vec3::new(mob.velocity.x * 0.25, mob.velocity.y, mob.velocity.z * 0.25);
                        }
                    }
                }
                StrikeEvent::Pull { at, radius, strength, seconds } => {
                    self.pulls.push(ServerPull { at, radius, strength, left: seconds, dim });
                }
                StrikeEvent::Chain { at, radius, hops, damage } => {
                    let mut points = vec![at];
                    let mut cur = at;
                    let mut visited = Vec::new();
                    for _ in 0..hops {
                        let mut nearest = None;
                        let mut nearest_dist = radius;
                        for (idx, mob) in game.mobs.iter().enumerate() {
                            if !mob.alive() || visited.contains(&idx) {
                                continue;
                            }
                            let p = DVec3::new(mob.position.x as f64, mob.position.y as f64, mob.position.z as f64);
                            let d = cur.distance(p);
                            if d <= nearest_dist {
                                nearest_dist = d;
                                nearest = Some((idx, p));
                            }
                        }
                        if let Some((idx, p)) = nearest {
                            visited.push(idx);
                            let mob = &mut game.mobs[idx];
                            let source = Damage {
                                kind: DamageKind::Generic,
                                attacker: None,
                                from: Some(cur.to_array()),
                            };
                            mob.damage(damage as f32, source);
                            mob.invulnerable = 0;
                            points.push(p);
                            cur = p;
                        } else {
                            break;
                        }
                    }
                    lock(&STRIKES_SHOWN).push(StrikeShown::Chain { points });
                }
                StrikeEvent::Seek { from, near, radius, seconds } => {
                    let mut nearest = None;
                    let mut nearest_dist = radius;
                    for mob in game.mobs.iter().filter(|m| m.alive()) {
                        let p = DVec3::new(mob.position.x as f64, mob.position.y as f64, mob.position.z as f64);
                        let d = near.distance(p);
                        if d <= nearest_dist {
                            nearest_dist = d;
                            nearest = Some(p);
                        }
                    }
                    let to = if let Some(target_pos) = nearest {
                        target_pos
                    } else {
                        let angle = (seed as f64 * 1.6180339887).fract() * std::f64::consts::TAU;
                        let dist = 2.0 + (seed as f64 * 0.70710678).fract() * 6.0;
                        let rx = (near.x + dist * angle.cos()).floor() as i32;
                        let rz = (near.z + dist * angle.sin()).floor() as i32;
                        let sy = ground_at(&game.world, game.res.facts(), rx, rz, world_top(dim), world_floor(dim))
                            .unwrap_or(near.y.floor() as i32);
                        DVec3::new(rx as f64 + 0.5, sy as f64 + 1.0, rz as f64 + 0.5)
                    };
                    lock(&STRIKES_SHOWN).push(StrikeShown::Seek { from, to, seconds });
                    self.scheduled.push(ScheduledExplosion { at: to, delay: seconds, dim, strike: source });
                }
                StrikeEvent::Show { fx, at, seconds } => {
                    if fx == Shown::Wall {
                        // The plan already computed the perpendicular trench.
                        for event in events {
                            if let StrikeEvent::Carve(Carve::Trench { from, to, .. }) = *event {
                                lock(&STRIKES_SHOWN).push(StrikeShown::Wall { from, to, seconds });
                                break;
                            }
                        }
                    } else {
                        lock(&STRIKES_SHOWN).push(StrikeShown::Fx { fx, at, seconds });
                    }
                }
                StrikeEvent::Shell { from, to, seconds, weapon } => {
                    lock(&STRIKES_SHOWN).push(StrikeShown::Shell { from, to, seconds, weapon });
                }
                StrikeEvent::Impact { at, weapon, radius } => {
                    lock(&STRIKES_SHOWN).push(StrikeShown::Impact { at, radius, weapon });
                }
                StrikeEvent::Wave { center, radius, strength, damaging, sound } => {
                    let wave = Wave {
                        center,
                        start: self.clock,
                        radius,
                        strength,
                        damaging,
                    };
                    self.waves.push(ServerWave {
                        wave,
                        last: self.clock,
                        cursor: RingCursor::new([center.x.floor() as i32, center.z.floor() as i32]),
                        pending: Default::default(),
                        dim: dimension_of(dim),
                    });
                    lock(&STRIKES_SHOWN).push(StrikeShown::Wave(wave, sound, source.map(|(id, w, _)| (id, w))));
                }
                StrikeEvent::LanceTick { .. } => {}
            }
            for dig in &mut self.digs[first_dig..] { dig.strike = source; }
        }
        self.count_excavation();
    }

    /// Takes out a share of the ground the strikes have marked: no more than a
    /// batch of blocks a tick, keeping ordinary carve sections contiguous.
    fn dig(&mut self, game: &mut Game<'_>) {
        let started = Instant::now();
        let budget = Duration::from_micros(DIG_MICROS);
        let Some(dim) = self.ship.as_ref().map(|ship| dim_of(ship.dimension)) else {
            self.digs.clear();
            self.uplifts.clear();
            self.excavation_cells = 0;
            return;
        };
        // A crossing drops the old level's work rather than carving the new one.
        self.digs.retain(|dig| dig.dimension == dim);
        self.uplifts.retain(|u| u.dimension == dim);
        self.count_excavation();
        if game.dim != dim {
            return;
        }
        let air = game.res.facts().air;
        let magma = game.pack.assets.registry.default_state("minecraft:magma_block").unwrap_or(air);
        let obsidian = game.pack.assets.registry.default_state("minecraft:obsidian").unwrap_or(magma);
        let blackstone = game.pack.assets.registry.default_state("minecraft:blackstone").unwrap_or(obsidian);
        let basalt = game.pack.assets.registry.default_state("minecraft:basalt").unwrap_or(blackstone);
        let smooth_basalt = game.pack.assets.registry.default_state("minecraft:smooth_basalt").unwrap_or(basalt);
        let tinted_glass = game.pack.assets.registry.default_state("minecraft:tinted_glass").unwrap_or(smooth_basalt);
        let sea_lantern = game.pack.assets.registry.default_state("minecraft:sea_lantern").unwrap_or(smooth_basalt);
        let mut samples = Vec::new();
        // CPU deadline also bounds empty/unloaded scans. The write ceiling limits
        // downstream client lighting and meshing even on a very fast server.
        let mut left = BATCH.min(DIG_WRITES);
        let mut changed = 0usize;
        // Writes go to the level in runs: one level context and one round of
        // level events per run instead of per block.
        let mut writes: Vec<([i32; 3], horizon_world::registry::BlockStateId)> = Vec::with_capacity(DIG_RUN);
        let mut sections = astral_crater::SectionBudget::default();
        for dig in &mut self.digs {
            dig.retry.retain(|(_, until)| game.ticks < *until);
        }
        'digs: for dig in &mut self.digs {
            if left == 0 || started.elapsed() >= budget {
                break;
            }
            while left > 0 && started.elapsed() < budget && (!dig.retry.is_empty() || !dig.excavation.is_done() || dig.has_columns()) {
                // Resolve only one column at a time, inside the CPU deadline.
                // The cached heightmap skips all the sky above its highest block.
                if dig.excavation.is_done() && dig.retry.is_empty() {
                    if let Some(clear) = &mut dig.clear {
                        if let Some(&[x, z]) = clear.columns.front() {
                            if !game.world.chunks.contains_key(&(x >> 4, z >> 4)) {
                                let until = *clear.waiting_until.get_or_insert(game.ticks + 200);
                                if game.ticks >= until {
                                    clear.columns.pop_front();
                                    clear.waiting_until = None;
                                    continue;
                                }
                                break;
                            }
                            clear.columns.pop_front();
                            clear.waiting_until = None;
                            let dx = x as f64 - clear.center[0] as f64;
                            let dz = z as f64 - clear.center[2] as f64;
                            let norm = if clear.radius > 0.0 { (dx * dx + dz * dz) / clear.radius.powi(2) } else { 0.0 };
                            // clear_top is an absolute Y; do not add center.y twice.
                            let cone_top = astral_crater::clear_top(norm, clear.center[1], clear.height);
                            let tops = clear.tops.entry((x >> 4, z >> 4)).or_insert_with(|| {
                                let mut tops = Box::new([0; 256]);
                                for lz in 0..16 {
                                    for lx in 0..16 {
                                        tops[(lz * 16 + lx) as usize] = game.world.motion_blocking(
                                            (x & !15) + lx, (z & !15) + lz, |s| {
                                                horizon_world::heightmap::HeightmapType::MotionBlocking
                                                    .is_opaque(horizon_world::worldgen::terrain::TerrainBlockStates::heightmap_state(&game.pack.assets.properties, s))
                                            }).saturating_sub(1);
                                    }
                                }
                                tops
                            });
                            let top = tops[((z & 15) * 16 + (x & 15)) as usize]
                                .min(cone_top).min(world_top(dim) - 1);
                            let bottom = clear.center[1].saturating_add(1).max(world_floor(dim));
                            dig.excavation = Excavation::new((bottom..=top).rev().map(|y| [x, y, z]).collect());
                            if dig.excavation.is_done() { continue; }
                        }
                    }
                }
                let to_take = left.min(256);
                let count = to_take.min(dig.retry.len());
                let mut batch: Vec<_> = dig.retry.drain(..count).collect();
                if batch.len() < to_take {
                    batch.extend(dig.excavation.next_batch(to_take - batch.len()).into_iter().map(|cell| (cell, u64::MAX)));
                }
                if batch.is_empty() {
                    break;
                }
                let mut batch = batch.into_iter();
                while let Some(([x, y, z], until)) = batch.next() {
                    if left == 0 || started.elapsed() >= budget {
                        // next_batch has advanced: preserve every unprocessed cell.
                        dig.retry.push(([x, y, z], until));
                        dig.retry.extend(batch);
                        break;
                    }
                    if !game.world.chunks.contains_key(&(x >> 4, z >> 4)) {
                        dig.retry.push(([x, y, z], if until == u64::MAX { game.ticks + 200 } else { until }));
                        continue;
                    }
                    let state = game.world.block(x, y, z);
                    let facts = game.res.facts();
                    if facts.get(state).hardness < 0.0 {
                        continue;
                    }
                    let target_state = match dig.kind {
                        DigKind::Air => {
                            if state == air || facts.is_air(state) {
                                continue;
                            }
                            air
                        }
                        DigKind::Rim => {
                            let f = facts.get(state);
                            if facts.is_air(state) || f.fluid != Fluid::None || !stops_things(facts, state) { continue; }
                            let surface = *dig.rim_surface.entry((x, z)).or_insert_with(|| {
                                ground_at(&game.world, facts, x, z, world_top(dim), world_floor(dim))
                            });
                            let Some(surface) = surface else { continue; };
                            let depth = surface - y;
                            match astral_scorch::rim_material(x, y, z, depth,
                                !facts.is_air(state) && stops_things(facts, state), f.fluid == Fluid::None) {
                                Some("obsidian") => obsidian,
                                Some("magma_block") => magma,
                                Some("blackstone") => blackstone,
                                Some("basalt") => basalt,
                                Some("smooth_basalt") => smooth_basalt,
                                _ => continue,
                            }
                        }
                        DigKind::Floor => {
                            let is_solid = !facts.is_air(state) && stops_things(facts, state);
                            let is_surface = is_solid || (facts.is_air(state) && y > world_floor(dim) && {
                                let below = game.world.block(x, y - 1, z);
                                !facts.is_air(below) && stops_things(facts, below)
                            });
                            if !is_surface {
                                continue;
                            }
                            let mut h = (x as u32 as u64).wrapping_mul(0xBF58476D1CE4E5B9)
                                ^ (z as u32 as u64).wrapping_mul(0x94D049BB133111EB);
                            h ^= h >> 30;
                            h = h.wrapping_mul(0x9E3779B97F4A7C15);
                            h ^= h >> 27;

                            let is_rare_lantern = (h % 64) == 0;
                            let streak_wobble = ((h % 7) as i32) - 3;
                            let streak_coord = x + 2 * z + streak_wobble;
                            let is_streak = streak_coord.rem_euclid(16) <= 2;

                            if is_rare_lantern {
                                sea_lantern
                            } else if is_streak {
                                tinted_glass
                            } else {
                                smooth_basalt
                            }
                        }
                    };
                    if state == target_state {
                        continue;
                    }
                    // One shared allowance across all queued strikes, including
                    // section-boundary AO neighbours dirtied by World::set.
                    if !sections.admit([x, y, z], DIG_SECTIONS) {
                        dig.retry.push(([x, y, z], until));
                        dig.retry.extend(batch);
                        break 'digs;
                    }
                    writes.push(([x, y, z], target_state));
                    if writes.len() >= DIG_RUN {
                        set_run(game, &mut writes);
                    }
                    changed += 1;
                    if dig.kind == DigKind::Air {
                        if let Some((id, weapon, center)) = dig.strike {
                            let mut seed = id ^ (x as u64).wrapping_mul(73856093) ^ (y as u64).wrapping_mul(19349663) ^ (z as u64).wrapping_mul(83492791);
                            let chance = if weapon == Weapon::Antimatter { 0.25 } else if weapon.is_super() { 0.15 } else if weapon == Weapon::Rod || weapon == Weapon::BlackHole { 0.1 } else { 0.05 };
                            if debris::random(&mut seed) < chance {
                                let strength = if weapon.is_super() { 1.0 } else { 0.6 };
                                if samples.len() < 256 { samples.push(DebrisSample { dim, cell: [x,y,z], block: state.0, center, strength, seed }); }
                                let cap = if weapon.is_super() { 120 } else if weapon == Weapon::Rod || weapon == Weapon::BlackHole { 60 } else { 0 };
                                let used = self.debris_counts.entry(id).or_default();
                                if *used < cap && game.falling.len() < 250 && game.res.facts().get(state).fluid == Fluid::None {
                                    // Same entity representation as LevelEvent::FallingBlock.
                                    let p = Vec3::new(x as f64 + 0.5, y as f64, z as f64 + 0.5);
                                    let away = DVec3::new(p.x - center.x, 0.0, p.z - center.z).normalize_or_zero();
                                    let speed = (0.45 + debris::random(&mut seed) * 0.7) * strength;
                                    game.falling.push(super::FallingBlock {
                                        id: 0, state, start_pos: horizon_core::position::BlockPos::new(x,y,z),
                                        position: p, previous: p,
                                        velocity: Vec3::new(away.x * speed, 0.6 + debris::random(&mut seed) * 0.8, away.z * speed),
                                        age: 1, no_gravity: false, drop_item: false, fall_distance: 0.0,
                                        stuck: Vec3::ZERO, damage: horizon_client::falling_block::Damage::default(), saved: None,
                                    });
                                    *used += 1;
                                }
                            }
                        }
                    }
                    left = left.saturating_sub(1);
                }
            }
        }
        set_run(game, &mut writes);
        {
            let mut queue = lock(&DEBRIS_SHOWN);
            let count = samples.len().min(1024usize.saturating_sub(queue.len()));
            queue.extend(samples.into_iter().take(count));
        }
        self.digs.retain(|dig| !dig.excavation.is_done() || !dig.retry.is_empty() || dig.has_columns());
        if self.digs.is_empty() && self.ship.as_ref().is_none_or(|s| !s.strikes.busy()) { self.debris_counts.clear(); }

        // Process uplifts
        for uplift in &mut self.uplifts {
            if left == 0 {
                break;
            }
            let (top, bottom) = (world_top(dim), world_floor(dim));
            while uplift.cursor < uplift.columns.len() && left > 0 && started.elapsed() < budget {
                let ([x, z], lift) = uplift.columns[uplift.cursor];
                // Finish a column atomically; don't discard its upper cells at
                // the smaller write ceiling. Uplift plans use short columns.
                if left < lift.max(0) as usize { break; }
                uplift.cursor += 1;
                if !game.world.chunks.contains_key(&(x >> 4, z >> 4)) {
                    continue;
                }
                let surf_y = surface_y(top, bottom, |y| stops_things(game.res.facts(), game.world.block(x, y, z)));
                if let Some(surf_y) = surf_y {
                    let state = game.world.block(x, surf_y, z);
                    let cannot_lift = {
                        let facts = game.res.facts();
                        facts.is_air(state) || facts.get(state).hardness < 0.0
                    };
                    if cannot_lift {
                        continue;
                    }
                    for dy in 1..=lift {
                        let ny = surf_y + dy;
                        if ny < top && left > 0 {
                            let cur = game.world.block(x, ny, z);
                            let is_air = game.res.facts().is_air(cur);
                            if is_air {
                                if let Err(e) = game.level_set_with_flags([x, ny, z], state, DIG_FLAGS) {
                                    log::error!("astral cruiser uplift: {e}");
                                } else {
                                    changed += 1;
                                }
                                left = left.saturating_sub(1);
                            }
                        }
                    }
                }
            }
        }
        self.uplifts.retain(|u| u.cursor < u.columns.len());
        self.count_excavation();
        lock(&SERVER_PERF).pending[1] += changed as f64;

        if let Some(ship) = &mut self.ship
            && !ship.strikes.busy()
            && ship.orbit.as_ref().is_none_or(|orbiting| orbiting.queued.is_none())
            && self.digs.is_empty()
            && self.uplifts.is_empty()
            && self.scheduled.is_empty()
            && self.waves.is_empty()
        {
            ship.strike_areas.clear();
        }
    }

    fn tick_pulls(&mut self, game: &mut Game<'_>) {
        let dim = game.dim;
        self.pulls.retain_mut(|pull| {
            if pull.dim != dim {
                return pull.left > 0.0;
            }
            pull.left -= TICK_SECONDS;
            for mob in game.mobs.iter_mut().filter(|m| m.alive()) {
                let p = DVec3::new(mob.position.x as f64, mob.position.y as f64, mob.position.z as f64);
                let d = p.distance(pull.at);
                if d <= pull.radius && d > 0.1 {
                    let dir = (pull.at - p).normalize();
                    let force = pull.strength * (1.0 - d / pull.radius).max(0.1) * 0.4;
                    mob.velocity = Vec3::new(
                        mob.velocity.x + dir.x * force,
                        mob.velocity.y + dir.y * force,
                        mob.velocity.z + dir.z * force,
                    );
                }
            }
            for item in game.dropped.iter_mut() {
                let p = item.bounds().min_position();
                let item_pos = DVec3::new(p.x as f64, p.y as f64, p.z as f64);
                let d = item_pos.distance(pull.at);
                if d <= pull.radius && d > 0.1 {
                    let dir = (pull.at - item_pos).normalize();
                    let force = pull.strength * (1.0 - d / pull.radius).max(0.1) * 0.3;
                    item.velocity = Vec3::new(
                        item.velocity.x + dir.x * force,
                        item.velocity.y + dir.y * force,
                        item.velocity.z + dir.z * force,
                    );
                }
            }
            pull.left > 0.0
        });
    }

    fn tick_scheduled(&mut self, game: &mut Game<'_>) {
        let dim = game.dim;
        let mut due = Vec::new();
        self.scheduled.retain_mut(|s| {
            s.delay -= TICK_SECONDS;
            if s.delay <= 0.0 {
                due.push((s.at, s.dim, s.strike));
                false
            } else {
                true
            }
        });
        let floor = world_floor(dim);
        let cells = |at: DVec3| [at.x.floor() as i32, at.y.floor() as i32, at.z.floor() as i32];
        for (at, s_dim, strike) in due {
            if s_dim == dim {
                let mut dig = Dig::new(dim, sphere(cells(at), 4.0, floor));
                dig.strike = strike;
                self.digs.push(dig);
                self.strike_events(
                    game,
                    &[StrikeEvent::Damage { at, radius: 5.0, amount: 30.0, falloff: true }],
                    game.ticks,
                    dim,
                );
                lock(&STRIKES_SHOWN).push(StrikeShown::Impact { at, radius: 5.0, weapon: Weapon::Missiles });
            }
        }
    }

    fn tick_waves(&mut self, game: &mut Game<'_>) {
        self.clock += TICK_SECONDS;
        WAVE_CLOCK.store(self.clock.to_bits(), std::sync::atomic::Ordering::Relaxed);
        let now = self.clock;
        let dim = game.dim;
        let start_all_scorch = Instant::now();
        let scorch_budget = Duration::from_millis(2);

        for wave_entry in &mut self.waves {
            if dim_of(wave_entry.dim) != dim {
                continue;
            }
            let before = wave_entry.last;
            let wave = &wave_entry.wave;
            let front = wave.front(now);

            // Mobs
            for mob in game.mobs.iter_mut().filter(|m| m.alive()) {
                let p = DVec3::new(mob.position.x as f64, mob.position.y as f64, mob.position.z as f64);
                if wave.crossed(p, before, now) {
                    let dist = p.distance(wave.center);
                    let fo = wave.falloff(dist);
                    let kb_mag = wave.strength * fo * 2.5;
                    let dx = p.x - wave.center.x;
                    let dz = p.z - wave.center.z;
                    let h_len = dx.hypot(dz);
                    let (ux, uz) = if h_len <= 1e-9 { (1.0, 0.0) } else { (dx / h_len, dz / h_len) };
                    mob.velocity = Vec3::new(
                        mob.velocity.x + ux * kb_mag,
                        mob.velocity.y + 0.5 * wave.strength * fo + 0.2,
                        mob.velocity.z + uz * kb_mag,
                    );
                    if wave.damaging {
                        let damage = 60.0 * wave.strength * fo;
                        if damage > 0.0 {
                            let source = Damage {
                                kind: DamageKind::Generic,
                                attacker: None,
                                from: Some(wave.center.to_array()),
                            };
                            mob.damage(damage as f32, source);
                            mob.invulnerable = 0;
                        }
                    }
                }
            }

            // Ship and pilot/crew
            if let Some(ship) = &mut self.ship {
                let pose = ship.flight.snapshot().pose;
                let ship_pos = pose.position;
                if wave.crossed(ship_pos, before, now) {
                    let push = astral_shockwave::ship_push(wave, ship_pos);
                    let cur_vel = ship.flight.snapshot().velocity;
                    ship.flight.carry(pose, cur_vel + push);

                    // A player standing on deck (aboard but not piloting) gets the same push as velocity
                    if ship.crew == Crew::Walking && let Some(player) = game.player.as_mut() {
                        player.velocity = Vec3::new(
                            player.velocity.x + push.x,
                            player.velocity.y + push.y,
                            player.velocity.z + push.z,
                        );
                    }
                }
            }

            // Other players
            if let Some(player) = game.player.as_mut() {
                let p = DVec3::new(player.position.x, player.position.y, player.position.z);
                let piloting = self.ship.as_ref().is_some_and(|s| s.crew == Crew::Piloting);
                let walking = self.ship.as_ref().is_some_and(|s| s.crew == Crew::Walking);
                if !piloting && !walking && wave.crossed(p, before, now) {
                    let dist = p.distance(wave.center);
                    let fo = wave.falloff(dist);
                    let kb_mag = wave.strength * fo * 2.5;
                    let dx = p.x - wave.center.x;
                    let dz = p.z - wave.center.z;
                    let h_len = dx.hypot(dz);
                    let (ux, uz) = if h_len <= 1e-9 { (1.0, 0.0) } else { (dx / h_len, dz / h_len) };
                    player.velocity = Vec3::new(
                        player.velocity.x + ux * kb_mag,
                        player.velocity.y + 0.5 * wave.strength * fo + 0.2,
                        player.velocity.z + uz * kb_mag,
                    );
                }
            }

            // Scorching behind front for damaging waves
            if wave.damaging {
                let _timer = PerfTimer::server(2);
                let mut cols_left = 6000;
                let air = game.res.facts().air;
                let (world_top_y, world_bot_y) = (world_top(dim), world_floor(dim));
                let fire_block = game.pack.assets.registry.default_state("minecraft:fire");
                let dirt_block = game.pack.assets.registry.default_state("minecraft:dirt").unwrap_or(air);
                let coarse_dirt = game.pack.assets.registry.default_state("minecraft:coarse_dirt").unwrap_or(dirt_block);

                while cols_left > 0 && start_all_scorch.elapsed() < scorch_budget {
                    let batch_size = cols_left.min(64);
                    if wave_entry.pending.is_empty() {
                        wave_entry.pending.extend(wave_entry.cursor.advance(front, batch_size, |x, z| {
                            game.world.chunks.contains_key(&(x >> 4, z >> 4))
                        }));
                    }
                    if wave_entry.pending.is_empty() {
                        break;
                    }

                    while let Some([x, z]) = wave_entry.pending.pop_front() {
                        cols_left -= 1;
                        let scan_top = ((wave.center.y + 40.0).floor() as i32).min(world_top_y);
                        let surf = scan_surface(scan_top, world_bot_y, |y| stops_things(game.res.facts(), game.world.block(x, y, z)));
                        if let Some(surf_y) = surf {
                            let mut depth = 0;
                            for y in (surf_y - 12..=surf_y).rev() {
                                if y < world_bot_y {
                                    break;
                                }
                                let state = game.world.block(x, y, z);
                                let (hardness, name) = {
                                    let facts = game.res.facts();
                                    let f = facts.get(state);
                                    (f.hardness, f.name.clone())
                                };
                                if hardness < 0.0 {
                                    break;
                                }
                                let rng = ((x as u64).wrapping_mul(3129871) ^ (z as u64).wrapping_mul(6128731) ^ (y as u64).wrapping_mul(8231)).wrapping_add(game.ticks);
                                let action = if wave.strength >= 0.5 || wave.damaging {
                                    let dist = (x as f64 + 0.5 - wave.center.x).hypot(z as f64 + 0.5 - wave.center.z);
                                    let d_over_r = if wave.radius > 0.0 { dist / wave.radius } else { 1.0 };
                                    erode(&name, d_over_r, depth, wave.damaging, wave.strength, rng)
                                } else {
                                    burn(&name, depth, rng)
                                };
                                match action {
                                    Burn::Air => {
                                        let _ = game.level_set_with_flags([x, y, z], air, DIG_FLAGS);
                                    }
                                    Burn::Dirt => {
                                        let _ = game.level_set_with_flags([x, y, z], dirt_block, DIG_FLAGS);
                                    }
                                    Burn::CoarseDirt => {
                                        let _ = game.level_set_with_flags([x, y, z], coarse_dirt, DIG_FLAGS);
                                    }
                                    Burn::Fire => {
                                        if let Some(fire) = fire_block {
                                            let above_air = y + 1 <= world_top_y && game.res.facts().is_air(game.world.block(x, y + 1, z));
                                            if above_air {
                                                let _ = game.level_set_with_flags([x, y + 1, z], fire, DIG_FLAGS);
                                            }
                                        }
                                    }
                                    Burn::Keep => {
                                        if depth > 0 {
                                            break;
                                        }
                                    }
                                }
                                depth += 1;
                                // Finish this short column atomically. The rest of
                                // the cursor batch remains pending at the deadline.
                            }
                        }
                        if start_all_scorch.elapsed() >= scorch_budget {
                            break;
                        }
                    }
                }
            }

            wave_entry.last = now;
        }

        self.waves.retain(|w| {
            self.ship.as_ref().is_some_and(|ship| ship.dimension == w.dim)
                && (!w.wave.done(self.clock) || (w.wave.damaging
                    && (!w.pending.is_empty() || !w.cursor.exhausted(w.wave.radius))))
        });
    }
}

/// How far strikes reach from the ship, across.
fn orbit_reach() -> f64 {
    super::astral_orbit::RANGE
}

/// The server thread ends with the world: the ship is kept as it stands, and the
/// window must not take it for the next world's.
impl Drop for Server {
    fn drop(&mut self) {
        if self.since_save > 0 {
            self.save();
        }
        *lock(&PUBLISHED) = None;
    }
}

/// Where the player is with regard to the ship.
#[derive(Clone, Copy, Debug, Default)]
enum Mode {
    #[default]
    Outside,
    Piloting,
    Walking {
        passenger: Passenger,
        /// The feet a tick ago, for a smooth view.
        before: DVec3,
    },
}

/// A shot being shown, and the ticks it has left.
#[derive(Clone, Copy, Debug)]
struct Flash {
    shot: ShotEvent,
    ticks: u32,
}

/// A portal as the window shows it.
#[derive(Clone, Copy, Debug)]
struct Doorway {
    gate: Gate,
    /// How far it has opened, 0 to 1.
    opened: f32,
    /// The eye has gone through: the far side now fills the screen, over a game
    /// whose own scene is still arriving.
    through: bool,
    veil: f32,
    veiled: u32,
    /// A jump's arrival, prepared unseen: no opening stands there.
    hidden: bool,
    /// The chunks of the far side being shown.
    area: ((i32, i32), i32),
    flipped: bool,
}

impl Doorway {
    /// The portal's plane in the world the game is drawing.
    fn plane(&self, game: &Game<'_>) -> DVec3 {
        if game.dim == dim_of(self.gate.to) { self.gate.exit } else { self.gate.entry }
    }
}

struct HeatedSurface {
    weapon: Weapon,
    target: DVec3,
    elapsed: f64,
    columns: Vec<(i32, i32)>,
    cursor: usize,
    cells: Vec<(DVec3, u32)>,
}

#[derive(Clone, Debug, Default)]
struct HeightCacheEntry {
    center: DVec3,
    radius: f64,
    min_x: f64,
    min_z: f64,
    step: f64,
    cols: usize,
    rows: usize,
    heights: Vec<f32>,
}

#[derive(Default)]
struct Client {
    mode: Mode,
    previous: Option<Published>,
    current: Option<Published>,
    /// The ship's yaw at the last frame: a walker's view turns with the ship.
    shown_yaw: f64,
    /// Outside, and carried along by the hull under them.
    riding: bool,
    /// Standing at the outer door, which they have already been let out of.
    at_door: bool,
    /// The pilot's view is on, and the player's own view to come back to.
    chasing: bool,
    weapon: WeaponKind,
    /// The pilot's line of sight at the last frame.
    eye: DVec3,
    sight: DVec3,
    /// How much nearer than usual the pilot's camera hangs.
    chase_closer: f64,
    doorway: Option<Doorway>,
    /// How far a gathering jump's light has turned, and how long since a jump
    /// came out, in ticks. The white of the flash over the screen.
    warp_spin: f64,
    arrival: Option<f32>,
    flash: f32,
    /// The player's own far terrain and draw distance, while a dash has them.
    kept_view: Option<(u32, i32)>,
    far_window: bool,
    foot_before: Option<DVec3>,
    seen_dim: Option<Dim>,
    /// Whether this game has looked if the tutorial is due.
    lesson_checked: bool,
    /// The pilot looks out from the ship itself.
    ship_eye: bool,
    /// The game's camera last frame, to see F5 go by.
    seen_camera: Option<CameraType>,
    /// The panel's coordinates, as last used.
    target: Option<DVec3>,
    /// Each turret's turn and elevation, a tick ago and now.
    turrets_before: [[f64; 2]; TURRETS],
    turrets: [[f64; 2]; TURRETS],
    flashes: Vec<Flash>,
    /// The orbital strike picked, and how far down the orbit view hangs.
    strike: Weapon,
    strike_hud: StrikeHud,
    orbit_zoom: f64,
    /// Whether the view has been turned to the ground for this orbit.
    orbit_seen: bool,
    /// Shells and hits being shown, with their age in seconds.
    strikes_shown: Vec<(StrikeShown, f32)>,
    /// Drained only by the window renderer; retained visual events never replay.
    pending_fx: Vec<StrikeShown>,
    effects: EffectEvents,
    fx_launches: Vec<(u64, Weapon, DVec3, f32)>,
    fx_seed: u64,
    fx_lasers: Vec<(ShotEvent, DVec3)>,
    fx_records: Vec<FxSpawn>,
    last_engine_fx: f32,
    ramp_open: f64,
    ramp_frame: Option<Instant>,
    guns: Guns,
    debris: Debris,
    debris_dim: Option<Dim>,
    heated_surfaces: Vec<HeatedSurface>,
    strike_selected: bool,
    /// A walker came through a portal to here: a pad of light is due if there is
    /// no ground under them. Then the pad itself, in its level.
    pad_wanted: Option<(Dim, DVec3)>,
    pad: Option<(Dim, Pad)>,
    /// How far the pilot's camera is lifted or lowered out of the clouds.
    cloud_lift: f64,
    /// The player's clouds setting, while orbit has them off.
    kept_clouds: Option<String>,
    was_attacking: bool,
    pilot_start_pos: Option<DVec3>,
    onboarding: OnboardingProgress,
    onboarding_init: bool,
    player_pos: Option<DVec3>,
    scan_heights: Vec<HeightCacheEntry>,
    last_boarded: Option<Instant>,
}

thread_local! {
    static SERVER: RefCell<Option<Server>> = const { RefCell::new(None) };
    static CLIENT: RefCell<Option<Client>> = const { RefCell::new(None) };
    static COLLECTING: Cell<bool> = const { Cell::new(false) };
    /// The ship as the rain falls on it, on the window's thread.
    static ROOF: Cell<Option<ShipPose>> = const { Cell::new(None) };
    /// The server's chunk radius, while a dash has it.
    static KEPT_RADIUS: Cell<Option<i32>> = const { Cell::new(None) };
    static LIGHT_VIEW: Cell<bool> = const { Cell::new(false) };
    static PILOT_VIEW: Cell<bool> = const { Cell::new(false) };
    static KEPT_TRACKING: Cell<Option<f32>> = const { Cell::new(None) };
}

fn server_tick(game: &mut Game<'_>) {
    if game.world_root.as_os_str().is_empty() {
        return;
    }
    SERVER.with_borrow_mut(|slot| {
        let server = slot.get_or_insert_with(|| Server::open(&game.world_root));
        server.landing = server.landing.saturating_sub(1);
        for order in std::mem::take(&mut *lock(&SERVER_ORDERS)) {
            server.apply(order, game);
        }
        server.tablet_orders();
        server.fly(game);
        server.fire(game);
        server.tick_pulls(game);
        server.tick_scheduled(game);
        { let _timer = PerfTimer::server(3); server.tick_waves(game); }
        { let _timer = PerfTimer::server(0); server.dig(game); }
        lock(&SERVER_PERF).pending[4] = game.falling.len() as f64;
        lock(&SERVER_PERF).finish();
        *lock(&PUBLISHED) = server.published();
    });
    scene::mirror(game);
}

/// Playing, with no panel open: the keys are the pilot's.
fn at_the_controls(game: &Game<'_>) -> bool {
    game.native_playing() && !game.dialog_open()
}

fn held(game: &mut Game<'_>, name: &str) -> bool {
    game.native_mapping(name).is_some_and(|(down, _)| down)
}

fn pilot_keys(game: &mut Game<'_>) -> PilotKeys {
    PilotKeys {
        forward: held(game, "key.forward"),
        back: held(game, "key.back"),
        left: held(game, "key.left"),
        right: held(game, "key.right"),
        up: held(game, "key.jump"),
        down: held(game, "key.sneak"),
        boost: held(game, "key.sprint"),
    }
}

fn walk_keys(game: &mut Game<'_>) -> WalkKeys {
    WalkKeys {
        forward: held(game, "key.forward"),
        back: held(game, "key.back"),
        left: held(game, "key.left"),
        right: held(game, "key.right"),
        jump: held(game, "key.jump"),
        sprint: held(game, "key.sprint"),
    }
}

/// Puts the player at ship points on the last two poses of the ship.
fn place_player(
    game: &mut Game<'_>,
    previous: &ShipPose,
    before: DVec3,
    current: &ShipPose,
    at: DVec3,
) {
    let (from, to) = (previous.local_to_world(before), current.local_to_world(at));
    game.previous_position = Vec3::new(from.x, from.y, from.z);
    if let Some(player) = game.player.as_mut() {
        player.position = Vec3::new(to.x, to.y, to.z);
        player.velocity = Vec3::ZERO;
        player.fall_distance = 0.0;
    }
}

/// The player's new place, and the server told of it.
fn change_mode(client: &mut Client, mode: Mode) {
    let was_piloting = matches!(client.mode, Mode::Piloting);
    let was_aboard = !matches!(client.mode, Mode::Outside);
    let is_aboard = !matches!(mode, Mode::Outside);
    client.mode = mode;
    client.riding = false;
    if !was_aboard && is_aboard {
        client.last_boarded = Some(Instant::now());
        record_onboarding_event(OnboardingEvent::Boarded);
    }
    if !was_piloting && matches!(mode, Mode::Piloting) {
        record_onboarding_event(OnboardingEvent::Seated);
    }
    let crew = match mode {
        Mode::Outside => Crew::Away,
        Mode::Piloting => Crew::Piloting,
        Mode::Walking { .. } => Crew::Walking,
    };
    lock(&SERVER_ORDERS).push(ServerOrder::Crew(crew));
}

fn on_the_bridge() -> Mode {
    let stand = layout().bridge_stand();
    Mode::Walking {
        passenger: Passenger::at(stand),
        before: stand,
    }
}

/// Someone outside and the hull: it carries them, and lifts them out if it flew
/// into them. The server is told, so that the ride does not count as a fall.
fn meet_the_hull(game: &mut Game<'_>, client: &mut Client, previous: &ShipPose, current: &ShipPose) {
    let Some(player) = game.player.as_mut() else {
        return;
    };
    let at = DVec3::new(player.position.x, player.position.y, player.position.z);
    let local = previous.world_to_local(at);
    let spans = layout().hull_spans(local.x, local.z);
    let (moved, riding) = match pilot::footing(local.y, &spans, CARRY_GAP) {
        Footing::Clear => (at, false),
        Footing::Carried => (current.local_to_world(local), true),
        Footing::Buried(top) => {
            player.velocity = Vec3::ZERO;
            (current.local_to_world(DVec3::new(local.x, top, local.z)), true)
        }
    };
    player.position = Vec3::new(moved.x, moved.y, moved.z);
    if riding != client.riding {
        client.riding = riding;
        let crew = if riding { Crew::Riding } else { Crew::Away };
        lock(&SERVER_ORDERS).push(ServerOrder::Crew(crew));
    }
}

fn beam_aboard(ship: &Published, local: DVec3) {
    let at = ship.pose.local_to_world(local).to_array();
    lock(&SERVER_ORDERS).push(ServerOrder::Beam(at));
}

fn firing_sound(kind: WeaponKind) -> (&'static str, f32, f32) {
    match kind {
        WeaponKind::Turrets => ("minecraft:entity.blaze.shoot", 0.5, 1.9),
        WeaponKind::NosePulse => ("minecraft:entity.firework_rocket.blast", 1.0, 0.7),
        WeaponKind::BellyBeam => ("minecraft:block.beacon.ambient", 1.0, 1.6),
        WeaponKind::MainBeam => ("minecraft:block.conduit.ambient.short", 2.0, 0.6),
    }
}

/// How a shot looks: how long it stays, its width, its flash and its colour.
fn shot_look(kind: WeaponKind) -> (u32, f32, f32, [f32; 3]) {
    match kind {
        WeaponKind::Turrets => (3, 0.22, 1.3, [0.2, 1.0, 0.72]),
        WeaponKind::NosePulse => (4, 0.42, 2.4, [1.0, 0.68, 0.2]),
        WeaponKind::BellyBeam => (2, 1.1, 3.6, [0.3, 1.0, 0.42]),
        WeaponKind::MainBeam => (2, 2.6, 7.0, [0.5, 0.8, 1.0]),
    }
}

/// New shots start to show, and old ones fade.
fn show_shots(game: &mut Game<'_>, client: &mut Client, pose: &ShipPose) {
    client.flashes.retain_mut(|flash| {
        flash.ticks -= 1;
        flash.ticks > 0
    });
    for shot in std::mem::take(&mut *lock(&SHOTS)) {
        record_onboarding_event(OnboardingEvent::Fired);
        let (event, volume, pitch) = firing_sound(shot.kind);
        let at = pose.local_to_world(layout().hardpoints[shot.mount].position);
        game.audio.play(event, Some(at.to_array()), volume, pitch);
        client.fx_lasers.push((shot, at));
        client.flashes.push(Flash { shot, ticks: shot_look(shot.kind).0 });
    }
}

/// The turrets swing toward what the pilot looks at, and to rest ahead otherwise.
fn aim_turrets(client: &mut Client, pose: &ShipPose) {
    client.turrets_before = client.turrets;
    let piloting = matches!(client.mode, Mode::Piloting);
    let reach = sight_reach(client.eye, pose.position, SHOT_RANGE);
    let target = pose.world_to_local(client.eye + client.sight * reach);
    for (index, turret) in client.turrets.iter_mut().enumerate() {
        let wanted = if piloting {
            pilot::turret_angles(target, layout().hardpoints[index].position)
        } else {
            [0.0, 0.0]
        };
        turret[0] = pilot::ease_angle(turret[0], wanted[0], TURRET_SPEED);
        turret[1] = pilot::ease_angle(turret[1], wanted[1].max(TURRET_LOWEST), TURRET_SPEED);
    }
}

/// This frame's light from the guns.
fn bolts(client: &Client, ship: &Published, pose: &ShipPose) -> Vec<Bolt> {
    let mut bolts: Vec<Bolt> = client
        .flashes
        .iter()
        .map(|flash| {
            let (ticks, width, burst, [r, g, b]) = shot_look(flash.shot.kind);
            let fade = flash.ticks as f32 / ticks as f32;
            Bolt {
                from: pose.local_to_world(layout().hardpoints[flash.shot.mount].position),
                to: flash.shot.to,
                width,
                flash: if flash.shot.struck { burst } else { 0.0 },
                color: [r, g, b, fade],
            }
        })
        .collect();
    if ship.charge > 0.0 {
        let nose = pose.local_to_world(layout().hardpoints[11].position);
        bolts.push(Bolt {
            from: nose,
            to: nose,
            width: 0.0,
            flash: 2.0 + 7.0 * ship.charge as f32,
            color: [0.5, 0.8, 1.0, ship.charge as f32],
        });
    }
    let rush = ((ship.velocity.length() - RUSH_FROM) / RUSH_OVER).clamp(0.0, 1.0) as f32;
    let [r, g, b] = if ship.jump.is_some() { [0.8, 0.4, 1.0] } else { [0.75, 0.88, 1.0] };
    bolts.extend(pilot::streaks(pose.position, ship.velocity).into_iter().map(|(from, to)| Bolt {
        from,
        to,
        width: 0.25,
        flash: 0.0,
        color: [r, g, b, 0.6 * rush],
    }));
    bolts
}

/// What orbital strikes show this frame: the aim, the strike under way, and the
/// shells and hits the server has told of.
fn strike_fx(client: &Client, ship: &Published, pose: &ShipPose) -> Vec<StrikeFx> {
    let mut fx = Vec::new();
    let orbit = ship.orbit.as_ref().filter(|orbit| orbit.up);
    let muzzle = |w| pose.local_to_world(client.guns.muzzle(w));
    let charge = ship
        .strikes
        .iter()
        .find(|strike| strike.weapon == Weapon::Ion)
        .map_or(0.0, |strike| (strike.elapsed / 3.0).min(1.0) as f32);
    if let Some(at) = orbit.and_then(|orbit| orbit.aim).filter(|_| matches!(client.mode, Mode::Piloting)) {
        let radius = super::astral_orbit::spec(client.strike).reach;
        fx.push(StrikeFx::Aim { at, radius, charge });
    }
    for strike in &ship.strikes {
        match strike.weapon {
            Weapon::Ion if strike.target.is_some() && strike.elapsed >= 3.0 => {
                let to = strike.target.unwrap();
                let age = ((strike.elapsed - 3.0) / 4.0).min(1.0) as f32;
                fx.push(StrikeFx::IonBeam { from: muzzle(Weapon::Ion), to, age });
            }
            Weapon::Ion if strike.target.is_some() => {
                let at = strike.target.unwrap();
                fx.push(StrikeFx::Aim { at, radius: 16.0, charge });
            }
            Weapon::Rod if strike.target.is_some() => {
                let to = strike.target.unwrap();
                let progress = (strike.elapsed / 2.5).min(1.0) as f32;
                fx.push(StrikeFx::Rod { from: muzzle(Weapon::Rod), to, progress });
            }
            _ => {}
        }
    }
    if let Some(to) = orbit.and_then(|orbit| orbit.lance) {
        fx.push(StrikeFx::Lance { from: muzzle(Weapon::Lance), to });
    }
    for (shown, age) in &client.strikes_shown {
        match shown {
            StrikeShown::Shell { from, to, seconds, weapon } if *weapon != Weapon::Meteors => {
                fx.push(StrikeFx::Shell { from: *from, to: *to, progress: (*age / *seconds as f32).min(1.0) });
            }
            StrikeShown::Impact { at, radius, .. } => {
                fx.push(StrikeFx::Impact { at: *at, radius: *radius, age: (*age / IMPACT_SECONDS).min(1.0) });
            }
            _ => {}
        }
    }
    fx
}

/// Maps active strikes and shockwaves to light effects (ArsenalFx).
fn arsenal_fx_list(client: &Client, belly_muzzle: DVec3) -> Vec<ArsenalFx> {
    let mut out = Vec::new();
    let wave_clock = f64::from_bits(WAVE_CLOCK.load(std::sync::atomic::Ordering::Relaxed));

    for (shown, age) in &client.strikes_shown {
        match shown {
            StrikeShown::Fx { fx, at, seconds } => {
                let age_frac = (*age as f64 / *seconds).clamp(0.0, 1.0) as f32;
                match fx {
                    Shown::Mushroom | Shown::Pillar | Shown::BlackHole | Shown::Frost => {}
                    Shown::Quake => {
                        let seed = at.x.to_bits() ^ (at.z.to_bits() << 1) ^ at.y.to_bits();
                        out.push(ArsenalFx::Quake {
                            at: *at,
                            length: 150.0,
                            seed,
                            age: age_frac,
                        });
                    }
                    Shown::Wall => {}
                    Shown::GridBeams => {
                        out.push(ArsenalFx::Grid {
                            at: *at,
                            half: 30.0,
                            step: 12.0,
                            age: age_frac,
                        });
                    }
                    Shown::Drill => {
                        out.push(ArsenalFx::Drill {
                            from: belly_muzzle,
                            to: *at,
                            age: age_frac,
                        });
                    }
                    Shown::Scan => {}
                    Shown::Supernova => {
                        if (*age as f64) < astral_arsenal::spec(Weapon::Supernova).charge {
                            out.push(ArsenalFx::Supernova { at: *at, age: age_frac });
                        }
                    }
                    Shown::Erasure => {
                        if (*age as f64) < astral_arsenal::spec(Weapon::Erasure).charge {
                            out.push(ArsenalFx::Erasure { at: *at, radius: 120.0, age: age_frac });
                        }
                    }
                }
            }
            StrikeShown::Shell { from, to, seconds, weapon } if *weapon == Weapon::Meteors => {
                let progress = (*age as f64 / *seconds).clamp(0.0, 1.0) as f32;
                out.push(ArsenalFx::Meteor { from: *from, to: *to, progress });
            }
            StrikeShown::Seek { from, to, seconds } => {
                let progress = (*age as f64 / *seconds).clamp(0.0, 1.0) as f32;
                out.push(ArsenalFx::Missile { from: *from, to: *to, progress });
            }
            StrikeShown::Wall { from, to, seconds } => {
                let age = (*age as f64 / *seconds).clamp(0.0, 1.0) as f32;
                out.push(ArsenalFx::Wall { from: *from, to: *to, age });
            }
            StrikeShown::Chain { points } => {
                let mut pts = [DVec3::ZERO; 13];
                let count = points.len().min(13);
                for (i, p) in points.iter().take(count).enumerate() {
                    pts[i] = *p;
                }
                out.push(ArsenalFx::Chain {
                    points: pts,
                    count: count as u8,
                    age: (*age / 0.8).clamp(0.0, 1.0),
                });
            }
            StrikeShown::Wave(wave, _, _) => {
                let front = wave.front(wave_clock);
                let super_ = wave.strength >= 1.5;
                out.push(ArsenalFx::WaveRing {
                    at: wave.center,
                    front,
                    strength: wave.strength as f32,
                    super_,
                });
            }
            _ => {}
        }
    }
    out
}

fn scan_fx_bolts(game: &Game<'_>, client: &mut Client) -> Vec<Bolt> {
    let mut out = Vec::new();
    client.scan_heights.retain(|entry| {
        client.strikes_shown.iter().any(|(s, age)| {
            if let StrikeShown::Fx { fx: Shown::Scan, at, seconds } = s {
                (*age as f64) < *seconds && at.distance(entry.center) < 1.0
            } else {
                false
            }
        })
    });

    let mut requests = Vec::new();
    for (shown, age) in &client.strikes_shown {
        if let StrikeShown::Fx { fx: Shown::Scan, at, seconds } = shown {
            if (*age as f64) < *seconds {
                let age_frac = (*age / *seconds as f32).clamp(0.0, 1.0);
                requests.push((*at, age_frac));
            }
        }
    }

    for (at, age_frac) in requests {
        let is_supernova = client.strikes_shown.iter().any(|(s, _)| {
            matches!(s, StrikeShown::Fx { fx: Shown::Supernova, at: s_at, .. } if (s_at.x - at.x).hypot(s_at.z - at.z) < 5.0)
        }) || client.current.as_ref().is_some_and(|ship| {
            ship.strikes.iter().any(|s| s.weapon == Weapon::Supernova && s.target.is_some_and(|t| (t.x - at.x).hypot(t.z - at.z) < 5.0))
        });

        let (radius, color) = if is_supernova {
            let spec = astral_arsenal::spec(Weapon::Supernova);
            let r = ((spec.color >> 16) & 255) as f32 / 255.0;
            let g = ((spec.color >> 8) & 255) as f32 / 255.0;
            let b = (spec.color & 255) as f32 / 255.0;
            (spec.reach.min(150.0), [r, g, b])
        } else {
            let spec = astral_arsenal::spec(Weapon::Erasure);
            let r = ((spec.color >> 16) & 255) as f32 / 255.0;
            let g = ((spec.color >> 8) & 255) as f32 / 255.0;
            let b = (spec.color & 255) as f32 / 255.0;
            (spec.reach.min(150.0), [r, g, b])
        };

        let cache_idx = match client.scan_heights.iter().position(|e| e.center.distance(at) < 1.0 && (e.radius - radius).abs() < 1.0) {
            Some(i) => i,
            None => {
                let step = 3.2;
                let min_x = at.x - radius - step;
                let max_x = at.x + radius + step;
                let min_z = at.z - radius - step;
                let max_z = at.z + radius + step;
                let cols = ((max_x - min_x) / step).ceil() as usize + 2;
                let rows = ((max_z - min_z) / step).ceil() as usize + 2;
                let mut heights = Vec::with_capacity(cols * rows);
                let facts = game.res.facts();
                for rz in 0..rows {
                    for cx in 0..cols {
                        let wx = min_x + cx as f64 * step;
                        let wz = min_z + rz as f64 * step;
                        let bx = wx.floor() as i32;
                        let bz = wz.floor() as i32;
                        if !game.world.chunks.contains_key(&(bx >> 4, bz >> 4)) {
                            heights.push(f32::NAN);
                        } else {
                            let y = game.world.motion_blocking(bx, bz, |state| !facts.is_air(state));
                            heights.push(y as f32);
                        }
                    }
                }
                client.scan_heights.push(HeightCacheEntry {
                    center: at,
                    radius,
                    min_x,
                    min_z,
                    step,
                    cols,
                    rows,
                    heights,
                });
                client.scan_heights.len() - 1
            }
        };

        let entry = &client.scan_heights[cache_idx];
        let min_x = entry.min_x;
        let min_z = entry.min_z;
        let step = entry.step;
        let cols = entry.cols;
        let rows = entry.rows;
        let heights = &entry.heights;

        let bolts = scan_bolts(at, radius, age_frac, color, |wx, wz| {
            let c = ((wx - min_x) / step).round() as isize;
            let r = ((wz - min_z) / step).round() as isize;
            if c >= 0 && c < cols as isize && r >= 0 && r < rows as isize {
                let val = heights[r as usize * cols + c as usize];
                if val.is_nan() { None } else { Some(val as f64) }
            } else {
                None
            }
        });
        out.extend(bolts);
    }
    out
}

/// A dash's stages as the crew hears them.
fn dash_sounds(game: &mut Game<'_>, before: &Published, now: &Published) {
    let stage = |ship: &Published| ship.dash.map(|dash| dash.stage);
    let event = match (stage(before), stage(now)) {
        (Some(Stage::Warming), Some(Stage::Charging)) => "minecraft:block.beacon.activate",
        (Some(Stage::Charging), Some(Stage::Flying)) => "minecraft:item.trident.riptide_3",
        (Some(Stage::Flying | Stage::Braking), None) => "minecraft:block.beacon.deactivate",
        _ if before.jump.is_none() && now.jump.is_some() => "minecraft:block.respawn_anchor.charge",
        _ if before.leaps != now.leaps => "minecraft:entity.enderman.teleport",
        _ if before.jump.is_some() && now.jump.is_none() => "minecraft:block.beacon.deactivate",
        _ => return,
    };
    let at = now.pose.local_to_world(layout().pilot_seat());
    game.audio.play(event, Some(at.to_array()), 3.0, 1.0);
}

/// What a dash needs loaded. The host writes these areas anew each tick, so they
/// go back in just before the level sees to its chunks.
pub(super) fn keep_loaded(game: &mut Game<'_>) {
    if game.native_window() {
        return;
    }
    SERVER.with_borrow(|slot| {
        let ship = slot.as_ref().and_then(|server| server.ship.as_ref());
        // A guest is told of mobs near its feet, and the pilot's are in the nose:
        // the ones aft and round the hull would never reach the window at all.
        let piloted = ship.is_some_and(|ship| ship.crew == Crew::Piloting);
        match (piloted, KEPT_TRACKING.get()) {
            (true, kept) => {
                let usual = kept.unwrap_or(game.settings.entity_distance);
                KEPT_TRACKING.set(Some(usual));
                game.settings.entity_distance = pilot::mob_reach(f64::from(usual), true) as f32;
            }
            (false, Some(usual)) => {
                game.settings.entity_distance = usual;
                KEPT_TRACKING.set(None);
            }
            (false, None) => {}
        }
        let here = ship.filter(|ship| dim_of(ship.dimension) == game.dim);
        if slot.as_ref().is_some_and(|server| server.light) && here.is_some() {
            // Crossing over the world: a small area round the ship, not the whole
            // view, or every chunk under the way would be made and thrown away.
            let radius = game.streaming.radius;
            KEPT_RADIUS.set(Some(KEPT_RADIUS.get().unwrap_or(radius)));
            game.streaming.radius = DASH_VIEW;
            game.streaming.extra.clear();
            let middle = game.streaming.center;
            game.streaming.extra.push((middle, DASH_VIEW));
        } else if here.is_some()
            && let Some(radius) = KEPT_RADIUS.take()
        {
            game.streaming.radius = radius;
        }
        let dashing = here.and_then(|ship| ship.dash.as_ref());
        let jumping = here.and_then(|ship| ship.jump.as_ref());
        // The ground stays through the charge, impacts and final excavation.
        let strike_areas = ship.into_iter().flat_map(|ship| ship.strike_areas.iter())
            .filter(|(dimension, _)| dim_of(*dimension) == game.dim)
            .map(|(_, area)| *area);
        let passage = ship.and_then(|ship| ship.passage.as_ref());
        let near = passage.filter(|passage| dim_of(passage.gate.from) == game.dim);
        let far = passage.filter(|passage| dim_of(passage.gate.to) == game.dim);
        let light = slot.as_ref().is_some_and(|server| server.light);
        let areas = dashing.iter().flat_map(|dashing| areas_in_flight(&dashing.areas, light).iter().copied());
        let wanted = areas
            .chain(far.map(|p| widened(p.exit_area())))
            .chain(near.map(|p| widened(p.entry_area())))
            .chain(jumping.map(|jumping| jumping.area))
            .chain(strike_areas);
        for area in wanted {
            if !game.streaming.extra.contains(&area) {
                game.streaming.extra.push(area);
            }
        }
    });
}

/// The ship's top over a column of the world, where it stands over it.
fn roof(x: i32, z: i32) -> Option<i32> {
    let pose = ROOF.get()?;
    let (x, z) = (f64::from(x) + 0.5, f64::from(z) + 0.5);
    let top = pilot::roof_at(&pose, x, z, |x, z| layout().hull_spans(x, z))?;
    Some(top.ceil() as i32)
}

/// How high rain falls to over a column: no further than the hull.
pub(super) fn rain_height(height: i32, x: i32, z: i32) -> i32 {
    roof(x, z).map_or(height, |roof| height.max(roof))
}

/// How far off entities are drawn: the game's own, or more for the pilot's view.
pub(super) fn mob_reach(value: f64) -> f64 {
    pilot::mob_reach(value, PILOT_VIEW.get())
}

/// How far the view reaches, for the fog: past the ground under a light dash.
pub(super) fn terrain_distance(value: i32) -> i32 {
    fog_reach(value, LIGHT_VIEW.get())
}

/// Rain splashing on the ground: not in the dry under the hull.
pub(super) fn splash_height(height: Option<i32>, x: i32, z: i32) -> Option<i32> {
    height.filter(|&height| roof(x, z).is_none_or(|roof| height >= roof))
}

/// The levels a server tick runs: the game's own choice, and the far side of a
/// portal, so that it loads and lives with nobody in it. The game's list comes
/// from the same hook entered again, which then lets the game's own code run.
pub(super) fn levels_to_tick(game: &Game<'_>) -> ControlFlow<Vec<Dim>> {
    let kept = *lock(&KEPT_LEVEL);
    let Some(kept) = kept.filter(|_| !COLLECTING.get()) else {
        return ControlFlow::Continue(());
    };
    COLLECTING.set(true);
    let mut dims = game.levels_to_tick();
    COLLECTING.set(false);
    if !dims.contains(&kept) {
        dims.push(kept);
        let order = [Dim::Overworld, Dim::Nether, Dim::End];
        dims.sort_by_key(|dim| order.iter().position(|d| d == dim));
    }
    ControlFlow::Break(dims)
}

/// Whether the game's own scene has come in round the player.
fn scene_ready(game: &Game<'_>) -> bool {
    let Some(player) = game.native_player() else {
        return false;
    };
    let (x, z) = ((player.x.floor() as i32) >> 4, (player.z.floor() as i32) >> 4);
    let around = -SCENE_READY_REACH..=SCENE_READY_REACH;
    around.clone().all(|dx| {
        around.clone().all(|dz| game.world.chunks.contains_key(&(x + dx, z + dz)))
    }) && game.streaming.pending() == 0
}

/// The charge of a jump gathering, 0 to 1.
fn warp_power(ship: Option<&Published>) -> f32 {
    let jump = ship.and_then(|ship| ship.jump);
    jump.filter(|jump| jump.stage == JumpStage::Gathering).map_or(0.0, |jump| jump.share as f32)
}

/// How fast a jump's light turns now, in radians a second.
fn warp_rate(ship: Option<&Published>) -> f64 {
    let power = f64::from(warp_power(ship));
    WARP_SPIN.0 + (WARP_SPIN.1 - WARP_SPIN.0) * power * power
}

fn first_lesson(_game: &mut Game<'_>, client: &mut Client) {
    if client.lesson_checked || !matches!(client.mode, Mode::Piloting) {
        return;
    }
    client.lesson_checked = true;
}

/// While the ship crosses over the world far terrain draws a window moving with
/// the ship, and afterwards the player's own settings come back.
fn ease_the_view(game: &mut Game<'_>, client: &mut Client) {
    let light = client.current.as_ref().is_some_and(|ship| ship.light);
    match (light, client.kept_view) {
        (true, None) => {
            let lod = game.settings.horizon.lod_distance;
            client.kept_view = Some((lod, game.streaming.radius));
            LIGHT_VIEW.set(true);
            game.streaming.radius = DASH_VIEW;
            let left = client.current.as_ref().and_then(|ship| ship.dash).map_or(0.0, |dash| dash.left);
            client.far_window = dash_shows_far_terrain(left);
        }
        (false, Some(kept)) => {
            astral_ship::set_far_lead(None);
            restore_view(game, kept);
            client.kept_view = None;
            client.far_window = false;
        }
        _ => {}
    }
    if light {
        if client.far_window {
            let velocity = client.current.as_ref().map_or(DVec3::ZERO, |ship| ship.velocity);
            let speed = velocity.length();
            let (middle, radius) = far_reach(speed);
            game.settings.horizon.lod_distance = radius;
            let horizontal = velocity.x.hypot(velocity.z);
            let lead = if horizontal < 1.0 {
                [0.0, 0.0]
            } else {
                [velocity.x / horizontal * middle, velocity.z / horizontal * middle]
            };
            astral_ship::set_far_lead(Some(lead));
        } else {
            game.settings.horizon.lod_distance = 0;
            astral_ship::set_far_lead(None);
        }
    }
}

fn restore_view(game: &mut Game<'_>, (lod, radius): (u32, i32)) {
    astral_ship::set_far_lead(None);
    LIGHT_VIEW.set(false);
    game.settings.horizon.lod_distance = lod;
    game.streaming.radius = radius;
}

/// A portal's life in the window: it opens when the server has one, and goes when
/// the server's has gone and the game's own scene has taken over from its view.
fn tend_doorway(game: &mut Game<'_>, client: &mut Client) {
    let passage = client.current.as_ref().and_then(|ship| ship.passage);
    match (&mut client.doorway, passage) {
        (None, Some(view)) => {
            let flipped = !view.hidden && game.dim == dim_of(view.gate.to);
            let (target_dim, target_area) = if flipped {
                (dim_of(view.gate.from), view.near_area)
            } else {
                (dim_of(view.gate.to), view.area)
            };
            client.doorway = Some(Doorway {
                gate: view.gate,
                opened: 0.0,
                through: false,
                veil: 0.0,
                veiled: 0,
                hidden: view.hidden,
                area: target_area,
                flipped,
            });
            scene::open(game, target_dim, target_area);
        }
        (Some(door), Some(view)) => {
            door.gate = view.gate;
            if !door.hidden && !view.hidden {
                let flipped = game.dim == dim_of(view.gate.to);
                if door.flipped != flipped {
                    door.flipped = flipped;
                    let (target_dim, target_area) = if flipped {
                        (dim_of(view.gate.from), view.near_area)
                    } else {
                        (dim_of(view.gate.to), view.area)
                    };
                    door.area = target_area;
                    scene::open(game, target_dim, target_area);
                } else {
                    let target_area = if flipped { view.near_area } else { view.area };
                    // A search for a cave moves on: the far side shown moves with it.
                    if door.area != target_area {
                        door.area = target_area;
                        let target_dim = if flipped { dim_of(view.gate.from) } else { dim_of(view.gate.to) };
                        scene::open(game, target_dim, target_area);
                    }
                }
            } else if door.area != view.area {
                door.area = view.area;
                scene::open(game, dim_of(view.gate.to), view.area);
            }
        }
        (Some(door), None) if !door.through => {
            client.doorway = None;
            scene::close();
        }
        _ => {}
    }
    let Some(door) = &mut client.doorway else {
        return;
    };
    door.opened = (door.opened + 1.0 / OPENING_TICKS).min(1.0);
    if door.through {
        door.veiled += 1;
        let over = game.dim == dim_of(door.gate.to) && scene_ready(game);
        if over || door.veiled > VEIL_TICKS {
            door.veil -= VEIL_FADE;
        }
        if door.veil <= 0.0 {
            client.doorway = None;
            scene::close();
        }
    }
}

/// This frame's portal, for the renderer of its far side.
fn show_portal(game: &mut Game<'_>, client: &Client) {
    let Some(door) = client.doorway else {
        return;
    };
    let far_side = !door.hidden && game.dim == dim_of(door.gate.to);
    let gate = if far_side { door.gate.flipped() } else { door.gate };
    let passage = client.current.as_ref().and_then(|ship| ship.passage);
    let ready = door.through || passage.is_none_or(|view| view.stage != PassStage::Preparing);
    let plane = gate.entry;
    let eased = door.opened * door.opened * (3.0 - 2.0 * door.opened);
    // The ship itself is not shown through a flipped portal; that is accepted.
    scene::update(game, |game, dim| {
        let environment = match dim {
            Dim::Overworld => game.res.environment(),
            Dim::Nether => game.res.nether_environment(),
            Dim::End => game.res.end_environment(),
        };
        let sky = environment.sample(
            game.world.day_time,
            Some(environment.base_rgb("minecraft:visual/sky_color")),
            Some(environment.base_rgb("minecraft:visual/fog_color")),
        );
        PortalPose {
            entry: plane,
            rotation: gate.rotation,
            opening: OPENING,
            opened: eased,
            shift: if door.hidden { DVec3::ZERO } else { gate.shift() },
            ready,
            shown: !door.hidden && !door.through,
            veil: door.veil,
            distance: FAR_SIDE_FOG,
            sky,
            skybox: dim == Dim::Overworld,
            end_sky: dim == Dim::End,
            nether: dim == Dim::Nether,
            going: passage.is_some_and(|view| matches!(view.stage, PassStage::Going | PassStage::Through)),
        }
    });
}

fn play_wave_sound(game: &mut Game<'_>, sound: WaveSound, volume: f32) {
    use horizon_client_render::sound_engine::Sounds;
    if volume <= 0.001 {
        return;
    }
    let event_names = game.audio.event_names();
    let has = |name: &str| {
        let norm = horizon_client_render::sound_events::normalize(name);
        event_names.binary_search(&norm).is_ok() || event_names.contains(&norm)
    };
    let mut play = |name: &str| {
        if has(name) {
            let norm = horizon_client_render::sound_events::normalize(name);
            let instance = horizon_client_render::sound_engine::SoundInstance::for_ui(
                &norm,
                1.0,
                volume,
                horizon_client_render::sound_engine::SoundEngine::unseeded(),
            );
            game.audio.play_instance(instance, false);
        }
    };
    match sound {
        WaveSound::Small => {
            play("entity.generic.explode");
        }
        WaveSound::Medium => {
            play("entity.generic.explode");
            play("entity.lightning_bolt.thunder");
        }
        WaveSound::Large => {
            play("entity.generic.explode");
            play("entity.lightning_bolt.thunder");
            play("entity.warden.sonic_boom");
        }
        WaveSound::Super => {
            play("entity.generic.explode");
            play("entity.lightning_bolt.thunder");
            play("entity.warden.sonic_boom");
            play("entity.wither.spawn");
            play("entity.ender_dragon.death");
        }
        WaveSound::Frost => {
            play("block.glass.break");
            play("entity.player.hurt_freeze");
        }
    }
}

/// The window's tick: after the player's own physics, which the ship overrides.
fn window_tick(game: &mut Game<'_>) {
    update_current_lang(game);
    if LEVEL_CHANGED.swap(false, std::sync::atomic::Ordering::Relaxed) {
        adopt(game);
    }
    let mut latest = lock(&PUBLISHED).clone();
    CLIENT.with_borrow_mut(|slot| {
        let client = slot.get_or_insert_with(Client::default);
        // The server's ship crosses a moment before the game does: until then
        // the window goes on seeing it from this side.
        if let (Some(ship), Some(door)) = (&mut latest, client.doorway)
            && ship.dimension == door.gate.to
            && game.dim == dim_of(door.gate.from)
        {
            ship.pose.position -= door.gate.shift();
            ship.dimension = door.gate.from;
        }
        let arriving = client.current.is_none();
        client.previous = client.current.clone();
        client.current = latest;
        if let (Some(before), Some(now), Some(door)) =
            (&mut client.previous, client.current.as_ref(), client.doorway)
            && before.dimension != now.dimension
        {
            before.pose.position += door.gate.shift();
            before.dimension = now.dimension;
        }
        // A jump: no flight from there to here to smooth over.
        if let (Some(before), Some(now)) = (&client.previous, &client.current)
            && before.leaps != now.leaps
        {
            client.previous = client.current.clone();
            client.flash = 1.0;
            client.arrival = Some(0.0);
            if let Some(door) = &mut client.doorway {
                door.through = true;
                door.veil = 1.0;
            }
        }
        client.flash = (client.flash - FLASH_FADE).max(0.0);
        let tick = TICK_SECONDS as f32;
        let wave_clock = f64::from_bits(WAVE_CLOCK.load(std::sync::atomic::Ordering::Relaxed));
        client.strikes_shown.retain_mut(|(shown, age)| {
            *age += tick;
            match shown {
                StrikeShown::Shell { seconds, .. } => (*age as f64) < *seconds,
                StrikeShown::Impact { .. } => *age < IMPACT_SECONDS,
                StrikeShown::Wave(wave, _, _) => !wave.done(wave_clock),
                StrikeShown::Fx { seconds, .. } => (*age as f64) < *seconds,
                StrikeShown::Chain { .. } => *age < 0.8,
                StrikeShown::Seek { seconds, .. } => (*age as f64) < *seconds,
                StrikeShown::Wall { seconds, .. } => (*age as f64) < *seconds,
                StrikeShown::Launch { .. } => false,
            }
        });
        tick_spectacle(game, client);
        let mut incoming: Vec<_> = lock(&STRIKES_SHOWN).drain(..).collect();
        if let Some(ship) = &client.current {
            for shown in &mut incoming {
                let weapon = match shown {
                    StrikeShown::Shell { from, weapon, .. } => { *from = ship.pose.local_to_world(client.guns.muzzle(*weapon)); Some(*weapon) },
                    StrikeShown::Seek { from, .. } => { *from = ship.pose.local_to_world(client.guns.muzzle(Weapon::Missiles)); Some(Weapon::Missiles) },
                    StrikeShown::Chain { points } => { points.insert(0, ship.pose.local_to_world(client.guns.muzzle(Weapon::Chain))); Some(Weapon::Chain) },
                    _ => None,
                };
                if let Some(w) = weapon { client.guns.0[guns::battery(w) as usize].fire(); }
            }
        }
        client.pending_fx.extend(incoming.iter().cloned());
        client.strikes_shown.extend(incoming.into_iter().map(|shown| (shown, 0.0)));
        let cam_pos = game.native_player().map_or(DVec3::ZERO, |p| DVec3::new(p.x, p.y, p.z));
        let volume_setting = lock(&TABLET_SAVED).as_ref().map_or(1.0, |t| t.volume);
        for (shown, _) in &client.strikes_shown {
            if let StrikeShown::Wave(wave, sound, _) = shown {
                if wave.crossed(cam_pos, wave_clock - tick as f64, wave_clock) {
                    let dist = wave.center.distance(cam_pos);
                    let falloff = wave.falloff(dist) as f32;
                    let volume = falloff * volume_setting;
                    play_wave_sound(game, *sound, volume);
                }
            }
        }
        if !client.onboarding_init {
            if let Some(saved) = lock(&TABLET_SAVED).as_ref() {
                if !saved.onboarding.is_empty() {
                    client.onboarding = OnboardingProgress::parse(&saved.onboarding);
                }
                client.onboarding_init = true;
            }
        }
        client.player_pos = game.native_player().map(|p| DVec3::new(p.x, p.y, p.z));
        // Into orbit: the view turns to the ground once, from behind the ship.
        let in_orbit = client.current.as_ref().is_some_and(|ship| ship.orbit.as_ref().is_some_and(|orbit| orbit.up));
        if in_orbit && !client.orbit_seen && matches!(client.mode, Mode::Piloting) {
            record_onboarding_event(OnboardingEvent::Orbit);
            game.pitch = 89.0;
            client.orbit_zoom = ORBIT_START_ZOOM;
            game.camera_type = CameraType::ThirdPersonBack;
            client.seen_camera = Some(CameraType::ThirdPersonBack);
            client.ship_eye = false;
        }
        // From orbit the clouds would hide the ground: off while up there, as they were after.
        match (in_orbit, client.kept_clouds.is_some()) {
            (true, false) => {
                client.kept_clouds = Some(std::mem::replace(&mut game.settings.clouds, "false".into()));
            }
            (false, true) => {
                if let Some(clouds) = client.kept_clouds.take() {
                    game.settings.clouds = clouds;
                }
            }
            _ => {}
        }
        client.orbit_seen = in_orbit;
        tend_pad(game, client);
        client.arrival = client.arrival.map(|ticks| ticks + 1.0).filter(|&t| t < ARRIVAL_TICKS);
        client.warp_spin += warp_rate(client.current.as_ref()) * TICK_SECONDS;
        tend_doorway(game, client);
        ease_the_view(game, client);
        first_lesson(game, client);
        if client.chasing && !matches!(client.mode, Mode::Piloting) {
            client.chasing = false;
            game.camera_type = CameraType::FirstPerson;
        }
        let Some(current) = client.current.clone() else {
            client.mode = Mode::Outside;
            return;
        };
        let here = dim_of(current.dimension) == game.dim;
        if arriving && current.aboard {
            // The world was closed with the player on board: they wake on the bridge.
            if here {
                beam_aboard(&current, layout().bridge_stand());
                change_mode(client, on_the_bridge());
            } else {
                change_mode(client, Mode::Outside);
            }
        }
        let parted = !here || game.screen == Screen::Dead || !game.native_on(ID);
        if parted && !matches!(client.mode, Mode::Outside) {
            change_mode(client, Mode::Outside);
        }
        let previous = client.previous.clone().unwrap_or_else(|| current.clone());
        show_shots(game, client, &current.pose);
        aim_turrets(client, &current.pose);
        dash_sounds(game, &previous, &current);
        match &mut client.mode {
            Mode::Outside => {
                if here && game.native_on(ID) {
                    meet_the_hull(game, client, &previous.pose, &current.pose);
                }
            }
            Mode::Piloting => {
                if client.pilot_start_pos.is_none() {
                    client.pilot_start_pos = Some(current.pose.position);
                } else if let Some(start) = client.pilot_start_pos {
                    if current.pose.position.distance(start) > 20.0 {
                        record_onboarding_event(OnboardingEvent::Flew);
                    }
                }
                let playing = at_the_controls(game) && !wheel_active(game);
                let firing = playing && held(game, "key.attack");
                let just_fired = firing && !client.was_attacking;
                client.was_attacking = firing;

                let in_orbit = current.orbit.is_some();
                if just_fired && !in_orbit && client.strike_selected {
                    let facts = game.res.facts();
                    let world = &game.world;
                    let cell = |x: i32, y: i32, z: i32| {
                        if !world.chunks.contains_key(&(x >> 4, z >> 4)) {
                            ShotCell::Unloaded
                        } else if stops_things(facts, world.block(x, y, z)) {
                            ShotCell::Solid
                        } else {
                            ShotCell::Clear
                        }
                    };
                    let hit = first_hit(client.eye, client.sight, 384.0, cell, &[]);
                    if hit.struck == Struck::Block {
                        lock(&ORDERS).push(Order::Strike(client.strike, hit.point.x, hit.point.z, Some(hit.point.y)));
                    } else {
                        game.native_chat(&format!("Astral Cruiser: {}", tr(current_lang(), "chat.no_target")));
                    }
                }

                let keys = if playing { pilot_keys(game) } else { NO_KEYS };
                *lock(&CONTROLS) = Controls {
                    keys,
                    look: if wheel_active(game) { (-pilot::yaw_of(current.pose.rotation).to_degrees()) as f32 } else { game.yaw },
                    firing: firing && (in_orbit || !client.strike_selected),
                    weapon: client.weapon,
                    strike: client.strike,
                    eye: client.eye,
                    sight: client.sight,
                };
                let seat = layout().pilot_seat();
                place_player(game, &previous.pose, seat, &current.pose, seat);
            }
            Mode::Walking { passenger, before } => {
                let keys = if at_the_controls(game) {
                    walk_keys(game)
                } else {
                    WalkKeys::default()
                };
                let look = -f64::from(game.yaw).to_radians();
                let heading = look - pilot::yaw_of(current.pose.rotation);
                *before = passenger.local;
                passenger.tick(&keys, heading, layout(), TICK_SECONDS);
                let (from, to) = (*before, passenger.local);
                place_player(game, &previous.pose, from, &current.pose, to);
                let at_door = layout().in_outer_door(to);
                if at_door && !client.at_door {
                    lock(&ORDERS).push(Order::Leave);
                }
                client.at_door = at_door;
            }
        }
    });
}

/// The ship carries its player. The server would judge each block it descends,
/// and the beam's one long step, as a fall that hurts where it lands.
pub(super) fn guest_tick(host: &mut LanHost, conn: u32) {
    let owner = host.hub.singleplayer_owner;
    let found = host.hub.guests.iter().position(|g| g.conn == conn && owner == Some(g.uuid));
    let Some(index) = found else {
        return;
    };
    let (carried, beam, leap) = SERVER.with_borrow_mut(|slot| {
        slot.as_mut().map_or((false, None, None), |s| {
            s.owner = Some(conn);
            (s.carrying(), s.beam.take(), s.leap.take())
        })
    });
    if let Some(at) = beam {
        host.hub.teleport(index, Some(at));
    }
    if let Some(by) = leap {
        let at = DVec3::from_array(host.hub.guests[index].position) + by;
        host.hub.teleport(index, Some(at.to_array()));
    }
    SERVER.with_borrow_mut(|slot| {
        let (Some(server), guest) = (slot, &mut host.hub.guests[index]) else {
            return;
        };
        match (server.light, server.kept_view) {
            (true, None) => {
                server.kept_view = Some(guest.view_distance);
                guest.view_distance = DASH_VIEW;
                guest.center = None;
            }
            (false, Some(view)) => {
                guest.view_distance = view;
                guest.center = None;
                server.kept_view = None;
            }
            _ => {}
        }
    });
    if carried {
        let guest = &mut host.hub.guests[index];
        guest.moves.clear();
        if let Some(motion) = guest.motion.as_mut() {
            motion.fall_distance = 0.0;
        }
    } else if ON_PAD.load(std::sync::atomic::Ordering::Relaxed)
        && let Some(motion) = host.hub.guests[index].motion.as_mut()
    {
        // Standing on the pad of light, which the server does not know of.
        motion.fall_distance = 0.0;
    }
}

pub(super) fn tick(game: &mut Game<'_>) {
    if game.native_window() {
        window_tick(game);
    } else {
        server_tick(game);
    }
}

fn sit_down(game: &mut Game<'_>, client: &mut Client) {
    client.pilot_start_pos = client.current.as_ref().map(|s| s.pose.position);
    change_mode(client, Mode::Piloting);
    client.chasing = true;
    game.camera_type = CameraType::ThirdPersonBack;
    client.seen_camera = Some(CameraType::ThirdPersonBack);
    game.native_chat(&format!("Astral Cruiser: {}", tr(current_lang(), "chat.welcome_helm")));
}

/// Through the door, for someone near it or standing on the hull.
fn come_in(game: &mut Game<'_>, client: &mut Client, ship: &Published) -> bool {
    let door = ship.pose.local_to_world(layout().airlock_exit());
    let near = client.riding
        || game.native_player().is_some_and(|p| {
            pilot::near_door(DVec3::new(p.x, p.y, p.z), door, DOOR_REACH)
        });
    let Some(airlock) = layout().room(RoomKind::Airlock).filter(|_| near) else {
        return false;
    };
    let inside = Mode::Walking {
        passenger: Passenger::at(airlock.entry),
        before: airlock.entry,
    };
    beam_aboard(ship, airlock.entry);
    change_mode(client, inside);
    true
}

fn arrival(game: &Game<'_>, x: i32, y: i32, z: i32) -> Arrival {
    if !game.world.chunks.contains_key(&(x >> 4, z >> 4)) {
        return Arrival::Unknown;
    }
    let (facts, state) = (game.res.facts(), game.world.block(x, y, z));
    if facts.is_air(state) {
        return Arrival::Open;
    }
    let block = facts.get(state);
    if block.fluid == Fluid::Lava || block.name.ends_with("fire") {
        Arrival::Danger
    } else if block.fluid != Fluid::None || block.full_collision || block.legacy_solid {
        Arrival::Ground
    } else {
        Arrival::Open
    }
}

/// The player out through the airlock, if there is room on the other side.
/// Whoever can fly stays by the door; the others are beamed down to the ground,
/// as the door may be hundreds of blocks above it.
fn disembark(game: &mut Game<'_>, client: &mut Client, ship: &Published) {
    let door = ship.pose.local_to_world(layout().airlock_exit());
    let (x, y, z) = (door.x.floor() as i32, door.y.floor() as i32, door.z.floor() as i32);
    let facts = game.res.facts();
    let blocked = (0..2).any(|up| facts.get(game.world.block(x, y + up, z)).solid_render);
    if blocked {
        let lang = current_lang();
        let msg = match lang {
            Lang::Ru => "Astral Cruiser: внешний люк заблокирован.",
            Lang::En => "Astral Cruiser: something blocks the outer door.",
        };
        game.native_chat(msg);
        return;
    }
    let may_fly = game.player.as_ref().is_some_and(|p| p.may_fly);
    let ground = pilot::landing_y(y, world_floor(game.dim), |at| arrival(game, x, at, z));
    let exit = match ground {
        _ if may_fly => door,
        Some(ground) => DVec3::new(door.x, f64::from(ground), door.z),
        None => {
            let lang = current_lang();
            let msg = match lang {
                Lang::Ru => "Astral Cruiser: под люком нет безопасной земли. Прилетите в другое место.",
                Lang::En => "Astral Cruiser: no safe ground under the door. Fly somewhere else.",
            };
            game.native_chat(msg);
            return;
        }
    };
    change_mode(client, Mode::Outside);
    lock(&SERVER_ORDERS).push(ServerOrder::Beam(exit.to_array()));
    if let Some(player) = game.player.as_mut() {
        player.velocity = Vec3::ZERO;
        player.fall_distance = 0.0;
        player.flying = player.may_fly;
    }
    if !may_fly {
        let lang = current_lang();
        let msg = match lang {
            Lang::Ru => "Astral Cruiser: перемещены вниз. /astral enter возвращает на корабль рядом с люком.",
            Lang::En => "Astral Cruiser: beamed down. /astral enter brings you back from near the door.",
        };
        game.native_chat(msg);
    }
}

fn dash_line(dash: &DashView) -> String {
    let lang = current_lang();
    let [x, y, z] = dash.target.to_array();
    let stage = match dash.stage {
        Stage::Warming => match lang {
            Lang::Ru => format!("подготовка назначения {:.0}%", dash.share * 100.0),
            Lang::En => format!("preparing the destination {:.0}%", dash.share * 100.0),
        },
        Stage::Charging => match lang {
            Lang::Ru => format!("зарядка {:.0}%", dash.share * 100.0),
            Lang::En => format!("charging {:.0}%", dash.share * 100.0),
        },
        Stage::Flying => match lang {
            Lang::Ru => format!("{:.0} блоков осталось", dash.left),
            Lang::En => format!("{:.0} blocks to go", dash.left),
        },
        Stage::Braking => match lang {
            Lang::Ru => "торможение".to_string(),
            Lang::En => "braking".to_string(),
        },
    };
    match lang {
        Lang::Ru => format!("Рывок к {x:.0} {y:.0} {z:.0}: {stage}"),
        Lang::En => format!("Dash to {x:.0} {y:.0} {z:.0}: {stage}"),
    }
}

/// A dialog releases the cursor without pausing the integrated server.
fn open_panel(game: &mut Game<'_>, client: &Client) {
    super::astral_tablet_draw::open(game, tablet_snapshot(client), client.strike as usize);
    if game.dialog_open() {
        record_onboarding_event(OnboardingEvent::OpenedTablet);
    }
}

/// The adapter from the 16 orbital strikes to tablet rows.
fn weapon_rows(cooldowns: [f64; WEAPON_COUNT]) -> Vec<WeaponRow> {
    let lang = current_lang();
    Weapon::ALL.iter().enumerate().map(|(i, &weapon)| {
        let spec = astral_arsenal::spec(weapon);
        let color = spec.color;
        let group = match spec.group {
            Group::Classic => 0,
            Group::Super => 1,
            Group::Zone => 2,
            Group::Special => 3,
            Group::Precise => 4,
        };
        WeaponRow {
            name: astral_arsenal::display_name(weapon, lang),
            blurb: astral_arsenal::display_blurb(weapon, lang),
            color: [
                ((color >> 16) & 255) as f32 / 255.0,
                ((color >> 8) & 255) as f32 / 255.0,
                (color & 255) as f32 / 255.0,
                1.0,
            ],
            cooldown_left: cooldowns[i],
            cooldown: spec.cooldown,
            energy: if spec.energy > 0.0 { spec.energy } else { spec.energy_per_second },
            reach: spec.reach,
            waves: spec.waves.len(),
            group,
        }
    }).collect()
}

fn tablet_dimension(dim: Dimension) -> &'static str {
    match dim { Dimension::Overworld => "overworld", Dimension::Nether => "nether", Dimension::End => "end" }
}

fn tablet_snapshot(client: &Client) -> TabletData {
    let saved = lock(&TABLET_SAVED).clone().unwrap_or_default();
    let ship = client.current.as_ref();
    let orbit = ship.and_then(|s| s.orbit.as_ref());
    let pos = ship.map_or_else(|| client.player_pos.unwrap_or(DVec3::ZERO), |s| s.pose.position);
    let cooldowns = ship.map_or([0.0; WEAPON_COUNT], |s| s.cooldowns);
    let capacity = Battery::new().energy;
    TabletData {
        weapons: weapon_rows(cooldowns),
        energy: ship.map_or(0.0, |s| s.energy * capacity), capacity,
        heat: [ship.map_or(0.0, |s| s.heat); 4],
        speed: ship.map_or(0.0, |s| s.velocity.length()), altitude: pos.y, pos,
        dim: ship.map_or_else(|| "overworld".into(), |s| tablet_dimension(s.dimension).into()),
        heading: ship.map_or(0.0, |s| pilot::yaw_of(s.pose.rotation)),
        aim: orbit.and_then(|o| o.aim).or(client.target),
        waypoints: saved.waypoints, shake: saved.shake, volume: saved.volume,
        orbit: orbit.is_some(),
        lang: saved.lang,
    }
}

pub(super) fn tablet_data() -> TabletData {
    CLIENT.with_borrow(|slot| tablet_snapshot(slot.as_ref().unwrap_or(&Client::default())))
}

/// Use the same orders and portal commands as the existing control panel.
pub(super) fn tablet_action(game: &mut Game<'_>, action: TabletAction) {
    match action {
        TabletAction::Select(i) => {
            if let Some(kind) = Weapon::from_index(i) {
                CLIENT.with_borrow_mut(|slot| { if let Some(c) = slot { c.strike = kind; } });
            }
        }
        TabletAction::Fire { weapon, x, z } => {
            if x.is_finite() && z.is_finite() {
                if let Some(kind) = Weapon::from_index(weapon) { lock(&ORDERS).push(Order::Strike(kind, x, z, None)); }
            }
        }
        TabletAction::Dash { x, y, z } => {
            if DVec3::new(x, y, z).is_finite() {
                lock(&ORDERS).push(Order::Dash(Some(DVec3::new(x, y, z))));
            }
        }
        TabletAction::FlyTo(i) => {
            let point = lock(&TABLET_SAVED).as_ref().and_then(|s| s.waypoints.get(i)).cloned();
            if let Some(point) = point {
                if point.dim == tablet_dimension(dimension_of(game.dim)) {
                    lock(&ORDERS).push(Order::Dash(Some(point.pos)));
                } else {
                    let lang = current_lang();
                    let msg = match lang {
                        Lang::Ru => "Точка в другом измерении: сначала откройте портал.",
                        Lang::En => "Waypoint is in another dimension: open a portal first.",
                    };
                    game.native_chat(msg);
                }
            }
        }
        TabletAction::OpenPortal(i) => {
            if let Some(cmd) = ["astral portal overworld", "astral portal nether", "astral portal end"].get(i) {
                command(game, cmd, &mut false);
            }
        }
        TabletAction::CallShip => call_ship(game),
        TabletAction::Board => {
            lock(&ORDERS).push(Order::Board);
        }
        TabletAction::Seat => {
            lock(&ORDERS).push(Order::Seat);
        }
        TabletAction::Leave => {
            lock(&ORDERS).push(Order::Leave);
        }
        TabletAction::ResetOnboarding => {
            reset_onboarding(game);
        }
        TabletAction::SetLang(setting) => {
            if let Some(saved) = lock(&TABLET_SAVED).as_mut() {
                saved.lang = setting;
            }
            let resolved = resolve(setting, &game.settings.language);
            CURRENT_LANG.store(if resolved == Lang::En { 1 } else { 0 }, Ordering::Relaxed);
            lock(&TABLET_ORDERS).push(action);
        }
        TabletAction::SaveWaypoint | TabletAction::DeleteWaypoint(_) |
        TabletAction::SetShake(_) | TabletAction::SetVolume(_) => lock(&TABLET_ORDERS).push(action),
        TabletAction::Close => super::astral_tablet_draw::close(game),
    }
}

impl Server {
    fn tablet_orders(&mut self) {
        let actions = std::mem::take(&mut *lock(&TABLET_ORDERS));
        if actions.is_empty() { return; }
        let mut stored = lock(&TABLET_SAVED);
        let saved = stored.get_or_insert_with(TabletSave::default);
        for action in actions {
            match action {
                TabletAction::SaveWaypoint if saved.waypoints.len() < 8 => {
                    if let Some(ship) = &self.ship {
                        let template = tr(current_lang(), "waypoint.default_name");
                        let n = (1..).find(|n| !saved.waypoints.iter().any(|p| p.name == template.replace("{}", &n.to_string()))).unwrap_or(1);
                        saved.waypoints.push(Waypoint { name: template.replace("{}", &n.to_string()),
                             pos: ship.flight.snapshot().pose.position, dim: tablet_dimension(ship.dimension).into() });
                    }
                }
                TabletAction::DeleteWaypoint(i) if i < saved.waypoints.len() => { saved.waypoints.remove(i); }
                TabletAction::SetShake(value) => saved.shake = value,
                TabletAction::SetVolume(value) => saved.volume = value,
                TabletAction::SetLang(setting) => saved.lang = setting,
                TabletAction::ResetOnboarding => {
                    saved.onboarding = String::new();
                }
                _ => {}
            }
        }
        saved.normalize();
        if let Err(e) = saved.save_atomic(&self.save_path.with_file_name("tablet.json")) {
            let lang = current_lang();
            let prefix = match lang {
                Lang::Ru => "Планшет: не удалось сохранить настройки",
                Lang::En => "Tablet: failed to save settings",
            };
            lock(&NOTICES).push(format!("{prefix}: {e}"));
        }
    }
}

fn jump_line(jump: &JumpView) -> String {
    let lang = current_lang();
    let [x, y, z] = jump.target.to_array();
    let stage = match jump.stage {
        JumpStage::Gathering => match lang {
            Lang::Ru => format!("зарядка {:.0}%", jump.share * 100.0),
            Lang::En => format!("charging {:.0}%", jump.share * 100.0),
        },
        JumpStage::Arriving => match lang {
            Lang::Ru => "прибытие".to_string(),
            Lang::En => "arriving".to_string(),
        },
    };
    match lang {
        Lang::Ru => format!("Прыжок к {x:.0} {y:.0} {z:.0}: {stage}"),
        Lang::En => format!("Jump to {x:.0} {y:.0} {z:.0}: {stage}"),
    }
}

fn portal_line(view: &PassView) -> String {
    let lang = current_lang();
    let to = match (view.gate.to, lang) {
        (Dimension::Overworld, Lang::Ru) => "Обычный мир",
        (Dimension::Overworld, Lang::En) => "the Overworld",
        (Dimension::Nether, Lang::Ru) => "Незер",
        (Dimension::Nether, Lang::En) => "the Nether",
        (Dimension::End, Lang::Ru) => "Энд",
        (Dimension::End, Lang::En) => "the End",
    };
    let stage = match view.stage {
        PassStage::Preparing => match view.looking {
            Some((window, windows)) => match lang {
                Lang::Ru => format!("поиск места {window}/{windows}, {:.0}%", view.share * 100.0),
                Lang::En => format!("looking for room {window}/{windows}, {:.0}%", view.share * 100.0),
            },
            None => match lang {
                Lang::Ru => format!("подготовка той стороны {:.0}%", view.share * 100.0),
                Lang::En => format!("preparing the far side {:.0}%", view.share * 100.0),
            },
        },
        PassStage::Open => match lang {
            Lang::Ru => "открыт. Влетайте носом вперёд, или /astral portal go; /astral portal cancel закроет его".to_string(),
            Lang::En => "open. Fly in nose first, or /astral portal go; /astral portal cancel closes it".to_string(),
        },
        PassStage::Going | PassStage::Through => match lang {
            Lang::Ru => "проход сквозь портал".to_string(),
            Lang::En => "going through".to_string(),
        },
    };
    match lang {
        Lang::Ru => format!("Портал в {to}: {stage}"),
        Lang::En => format!("Portal to {to}: {stage}"),
    }
}

fn status_line(client: &Client) -> String {
    let lang = current_lang();
    let Some(p) = &client.current else {
        return match lang {
            Lang::Ru => "astral status: нет корабля".to_string(),
            Lang::En => "astral status: no ship".to_string(),
        };
    };
    let aboard = match (&client.mode, lang) {
        (Mode::Outside, Lang::Ru) => "снаружи".to_string(),
        (Mode::Outside, Lang::En) => "outside".to_string(),
        (Mode::Piloting, Lang::Ru) => "за штурвалом".to_string(),
        (Mode::Piloting, Lang::En) => "piloting".to_string(),
        (Mode::Walking { passenger, .. }, Lang::Ru) => format!(
            "идёт на {:.1} {:.1} {:.1}",
            passenger.local.x, passenger.local.y, passenger.local.z
        ),
        (Mode::Walking { passenger, .. }, Lang::En) => format!(
            "walking at {:.1} {:.1} {:.1}",
            passenger.local.x, passenger.local.y, passenger.local.z
        ),
    };
    let dash = p.dash.map_or(String::new(), |dash| format!(", {}", dash_line(&dash)));
    let portal = p
        .passage
        .filter(|view| !view.hidden)
        .map_or(String::new(), |view| format!(", {}", portal_line(&view)));
    let jump = p.jump.map_or(String::new(), |jump| format!(", {}", jump_line(&jump)));
    format!(
        "astral status: {:?} at {:.1} {:.1} {:.1}, {:.1} b/s, yaw {:.1}, energy {:.0}%, heat {:.0}%, {aboard}{dash}{portal}{jump}",
        p.dimension,
        p.pose.position.x,
        p.pose.position.y,
        p.pose.position.z,
        p.velocity.length(),
        pilot::yaw_of(p.pose.rotation).to_degrees(),
        p.energy * 100.0,
        p.heat * 100.0,
    )
}

fn step_of(order: Order) -> Option<Step> {
    match order {
        Order::Spawn => Some(Step::Spawn),
        Order::Remove => Some(Step::Remove),
        Order::Board => Some(Step::Board),
        Order::Enter => Some(Step::Enter),
        Order::Leave => Some(Step::Leave),
        Order::Seat
        | Order::Cruise(_)
        | Order::Go(_)
        | Order::Weapon(_)
        | Order::NextWeapon(_)
        | Order::Blast(_)
        | Order::Dash(_)
        | Order::Portal(_)
        | Order::PortalGo
        | Order::Jump(_)
        | Order::Depart { .. }
        | Order::Speeds(_)
        | Order::Stop
        | Order::Panel
        | Order::Tutorial(_)
        | Order::ShipEye
        | Order::Status
        | Order::Orbit
        | Order::Strike(..) => None,
    }
}

fn act(game: &mut Game<'_>, client: &mut Client, order: Order) {
    let lang = current_lang();
    let chat = |game: &mut Game<'_>, text: &str| {
        game.native_chat(tr(lang, text));
    };
    if game.integrated.is_none() && !matches!(order, Order::Status) {
        chat(game, "Astral Cruiser works in single player only for now.");
        return;
    }
    let ship = client.current.clone();
    let place = match &ship {
        None => ShipIs::Missing,
        Some(s) if dim_of(s.dimension) == game.dim => ShipIs::Here,
        Some(_) => ShipIs::Elsewhere,
    };
    let aboard = !matches!(client.mode, Mode::Outside);
    if let Some(why) = step_of(order).and_then(|step| pilot::refusal(step, aboard, place)) {
        chat(game, why);
        return;
    }
    match (order, ship) {
        (Order::Spawn, _) => {
            let Some(player) = game.native_player() else {
                return;
            };
            let at = DVec3::new(player.x, player.y, player.z);
            lock(&SERVER_ORDERS).push(ServerOrder::Spawn(SaveState {
                dimension: dimension_of(game.dim),
                pose: pilot::spawn_pose(at, player.yaw),
                aboard: false,
                energy: Battery::new().energy,
                heat: 0.0,
                speeds: Speeds::default(),
            }));
            let board_key = key_name(&game.native_mod_key(ID, BOARD_KEY, BOARD_KEY_DEFAULT));
            let msg = match lang {
                Lang::Ru => format!("Astral Cruiser: корабль рядом. [{board_key}] — подняться на борт."),
                Lang::En => format!("Astral Cruiser: ship is nearby. [{board_key}] — board the ship."),
            };
            game.native_chat(&msg);
        }
        (Order::Remove, _) => {
            lock(&SERVER_ORDERS).push(ServerOrder::Remove);
            chat(game, "Astral Cruiser: ship removed.");
        }
        (Order::Status, _) => {
            let line = status_line(client);
            log::info!("{line}");
            game.native_chat(&line);
        }
        // In orbit the gun keys pick a strike, in the same order.
        (Order::Weapon(kind), Some(ship)) if ship.orbit.is_some() => {
            let index = WeaponKind::ALL.iter().position(|&k| k == kind).unwrap_or(0);
            client.strike = Weapon::ALL[index];
            client.strike_selected = true;
        }
        (Order::Weapon(kind), _) => { client.weapon = kind; client.strike_selected = false; },
        (Order::NextWeapon(by), _) => { client.weapon = client.weapon.stepped(by); client.strike_selected = false; },
        (Order::Orbit, Some(_)) if matches!(client.mode, Mode::Piloting) => {
            lock(&SERVER_ORDERS).push(ServerOrder::Orbit);
        }
        (Order::Orbit, Some(_)) => chat(game, "Astral Cruiser: only from the helm."),
        (Order::Strike(kind, x, z, y_opt), Some(_)) if aboard => {
            lock(&SERVER_ORDERS).push(ServerOrder::Strike(kind, x, z, y_opt));
        }
        (Order::Strike(..), Some(_)) => chat(game, "Astral Cruiser: you are not aboard."),
        (Order::Blast(on), _) => {
            lock(&SERVER_ORDERS).push(ServerOrder::Blast(on));
            let state = match (on, lang) {
                (true, Lang::Ru) => "включены: выстрелы разрушают блоки",
                (true, Lang::En) => "on: shots tear up the world",
                (false, Lang::Ru) => "выключены",
                (false, Lang::En) => "off",
            };
            let msg = match lang {
                Lang::Ru => format!("Astral Cruiser: взрывы {state}."),
                Lang::En => format!("Astral Cruiser: blasting is {state}."),
            };
            game.native_chat(&msg);
        }
        (Order::Panel, _) => open_panel(game, client),
        (Order::Tutorial(_), _) => {},
        (Order::ShipEye, _) if matches!(client.mode, Mode::Piloting) => {
            client.ship_eye = !client.ship_eye;
            game.camera_type = CameraType::ThirdPersonBack;
            client.seen_camera = Some(CameraType::ThirdPersonBack);
        }
        (Order::ShipEye, _) => chat(game, "Astral Cruiser: only from the helm."),
        // The seat key is often pressed with no ship about.
        (Order::Seat, None) => {}
        (_, None) => chat(game, "Astral Cruiser: no ship here. Use /astral spawn."),
        (Order::Board, Some(ship)) => {
            client.last_boarded = Some(Instant::now());
            if matches!(client.mode, Mode::Outside) {
                beam_aboard(&ship, layout().pilot_seat());
            }
            sit_down(game, client);
        }
        (Order::Seat, Some(ship)) => {
            if client.last_boarded.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
                return;
            }
            match client.mode {
                Mode::Piloting => change_mode(client, on_the_bridge()),
                Mode::Walking { passenger, .. } => {
                    if passenger.local.distance(layout().pilot_seat()) <= SEAT_REACH {
                        sit_down(game, client);
                    }
                }
                Mode::Outside => {}
            }
        },
        (Order::Go(kind), Some(_)) => match layout().room(kind) {
            Some(room) if aboard => {
                let there = Mode::Walking {
                    passenger: Passenger::at(room.entry),
                    before: room.entry,
                };
                change_mode(client, there);
            }
            _ => chat(game, "Astral Cruiser: you are not aboard."),
        },
        (Order::Enter, Some(ship)) => {
            if !come_in(game, client, &ship) {
                chat(game, "Astral Cruiser: too far from the airlock door.");
            }
        }
        (Order::Leave, Some(ship)) if ship.dash.is_some() => {
            chat(game, "Astral Cruiser: not during a dash. /astral dash cancel stops it.");
        }
        (Order::Leave, Some(ship)) if ship.jump.is_some() || ship.passage.is_some_and(|view| {
            matches!(view.stage, PassStage::Going | PassStage::Through)
        }) => {
            chat(game, "Astral Cruiser: not now. /astral stop calls it off.");
        }
        (Order::Leave, Some(ship)) => disembark(game, client, &ship),
        (Order::Dash(_) | Order::Jump(_) | Order::Depart { .. }, Some(_)) if !aboard => {
            chat(game, "Astral Cruiser: board the ship first.");
        }
        (Order::Dash(to), Some(_)) => {
            client.target = to.or(client.target);
            lock(&SERVER_ORDERS).push(ServerOrder::Dash(to));
        }
        (Order::Jump(to), Some(_)) => {
            client.target = Some(to);
            lock(&SERVER_ORDERS).push(ServerOrder::Jump(to));
        }
        (Order::Depart { jump, to, speeds }, Some(_)) => {
            client.target = Some(to);
            let mut orders = lock(&SERVER_ORDERS);
            orders.push(ServerOrder::Speeds(speeds));
            orders.push(if jump { ServerOrder::Jump(to) } else { ServerOrder::Dash(Some(to)) });
        }
        (Order::Speeds(speeds), Some(_)) => {
            lock(&SERVER_ORDERS).push(ServerOrder::Speeds(speeds));
        }
        (Order::Stop, Some(_)) => lock(&SERVER_ORDERS).push(ServerOrder::Stop),
        (Order::Portal(_) | Order::PortalGo, Some(_)) if !aboard => {
            chat(game, "Astral Cruiser: board the ship first.");
        }
        (Order::Portal(to), Some(_)) => lock(&SERVER_ORDERS).push(ServerOrder::Portal(to)),
        (Order::PortalGo, Some(_)) => lock(&SERVER_ORDERS).push(ServerOrder::PortalGo),
        (Order::Cruise(cruise), Some(_)) => {
            lock(&SERVER_ORDERS).push(ServerOrder::Cruise(cruise));
        }
    }
}

fn check_foot_crossing(game: &Game<'_>, client: &mut Client) {
    if client.seen_dim != Some(game.dim) {
        client.seen_dim = Some(game.dim);
        client.foot_before = None;
    }
    let passage = client.current.as_ref().and_then(|ship| ship.passage);
    let open_gate = match (client.mode, passage) {
        (Mode::Outside, Some(view)) if view.stage == PassStage::Open && !view.hidden => {
            if game.dim == dim_of(view.gate.from) {
                Some(view.gate)
            } else if game.dim == dim_of(view.gate.to) {
                Some(view.gate.flipped())
            } else {
                None
            }
        }
        _ => None,
    };
    let Some(gate) = open_gate else {
        client.foot_before = None;
        return;
    };
    let Some(player) = game.native_player() else {
        client.foot_before = None;
        return;
    };
    let current_pos = DVec3::new(player.x, player.y, player.z);
    if let Some(before) = client.foot_before {
        if gate.crossing(before, current_pos) {
            let at = current_pos + gate.shift();
            lock(&SERVER_ORDERS).push(ServerOrder::WalkThrough { at, to: gate.to });
            client.pad_wanted = Some((dim_of(gate.to), at));
            client.foot_before = None;
            return;
        }
    }
    client.foot_before = Some(current_pos);
}

pub(super) fn window_frame(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    update_current_lang(game);
    lock(&FRAME_PERF).finish();
    apply_onboarding_events();
    {
        let _timer = tablet_perf();
        super::astral_tablet_draw::sync_host(game);
        super::astral_tablet_draw::take_click(game);
    }
    if WHEEL_CLICK.swap(false, Ordering::Relaxed) && wheel_active(game) {
        apply_wheel_selection(game);
    }
    let orders = std::mem::take(&mut *lock(&ORDERS));
    if LEVEL_CHANGED.swap(false, std::sync::atomic::Ordering::Relaxed) {
        adopt(game);
    }
    CLIENT.with_borrow_mut(|slot| {
        let client = slot.get_or_insert_with(Client::default);
        // Going through a portal the player is gone for a moment, not the world.
        if game.native_player().is_none() && client.doorway.is_none() {
            // No world: nothing known of the last one may reach the next.
            if let Some(kept) = client.kept_view {
                restore_view(game, kept);
            }
            if let Some(clouds) = client.kept_clouds.take() {
                game.settings.clouds = clouds;
            }
            *client = Client::default();
            lock(&SERVER_ORDERS).clear();
            lock(&NOTICES).clear();
            astral_ship::set_view(None);
            astral_ship::set_clip(None);
            astral_ship::set_swirl(None);
            astral_portal::set_shake(None);
            ROOF.set(None);
            scene::close();
            return;
        }
        for order in orders {
            act(game, client, order);
        }
        for line in std::mem::take(&mut *lock(&NOTICES)) {
            game.native_chat(&line);
        }
        SCENE_RADIUS.store(
            scene_radius(game.settings.render_distance as i32),
            std::sync::atomic::Ordering::Relaxed,
        );
        check_foot_crossing(game, client);
        show_portal(game, client);
        window_effects(game, client);
    });
}

fn shown_weapon(fx: Shown) -> Option<Weapon> {
    Some(match fx {
        Shown::Mushroom => Weapon::Antimatter,
        Shown::Pillar => Weapon::SolarPillar,
        Shown::Quake => Weapon::Tectonic,
        Shown::BlackHole => Weapon::BlackHole,
        Shown::Frost => Weapon::Cryo,
        Shown::GridBeams => Weapon::Grid,
        Shown::Drill => Weapon::Drill,
        Shown::Wall => Weapon::PlasmaWall,
        Shown::Supernova => Weapon::Supernova,
        Shown::Erasure => Weapon::Erasure,
        Shown::Scan => return None,
    })
}

fn fx_strength(w: Weapon) -> f32 {
    let spec = astral_arsenal::spec(w);
    if w.is_super() { 1.5 } else if spec.energy >= 400.0 || spec.waves.iter().any(|wave| wave.strength >= 0.8) { 1.0 } else { 0.5 }
}

/// All pool writes happen here, on the WINDOW thread. The retained ArsenalFx
/// list is deliberately separate from the drained, one-shot event queue.
fn window_effects(game: &mut Game<'_>, client: &mut Client) {
    let now = astral_fx_gpu::now();
    let pose = client.current.as_ref().filter(|s| dim_of(s.dimension) == game.dim).map(|s| s.pose);
    animate_ramp(game, client, pose.as_ref());
    let ship = client.current.as_ref().filter(|s| dim_of(s.dimension) == game.dim);
    for shown in client.pending_fx.drain(..) {
        let Some(ship) = &ship else { continue; };
        match shown {
            StrikeShown::Launch { id, weapon, target } => {
                // Shells and missiles supply their own per-projectile launch;
                // Rod is started from the active snapshot before its hit event.
                if !matches!(weapon, Weapon::Barrage | Weapon::Cluster | Weapon::Meteors | Weapon::Missiles | Weapon::Rod) {
                    let charge = astral_arsenal::spec(weapon).charge;
                    let age = ship.strikes.iter().filter(|s| s.weapon == weapon && s.target == Some(target))
                        .map(|s| s.elapsed).fold(charge, f64::min);
                    if !client.fx_launches.iter().any(|(key, ..)| *key == id) {
                        client.fx_launches.push((id, weapon, target, now + (charge - age).max(0.0) as f32));
                    }
                }
            }
            StrikeShown::Shell { from, to, seconds, weapon } => {
                client.effects.phase(weapon, Phase::Launch, from, (to - from).normalize_or_zero(), now);
                client.effects.projectile(weapon, from, to, seconds as f32, now);
            }
            StrikeShown::Seek { from, to, seconds } => {
                client.effects.phase(Weapon::Missiles, Phase::Launch, from, (to - from).normalize_or_zero(), now);
                client.effects.projectile(Weapon::Missiles, from, to, seconds as f32, now);
            }
            StrikeShown::Impact { at, weapon, .. } => client.effects.impact(weapon, at, DVec3::Y, now),
            StrikeShown::Fx { fx, at, .. } => {
                if let Some(w) = shown_weapon(fx) { client.effects.impact(w, at, DVec3::Y, now); }
            }
            StrikeShown::Wave(wave, _, Some((id, weapon))) => {
                if client.effects.first_wave(id, now) { client.effects.impact(weapon, wave.center, DVec3::Y, now); }
            }
            StrikeShown::Wall { from, to, .. } => client.effects.impact(Weapon::PlasmaWall, (from + to) * 0.5, DVec3::Y, now),
            StrikeShown::Chain { points } => {
                for at in points.into_iter().skip(1) { client.effects.impact(Weapon::Chain, at, DVec3::Y, now); }
            }
            _ => {}
        }
    }
    client.fx_launches.retain(|&(id, weapon, target, due)| {
        if due > now { return true; }
        if let Some(ship) = &ship {
            client.effects.launch_once(id, weapon, ship.pose.local_to_world(client.guns.muzzle(weapon)), target, now);
        }
        false
    });
    let camera = if ship.is_none() || matches!(client.mode, Mode::Outside) {
        game.native_player().map_or(client.eye, |p| DVec3::new(p.x, p.y + 1.62, p.z))
    } else { client.eye };
    for debug in std::mem::take(&mut *lock(&FX_DEBUG)) {
        let dir = pilot::look_along(game.yaw, game.pitch).normalize_or_zero();
        let eye = game.native_player().map_or(camera, |p| DVec3::new(p.x, p.y + 1.62, p.z));
        let at = eye + dir * 60.0;
        if let Some(w) = debug {
            for (phase, delay) in [(Phase::Launch, 0.0), (Phase::Flight, 0.3), (Phase::Impact, 0.7), (Phase::After, 2.2)] {
                client.effects.phase(w, phase, at, dir, now + delay);
            }
        } else {
            let right = pilot::look_along(game.yaw - 90.0, 0.0);
            for (i, prim) in [Prim::Beam, Prim::Glow, Prim::Fireball, Prim::Smoke, Prim::Spark, Prim::Shell].into_iter().enumerate() {
                let point = at + right * ((i as f64 - 2.5) * 10.0);
                if !within_spawn_range(prim, point, camera) { continue; }
                client.fx_seed = client.fx_seed.wrapping_add(1);
                astral_fx_gpu::spawn(FxSpawn { prim, at: point,
                    vec: match prim { Prim::Beam => point + DVec3::Y * 8.0, Prim::Shell => DVec3::Y, _ => DVec3::Y * 2.0 },
                    size: 2.0, grow: 0.3, color: [0.3, 0.6, 1.0, 0.8], born: now, life: 5.0, seed: client.fx_seed as u32 });
            }
        }
    }
    let out = &mut client.fx_records;
    out.clear();
    for event in client.effects.advance(now) {
        let strength = fx_strength(event.weapon);
        client.fx_seed = client.fx_seed.wrapping_add(1);
        fx_recipe::spawn(fx_recipe::recipe(event.weapon), event.phase, event.at, event.dir, strength,
            now, distance_share(camera.distance(event.at)), client.fx_seed, out);
        if event.phase == Phase::Impact {
            let (color, radius) = fx_recipe::impact_light(event.weapon);
            astral_fx_gpu::light(FxLight { at: event.at, color: color.map(|c| c * strength), radius: radius * strength,
                born: now, life: if event.weapon.is_super() { 8.0 } else { 3.0 } });
        }
    }
    for (shot, muzzle) in client.fx_lasers.drain(..) {
        let dir = (shot.to - muzzle).normalize_or_zero();
        for (phase, at) in [(Phase::Launch, muzzle), (Phase::Impact, shot.to)] {
            if phase == Phase::Impact && !shot.struck { continue; }
            client.fx_seed = client.fx_seed.wrapping_add(1);
            fx_recipe::spawn(fx_recipe::laser_recipe(shot.kind), phase, at, dir, 0.5,
                now, distance_share(camera.distance(at)), client.fx_seed, out);
        }
    }
    let speed = ship.as_ref().map_or(0.0, |s| s.velocity.length());
    let thrust = (speed / FULL_POWER_SPEED).clamp(0.0, 1.0) as f32;
    astral_ship::set_thrust(if speed > 0.01 { thrust } else { 0.3 });
    if let Some(ship) = &ship && speed > 0.01 && now - client.last_engine_fx >= 0.05 {
        client.last_engine_fx = now;
        for nozzle in layout().parts.iter().filter(|p| p.material == horizon_client::gfx::astral_layout::Material::Engine && p.min.z <= -70.0) {
            let center = DVec3::new((nozzle.min.x + nozzle.max.x) * 0.5, (nozzle.min.y + nozzle.max.y) * 0.5, nozzle.min.z - 0.1);
            let at = ship.pose.local_to_world(center);
            if !within_spawn_range(Prim::Glow, at, camera) { continue; }
            client.fx_seed = client.fx_seed.wrapping_add(1);
            out.push(FxSpawn { prim: Prim::Glow, at, vec: DVec3::ZERO,
                size: 1.5 + thrust * 5.0, grow: 2.0, color: [0.3, 0.6, 1.0, 0.8], born: now, life: 0.6, seed: client.fx_seed as u32 });
        }
    }
    out.retain(|s| within_spawn_range(s.prim, s.at, camera));
    astral_fx_gpu::spawn_all(out);
    if FX_PERF_WANTED.swap(false, Ordering::Relaxed) {
        notice(&format!("Astral fx: {}/32768", astral_fx_gpu::count()));
    }
}

/// Decorative until the static room floor and belly collision can be opened
/// together. Keep a rider safe even if they were placed onto it by a command.
fn animate_ramp(game: &mut Game<'_>, client: &mut Client, pose: Option<&ShipPose>) {
    let frame = Instant::now();
    let dt = client.ramp_frame.replace(frame).map_or(0.0, |last| frame.duration_since(last).as_secs_f64());
    let Some(pose) = pose else { client.ramp_open = 0.0; return; };
    let (low, high) = layout().ramp();
    let belly = pose.local_to_world(DVec3::new((low.x + high.x) * 0.5, low.y, (low.z + high.z) * 0.5));
    let (x, z) = (belly.x.floor() as i32, belly.z.floor() as i32);
    let height = if game.world.chunks.contains_key(&(x >> 4, z >> 4)) {
        ground_at(&game.world, game.res.facts(), x, z, (belly.y.floor() as i32).min(world_top(game.dim)), world_floor(game.dim))
            .map_or(f64::INFINITY, |ground| belly.y - (f64::from(ground) + 1.0))
    } else { f64::INFINITY };
    let target = ShipLayout::ramp_target(height);
    if target < client.ramp_open && let Mode::Walking { passenger, before } = &mut client.mode
        && layout().on_ramp(client.ramp_open, passenger.local)
    {
        let from = passenger.local;
        let inside = layout().ramp_rider_inside();
        *passenger = Passenger::at(inside);
        *before = inside;
        place_player(game, pose, from, pose, inside);
    }
    client.ramp_open = ShipLayout::ramp_step(client.ramp_open, target, dt);
}

fn guns_with_ramp(client: &Client) -> Vec<horizon_client::gfx::astral_hull::HullVertex> {
    use horizon_client::gfx::astral_layout::{Material, Prop, Shape};
    let mut vertices = astral_gun_mesh::mesh(&client.guns);
    if client.ramp_open <= 0.0 { return vertices; }
    let (top, bottom, half_width) = layout().ramp_pose(client.ramp_open);
    let axis = top - bottom;
    let rotation = DQuat::from_rotation_arc(DVec3::Z, axis.normalize());
    let half = DVec3::new(half_width.x.abs(), 0.2, axis.length() * 0.5);
    let center = (top + bottom) * 0.5 - rotation * DVec3::Y * half.y;
    let prop = Prop { min: -half, max: half, material: Material::Hazard, shape: Shape::Block,
        solid: false, slide: DVec3::ZERO, door: DVec3::ZERO };
    for mut v in horizon_client::gfx::astral_hull::prop_vertices(&prop) {
        v.position = (center + rotation * DVec3::from_array(v.position.map(f64::from))).as_vec3().to_array();
        v.normal = (rotation * DVec3::from_array(v.normal.map(f64::from))).as_vec3().to_array();
        vertices.push(v);
    }
    vertices
}

/// How high the ground stands under the pilot's camera, kept out of a hillside.
fn above_ground(game: &Game<'_>, at: DVec3) -> DVec3 {
    let facts = game.res.facts();
    let (x, z) = (at.x.floor() as i32, at.z.floor() as i32);
    let ground = game.world.motion_blocking(x, z, |state| !facts.is_air(state));
    DVec3::new(at.x, at.y.max(f64::from(ground) + CAMERA_CLEARANCE), at.z)
}

/// This frame's view, worked out together with where the ship is drawn: one
/// from the other's numbers, so that neither can shake against the other.
pub(super) fn camera(game: &mut Game<'_>, partial: f32) {
    if !game.native_window() {
        return;
    }
    update_current_lang(game);
    astral_ship::set_chase(None);
    PILOT_VIEW.set(false);
    CLIENT.with_borrow_mut(|slot| {
        let client = slot.get_or_insert_with(Client::default);
        let shown = client.current.as_ref().filter(|c| {
            dim_of(c.dimension) == game.dim && game.native_player().is_some() && game.native_on(ID)
        }).cloned();
        let Some(current) = shown else {
            astral_ship::set_view(None);
            astral_ship::set_swirl(None);
            astral_portal::set_shake(None);
            ROOF.set(None);
            return;
        };
        let previous = client.previous.clone().unwrap_or_else(|| current.clone());
        let alpha = f64::from(partial);
        let pose = ShipPose::interpolate(&previous.pose, &current.pose, alpha);
        ROOF.set(Some(pose));
        let yaw = pilot::yaw_of(pose.rotation);
        let eye = DVec3::Y * f64::from(game.camera_eye_height().unwrap_or(STANDING_EYE));
        let feet = match client.mode {
            Mode::Outside => None,
            Mode::Piloting => {
                // A dash or a jump turns the ship, and the pilot looks where it goes.
                if current.dash.is_some() || current.jump.is_some() {
                    game.yaw -= (yaw - client.shown_yaw).to_degrees() as f32;
                }
                Some(layout().pilot_seat())
            }
            Mode::Walking { passenger, before } => {
                // The ship carries a walker round with it.
                game.yaw -= (yaw - client.shown_yaw).to_degrees() as f32;
                Some(before.lerp(passenger.local, alpha))
            }
        };
        client.shown_yaw = yaw;
        let chase = matches!(client.mode, Mode::Piloting) && !game.camera_type.first_person();
        PILOT_VIEW.set(chase);
        // F5 at the helm: the bridge, the ship's own view, behind, ahead. The game
        // has no such step, so its turn to the view behind stops here first.
        match (client.seen_camera, game.camera_type, client.ship_eye) {
            (Some(CameraType::FirstPerson), CameraType::ThirdPersonBack, false) if chase => {
                client.ship_eye = true;
            }
            (_, CameraType::ThirdPersonFront, true) => {
                client.ship_eye = false;
                game.camera_type = CameraType::ThirdPersonBack;
            }
            _ => {}
        }
        client.seen_camera = Some(game.camera_type);
        let chase = matches!(client.mode, Mode::Piloting) && !game.camera_type.first_person();
        client.ship_eye &= chase;
        // Flying through a portal, a view from ahead of the ship has no place.
        let passing = client.doorway.filter(|_| {
            current.passage.is_some_and(|view| {
                matches!(view.stage, PassStage::Going | PassStage::Through)
            })
        });
        let front = game.camera_type == CameraType::ThirdPersonFront && passing.is_none();
        let closer = passing.map_or(0.0, |door| {
            let past = (pose.position - door.plane(game)).dot(door.gate.forward());
            pilot::CHASE_BEHIND - chase_behind(past, pilot::CHASE_BEHIND)
        });
        client.chase_closer += (closer - client.chase_closer).clamp(-CHASE_EASE, CHASE_EASE);
        let behind = pilot::CHASE_BEHIND - client.chase_closer;
        let mut view_from = if client.ship_eye {
            pose.local_to_world(pilot::SHIP_EYE)
        } else if chase {
            let wanted = pilot::chase_camera_at(pose.position, game.yaw, game.pitch, front, behind);
            above_ground(game, wanted)
        } else {
            pose.local_to_world(feet.unwrap_or(DVec3::ZERO) + eye)
        };
        // In orbit the pilot's view hangs under the belly, over the ground.
        let orbit_up = current.orbit.as_ref().is_some_and(|orbit| orbit.up);
        if chase && passing.is_none() && orbit_up {
            view_from = pilot::orbit_camera(pose.position, client.orbit_zoom).0;
        }
        // The view from behind keeps out of the cloud layer, on the ship's side of it.
        let natural = view_from.y;
        let wanted = if chase && !client.ship_eye && passing.is_none() && !orbit_up && game.dim == Dim::Overworld {
            out_of_clouds(natural, pose.position.y, CLOUD_BOTTOM, CLOUD_TOP, CLOUD_MARGIN) - natural
        } else {
            0.0
        };
        client.cloud_lift = ease_to(client.cloud_lift, wanted, CLOUD_EASE);
        view_from.y += client.cloud_lift;
        if let Some(door) = passing.filter(|door| !door.through) {
            let gate = Gate { entry: door.plane(game), ..door.gate };
            if chase {
                view_from = through_opening(&gate, view_from);
            }
            if gate.past(view_from) >= -THROUGH_MARGIN
                && let Some(open) = &mut client.doorway
            {
                open.through = true;
                open.veil = 1.0;
                let seat = feet.unwrap_or(layout().pilot_seat());
                lock(&SERVER_ORDERS).push(ServerOrder::Cross(seat));
            }
        }
        let clip = passing.filter(|door| {
            !door.through && !door.hidden && game.dim == dim_of(door.gate.from)
        });
        astral_ship::set_clip(clip.map(|door| (door.gate.entry, door.gate.forward())));
        client.sight = pilot::look_along(game.yaw, game.pitch);
        client.eye = if front { pose.position } else { view_from };
        let mut turrets = [[0.0; 2]; TURRETS];
        for (index, turret) in turrets.iter_mut().enumerate() {
            let (before, now) = (client.turrets_before[index], client.turrets[index]);
            *turret = [0, 1].map(|axis| pilot::between_angles(before[axis], now[axis], alpha) as f32);
        }
        let spin = client.warp_spin + warp_rate(Some(&current)) * TICK_SECONDS * alpha;
        let power = warp_power(Some(&current));
        let stern = pose.local_to_world(DVec3::NEG_Z * STERN);
        astral_ship::set_swirl((power > 0.0).then_some(Swirl {
            middle: pose.local_to_world(DVec3::NEG_Z * (STERN + 34.0)),
            rotation: pose.rotation,
            radius: 14.0 + 46.0 * f64::from(power),
            spin: -spin * 0.7,
            power: power * 0.55,
        }));
        let belly_muzzle = pose.local_to_world(client.guns.muzzle(Weapon::Drill));
        *lock(&GUN_MUZZLES) = std::array::from_fn(|i| client.guns.0[i].pose().muzzle);
        *lock(&GUN_READY) = std::array::from_fn(|i| client.guns.0[i].pose().extension >= 1.0);
        { let _timer = PerfTimer::frame(4); astral_ship::set_guns(guns_with_ramp(client)); }
        astral_ship::set_cubes(spectacle_cubes(game, client));

        let mut light = bolts(client, &current, &pose);
        light.extend(astral_gun_mesh::light(&client.guns, pose.position, pose.rotation));
        let debris_timer = PerfTimer::frame(2);
        for p in client.debris.particles.iter().rev().take(256).filter(|p| p.heat > 0.3) {
            light.push(Bolt { from: p.pos - p.vel * 0.05, to: p.pos, width: 0.08, flash: 0.0, color: [1.0, p.heat as f32 * 0.65, 0.05, (p.heat * p.life.min(1.0)) as f32] });
        }
        drop(debris_timer);
        {
            for strike in &current.strikes {
                let p = client.guns.0[guns::battery(strike.weapon) as usize].pose();
                if let Some(to) = strike.target.filter(|_| p.recoil > 0.0) {
                    light.push(Bolt { from: pose.local_to_world(p.muzzle), to, width: (0.2 + p.recoil * 0.6) as f32, flash: 0.0, color: [0.8, 0.65, 1.0, p.recoil as f32] });
                }
            }
        }
        { let _timer = PerfTimer::frame(0); light.extend(strike_bolts(&strike_fx(client, &current, &pose))); }
        { let _timer = PerfTimer::frame(1); light.extend(arsenal_bolts(&arsenal_fx_list(client, belly_muzzle))); }
        light.extend(scan_fx_bolts(game, client));
        if let Some((_, pad)) = client.pad.filter(|(dim, _)| *dim == game.dim) {
            light.extend(pad_bolts(&pad));
        }
        light.extend(warp_bolts(stern, pose.rotation, spin, power));
        if let Some(ticks) = client.arrival {
            light.extend(arrival_ring(pose.position, pose.rotation, ticks / ARRIVAL_TICKS));
        }
        light.truncate(4096);
        lock(&FRAME_PERF).pending[6] += light.len() as f64;
        astral_ship::set_bolts(light);
        astral_ship::set_view(Some(ShipView {
            position: pose.position,
            rotation: pose.rotation,
            anchor: feet
                .filter(|_| game.camera_type.first_person())
                .map(|feet| feet + eye),
            power: (current.velocity.length() / FULL_POWER_SPEED).min(1.0) as f32,
            fog: !chase,
            turrets,
        }));
        if chase {
            astral_ship::set_chase(Some(view_from));
        }

        let wave_clock = f64::from_bits(WAVE_CLOCK.load(std::sync::atomic::Ordering::Relaxed));
        let now = wave_clock + alpha * TICK_SECONDS;
        let waves: Vec<Wave> = client
            .strikes_shown
            .iter()
            .filter_map(|(s, _)| match s {
                StrikeShown::Wave(w, _, _) => Some(*w),
                _ => None,
            })
            .collect();
        let cam_pos = if matches!(client.mode, Mode::Outside) {
            game.native_player().map_or(view_from, |p| DVec3::new(p.x, p.y, p.z) + eye)
        } else {
            view_from
        };
        let shake_setting = lock(&TABLET_SAVED).as_ref().map_or(1.0, |t| t.shake) as f64;
        let total = (super::astral_shockwave::shake_total(&waves, cam_pos, now) * shake_setting)
            .clamp(0.0, super::astral_shockwave::SHAKE_CAP);
        let (offset, shake_yaw, shake_pitch) = super::astral_shockwave::shake_offset(total, now);

        let mut push = DVec3::ZERO;
        for w in &waves {
            let decay = w.arrival(pose.position).map_or(0.0, |arr| {
                if now < arr { 0.0 } else { (-(now - arr) / 0.5).exp() }
            });
            if decay > 0.001 {
                push += super::astral_shockwave::ship_push(w, pose.position) * decay;
            }
        }
        let local_push = pose.rotation.inverse() * push;
        let roll_deg = if matches!(client.mode, Mode::Outside) {
            0.0
        } else {
            (local_push.x.clamp(-8.0, 8.0) / 8.0 * 6.0) as f32
        };

        if total > 0.0001 || roll_deg.abs() > 0.001 {
            let shake_matrix = Mat4::from_rotation_z(roll_deg.to_radians())
                * Mat4::from_rotation_x(shake_pitch.to_radians())
                * Mat4::from_rotation_y(shake_yaw.to_radians())
                * Mat4::from_translation(-offset.as_vec3());
            astral_portal::set_shake(Some(shake_matrix));
        } else {
            astral_portal::set_shake(None);
        }
    });
}

/// Server-side hull cells covering a body's swept query area, in this level only.
fn hull_boxes_near(game: &Game<'_>, area: Aabb) -> Vec<Aabb> {
    let pose = SERVER.with_borrow(|slot| {
        let ship = slot.as_ref()?.ship.as_ref()?;
        (dim_of(ship.dimension) == game.dim).then(|| ship.flight.snapshot().pose)
    });
    let Some(pose) = pose else { return Vec::new() };
    // Bound even very fast projectiles' queries to the ship, rather than
    // rasterizing the whole distance they can travel in a tick.
    let radius = HULL_HALF_EXTENTS.x.hypot(HULL_HALF_EXTENTS.z) + HULL_CELL;
    let extent = Vec3::new(radius, HULL_HALF_EXTENTS.y, radius);
    let center = Vec3::new(pose.position.x, pose.position.y, pose.position.z);
    let ship_area = Aabb::new(center.subtract(extent), center.add_vector(extent));
    if !ship_area.intersects(area) { return Vec::new() }
    let (lo, hi) = (area.min_position(), area.max_position());
    let (slo, shi) = (ship_area.min_position(), ship_area.max_position());
    let lo = Vec3::new(lo.x.max(slo.x), lo.y.max(slo.y), lo.z.max(slo.z));
    let hi = Vec3::new(hi.x.min(shi.x), hi.y.min(shi.y), hi.z.min(shi.z));
    let at = DVec3::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0, (lo.z + hi.z) / 2.0);
    let reach = ((hi.x - lo.x).max(hi.y - lo.y).max(hi.z - lo.z) / 2.0) + HULL_CELL;
    pilot::solid_cells(&pose, at, reach, HULL_CELL, |x, z| layout().hull_spans(x, z))
        .into_iter()
        .map(|[x, y, z, right, top, front]| {
            Aabb::new(Vec3::new(x, y, z), Vec3::new(right, top, front))
        })
        .filter(|b| b.intersects(area))
        .collect()
}

/// The library's collision solver cannot borrow Game. Lend it just the cells
/// around this phase's bodies, scoped to the same world and server thread.
pub(super) fn hull_item_tick(game: &mut Game<'_>, running: bool) -> ControlFlow<horizon_client::Result<()>> {
    use horizon_client::collisions::astral_hull_collision as hull;
    if !running || hull::active(1) { return ControlFlow::Continue(()) }
    let boxes = game.dropped.iter().flat_map(|item| {
        hull_boxes_near(game, item.bounds().expand_towards(item.velocity)
            .inflate(Vec3::new(HULL_REACH, HULL_REACH, HULL_REACH)))
    }).collect::<Vec<_>>();
    if boxes.is_empty() { return ControlFlow::Continue(()) }
    let _scope = hull::enter(1, &game.world, boxes);
    // The macro-free caller keeps the item movement inside this scope.
    ControlFlow::Break(game.tick_world(running))
}

pub(super) fn hull_mob_tick(game: &mut Game<'_>) -> ControlFlow<horizon_client::Result<()>> {
    use horizon_client::collisions::astral_hull_collision as hull;
    if hull::active(2) { return ControlFlow::Continue(()) }
    let boxes = game.mobs.iter().flat_map(|mob| {
        let reach = HULL_REACH + mob.kind.step_height;
        hull_boxes_near(game, mob.bounds().expand_towards(mob.velocity)
            .inflate(Vec3::new(reach, reach, reach)))
    }).collect::<Vec<_>>();
    if boxes.is_empty() { return ControlFlow::Continue(()) }
    let _scope = hull::enter(2, &game.world, boxes);
    ControlFlow::Break(game.tick_creature_entities())
}

pub(super) fn hull_arrow_tick(
    game: &mut Game<'_>,
    tick: impl FnOnce(&mut Game<'_>) -> horizon_client::Result<()>,
) -> ControlFlow<horizon_client::Result<()>> {
    use horizon_client::collisions::astral_hull_collision as hull;
    if hull::active(3) { return ControlFlow::Continue(()) }
    let mut boxes = game.creatures.projectiles.iter().filter(|p| p.arrow.is_some()).flat_map(|p| {
        hull_boxes_near(game, horizon_client::arrows::arrow_bounds(p.position)
            .expand_towards(p.velocity).inflate(Vec3::new(HULL_REACH, HULL_REACH, HULL_REACH)))
    }).collect::<Vec<_>>();
    if boxes.is_empty() { return ControlFlow::Continue(()) }
    // The macro-free caller ticks mobs before projectiles. Keep both sets in
    // its innermost scope: boxes_near deliberately reads only the top scope.
    boxes.extend(game.mobs.iter().flat_map(|mob| {
        let reach = HULL_REACH + mob.kind.step_height;
        hull_boxes_near(game, mob.bounds().expand_towards(mob.velocity)
            .inflate(Vec3::new(reach, reach, reach)))
    }));
    // Suppress the mob wrapper on re-entry, whichever HEAD helper runs first.
    // The phase-3 scope below already supplies its cells as well as arrows'.
    let _mob_scope = hull::enter(2, &game.world, Vec::new());
    let _scope = hull::enter(3, &game.world, boxes);
    ControlFlow::Break(tick(game))
}

/// The hull as solid boxes around someone outside, added to what the player's
/// own motion already collides with. The game's list comes from the same hook
/// entered again, which then lets the game's own code run.
pub(super) fn solid_hull(game: &Game<'_>) -> ControlFlow<Vec<Aabb>> {
    if COLLECTING.get() {
        return ControlFlow::Continue(());
    }
    let (ship, pad) = CLIENT.with_borrow(|slot| {
        let Some(client) = slot.as_ref() else {
            return (None, None);
        };
        let outside = matches!(client.mode, Mode::Outside);
        let ship = client.current.as_ref().filter(|c| outside && dim_of(c.dimension) == game.dim).cloned();
        let pad = client.pad.filter(|(dim, _)| *dim == game.dim).map(|(_, pad)| pad);
        (ship, pad)
    });
    let Some(player) = game.native_player().filter(|_| ship.is_some() || pad.is_some()) else {
        return ControlFlow::Continue(());
    };
    COLLECTING.set(true);
    let mut boxes = game.player_entity_collisions();
    COLLECTING.set(false);
    if let Some(ship) = ship {
        let at = DVec3::new(player.x, player.y, player.z);
        let cells = pilot::solid_cells(&ship.pose, at, HULL_REACH, HULL_CELL, |x, z| {
            layout().hull_spans(x, z)
        });
        boxes.extend(cells.into_iter().map(|[x, y, z, right, top, front]| {
            Aabb::new(Vec3::new(x, y, z), Vec3::new(right, top, front))
        }));
    }
    if let Some(pad) = pad {
        let ([x0, y0, z0], [x1, y1, z1]) = pad_box(&pad);
        boxes.push(Aabb::new(Vec3::new(x0, y0, z0), Vec3::new(x1, y1, z1)));
    }
    ControlFlow::Break(boxes)
}

/// The walker stands on the pad: the server must not count it as a fall.
static ON_PAD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The pad of light under a portal's exit over nothing: made once the walker is
/// through and the ground there is known, gone once the portal is shut and nobody
/// stands on it.
fn tend_pad(game: &Game<'_>, client: &mut Client) {
    if let Some((dim, at)) = client.pad_wanted
        && game.dim == dim
    {
        let (x, z) = (at.x.floor() as i32, at.z.floor() as i32);
        if game.world.chunks.contains_key(&(x >> 4, z >> 4)) {
            let facts = game.res.facts();
            let world = &game.world;
            let solid = |x: i32, y: i32, z: i32| stops_things(facts, world.block(x, y, z));
            client.pad = pad_for(at, solid).map(|pad| (dim, pad));
            client.pad_wanted = None;
        }
    }
    let standing = match (client.pad, game.native_player()) {
        (Some((dim, pad)), Some(player)) if dim == game.dim => {
            let ([x0, _, z0], [x1, top, z1]) = pad_box(&pad);
            (x0..=x1).contains(&player.x) && (z0..=z1).contains(&player.z) && (player.y - top).abs() < 0.6
        }
        _ => false,
    };
    ON_PAD.store(standing, std::sync::atomic::Ordering::Relaxed);
    let portal_open = client
        .current
        .as_ref()
        .and_then(|ship| ship.passage)
        .is_some_and(|view| view.stage == PassStage::Open && !view.hidden);
    if client.pad.is_some() && !pad_stays(portal_open, standing) {
        client.pad = None;
    }
}

/// The pad drawn as a lit grid.
fn pad_bolts(pad: &Pad) -> Vec<Bolt> {
    let ([x0, _, z0], [x1, top, z1]) = pad_box(pad);
    let y = top + 0.02;
    let color = [0.45, 0.95, 1.0, 0.7];
    (0..=9)
        .flat_map(|step| {
            let share = f64::from(step) / 9.0;
            let (x, z) = (x0 + (x1 - x0) * share, z0 + (z1 - z0) * share);
            [
                Bolt { from: DVec3::new(x, y, z0), to: DVec3::new(x, y, z1), width: 0.12, flash: 0.0, color },
                Bolt { from: DVec3::new(x0, y, z), to: DVec3::new(x1, y, z), width: 0.12, flash: 0.0, color },
            ]
        })
        .collect()
}

/// A labelled bar on the pilot's panel: `share` of it lit.
fn gauge(game: &Game<'_>, batch: &mut HudBatch, at: [f32; 2], share: f64, argb: u32, name: &str) {
    const WIDTH: f32 = 90.0;
    let [x, y] = at;
    game.native_fill(batch, [x, y, WIDTH, 5.0], 0x90000000);
    game.native_fill(batch, [x, y, WIDTH * share.clamp(0.0, 1.0) as f32, 5.0], argb);
    game.native_text(batch, name, [x + WIDTH + 4.0, y - 2.0], 0xB8D8E0, true);
}

/// What the pilot needs to see: the guns, their state, and where they point.
fn pilot_panel(game: &Game<'_>, batch: &mut HudBatch, ship: &Published, weapon: WeaponKind) {
    let lang = current_lang();
    const LEFT: f32 = 8.0;
    let bottom = batch.height - 34.0;
    let mut y = bottom - 76.0;
    for (index, kind) in WeaponKind::ALL.into_iter().enumerate() {
        let selected = kind == weapon;
        if selected {
            game.native_fill(batch, [LEFT - 3.0, y - 2.0, 96.0, 12.0], 0x6030A0B0);
        }
        let color = if selected { 0x73F2FF } else { 0x8AA0A8 };
        let localized_name = if lang == Lang::Ru { GUN_KEYS[index].2 } else { GUN_KEYS[index].1 };
        let name = format!("{} {}", index + 1, localized_name);
        game.native_text(batch, &name, [LEFT, y], color, true);
        y += 12.0;
    }
    gauge(game, batch, [LEFT, bottom - 22.0], ship.energy, 0xFF40D0FF, tr(lang, "energy"));
    let heat = if ship.locked { 0xFFFF5030 } else { 0xFFFFA030 };
    let name = tr(lang, if ship.locked { "overheated" } else { "heat" });
    gauge(game, batch, [LEFT, bottom - 12.0], ship.heat, heat, name);
    if weapon == WeaponKind::MainBeam {
        gauge(game, batch, [LEFT, bottom - 2.0], ship.charge, 0xFF90C8FF, tr(lang, "charge"));
    }
}

/// The strikes in orbit, each with its cooldown, and the ship's energy.
fn orbit_panel(
    game: &Game<'_>,
    batch: &mut HudBatch,
    ship: &Published,
    orbit: &OrbitView,
    picked: Weapon,
    strike_hud: StrikeHud,
) {
    let lang = current_lang();
    let (x, y) = (8.0, 44.0);
    match strike_hud {
        StrikeHud::Hidden => return,
        StrikeHud::Line => {
            let selected_idx = Weapon::ALL.iter().position(|&w| w == picked).unwrap_or(0);
            let cooling = orbit.cooldowns[selected_idx].max(0.0);
            let state = if cooling == 0.0 { tr(lang, "READY").to_string() } else { format!("{cooling:.1}s") };
            let energy_val = ship.energy.clamp(0.0, 1.0) * super::astral_weapons::CAPACITY;
            let display_name = astral_arsenal::display_name(picked, lang);
            let en_label = tr(lang, "tablet.en_short");
            let line = format!("▶ {} {}  {} {:.0}", display_name, state, en_label, energy_val);
            let accent = strike_color(picked);
            let line_w = game.native_text_width(&line) + 16.0;
            let start = batch.vertices.len();
            orbit_frame(game, batch, [x, y, line_w, 18.0], accent);
            game.native_text(batch, &line, [x + 8.0, y + 4.0], if cooling == 0.0 { 0xFFFFFF } else { 0xC0CED8 }, true);
            batch.scale_since(start, x, y, 0.85, 0.85);
            return;
        }
        StrikeHud::Full => {}
    }

    let scale = 0.85;
    let width = 208.0;
    let row_height = 13.0;
    let header_height = 20.0;
    let footer_height = 32.0;
    let window_size = 5;
    let height = header_height + window_size as f32 * row_height + footer_height;
    let accent = strike_color(picked);

    let selected_idx = Weapon::ALL.iter().position(|&w| w == picked).unwrap_or(0);
    let count = Weapon::ALL.len();
    let max_start = count.saturating_sub(window_size);
    let ideal_start = selected_idx.saturating_sub(2);
    let start_idx = ideal_start.min(max_start);
    let end_idx = start_idx + window_size;

    let more_above = start_idx > 0;
    let more_below = end_idx < count;

    let start = batch.vertices.len();
    orbit_frame(game, batch, [x, y, width, height], accent);
    game.native_text(batch, tr(lang, "ORBITAL ARSENAL"), [x + 10.0, y + 5.0], accent, true);
    if more_above {
        game.native_text(batch, "▲", [x + width - 18.0, y + 5.0], accent, true);
    }
    game.native_fill(batch, [x + 10.0, y + 17.0, width - 20.0, 1.0], 0x603D5364);

    for (row_idx, index) in (start_idx..end_idx).enumerate() {
        let kind = Weapon::ALL[index];
        let row = y + 19.0 + row_idx as f32 * row_height;
        let color = strike_color(kind);
        let selected = kind == picked;
        if selected {
            game.native_fill(batch, [x + 4.0, row - 1.0, width - 8.0, row_height], 0x28000000 | color);
            game.native_fill(batch, [x + 4.0, row - 1.0, 2.0, row_height], 0xFF000000 | color);
        }
        let cooling = orbit.cooldowns[index].max(0.0);
        let ready = cooling == 0.0;
        game.native_fill(batch, [x + 8.0, row + 2.0, 4.0, 4.0], 0xFF000000 | color);
        let name = astral_arsenal::display_name(kind, lang);
        game.native_text(batch, name, [x + 16.0, row], if selected { 0xFFFFFF } else { 0xC0CED8 }, true);
        let state = if ready { tr(lang, "READY").to_string() } else { format!("{cooling:.1}s") };
        game.native_text(batch, &state, [x + 160.0, row], if ready { color } else { 0x9AABB8 }, true);
        let share = 1.0 - cooling / astral_arsenal::spec(kind).cooldown;
        if !ready {
            orbit_bar(game, batch, [x + 16.0, row + 10.0, width - 28.0, 2.0], share, color);
        }
    }

    let rows_bottom = y + 19.0 + window_size as f32 * row_height;
    if more_below {
        game.native_text(batch, "▼", [x + width - 18.0, rows_bottom - 10.0], accent, true);
    }

    let footer_y = rows_bottom + 1.0;
    game.native_fill(batch, [x + 10.0, footer_y, width - 20.0, 1.0], 0x603D5364);
    let spec = astral_arsenal::spec(picked);
    let cost = if spec.energy > 0.0 { spec.energy } else { spec.energy_per_second };
    let short = ship.energy * super::astral_weapons::CAPACITY < cost;
    let cost_label = tr(lang, "COST");
    let cost_text = if spec.energy_per_second > 0.0 { format!("{cost_label} {:.0}/s", spec.energy_per_second) } else { format!("{cost_label} {cost:.0}") };
    let energy_label = tr(lang, "ENERGY");
    let energy = format!(
        "{energy_label} {:.0}/{:.0}",
        ship.energy.clamp(0.0, 1.0) * super::astral_weapons::CAPACITY,
        super::astral_weapons::CAPACITY
    );
    game.native_text(batch, &energy, [x + 10.0, footer_y + 4.0], 0xC0CED8, true);
    game.native_text(batch, &cost_text, [x + 132.0, footer_y + 4.0], if short { 0xFF6060 } else { accent }, true);
    orbit_bar(game, batch, [x + 10.0, footer_y + 18.0, width - 20.0, 6.0], ship.energy, if short { 0xFF6060 } else { 0x59D9FF });

    batch.scale_since(start, x, y, scale, scale);
}

fn strike_color(kind: Weapon) -> u32 {
    super::astral_arsenal::spec(kind).color
}

fn orbit_frame(game: &Game<'_>, batch: &mut HudBatch, rect: [f32; 4], color: u32) {
    let [x, y, w, h] = rect;
    game.native_fill(batch, rect, 0xD8101822);
    for edge in [[x, y, w, 1.0], [x, y + h - 1.0, w, 1.0], [x, y, 1.0, h], [x + w - 1.0, y, 1.0, h]] {
        game.native_fill(batch, edge, 0xA0000000 | color);
    }
}

fn orbit_bar(game: &Game<'_>, batch: &mut HudBatch, rect: [f32; 4], share: f64, color: u32) {
    let [x, y, w, h] = rect;
    game.native_fill(batch, rect, 0xFF293644);
    game.native_fill(batch, [x, y, w * share.clamp(0.0, 1.0) as f32, h], 0xFF000000 | color);
}

/// Keep the target card clear of the strike list at larger GUI scales.
fn orbit_target(game: &Game<'_>, batch: &mut HudBatch, ship: &Published, orbit: &OrbitView, picked: Weapon) {
    let lang = current_lang();
    let scale = 0.6;
    let base_w = 208.0;
    let base_h = 54.0;
    let cx = batch.width / 2.0;
    let x = cx - base_w / 2.0;
    let y = (batch.height - 58.0).max(48.0);
    let start = batch.vertices.len();
    let accent = strike_color(picked);
    orbit_frame(game, batch, [x, y, base_w, base_h], accent);
    if !orbit.up {
        let top = if ship.dimension == Dimension::Overworld { 320.0 } else { 256.0 };
        let target = top + 26.0;
        game.native_text(batch, tr(lang, "CLIMBING TO ORBIT"), [x + 10.0, y + 8.0], accent, true);
        let blocks_str = match lang {
            Lang::Ru => "блоков",
            Lang::En => "blocks",
        };
        let alt_prefix = match lang {
            Lang::Ru => "ВЫС",
            Lang::En => "ALT",
        };
        let altitude = format!("{alt_prefix} {:.0} / {target:.0} {blocks_str}", ship.pose.position.y);
        game.native_text(batch, &altitude, [x + 10.0, y + 23.0], 0xC0CED8, true);
        orbit_bar(game, batch, [x + 10.0, y + 41.0, 188.0, 4.0], ship.pose.position.y / target, accent);
        batch.scale_since(start, cx, y, scale, scale);
        return;
    }
    let (label, color) = if let Some(aim) = orbit.aim {
        let delta = aim - ship.pose.position;
        let distance = (delta.x * delta.x + delta.z * delta.z).sqrt();
        let blocks_str = match lang {
            Lang::Ru => "блоков",
            Lang::En => "blocks",
        };
        let b_str = match lang {
            Lang::Ru => "б",
            Lang::En => "b",
        };
        if distance > super::astral_orbit::RANGE {
            let out_str = tr(lang, "OUT OF RANGE");
            (format!("{distance:.0} {b_str}  {out_str}"), 0xFF6060)
        } else {
            let target_str = tr(lang, "TARGET");
            (format!("{target_str} {distance:.0} {blocks_str}"), 0xDCEAF2)
        }
    } else {
        (tr(lang, "NO TARGET").to_string(), 0x9AABB8)
    };
    game.native_text(batch, &label, [x + 10.0, y + 8.0], color, true);
    if let Some(strike) = orbit.strikes.first() {
        let spec = astral_arsenal::spec(strike.weapon);
        let duration = if spec.duration > 0.0 { spec.duration } else { spec.charge + 1.0 };
        let color = strike_color(strike.weapon);
        let display_name = astral_arsenal::display_name(strike.weapon, lang);
        let active_str = tr(lang, "ACTIVE");
        let label = format!("{display_name}  {active_str}");
        game.native_text(batch, &label, [x + 10.0, y + 23.0], color, true);
        orbit_bar(game, batch, [x + 10.0, y + 41.0, 188.0, 4.0], strike.elapsed / duration.max(0.1), color);
    } else {
        game.native_text(batch, tr(lang, "AWAITING STRIKE"), [x + 10.0, y + 29.0], 0x9AABB8, true);
    }
    batch.scale_since(start, cx, y, scale, scale);
}

/// A sight in the middle of the screen, as the game draws none in third person.
fn sight_mark(game: &Game<'_>, batch: &mut HudBatch) {
    let (x, y) = (batch.width / 2.0, batch.height / 2.0);
    for bar in [
        [x - 7.0, y - 0.5, 4.0, 1.0],
        [x + 3.0, y - 0.5, 4.0, 1.0],
        [x - 0.5, y - 7.0, 1.0, 4.0],
        [x - 0.5, y + 3.0, 1.0, 4.0],
    ] {
        game.native_fill(batch, bar, 0xD073F2FF);
    }
}

fn current_hint(game: &Game<'_>, client: &Client) -> Option<String> {
    if matches!(client.mode, Mode::Piloting) {
        return None;
    }
    let player = game.native_player()?;
    let player_pos = DVec3::new(player.x, player.y, player.z);
    let ship_opt = client.current.as_ref();
    let aboard = !matches!(client.mode, Mode::Outside)
        || ship_opt.is_some_and(|s| s.aboard);

    let lang = current_lang();
    match client.mode {
        Mode::Outside if !aboard => {
            if let Some(ship) = ship_opt.filter(|s| dim_of(s.dimension) == game.dim) {
                if within_boarding(ship.pose.position, player_pos) {
                    let key = key_name(&game.native_mod_key(ID, BOARD_KEY, BOARD_KEY_DEFAULT));
                    return Some(tr(lang, "hint.board").replace("{}", &key));
                }
            }
            let no_ship_near = match ship_opt {
                None => true,
                Some(ship) => dim_of(ship.dimension) != game.dim || !within_boarding(ship.pose.position, player_pos),
            };
            if no_ship_near {
                let key = key_name(&game.native_mod_key(ID, CALL_KEY, CALL_KEY_DEFAULT));
                return Some(tr(lang, "hint.call").replace("{}", &key));
            }
        }
        Mode::Walking { passenger, .. } => {
            if passenger.local.distance(layout().pilot_seat()) <= SEAT_REACH {
                let key = key_name(&game.native_mod_key(ID, SEAT_KEY, SEAT_KEY_DEFAULT));
                return Some(tr(lang, "hint.seat").replace("{}", &key));
            }
        }
        _ => {}
    }
    None
}

fn draw_hint_pill(game: &Game<'_>, batch: &mut HudBatch, hint: &str) {
    let text_w = game.native_text_width(hint);
    let pill_w = text_w + 24.0;
    let pill_h = 18.0;
    let cx = batch.width / 2.0;
    let px = cx - pill_w / 2.0;
    let py = (batch.height - 38.0).max(10.0);
    game.native_fill(batch, [px, py, pill_w, pill_h], 0xD8101822);
    game.native_fill(batch, [px, py, pill_w, 1.5], 0xFF59D9FF);
    game.native_text(batch, hint, [px + 12.0, py + 4.0], 0xFFFFFF, true);
}

/// Steps the player has just done. They are queued: most happen while the
/// window's client state is already borrowed, and are applied at the start of
/// the next frame.
static ONBOARDING_EVENTS: Mutex<Vec<OnboardingEvent>> = Mutex::new(Vec::new());

fn record_onboarding_event(event: OnboardingEvent) {
    lock(&ONBOARDING_EVENTS).push(event);
}

fn apply_onboarding_events() {
    let events = std::mem::take(&mut *lock(&ONBOARDING_EVENTS));
    for event in events {
        apply_onboarding_event(event);
    }
}

fn apply_onboarding_event(event: OnboardingEvent) {
    CLIENT.with_borrow_mut(|slot| {
        let Some(client) = slot.as_mut() else { return; };
        let before_all_done = client.onboarding.all_done();
        client.onboarding.observe(event);
        if !before_all_done && client.onboarding.all_done() {
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0.0, |d| d.as_secs_f64());
            client.onboarding.finished_at = Some(now_secs);
        }
        let s = client.onboarding.to_string();
        let mut guard = lock(&TABLET_SAVED);
        if let Some(saved) = guard.as_mut() {
            if saved.onboarding != s {
                saved.onboarding = s;
                lock(&TABLET_ORDERS).push(TabletAction::Close);
            }
        }
    });
}

pub(super) fn reset_onboarding(game: &mut Game<'_>) {
    CLIENT.with_borrow_mut(|slot| {
        if let Some(client) = slot.as_mut() {
            client.onboarding.reset();
            let mut guard = lock(&TABLET_SAVED);
            if let Some(saved) = guard.as_mut() {
                saved.onboarding = client.onboarding.to_string();
            }
            lock(&TABLET_ORDERS).push(TabletAction::Close);
        }
    });
    game.native_chat(&format!("Astral Cruiser: {}", tr(current_lang(), "chat.onboarding_reset")));
}

fn draw_onboarding_card(game: &Game<'_>, batch: &mut HudBatch, client: &Client) {
    if client.onboarding.hidden {
        return;
    }
    let all_done = client.onboarding.all_done();
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    if all_done {
        if let Some(ts) = client.onboarding.finished_at {
            if now_secs - ts > 5.0 {
                return;
            }
        }
    }

    let lang = current_lang();
    let card_w = 170.0;
    let row_h = 13.0;
    let header_h = 20.0;
    let card_h = header_h + 9.0 * row_h + 6.0;
    let x = batch.width - card_w - 8.0;
    let y = 44.0;

    let start = batch.vertices.len();
    orbit_frame(game, batch, [x, y, card_w, card_h], 0x59D9FF);
    game.native_text(batch, tr(lang, "onboarding.title"), [x + 10.0, y + 5.0], 0x59D9FF, true);
    let help_key = key_name(&game.native_mod_key(ID, HELP_KEY, HELP_KEY_DEFAULT));
    let hide_hint = format!("[{help_key}] {}", tr(lang, "onboarding.hide"));
    let hw = game.native_text_width(&hide_hint);
    game.native_text(batch, &hide_hint, [x + card_w - hw - 8.0, y + 5.0], 0x7A8E9B, true);
    game.native_fill(batch, [x + 8.0, y + 17.0, card_w - 16.0, 1.0], 0x603D5364);

    if all_done {
        game.native_fill(batch, [x + 6.0, y + 24.0, card_w - 12.0, 24.0], 0x3040FF60);
        let done_str = format!("✔ {}", tr(lang, "onboarding.done"));
        game.native_text(batch, &done_str, [x + 14.0, y + 30.0], 0x40FF60, true);
    } else {
        let cur_step = client.onboarding.current();
        for (i, step) in OnboardingStep::ALL.into_iter().enumerate() {
            let ry = y + 19.0 + i as f32 * row_h;
            let done = client.onboarding.done[step.index()];
            let is_cur = cur_step == Some(step);

            if is_cur {
                game.native_fill(batch, [x + 4.0, ry - 1.0, card_w - 8.0, row_h], 0x3059D9FF);
                game.native_fill(batch, [x + 4.0, ry - 1.0, 2.0, row_h], 0xFF59D9FF);
            }

            let check_icon = if done { "✔" } else if is_cur { "▶" } else { "○" };
            let check_color = if done { 0x40FF60 } else if is_cur { 0xFFFFFF } else { 0x506270 };
            game.native_text(batch, check_icon, [x + 8.0, ry], check_color, true);

            let title_color = if is_cur { 0xFFFFFF } else if done { 0x8FA4B2 } else { 0x607280 };
            let step_title = match step {
                OnboardingStep::CallShip => tr(lang, "onboarding.call_ship"),
                OnboardingStep::Board => tr(lang, "onboarding.board"),
                OnboardingStep::Seat => tr(lang, "onboarding.seat"),
                OnboardingStep::Fly => tr(lang, "onboarding.fly"),
                OnboardingStep::Wheel => tr(lang, "onboarding.wheel"),
                OnboardingStep::Fire => tr(lang, "onboarding.fire"),
                OnboardingStep::Orbit => tr(lang, "onboarding.orbit"),
                OnboardingStep::Strike => tr(lang, "onboarding.strike"),
                OnboardingStep::Tablet => tr(lang, "onboarding.tablet"),
            };
            game.native_text(batch, step_title, [x + 20.0, ry], title_color, true);

            let key_str = match step {
                OnboardingStep::CallShip => key_name(&game.native_mod_key(ID, CALL_KEY, CALL_KEY_DEFAULT)),
                OnboardingStep::Board => key_name(&game.native_mod_key(ID, BOARD_KEY, BOARD_KEY_DEFAULT)),
                OnboardingStep::Seat => key_name(&game.native_mod_key(ID, SEAT_KEY, SEAT_KEY_DEFAULT)),
                OnboardingStep::Fly => "WASD".to_string(),
                OnboardingStep::Wheel => key_name(&game.native_mod_key(ID, WHEEL_KEY, WHEEL_KEY_DEFAULT)),
                OnboardingStep::Fire => tr(lang, "onboarding.lmb").to_string(),
                OnboardingStep::Orbit => key_name(&game.native_mod_key(ID, ORBIT_KEY, ORBIT_KEY_DEFAULT)),
                OnboardingStep::Strike => tr(lang, "onboarding.lmb").to_string(),
                OnboardingStep::Tablet => key_name(&game.native_mod_key(ID, PANEL_KEY, PANEL_KEY_DEFAULT)),
            };
            let key_label = format!("[{key_str}]");
            let kw = game.native_text_width(&key_label);
            let kc = if is_cur { 0xFFD070 } else if done { 0x708090 } else { 0x506070 };
            game.native_text(batch, &key_label, [x + card_w - kw - 8.0, ry], kc, true);
        }
    }
    batch.scale_since(start, x + card_w, y, 0.85, 0.85);
}

fn call_ship(game: &mut Game<'_>) {
    let ship_opt = lock(&PUBLISHED).clone();
    let Some(player) = game.native_player() else {
        return;
    };
    let player_pos = DVec3::new(player.x, player.y, player.z);
    match ship_opt {
        None => {
            lock(&ORDERS).push(Order::Spawn);
        }
        Some(ship) => {
            let dim_matches = dim_of(ship.dimension) == game.dim;
            let dist = ship.pose.position.distance(player_pos);
            if !dim_matches || !within_boarding(ship.pose.position, player_pos) {
                // Nobody is aboard to fly it, so it comes the way it was first called.
                lock(&SERVER_ORDERS).push(ServerOrder::Spawn(SaveState {
                    dimension: dimension_of(game.dim),
                    pose: pilot::spawn_pose(player_pos, player.yaw),
                    aboard: false,
                    energy: ship.energy * Battery::new().energy,
                    heat: ship.heat,
                    speeds: ship.speeds,
                }));
                record_onboarding_event(OnboardingEvent::CalledShip);
                game.native_chat(&format!("Astral Cruiser: {}", tr(current_lang(), "chat.ship_arrived")));
            } else {
                game.native_chat(&format!("Astral Cruiser: {}", tr(current_lang(), "chat.ship_already_here")));
            }
        }
    }
}

fn board_ship(game: &mut Game<'_>) {
    let ship_opt = lock(&PUBLISHED).clone();
    let Some(player) = game.native_player() else {
        return;
    };
    let player_pos = DVec3::new(player.x, player.y, player.z);
    let is_outside = CLIENT.with_borrow(|slot| slot.as_ref().map_or(true, |c| matches!(c.mode, Mode::Outside)));
    if !is_outside {
        return;
    }
    let Some(ship) = ship_opt else {
        game.native_chat(&format!("Astral Cruiser: {}", tr(current_lang(), "chat.ship_not_found")));
        return;
    };
    if dim_of(ship.dimension) != game.dim {
        game.native_chat(&format!("Astral Cruiser: {}", tr(current_lang(), "chat.ship_other_dim")));
        return;
    }
    if within_boarding(ship.pose.position, player_pos) {
        lock(&ORDERS).push(Order::Board);
    } else {
        game.native_chat(&format!("Astral Cruiser: {}", tr(current_lang(), "chat.ship_far_call")));
    }
}

/// Close enough to go aboard: the ship is big and hovers, so its airlock is out
/// of reach on foot; anywhere beside the hull will do.
fn within_boarding(ship: DVec3, player: DVec3) -> bool {
    ship.distance(player) <= layout().half_extents().length() + 80.0
}

pub(super) fn hud(game: &mut Game<'_>, batch: &mut HudBatch) {
    let _timer = tablet_perf();
    if !game.native_on(ID) {
        return;
    }
    update_current_lang(game);
    let (darken_alpha, flash_alpha) = CLIENT.with_borrow(|slot| {
        let Some(client) = slot.as_ref() else { return (0.0, 0.0); };
        let flash = detonation_flash(client);
        let mut max_darken = 0.0f32;
        let here = client.current.as_ref().is_some_and(|s| dim_of(s.dimension) == game.dim);
        if here {
            let cam = if client.eye != DVec3::ZERO {
                client.eye
            } else {
                client.player_pos.unwrap_or(DVec3::ZERO)
            };

            for (shown, age) in &client.strikes_shown {
                if let StrikeShown::Fx { fx: Shown::Supernova, at, .. } = shown {
                    if *age < 10.0 && cam.distance(*at) <= 600.0 {
                        let prog = (*age / 10.0).clamp(0.0, 1.0);
                        max_darken = max_darken.max(prog * 0.45);
                    }
                }
            }

            if let Some(ship) = client.current.as_ref() {
                for strike in &ship.strikes {
                    if strike.weapon == Weapon::Supernova {
                        let age = strike.elapsed as f32;
                        if age < 10.0 {
                            let at = strike.target.unwrap_or(cam);
                            if cam.distance(at) <= 600.0 {
                                let prog = (age / 10.0).clamp(0.0, 1.0);
                                max_darken = max_darken.max(prog * 0.45);
                            }
                        }
                    }
                }
            }
        }
        if flash > 0.0 {
            max_darken = 0.0;
        }
        (max_darken, flash)
    });
    if darken_alpha > 0.0 {
        let a = ((darken_alpha * 255.0) as u32) << 24;
        game.native_fill(batch, [0.0, 0.0, batch.width, batch.height], a | 0x081028);
    }
    if flash_alpha > 0.0 {
        let a = ((flash_alpha * 255.0) as u32) << 24;
        game.native_fill(batch, [0.0, 0.0, batch.width, batch.height], a | 0xFFFFFF);
    }

    CLIENT.with_borrow(|slot| {
        if let Some(client) = slot.as_ref() {
            draw_onboarding_card(game, batch, client);
        }
    });

    let hint = CLIENT.with_borrow(|slot| {
        slot.as_ref().and_then(|client| current_hint(game, client))
    });
    if let Some(hint_text) = hint {
        draw_hint_pill(game, batch, &hint_text);
    }

    let shown = CLIENT.with_borrow(|slot| {
        let client = slot.as_ref()?;
        let ship = client.current.clone()?;
        let aboard = !matches!(client.mode, Mode::Outside);
        let piloting = matches!(client.mode, Mode::Piloting);
        aboard.then_some((ship, client.weapon, piloting, client.flash, client.strike, client.strike_hud))
    });
    let Some((ship, weapon, piloting, flash, strike, strike_hud)) = shown else {
        return;
    };
    if flash > 0.0 {
        let alpha = (flash.clamp(0.0, 1.0) * 255.0) as u32;
        game.native_fill(batch, [0.0, 0.0, batch.width, batch.height], alpha << 24 | 0xE6CCFF);
    }
    let line = format!(
        "Astral Cruiser  {:.0} b/s  X {:.0}  Y {:.0}  Z {:.0}",
        ship.velocity.length(),
        ship.pose.position.x,
        ship.pose.position.y,
        ship.pose.position.z,
    );
    game.native_text(batch, &line, [8.0, 8.0], 0x73F2FF, true);
    if let Some(dash) = &ship.dash {
        game.native_text(batch, &dash_line(dash), [8.0, 19.0], 0xFFD070, true);
    }
    if let Some(view) = ship.passage.iter().find(|view| !view.hidden) {
        game.native_text(batch, &portal_line(view), [8.0, 19.0], 0x73F2FF, true);
    }
    if let Some(jump) = &ship.jump {
        game.native_text(batch, &jump_line(jump), [8.0, 19.0], 0xD8A0FF, true);
    }
    if let Some(orbit) = &ship.orbit {
        let hud_key_name = key_name(&game.native_mod_key(ID, HUD_KEY, HUD_KEY_DEFAULT));
        let lang = current_lang();
        let line = if orbit.up {
            tr(lang, "orbit.legend").replace("{}", &hud_key_name)
        } else {
            tr(lang, "orbit.cancel_climb").to_string()
        };
        game.native_text(batch, &line, [8.0, 30.0], 0xFF8060, true);
    }
    if piloting && let Some(orbit) = &ship.orbit {
        orbit_panel(game, batch, &ship, orbit, strike, strike_hud);
        orbit_target(game, batch, &ship, orbit, strike);
        sight_mark(game, batch);
    } else if piloting {
        pilot_panel(game, batch, &ship, weapon);
        if !game.camera_type.first_person() {
            sight_mark(game, batch);
        }
    }
}

fn piloting() -> bool {
    CLIENT.with_borrow(|slot| {
        slot.as_ref().is_some_and(|c| matches!(c.mode, Mode::Piloting))
    })
}

pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !game.native_on(ID) {
        return;
    }
    let chosen = |name, default| key == game.native_mod_key(ID, name, default);
    if chosen(WHEEL_KEY, WHEEL_KEY_DEFAULT) {
        if pressed {
            if piloting() && at_the_controls(game) {
                open_wheel(game);
            }
        } else if wheel_active(game) {
            apply_wheel_selection(game);
        }
        return;
    }
    if chosen(CALL_KEY, CALL_KEY_DEFAULT) {
        if pressed && at_the_controls(game) {
            call_ship(game);
        }
        return;
    }
    if chosen(BOARD_KEY, BOARD_KEY_DEFAULT) {
        if pressed && at_the_controls(game) {
            board_ship(game);
        }
        return;
    }
    if !pressed || !at_the_controls(game) {
        return;
    }
    if chosen(SEAT_KEY, SEAT_KEY_DEFAULT) {
        lock(&ORDERS).push(Order::Seat);
    } else if chosen(PANEL_KEY, PANEL_KEY_DEFAULT) {
        lock(&ORDERS).push(Order::Panel);
    } else if chosen(STOP_KEY, STOP_KEY_DEFAULT) {
        lock(&ORDERS).push(Order::Stop);
    } else if chosen(ORBIT_KEY, ORBIT_KEY_DEFAULT) && piloting() {
        lock(&ORDERS).push(Order::Orbit);
    } else if chosen(HUD_KEY, HUD_KEY_DEFAULT) && piloting() {
        CLIENT.with_borrow_mut(|slot| {
            if let Some(client) = slot.as_mut() {
                client.strike_hud = client.strike_hud.next();
            }
        });
    } else if chosen(HELP_KEY, HELP_KEY_DEFAULT) {
        CLIENT.with_borrow_mut(|slot| {
            if let Some(client) = slot.as_mut() {
                client.onboarding.hidden = !client.onboarding.hidden;
                let s = client.onboarding.to_string();
                let mut guard = lock(&TABLET_SAVED);
                if let Some(saved) = guard.as_mut() {
                    saved.onboarding = s;
                    lock(&TABLET_ORDERS).push(TabletAction::Close);
                }
            }
        });
    } else if let Some(index) = GUN_KEYS.iter().position(|gun| chosen(gun.0, gun.3)).filter(|_| piloting()) {
        lock(&ORDERS).push(Order::Weapon(WeaponKind::ALL[index]));
    }
}

/// While piloting the wheel goes through the guns, not the hotbar; in orbit it
/// takes the view nearer the ground or further off.
pub(super) fn wheel(game: &mut Game<'_>, steps: f32, claimed: &mut bool) {
    if game.native_on(ID) && piloting() && !game.dialog_open() {
        *claimed = true;
        let shift = held(game, "key.sneak");
        let zoomed = CLIENT.with_borrow_mut(|slot| {
            let client = slot.as_mut()?;
            client.current.as_ref()?.orbit.as_ref()?;
            if shift {
                // Shift and the wheel go through the strikes.
                client.strike_selected = true;
                client.strike = client.strike.stepped(if steps > 0.0 { -1 } else { 1 });
            } else {
                let step = if steps > 0.0 { ORBIT_ZOOM_STEP } else { -ORBIT_ZOOM_STEP };
                client.orbit_zoom = (client.orbit_zoom + step).clamp(0.0, pilot::ORBIT_ZOOM_MAX);
            }
            Some(())
        });
        if zoomed.is_none() {
            lock(&ORDERS).push(Order::NextWeapon(if steps > 0.0 { -1 } else { 1 }));
        }
    }
}

/// `/astral strike ion|barrage|rod|lance|... X Z`.
fn strike_order(words: &[&str]) -> Option<Order> {
    let [kind, x, z] = words else {
        return None;
    };
    let kind = match *kind {
        "ion" => Weapon::Ion,
        "barrage" => Weapon::Barrage,
        "rod" => Weapon::Rod,
        "lance" => Weapon::Lance,
        "antimatter" => Weapon::Antimatter,
        "pillar" => Weapon::SolarPillar,
        "tectonic" => Weapon::Tectonic,
        "meteors" => Weapon::Meteors,
        "cluster" => Weapon::Cluster,
        "cryo" => Weapon::Cryo,
        "blackhole" => Weapon::BlackHole,
        "chain" => Weapon::Chain,
        "wall" => Weapon::PlasmaWall,
        "missiles" => Weapon::Missiles,
        "drill" => Weapon::Drill,
        "grid" => Weapon::Grid,
        "erasure" => Weapon::Erasure,
        "supernova" => Weapon::Supernova,
        _ => return None,
    };
    let (x, z): (f64, f64) = (x.parse().ok()?, z.parse().ok()?);
    if !x.is_finite() || !z.is_finite() { return None; }
    Some(Order::Strike(kind, x, z, None))
}

fn room_named(name: &str) -> Option<RoomKind> {
    match name {
        "bridge" => Some(RoomKind::Bridge),
        "corridor" => Some(RoomKind::Corridor),
        "reactor" => Some(RoomKind::Reactor),
        "guns" => Some(RoomKind::Weapons),
        "cargo" => Some(RoomKind::Hangar),
        "airlock" => Some(RoomKind::Airlock),
        "cabins" => Some(RoomKind::Quarters),
        "mess" => Some(RoomKind::Mess),
        "sickbay" => Some(RoomKind::Medbay),
        _ => None,
    }
}

/// `off`, or a thrust from 0 to 1 and a turn in degrees a second.
fn cruise_order(words: &[&str]) -> Option<Order> {
    match words {
        ["off"] => Some(Order::Cruise(None)),
        [thrust, turn] => {
            let thrust: f64 = thrust.parse().ok()?;
            let turn: f64 = turn.parse().ok()?;
            (thrust.is_finite() && turn.is_finite())
                .then(|| Order::Cruise(Some((thrust.clamp(0.0, 1.0), turn.to_radians()))))
        }
        _ => None,
    }
}

/// X Y Z: numbers, or `~` and an offset from where the ship is.
fn place_named(x: &str, y: &str, z: &str) -> Option<DVec3> {
    let ship = lock(&PUBLISHED).as_ref().map_or(DVec3::ZERO, |ship| ship.pose.position);
    Some(DVec3::new(coordinate(x, ship.x)?, coordinate(y, ship.y)?, coordinate(z, ship.z)?))
}

fn speeds_named(cruise: &str, dash: &str) -> Option<Speeds> {
    Some(Speeds::limited(cruise.parse().ok()?, dash.parse().ok()?))
}

fn dash_order(words: &[&str]) -> Option<Order> {
    match words {
        ["cancel"] => Some(Order::Dash(None)),
        [x, y, z] => place_named(x, y, z).map(|to| Order::Dash(Some(to))),
        _ => None,
    }
}

/// Nothing to open the panel, or what one of its buttons sends.
fn panel_order(words: &[&str]) -> Option<Order> {
    match words {
        [] => Some(Order::Panel),
        [how, x, y, z, cruise, dash] => Some(Order::Depart {
            jump: *how == "jump",
            to: place_named(x, y, z)?,
            speeds: speeds_named(cruise, dash)?,
        }),
        _ => None,
    }
}

/// Where to, and in the Nether how: into a cave if one is near, or onto the roof.
fn portal_order(words: &[&str]) -> Option<Order> {
    let usual = |to| Some(Order::Portal(Some((to, ExitKind::Usual))));
    match words {
        ["overworld"] => usual(Dimension::Overworld),
        ["end"] => usual(Dimension::End),
        ["nether"] | ["nether", "cave"] => Some(Order::Portal(Some((Dimension::Nether, ExitKind::Cave)))),
        ["nether", "roof"] => Some(Order::Portal(Some((Dimension::Nether, ExitKind::Roof)))),
        ["cancel"] => Some(Order::Portal(None)),
        ["go"] => Some(Order::PortalGo),
        _ => None,
    }
}

pub(super) fn command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    let words: Vec<&str> = command.split_whitespace().collect();
    if words.first() != Some(&"astral") || !game.native_on(ID) {
        return;
    }
    *ran = true;
    if words.get(1) == Some(&"perf") {
        notice(&lock(&FRAME_PERF).report("frame ms/count", &["strike_fx", "arsenal_fx", "debris", "glow", "guns", "HUD/tablet", "bolts", "debris_cubes"]));
        notice(&lock(&SERVER_PERF).report("tick ms/count", &["dig_ms", "changed", "scorch_ms", "wave_ms", "falling"]));
        FX_PERF_WANTED.store(true, Ordering::Relaxed);
        return;
    }
    if words.get(1) == Some(&"fx") {
        let named = words.get(2).map(|s| s.to_ascii_lowercase());
        let name = named.as_deref().map(|s| match s { "solarpillar" => "pillar", "plasmawall" => "wall", _ => s });
        if name == Some("all") {
            lock(&FX_DEBUG).push(None);
        } else if let Some(Order::Strike(w, ..)) = name.and_then(|w| strike_order(&[w, "0", "0"])) {
            lock(&FX_DEBUG).push(Some(w));
        } else {
            notice("Astral: /astral fx WEAPON|all (weapon names as in /astral strike)");
        }
        return;
    }
    // Probe: the main and the portal renderer swap their meshes, nothing else.
    let order = match words.get(1).copied() {
        Some("spawn") => Some(Order::Spawn),
        Some("board") => Some(Order::Board),
        Some("walk") | Some("seat") => Some(Order::Seat),
        Some("enter") => Some(Order::Enter),
        Some("leave") => Some(Order::Leave),
        Some("cruise") => cruise_order(&words[2..]),
        Some("dash") => dash_order(&words[2..]),
        Some("jump") => match words[2..] {
            [x, y, z] => place_named(x, y, z).map(Order::Jump),
            _ => None,
        },
        Some("speed") => match words[2..] {
            [cruise, dash] => speeds_named(cruise, dash).map(Order::Speeds),
            _ => None,
        },
        Some("stop") => Some(Order::Stop),
        Some("panel") => panel_order(&words[2..]),
        Some("view") => Some(Order::ShipEye),
        Some("tutorial") => Some(Order::Tutorial(words.get(2).and_then(|page| page.parse().ok()).unwrap_or(0))),
        Some("portal") => portal_order(&words[2..]),
        Some("go") => words.get(2).and_then(|name| room_named(name)).map(Order::Go),
        Some("blast") => match words.get(2).copied() {
            Some("on") => Some(Order::Blast(true)),
            Some("off") => Some(Order::Blast(false)),
            _ => None,
        },
        Some("remove") => Some(Order::Remove),
        Some("status") => Some(Order::Status),
        Some("orbit") => Some(Order::Orbit),
        Some("strike") => strike_order(&words[2..]),
        _ => None,
    };
    match order {
        Some(order) => lock(&ORDERS).push(order),
        None => lock(&NOTICES).push(
            tr(current_lang(), "chat.astral_help").to_string(),
        ),
    }
}
// Window-thread animation and bounded surface sampling. No terrain is changed here.
fn tick_spectacle(game: &Game<'_>, client: &mut Client) {
    if client.debris_dim != Some(game.dim) {
        client.debris.particles.clear();
        client.heated_surfaces.clear();
        client.strikes_shown.clear();
        client.pending_fx.clear();
        client.fx_lasers.clear();
        client.fx_launches.clear();
        client.effects = EffectEvents::default();
        client.ramp_open = 0.0;
        client.ramp_frame = None;
        client.guns = Guns::default();
        *lock(&GUN_READY) = [false; 5];
        client.debris_dim = Some(game.dim);
    }
    let debris_timer = PerfTimer::frame(2);
    client.debris.particles.retain(|p| p.pos.distance_squared(client.eye) <= 512.0 * 512.0);
    for (shown, _) in &client.strikes_shown {
        if let StrikeShown::Fx { fx: Shown::BlackHole, at, .. } = shown {
            for p in &mut client.debris.particles {
                let delta = *at - p.pos;
                if delta.length_squared() < 120.0 * 120.0 { p.vel += delta.normalize_or_zero() * 90.0 * TICK_SECONDS; }
            }
        }
    }
    client.debris.tick(TICK_SECONDS);
    for sample in lock(&DEBRIS_SHOWN).drain(..) {
        if sample.dim == game.dim {
            client.debris.spawn(sample.cell, sample.block, sample.center, sample.strength, sample.seed);
        }
    }
    drop(debris_timer);
    let Some(ship) = &client.current else { *lock(&GUN_READY) = [false; 5]; return; };
    let orbit = ship.orbit.as_ref();
    let all_held = orbit.is_some();
    let target = orbit.and_then(|o| o.aim).unwrap_or(client.eye + client.sight * 384.0);
    for gun in &mut client.guns.0 {
        let selected = matches!(client.mode, Mode::Piloting) && client.strike_selected && gun.battery == guns::battery(client.strike);
        let active = ship.strikes.iter().find(|s| guns::battery(s.weapon) == gun.battery);
        let target = active.and_then(|s| s.target).unwrap_or(target);
        gun.tick(TICK_SECONDS, Some(ship.pose.world_to_local(target)), all_held || selected || active.is_some());
    }
    for strike in &ship.strikes {
        let gun = &mut client.guns.0[guns::battery(strike.weapon) as usize];
        gun.tick(0.0, strike.target.map(|p| ship.pose.world_to_local(p)), true);
        gun.sync(strike.weapon, strike.elapsed);
        if strike.weapon == Weapon::Rod && strike.elapsed < 2.5 {
            let already = client.previous.as_ref().is_some_and(|s| s.strikes.iter().any(|old|
                old.weapon == Weapon::Rod && old.target == strike.target && old.elapsed <= strike.elapsed));
            if !already && let Some(to) = strike.target {
                let muzzle = ship.pose.local_to_world(client.guns.muzzle(Weapon::Rod));
                let from = muzzle.lerp(to, (strike.elapsed / 2.5).clamp(0.0, 1.0));
                client.pending_fx.push(StrikeShown::Shell { from, to, seconds: 2.5 - strike.elapsed, weapon: Weapon::Rod });
            }
        }
    }
}

fn hot_color(game: &Game<'_>, block: u32, heat: f64, alpha: f64) -> [f32; 4] {
    let facts = game.res.facts();
    let index = facts.get(horizon_world::registry::BlockStateId(block)).map_color as usize;
    let rgb = horizon_client::blocks::MAP_COLORS.get(index).copied().unwrap_or(0x777777);
    let h = heat.clamp(0.0, 1.0) as f32;
    let base = [((rgb >> 16) & 255) as f32 / 255.0, ((rgb >> 8) & 255) as f32 / 255.0, (rgb & 255) as f32 / 255.0];
    let hot = [1.0, 0.25 + h * 0.75, h * h * 0.9];
    [base[0] * (1.0-h) + hot[0] * h, base[1] * (1.0-h) + hot[1] * h, base[2] * (1.0-h) + hot[2] * h, alpha.clamp(0.0,1.0) as f32]
}

fn spectacle_cubes(game: &Game<'_>, client: &mut Client) -> Vec<LightCube> {
    let debris_timer = PerfTimer::frame(2);
    let mut cubes = Vec::with_capacity(client.debris.particles.len() + 512);
    for p in &client.debris.particles {
        cubes.push(LightCube {
            center: p.pos, rotation: DQuat::from_rotation_x(p.spin.x) * DQuat::from_rotation_y(p.spin.y) * DQuat::from_rotation_z(p.spin.z),
            half: 0.18 + 0.18 * p.heat,
            color: hot_color(game, p.block, p.heat, p.life.min(1.0)),
        });
    }
    lock(&FRAME_PERF).pending[7] += cubes.len() as f64;
    drop(debris_timer);
    let _timer = PerfTimer::frame(3);
    let Some(ship) = client.current.as_ref().filter(|s| dim_of(s.dimension) == game.dim) else { client.heated_surfaces.clear(); return cubes; };
    client.heated_surfaces.retain(|cache| ship.strikes.iter().any(|s|
        s.weapon == cache.weapon && s.target == Some(cache.target) && s.elapsed >= cache.elapsed
            && s.elapsed < astral_arsenal::spec(s.weapon).charge.max(1.5)));
    let scan_start = Instant::now();
    let mut scans_left = 48usize;
    let mut heated = 0;
    for strike in &ship.strikes {
        let w = strike.weapon;
        if !w.is_super() && w != Weapon::Rod && w != Weapon::BlackHole { continue; }
        let charge = astral_arsenal::spec(w).charge.max(1.5);
        if strike.elapsed >= charge { continue; }
        let Some(target) = strike.target else { continue; };
        let index = match client.heated_surfaces.iter().position(|c| c.weapon == w && c.target == target) {
            Some(i) => i,
            None => {
                let radius = match w { Weapon::Antimatter => 100, Weapon::Tectonic => 150, Weapon::SolarPillar => 25, Weapon::BlackHole => 40, Weapon::Erasure => 120, Weapon::Supernova => 150, _ => 24 };
                let step = (radius / 8).max(2) as usize;
                let mut columns = Vec::new();
                for dx in (-radius..=radius).step_by(step) {
                    for dz in (-radius..=radius).step_by(step) {
                        if dx * dx + dz * dz <= radius * radius { columns.push((target.x.floor() as i32 + dx, target.z.floor() as i32 + dz)); }
                    }
                }
                client.heated_surfaces.push(HeatedSurface { weapon: w, target, elapsed: strike.elapsed, columns, cursor: 0, cells: Vec::new() });
                client.heated_surfaces.len() - 1
            }
        };
        let cache = &mut client.heated_surfaces[index];
        cache.elapsed = strike.elapsed;
        // Each sampled column is scanned once per charge, spread across frames.
        while cache.cursor < cache.columns.len() && scans_left > 0 && scan_start.elapsed() < Duration::from_micros(500) {
            let (x, z) = cache.columns[cache.cursor];
            cache.cursor += 1;
            scans_left -= 1;
            if !game.world.chunks.contains_key(&(x >> 4, z >> 4)) { continue; }
            if let Some(y) = ground_at(&game.world, game.res.facts(), x, z, (target.y as i32 + 24).min(world_top(game.dim)), (target.y as i32 - 48).max(world_floor(game.dim))) {
                cache.cells.push((DVec3::new(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5), game.world.block(x, y, z).0));
            }
        }
        let heat = (strike.elapsed / charge).clamp(0.0,1.0);
        for &(center, block) in &cache.cells {
            if heated >= 512 { break; }
            cubes.push(LightCube { center, rotation: DQuat::IDENTITY, half: 0.505, color: hot_color(game, block, heat, 0.15 + heat * 0.65) });
            heated += 1;
        }
    }
    cubes
}

fn detonation_flash(client: &Client) -> f32 {
    client.strikes_shown.iter().filter_map(|(s, age)| {
        if let StrikeShown::Impact { at, weapon, .. } = s {
            if *age < 0.5 && (weapon.is_super() || *weapon == Weapon::Rod || *weapon == Weapon::BlackHole) {
                let strength = if weapon.is_super() { 1.0 } else { 0.6 };
                let distance = client.eye.distance(*at);
                return Some((1.0 - age / 0.5) * (strength / (1.0 + distance / 250.0)) as f32);
            }
        }
        None
    }).fold(0.0, f32::max)
}

// Shared selection entry point for the flight wheel (None selects ordinary lasers).
pub(super) fn select_strike(weapon: Option<Weapon>) {
    CLIENT.with_borrow_mut(|slot| {
        if let Some(client) = slot {
            client.strike_selected = weapon.is_some();
            if let Some(weapon) = weapon { client.strike = weapon; }
        }
    });
}

static WHEEL_ACTIVE: AtomicBool = AtomicBool::new(false);
static WHEEL_SECTOR: AtomicIsize = AtomicIsize::new(-1);
static WHEEL_CLICK: AtomicBool = AtomicBool::new(false);

pub(super) fn wheel_active(game: &Game<'_>) -> bool {
    let active = WHEEL_ACTIVE.load(Ordering::Relaxed);
    if active && (!game.dialog_open() || game.screen != Screen::Playing || !game.native_on(ID)) {
        WHEEL_ACTIVE.store(false, Ordering::Relaxed);
        WHEEL_SECTOR.store(-1, Ordering::Relaxed);
        return false;
    }
    active
}

pub(super) fn open_wheel(game: &mut Game<'_>) {
    if !piloting() {
        return;
    }
    let marker = r#"{type:"minecraft:notice",title:"astral_cruiser:wheel",pause:false,can_close_with_escape:true}"#;
    let Ok((tag, _)) = horizon_world::nbt::parse_snbt(marker) else { return; };
    game.show_dialog(&tag);
    if !game.dialog_open() { return; }
    WHEEL_ACTIVE.store(true, Ordering::Relaxed);
    WHEEL_SECTOR.store(-1, Ordering::Relaxed);
}

pub(super) fn close_wheel(game: &mut Game<'_>) {
    WHEEL_ACTIVE.store(false, Ordering::Relaxed);
    WHEEL_SECTOR.store(-1, Ordering::Relaxed);
    game.clear_dialog();
}

pub(super) fn click_wheel(_game: &Game<'_>) -> ControlFlow<horizon_client::Result<()>> {
    WHEEL_CLICK.store(true, Ordering::Relaxed);
    ControlFlow::Break(Ok(()))
}

pub(super) fn key_wheel(game: &mut Game<'_>, key: KeyCode) -> ControlFlow<horizon_client::Result<()>> {
    if matches!(key, KeyCode::Escape) {
        close_wheel(game);
        return ControlFlow::Break(Ok(()));
    }
    // A dialog hears presses only, never releases: the wheel key pressed again
    // (or Enter / Space) takes what the cursor is on, as a click does.
    let bound = game.native_mod_key(ID, WHEEL_KEY, WHEEL_KEY_DEFAULT);
    let letter = bound.strip_prefix("key.keyboard.").filter(|name| name.len() == 1);
    let wheel_key = letter.is_some_and(|name| format!("{key:?}") == format!("Key{}", name.to_uppercase()));
    if wheel_key || matches!(key, KeyCode::Enter | KeyCode::Space) {
        apply_wheel_selection(game);
    }
    ControlFlow::Break(Ok(()))
}

fn apply_pick(pick: weapon_wheel::Pick) {
    match pick {
        weapon_wheel::Pick::Strike(weapon) => {
            select_strike(Some(weapon));
        }
        weapon_wheel::Pick::Lasers(kind) => {
            CLIENT.with_borrow_mut(|slot| {
                if let Some(client) = slot.as_mut() {
                    client.weapon = kind;
                }
            });
            select_strike(None);
        }
    }
}

pub(super) fn apply_wheel_selection(game: &mut Game<'_>) {
    if !WHEEL_ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    let (w, h) = game.gui_size();
    let wheel = weapon_wheel::layout(w, h);
    let cur_sec = WHEEL_SECTOR.load(Ordering::Relaxed);
    let open_sec = if cur_sec >= 0 { Some(cur_sec as usize) } else { None };
    let hover = weapon_wheel::hit(&wheel, game.cursor.0, game.cursor.1, open_sec);
    match hover {
        weapon_wheel::Hover::Item(sec_idx, item_idx) => {
            if let Some(sec) = wheel.sectors.get(sec_idx) {
                if let Some(pick) = sec.items.get(item_idx) {
                    apply_pick(*pick);
                }
            }
        }
        weapon_wheel::Hover::Sector(sec_idx) => {
            if let Some(sec) = wheel.sectors.get(sec_idx) {
                if let Some(pick) = sec.items.first() {
                    apply_pick(*pick);
                }
            }
        }
        weapon_wheel::Hover::None => {}
    }
    close_wheel(game);
}

fn laser_name(kind: WeaponKind, lang: Lang) -> &'static str {
    match kind {
        WeaponKind::Turrets => tr(lang, "Turrets"),
        WeaponKind::NosePulse => tr(lang, "Nose guns"),
        WeaponKind::BellyBeam => tr(lang, "Belly beam"),
        WeaponKind::MainBeam => tr(lang, "Main beam"),
    }
}

fn laser_blurb(kind: WeaponKind, lang: Lang) -> &'static str {
    match kind {
        WeaponKind::Turrets => tr(lang, "blurb.turrets"),
        WeaponKind::NosePulse => tr(lang, "blurb.nose"),
        WeaponKind::BellyBeam => tr(lang, "blurb.belly"),
        WeaponKind::MainBeam => tr(lang, "blurb.main"),
    }
}

fn sector_label(sec_idx: usize, lang: Lang) -> &'static str {
    match sec_idx {
        0 => tr(lang, "wheel.lasers"),
        1 => tr(lang, "wheel.classic"),
        2 => tr(lang, "wheel.super"),
        3 => tr(lang, "wheel.zone"),
        4 => tr(lang, "wheel.special"),
        5 => tr(lang, "wheel.precise"),
        _ => "",
    }
}

pub(super) fn draw_wheel(game: &mut Game<'_>, batch: &mut HudBatch) -> ControlFlow<()> {
    if !wheel_active(game) {
        return ControlFlow::Continue(());
    }
    let lang = current_lang();
    record_onboarding_event(OnboardingEvent::OpenedWheel);
    game.native_fill(batch, [0.0, 0.0, batch.width, batch.height], 0x880A121A);

    let (w, h) = (batch.width, batch.height);
    let wheel = weapon_wheel::layout(w, h);
    let cur_sec = WHEEL_SECTOR.load(Ordering::Relaxed);
    let open_sec = if cur_sec >= 0 { Some(cur_sec as usize) } else { None };
    let hover = weapon_wheel::hit(&wheel, game.cursor.0, game.cursor.1, open_sec);

    if let weapon_wheel::Hover::Sector(s) = hover {
        WHEEL_SECTOR.store(s as isize, Ordering::Relaxed);
    } else if let weapon_wheel::Hover::Item(s, _) = hover {
        WHEEL_SECTOR.store(s as isize, Ordering::Relaxed);
    }
    let cur_sec = WHEEL_SECTOR.load(Ordering::Relaxed);
    let open_sec = if cur_sec >= 0 { Some(cur_sec as usize) } else { None };

    let (active_strike, active_weapon, strike_selected, cooldowns) = CLIENT.with_borrow(|slot| {
        let Some(c) = slot.as_ref() else {
            return (Weapon::Ion, WeaponKind::Turrets, false, [0.0; WEAPON_COUNT]);
        };
        let cds = c.current.as_ref().map_or([0.0; WEAPON_COUNT], |s| s.cooldowns);
        (c.strike, c.weapon, c.strike_selected, cds)
    });

    let (cx, cy) = wheel.center;

    // 1. Inner ring: 6 wedge-shaped sectors
    for (i, sec) in wheel.sectors.iter().enumerate() {
        let is_hover = open_sec == Some(i);
        let cr = ((sec.color >> 16) & 255) as u8;
        let cg = ((sec.color >> 8) & 255) as u8;
        let cb = (sec.color & 255) as u8;

        let body_color = if is_hover {
            horizon_client::hud::rgba(38, 70, 95, 230)
        } else {
            horizon_client::hud::rgba(14, 24, 34, 200)
        };
        let edge_color = horizon_client::hud::rgba(cr, cg, cb, if is_hover { 255 } else { 190 });

        let segs = 10;
        let gap = 0.015;
        let start_a = sec.start + gap;
        let end_a = sec.end - gap;
        let seg_step = (end_a - start_a) / segs as f32;
        let r0 = wheel.inner_r0;
        let r1 = wheel.inner;
        let r_edge = r1 - 2.5;

        for j in 0..segs {
            let a0 = start_a + j as f32 * seg_step;
            let a1 = start_a + (j + 1) as f32 * seg_step;
            let (s0, c0) = (a0.sin(), a0.cos());
            let (s1, c1) = (a1.sin(), a1.cos());

            let p00 = [cx + r0 * s0, cy - r0 * c0];
            let p01 = [cx + r0 * s1, cy - r0 * c1];
            let pe0 = [cx + r_edge * s0, cy - r_edge * c0];
            let pe1 = [cx + r_edge * s1, cy - r_edge * c1];
            let p10 = [cx + r1 * s0, cy - r1 * c0];
            let p11 = [cx + r1 * s1, cy - r1 * c1];

            batch.fill_quad(&game.gui.atlas, [p00, p01, pe1, pe0], body_color);
            batch.fill_quad(&game.gui.atlas, [pe0, pe1, p11, p10], edge_color);
        }

        // Group label placed at sector middle
        let mid_a = (sec.start + sec.end) * 0.5;
        let r_mid = (r0 + r1) * 0.5;
        let lx = cx + r_mid * mid_a.sin();
        let ly = cy - r_mid * mid_a.cos();
        let label = sector_label(i, lang);
        let tw = game.native_text_width(label);
        let label_color = if is_hover { 0xFFFFFF } else { 0xC0CED8 };
        game.native_text(batch, label, [lx - tw * 0.5, ly - 4.0], label_color, true);
    }

    // 2. Outer ring: ONLY the items of the hovered (or last hovered) sector
    if let Some(target_sec) = open_sec {
        if let Some(sec) = wheel.sectors.get(target_sec) {
            for (it_idx, pick) in sec.items.iter().enumerate() {
                let (ix, iy) = weapon_wheel::item_pos(&wheel, target_sec, it_idx);
                let is_item_hover = hover == weapon_wheel::Hover::Item(target_sec, it_idx);

                let base_w = 84.0;
                let base_h = 22.0;
                let (pw, ph) = if is_item_hover {
                    (base_w + 6.0, base_h + 4.0)
                } else {
                    (base_w, base_h)
                };
                let px = ix - pw * 0.5;
                let py = iy - ph * 0.5;

                let (name, color, cd, max_cd, selected) = match pick {
                    weapon_wheel::Pick::Lasers(k) => {
                        let sel = !strike_selected && active_weapon == *k;
                        (laser_name(*k, lang), 0x59D9FF, 0.0, 1.0, sel)
                    }
                    weapon_wheel::Pick::Strike(w) => {
                        let spec = astral_arsenal::spec(*w);
                        let sel = strike_selected && active_strike == *w;
                        let c = cooldowns[*w as usize];
                        (astral_arsenal::display_name(*w, lang), spec.color, c, spec.cooldown, sel)
                    }
                };

                let bg = if is_item_hover {
                    0xF02B566B
                } else if selected {
                    0xD01E3C4D
                } else {
                    0xD8101C28
                };
                game.native_fill(batch, [px, py, pw, ph], bg);
                game.native_fill(batch, [px, py, pw, 1.5], color | 0xFF000000);
                game.native_fill(batch, [px, py, 3.0, ph], color | 0xFF000000);
                if selected {
                    game.native_fill(batch, [px + pw - 2.0, py, 2.0, ph], 0xFF73F2FF);
                }

                let name_color = if is_item_hover || selected { 0xFFFFFF } else { 0xD0E2EB };
                let fit_len = game.gui.atlas.font.fit(name, pw - 10.0);
                let name_trimmed: String = name.chars().take(fit_len).collect();
                game.native_text(
                    batch,
                    &name_trimmed,
                    [px + 6.0, py + if is_item_hover { 3.0 } else { 2.0 }],
                    name_color,
                    true,
                );

                if cd > 0.0 {
                    let cd_str = format!("{cd:.1}s");
                    game.native_text(
                        batch,
                        &cd_str,
                        [px + 6.0, py + if is_item_hover { 13.0 } else { 12.0 }],
                        0xFFA040,
                        true,
                    );
                    let share = 1.0 - (cd / max_cd.max(0.05)).clamp(0.0, 1.0);
                    game.native_fill(
                        batch,
                        [px + 2.0, py + ph - 2.0, (pw - 4.0) * share as f32, 1.5],
                        color | 0xFF000000,
                    );
                } else {
                    game.native_text(
                        batch,
                        tr(lang, "READY"),
                        [px + 6.0, py + if is_item_hover { 13.0 } else { 12.0 }],
                        if is_item_hover { 0x40FF60 } else { color },
                        true,
                    );
                }
            }
        }
    }

    // 3. What the cursor is on: below the wheel, above the hotbar. The hub in
    // the middle is too small for it and covered the sector labels.
    let (_, screen_h) = game.gui_size();
    let hw = (wheel.min_dim * 0.5).max(180.0);
    let hh = 46.0;
    let hx = cx - hw * 0.5;
    let hy = (screen_h - hh - 34.0).max(cy + wheel.min_dim * 0.2);
    orbit_frame(game, batch, [hx, hy, hw, hh], 0x59D9FF);

    match hover {
        weapon_wheel::Hover::Item(s_idx, it_idx) => {
            if let Some(sec) = wheel.sectors.get(s_idx) {
                if let Some(pick) = sec.items.get(it_idx) {
                    let (name, blurb, cost_cd, color) = match pick {
                        weapon_wheel::Pick::Lasers(k) => {
                            let b = laser_blurb(*k, lang);
                            (laser_name(*k, lang), b, tr(lang, "wheel.laser_energy").to_string(), 0x59D9FF)
                        }
                        weapon_wheel::Pick::Strike(w) => {
                            let name = astral_arsenal::display_name(*w, lang);
                            let blurb = astral_arsenal::display_blurb(*w, lang);
                            let spec = astral_arsenal::spec(*w);
                            let cd = cooldowns[*w as usize];
                            let cd_str = if cd > 0.0 { format!("{} {cd:.1}s", tr(lang, "wheel.cd")) } else { tr(lang, "wheel.ready").to_string() };
                            let cost = if spec.energy > 0.0 { spec.energy } else { spec.energy_per_second };
                            let cost_str = if spec.energy_per_second > 0.0 { format!("{} {:.0}/{}", tr(lang, "wheel.cost"), spec.energy_per_second, tr(lang, "tablet.sec_short")) } else { format!("{} {:.0}", tr(lang, "wheel.cost"), cost) };
                            (name, blurb, format!("{cost_str} • {cd_str}"), spec.color)
                        }
                    };
                    let nw = game.native_text_width(name);
                    game.native_text(batch, name, [cx - nw * 0.5, hy + 5.0], color, true);

                    let fit_len = game.gui.atlas.font.fit(blurb, hw - 10.0);
                    let blurb_trimmed: String = blurb.chars().take(fit_len).collect();
                    let bw = game.native_text_width(&blurb_trimmed);
                    game.native_text(batch, &blurb_trimmed, [cx - bw * 0.5, hy + 18.0], 0xC0CED8, true);

                    let cw = game.native_text_width(&cost_cd);
                    game.native_text(batch, &cost_cd, [cx - cw * 0.5, hy + 31.0], 0x59D9FF, true);
                }
            }
        }
        weapon_wheel::Hover::Sector(s_idx) => {
            if let Some(sec) = wheel.sectors.get(s_idx) {
                let title = sector_label(s_idx, lang);
                let info = format!("{} {}", sec.items.len(), tr(lang, "wheel.options"));
                let tw = game.native_text_width(title);
                game.native_text(batch, title, [cx - tw * 0.5, hy + 10.0], sec.color, true);
                let iw = game.native_text_width(&info);
                game.native_text(batch, &info, [cx - iw * 0.5, hy + 24.0], 0xC0CED8, true);
            }
        }
        weapon_wheel::Hover::None => {
            let msg = tr(lang, "wheel.hover_placeholder");
            let mw = game.native_text_width(msg);
            game.native_text(batch, msg, [cx - mw * 0.5, hy + hh * 0.5 - 4.0], 0x9AABB8, true);
        }
    }

    ControlFlow::Break(())
}
