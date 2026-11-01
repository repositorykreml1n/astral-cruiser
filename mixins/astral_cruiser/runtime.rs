// The ship in the running game. The server thread owns it: it flies the ship
// against the real world and saves it. The window only sends the pilot's input,
// keeps whoever is aboard on the ship and draws the latest snapshot. The two
// threads meet in the statics below; commands may arrive on either.
use super::astral_core::{
    CollisionQuery, Dimension, Flight, HULL_HALF_EXTENTS, NavigationLimits, NavigationTarget,
    ShipInput, ShipPose,
};
use super::astral_dash::{
    Dash, Moment, Stage, areas_in_flight, chunks_of, coordinate, dash_areas, dash_shows_far_terrain,
    far_reach, fog_reach, line_clear, plan, within_world,
};
use super::astral_jump::{Jump, JumpStage, Leap, arrival_area, plan_jump};
use super::astral_panel::{self as panel, PanelState, Place};
use super::astral_tutorial::{self as tutorial, key_name};
use super::astral_passage::{
    ExitKind, Finding, Gate, OPENING, PassStage, Passage, Stride, chase_behind, room_at,
    through_opening,
};
use super::astral_scene as scene;
use super::astral_pilot::{self as pilot, Arrival, Footing, PilotKeys, ShipIs, Step};
use super::astral_save::{SaveState, Speeds};
use super::astral_weapons::{
    Battery, Body, Cell as ShotCell, Struck, TURRETS, WeaponKind, first_hit, sight_reach,
    within_cone,
};
use super::lan::LanHost;
use super::perspective::CameraType;
use super::server_travel::LevelTeleport;
use super::{Dim, Game, Screen};
use glam::DVec3;
use horizon_client::blocks::Fluid;
use horizon_client::gfx::astral_hull::{Bolt, Swirl, arrival_ring, warp_bolts};
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
    sync::{Mutex, MutexGuard, OnceLock, PoisonError},
    time::{Duration, Instant},
};

pub const ID: &str = "astral_cruiser";
pub const NAME: &str = "Astral Cruiser";
pub const VERSION: &str = "0.1.0";
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
/// The setting that remembers the tutorial has been shown.
const LESSON_SEEN: &str = "tutorial_seen";
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
    /// Where the pilot's line of sight starts, and which way it runs.
    eye: DVec3,
    sight: DVec3,
}

const NO_CONTROLS: Controls = Controls {
    keys: NO_KEYS,
    look: f32::NAN,
    firing: false,
    weapon: WeaponKind::Turrets,
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
#[derive(Clone, Copy, Debug)]
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
    blasting: bool,
    /// Crossing over the world: only a little of it is kept loaded.
    light: bool,
}

static ORDERS: Mutex<Vec<Order>> = Mutex::new(Vec::new());
static SERVER_ORDERS: Mutex<Vec<ServerOrder>> = Mutex::new(Vec::new());
static CONTROLS: Mutex<Controls> = Mutex::new(NO_CONTROLS);
static SHOTS: Mutex<Vec<ShotEvent>> = Mutex::new(Vec::new());
static PUBLISHED: Mutex<Option<Published>> = Mutex::new(None);
/// What the server has to tell the player.
static NOTICES: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// The level a portal keeps running with nobody in it.
static KEPT_LEVEL: Mutex<Option<Dim>> = Mutex::new(None);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn notice(text: &str) {
    lock(&NOTICES).push(format!("Astral Cruiser: {text}"));
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
        }
    }

    fn carrying(&self) -> bool {
        self.landing > 0 || self.ship.as_ref().is_some_and(|s| s.crew != Crew::Away)
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
            notice(&format!("no jump: {why}."));
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
                    Err(why) => ended = Some(format!("no jump: {why}.")),
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
                    ended = Some(format!("arrived at {x:.1} {y:.1} {z:.1}."));
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
            notice(&format!("no dash: {why}."));
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
                        Err(why) => ended = Some(format!("no dash: {why}.")),
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
                        ended = Some(format!("arrived at {x:.1} {y:.1} {z:.1}."));
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
        if self.passage(game) || self.jump(game) || self.dash(game) {
            return;
        }
        let Some(ship) = &mut self.ship else {
            return;
        };
        let before = ship.flight.snapshot();
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
                        area: passage.exit_area(),
                        near_area: passage.entry_area(),
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
                blasting: self.blasting,
                light: self.light,
            }
        })
    }
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
        server.fly(game);
        server.fire(game);
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
    client.mode = mode;
    client.riding = false;
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
        let (event, volume, pitch) = firing_sound(shot.kind);
        let at = pose.local_to_world(layout().hardpoints[shot.mount].position);
        game.audio.play(event, Some(at.to_array()), volume, pitch);
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
        let passage = ship.and_then(|ship| ship.passage.as_ref());
        let near = passage.filter(|passage| dim_of(passage.gate.from) == game.dim);
        let far = passage.filter(|passage| dim_of(passage.gate.to) == game.dim);
        let light = slot.as_ref().is_some_and(|server| server.light);
        let areas = dashing.iter().flat_map(|dashing| areas_in_flight(&dashing.areas, light).iter().copied());
        let wanted = areas
            .chain(far.map(Passage::exit_area))
            .chain(near.map(Passage::entry_area))
            .chain(jumping.map(|jumping| jumping.area));
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

fn open_tutorial(game: &mut Game<'_>, page: usize) {
    let name = |name, default| key_name(&game.native_mod_key(ID, name, default));
    let keys = tutorial::Keys {
        seat: name(SEAT_KEY, SEAT_KEY_DEFAULT),
        panel: name(PANEL_KEY, PANEL_KEY_DEFAULT),
        guns: GUN_KEYS.map(|gun| name(gun.0, gun.3)),
        stop: name(STOP_KEY, STOP_KEY_DEFAULT),
        help: name(HELP_KEY, HELP_KEY_DEFAULT),
    };
    let russian = game.settings.language.starts_with("ru");
    match horizon_world::nbt::parse_snbt(&tutorial::dialog(page, russian, &keys)) {
        Ok((tag, _)) => game.show_dialog(&tag),
        Err(e) => log::error!("astral cruiser: the tutorial: {e}"),
    }
}

/// The first time a player takes the helm the tutorial opens by itself, once.
fn first_lesson(game: &mut Game<'_>, client: &mut Client) {
    if client.lesson_checked || !matches!(client.mode, Mode::Piloting) || game.dialog_open() {
        return;
    }
    client.lesson_checked = true;
    if game.native_setting(ID, LESSON_SEEN).is_none() {
        game.native_set_setting(ID, LESSON_SEEN, "true");
        open_tutorial(game, 0);
    }
}

/// While the ship crosses over the world far terrain draws a window moving with
/// the ship, and afterwards the player's own settings come back.
fn ease_the_view(game: &mut Game<'_>, client: &mut Client) {
    let light = client.current.is_some_and(|ship| ship.light);
    match (light, client.kept_view) {
        (true, None) => {
            let lod = game.settings.horizon.lod_distance;
            client.kept_view = Some((lod, game.streaming.radius));
            LIGHT_VIEW.set(true);
            game.streaming.radius = DASH_VIEW;
            let left = client.current.and_then(|ship| ship.dash).map_or(0.0, |dash| dash.left);
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
            let velocity = client.current.map_or(DVec3::ZERO, |ship| ship.velocity);
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
    let passage = client.current.and_then(|ship| ship.passage);
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
    let passage = client.current.and_then(|ship| ship.passage);
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

/// The window's tick: after the player's own physics, which the ship overrides.
fn window_tick(game: &mut Game<'_>) {
    let mut latest = *lock(&PUBLISHED);
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
        client.previous = client.current;
        client.current = latest;
        if let (Some(before), Some(now), Some(door)) =
            (&mut client.previous, client.current, client.doorway)
            && before.dimension != now.dimension
        {
            before.pose.position += door.gate.shift();
            before.dimension = now.dimension;
        }
        // A jump: no flight from there to here to smooth over.
        if let (Some(before), Some(now)) = (client.previous, client.current)
            && before.leaps != now.leaps
        {
            client.previous = client.current;
            client.flash = 1.0;
            client.arrival = Some(0.0);
            if let Some(door) = &mut client.doorway {
                door.through = true;
                door.veil = 1.0;
            }
        }
        client.flash = (client.flash - FLASH_FADE).max(0.0);
        client.arrival = client.arrival.map(|ticks| ticks + 1.0).filter(|&t| t < ARRIVAL_TICKS);
        client.warp_spin += warp_rate(client.current.as_ref()) * TICK_SECONDS;
        tend_doorway(game, client);
        ease_the_view(game, client);
        first_lesson(game, client);
        if client.chasing && !matches!(client.mode, Mode::Piloting) {
            client.chasing = false;
            game.camera_type = CameraType::FirstPerson;
        }
        let Some(current) = client.current else {
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
        let previous = client.previous.unwrap_or(current);
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
                let playing = at_the_controls(game);
                let keys = if playing { pilot_keys(game) } else { NO_KEYS };
                *lock(&CONTROLS) = Controls {
                    keys,
                    look: game.yaw,
                    firing: playing && held(game, "key.attack"),
                    weapon: client.weapon,
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
    change_mode(client, Mode::Piloting);
    client.chasing = true;
    game.camera_type = CameraType::ThirdPersonBack;
    client.seen_camera = Some(CameraType::ThirdPersonBack);
    game.native_chat("Astral Cruiser: WASD fly, Space and Shift climb and dive, Ctrl boosts, the ship follows your view. Left mouse fires, 1-4 or the wheel picks a weapon. N opens the control panel. F5 goes through the views, one of them the ship's own. V stands up.");
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
        game.native_chat("Astral Cruiser: something blocks the outer door.");
        return;
    }
    let may_fly = game.player.as_ref().is_some_and(|p| p.may_fly);
    let ground = pilot::landing_y(y, world_floor(game.dim), |at| arrival(game, x, at, z));
    let exit = match ground {
        _ if may_fly => door,
        Some(ground) => DVec3::new(door.x, f64::from(ground), door.z),
        None => {
            game.native_chat("Astral Cruiser: no safe ground under the door. Fly somewhere else.");
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
        game.native_chat("Astral Cruiser: beamed down. /astral enter brings you back from near the door.");
    }
}

fn dash_line(dash: &DashView) -> String {
    let [x, y, z] = dash.target.to_array();
    let stage = match dash.stage {
        Stage::Warming => format!("preparing the destination {:.0}%", dash.share * 100.0),
        Stage::Charging => format!("charging {:.0}%", dash.share * 100.0),
        Stage::Flying => format!("{:.0} blocks to go", dash.left),
        Stage::Braking => "braking".to_string(),
    };
    format!("Dash to {x:.0} {y:.0} {z:.0}: {stage}")
}

/// The panel, as the ship stands now.
fn open_panel(game: &mut Game<'_>, client: &Client) {
    let ship = client.current;
    let place = match ship {
        None => Place::NoShip,
        Some(_) if matches!(client.mode, Mode::Outside) => Place::Outside,
        Some(_) => Place::Aboard,
    };
    let position = ship.map_or(DVec3::ZERO, |ship| ship.pose.position);
    let status = ship.map_or(String::new(), |ship| {
        let [x, y, z] = position.to_array();
        format!("{:?}  X {x:.0}  Y {y:.0}  Z {z:.0}  {:.0}%", ship.dimension, ship.energy * 100.0)
    });
    let speeds = ship.map_or(Speeds::default(), |ship| ship.speeds);
    let open = ship.and_then(|ship| ship.passage);
    let state = PanelState {
        russian: game.settings.language.starts_with("ru"),
        place,
        status,
        dimension: ship.map_or(Dimension::Overworld, |ship| ship.dimension),
        target: client.target.unwrap_or(position).to_array(),
        cruise: speeds.cruise,
        dash: speeds.dash,
        blasting: ship.is_some_and(|ship| ship.blasting),
        busy: ship.is_some_and(|ship| ship.dash.is_some() || ship.passage.is_some()),
        portal_open: open.is_some_and(|view| view.stage == PassStage::Open),
    };
    match horizon_world::nbt::parse_snbt(&panel::dialog(&state)) {
        Ok((tag, _)) => game.show_dialog(&tag),
        Err(e) => log::error!("astral cruiser: the panel: {e}"),
    }
}

fn jump_line(jump: &JumpView) -> String {
    let [x, y, z] = jump.target.to_array();
    let stage = match jump.stage {
        JumpStage::Gathering => format!("charging {:.0}%", jump.share * 100.0),
        JumpStage::Arriving => "arriving".to_string(),
    };
    format!("Jump to {x:.0} {y:.0} {z:.0}: {stage}")
}

fn portal_line(view: &PassView) -> String {
    let to = match view.gate.to {
        Dimension::Overworld => "the Overworld",
        Dimension::Nether => "the Nether",
        Dimension::End => "the End",
    };
    let stage = match view.stage {
        PassStage::Preparing => match view.looking {
            Some((window, windows)) => {
                format!("looking for room {window}/{windows}, {:.0}%", view.share * 100.0)
            }
            None => format!("preparing the far side {:.0}%", view.share * 100.0),
        },
        PassStage::Open => "open. Fly in nose first, or /astral portal go; /astral portal cancel closes it".to_string(),
        PassStage::Going | PassStage::Through => "going through".to_string(),
    };
    format!("Portal to {to}: {stage}")
}

fn status_line(client: &Client) -> String {
    let Some(p) = client.current else {
        return "astral status: no ship".to_string();
    };
    let aboard = match client.mode {
        Mode::Outside => "outside".to_string(),
        Mode::Piloting => "piloting".to_string(),
        Mode::Walking { passenger, .. } => format!(
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
        | Order::Status => None,
    }
}

fn act(game: &mut Game<'_>, client: &mut Client, order: Order) {
    if game.integrated.is_none() && !matches!(order, Order::Status) {
        game.native_chat("Astral Cruiser works in single player only for now.");
        return;
    }
    let ship = client.current;
    let place = match ship {
        None => ShipIs::Missing,
        Some(s) if dim_of(s.dimension) == game.dim => ShipIs::Here,
        Some(_) => ShipIs::Elsewhere,
    };
    let aboard = !matches!(client.mode, Mode::Outside);
    if let Some(why) = step_of(order).and_then(|step| pilot::refusal(step, aboard, place)) {
        game.native_chat(why);
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
            game.native_chat("Astral Cruiser: ship spawned ahead of you. /astral board takes you to the controls.");
        }
        (Order::Remove, _) => {
            lock(&SERVER_ORDERS).push(ServerOrder::Remove);
            game.native_chat("Astral Cruiser: ship removed.");
        }
        (Order::Status, _) => {
            let line = status_line(client);
            log::info!("{line}");
            game.native_chat(&line);
        }
        (Order::Weapon(kind), _) => client.weapon = kind,
        (Order::NextWeapon(by), _) => client.weapon = client.weapon.stepped(by),
        (Order::Blast(on), _) => {
            lock(&SERVER_ORDERS).push(ServerOrder::Blast(on));
            let state = if on { "on: shots tear up the world" } else { "off" };
            game.native_chat(&format!("Astral Cruiser: blasting is {state}."));
        }
        (Order::Panel, _) => open_panel(game, client),
        (Order::Tutorial(page), _) => open_tutorial(game, page),
        (Order::ShipEye, _) if matches!(client.mode, Mode::Piloting) => {
            client.ship_eye = !client.ship_eye;
            game.camera_type = CameraType::ThirdPersonBack;
            client.seen_camera = Some(CameraType::ThirdPersonBack);
        }
        (Order::ShipEye, _) => game.native_chat("Astral Cruiser: only from the helm."),
        // The seat key is often pressed with no ship about.
        (Order::Seat, None) => {}
        (_, None) => game.native_chat("Astral Cruiser: no ship here. Use /astral spawn."),
        (Order::Board, Some(ship)) => {
            if matches!(client.mode, Mode::Outside) {
                beam_aboard(&ship, layout().pilot_seat());
            }
            sit_down(game, client);
        }
        (Order::Seat, Some(ship)) => match client.mode {
            Mode::Piloting => change_mode(client, on_the_bridge()),
            Mode::Walking { passenger, .. } => {
                if passenger.local.distance(layout().pilot_seat()) <= SEAT_REACH {
                    sit_down(game, client);
                }
            }
            Mode::Outside => {
                if dim_of(ship.dimension) == game.dim {
                    come_in(game, client, &ship);
                }
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
            _ => game.native_chat("Astral Cruiser: you are not aboard."),
        },
        (Order::Enter, Some(ship)) => {
            if !come_in(game, client, &ship) {
                game.native_chat("Astral Cruiser: too far from the airlock door.");
            }
        }
        (Order::Leave, Some(ship)) if ship.dash.is_some() => {
            game.native_chat("Astral Cruiser: not during a dash. /astral dash cancel stops it.");
        }
        (Order::Leave, Some(ship)) if ship.jump.is_some() || ship.passage.is_some_and(|view| {
            matches!(view.stage, PassStage::Going | PassStage::Through)
        }) => {
            game.native_chat("Astral Cruiser: not now. /astral stop calls it off.");
        }
        (Order::Leave, Some(ship)) => disembark(game, client, &ship),
        (Order::Dash(_) | Order::Jump(_) | Order::Depart { .. }, Some(_)) if !aboard => {
            game.native_chat("Astral Cruiser: board the ship first.");
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
            game.native_chat("Astral Cruiser: board the ship first.");
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
    let passage = client.current.and_then(|ship| ship.passage);
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
    let orders = std::mem::take(&mut *lock(&ORDERS));
    CLIENT.with_borrow_mut(|slot| {
        let client = slot.get_or_insert_with(Client::default);
        // Going through a portal the player is gone for a moment, not the world.
        if game.native_player().is_none() && client.doorway.is_none() {
            // No world: nothing known of the last one may reach the next.
            if let Some(kept) = client.kept_view {
                restore_view(game, kept);
            }
            *client = Client::default();
            lock(&SERVER_ORDERS).clear();
            lock(&NOTICES).clear();
            astral_ship::set_view(None);
            astral_ship::set_clip(None);
            astral_ship::set_swirl(None);
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
        check_foot_crossing(game, client);
        show_portal(game, client);
    });
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
    astral_ship::set_chase(None);
    PILOT_VIEW.set(false);
    CLIENT.with_borrow_mut(|slot| {
        let client = slot.get_or_insert_with(Client::default);
        let shown = client.current.filter(|c| {
            dim_of(c.dimension) == game.dim && game.native_player().is_some() && game.native_on(ID)
        });
        let Some(current) = shown else {
            astral_ship::set_view(None);
            astral_ship::set_swirl(None);
            ROOF.set(None);
            return;
        };
        let previous = client.previous.unwrap_or(current);
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
        let mut light = bolts(client, &current, &pose);
        light.extend(warp_bolts(stern, pose.rotation, spin, power));
        if let Some(ticks) = client.arrival {
            light.extend(arrival_ring(pose.position, pose.rotation, ticks / ARRIVAL_TICKS));
        }
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
    });
}

/// The hull as solid boxes around someone outside, added to what the player's
/// own motion already collides with. The game's list comes from the same hook
/// entered again, which then lets the game's own code run.
pub(super) fn solid_hull(game: &Game<'_>) -> ControlFlow<Vec<Aabb>> {
    if COLLECTING.get() {
        return ControlFlow::Continue(());
    }
    let ship = CLIENT.with_borrow(|slot| {
        let client = slot.as_ref()?;
        let outside = matches!(client.mode, Mode::Outside);
        client.current.filter(|c| outside && dim_of(c.dimension) == game.dim)
    });
    let (Some(ship), Some(player)) = (ship, game.native_player()) else {
        return ControlFlow::Continue(());
    };
    COLLECTING.set(true);
    let mut boxes = game.player_entity_collisions();
    COLLECTING.set(false);
    let at = DVec3::new(player.x, player.y, player.z);
    let cells = pilot::solid_cells(&ship.pose, at, HULL_REACH, HULL_CELL, |x, z| {
        layout().hull_spans(x, z)
    });
    boxes.extend(cells.into_iter().map(|[x, y, z, right, top, front]| {
        Aabb::new(Vec3::new(x, y, z), Vec3::new(right, top, front))
    }));
    ControlFlow::Break(boxes)
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
    const LEFT: f32 = 8.0;
    let bottom = batch.height - 34.0;
    let mut y = bottom - 76.0;
    for (index, kind) in WeaponKind::ALL.into_iter().enumerate() {
        let selected = kind == weapon;
        if selected {
            game.native_fill(batch, [LEFT - 3.0, y - 2.0, 96.0, 12.0], 0x6030A0B0);
        }
        let color = if selected { 0x73F2FF } else { 0x8AA0A8 };
        let name = format!("{} {}", index + 1, kind.name());
        game.native_text(batch, &name, [LEFT, y], color, true);
        y += 12.0;
    }
    gauge(game, batch, [LEFT, bottom - 22.0], ship.energy, 0xFF40D0FF, "energy");
    let heat = if ship.locked { 0xFFFF5030 } else { 0xFFFFA030 };
    let name = if ship.locked { "overheated" } else { "heat" };
    gauge(game, batch, [LEFT, bottom - 12.0], ship.heat, heat, name);
    if weapon == WeaponKind::MainBeam {
        gauge(game, batch, [LEFT, bottom - 2.0], ship.charge, 0xFF90C8FF, "charge");
    }
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

pub(super) fn hud(game: &mut Game<'_>, batch: &mut HudBatch) {
    let shown = CLIENT.with_borrow(|slot| {
        let client = slot.as_ref()?;
        let ship = client.current?;
        let aboard = !matches!(client.mode, Mode::Outside);
        let piloting = matches!(client.mode, Mode::Piloting);
        aboard.then_some((ship, client.weapon, piloting, client.flash))
    });
    let Some((ship, weapon, piloting, flash)) = shown.filter(|_| game.native_on(ID)) else {
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
    if piloting {
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
    if !pressed || !at_the_controls(game) || !game.native_on(ID) {
        return;
    }
    let chosen = |name, default| key == game.native_mod_key(ID, name, default);
    if chosen(SEAT_KEY, SEAT_KEY_DEFAULT) {
        lock(&ORDERS).push(Order::Seat);
    } else if chosen(PANEL_KEY, PANEL_KEY_DEFAULT) {
        lock(&ORDERS).push(Order::Panel);
    } else if chosen(HELP_KEY, HELP_KEY_DEFAULT) {
        lock(&ORDERS).push(Order::Tutorial(0));
    } else if chosen(STOP_KEY, STOP_KEY_DEFAULT) {
        lock(&ORDERS).push(Order::Stop);
    } else if let Some(index) = GUN_KEYS.iter().position(|gun| chosen(gun.0, gun.3)).filter(|_| piloting()) {
        lock(&ORDERS).push(Order::Weapon(WeaponKind::ALL[index]));
    }
}

/// While piloting the wheel goes through the guns, not the hotbar.
pub(super) fn wheel(game: &mut Game<'_>, steps: f32, claimed: &mut bool) {
    if game.native_on(ID) && piloting() && !game.dialog_open() {
        *claimed = true;
        lock(&ORDERS).push(Order::NextWeapon(if steps > 0.0 { -1 } else { 1 }));
    }
}

fn room_named(name: &str) -> Option<RoomKind> {
    match name {
        "bridge" => Some(RoomKind::Bridge),
        "corridor" => Some(RoomKind::Corridor),
        "reactor" => Some(RoomKind::Reactor),
        "guns" => Some(RoomKind::Weapons),
        "cargo" => Some(RoomKind::Cargo),
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
    let ship = lock(&PUBLISHED).map_or(DVec3::ZERO, |ship| ship.pose.position);
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
        _ => None,
    };
    match order {
        Some(order) => lock(&ORDERS).push(order),
        None => lock(&NOTICES).push(
            "/astral spawn | board | walk | seat | enter | leave | go ROOM | panel | tutorial | view | jump X Y Z | dash X Y Z | dash cancel | speed FLIGHT DASH | stop | portal overworld | portal nether | portal nether roof | portal end | portal go | portal cancel | blast on | blast off | cruise THRUST TURN | cruise off | remove | status. Rooms: bridge corridor reactor guns cargo airlock cabins mess sickbay"
                .to_string(),
        ),
    }
}
