// A dash to exact coordinates as rules: which way the ship can go, and how it
// moves along that way. The server runs this against the real world.
use super::astral_core::{
    CollisionQuery, NavigationError, NavigationLimits, NavigationTarget, ShipPose,
};
use glam::{DQuat, DVec3};

/// How long a dash waits for the place it goes to, in seconds.
const WARM_LIMIT: f64 = 60.0;
const CHARGE_SECONDS: f64 = 2.0;
/// How fast the ship swings round to face its way, in radians a second, and
/// how many of its angles on the way round are tested.
const TURN_RATE: f64 = 1.0;
const TURN_TESTS: u32 = 8;
/// How far apart along a way the hull is tested against the world.
const ROUTE_STEP: f64 = 4.0;
/// The slowest the ship goes on a leg, or it would never reach the end of it.
const CREEP: f64 = 2.0;
/// How far a dash reaches where it can't go over the world: all of the way has
/// to be loaded first.
const NO_SKY_REACH: f64 = 512.0;

const RISE: Pace = Pace { top: 96.0, acceleration: 48.0 };
const CROSS: Pace = Pace { top: 800.0, acceleration: 240.0 };

/// How fast a leg is flown, in blocks a second and blocks a second per second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pace {
    pub top: f64,
    pub acceleration: f64,
}

/// A straight stretch of the way, flown from a stop to a stop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leg {
    pub to: DVec3,
    pub pace: Pace,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    from: DVec3,
    legs: Vec<Leg>,
}

impl Route {
    fn through(from: DVec3, stops: &[(DVec3, Pace)]) -> Self {
        let mut at = from;
        let mut legs = Vec::new();
        for &(to, pace) in stops {
            if to != at {
                legs.push(Leg { to, pace });
                at = to;
            }
        }
        Self { from, legs }
    }

    /// Straight there, for a way known to be clear.
    pub fn direct(from: DVec3, to: DVec3) -> Self {
        Self::paced(from, to, CROSS)
    }

    /// Straight there at a pace of its own.
    pub fn paced(from: DVec3, to: DVec3, pace: Pace) -> Self {
        Self::through(from, &[(to, pace)])
    }

    /// Up over everything in the world, across at speed, and down on the target.
    pub fn over_the_world(from: DVec3, to: DVec3, height: f64) -> Self {
        let height = height.max(from.y).max(to.y);
        let (up, over) = (DVec3::new(from.x, height, from.z), DVec3::new(to.x, height, to.z));
        Self::through(from, &[(up, RISE), (over, CROSS), (to, RISE)])
    }

    #[allow(dead_code)] // For the tests.
    pub fn legs(&self) -> &[Leg] {
        &self.legs
    }

    /// The fast legs flown at this speed instead, gathered in as quickly.
    pub fn at_speed(mut self, top: f64) -> Self {
        for leg in self.legs.iter_mut().filter(|leg| leg.pace == CROSS) {
            leg.pace = Pace { top, acceleration: top * CROSS.acceleration / CROSS.top };
        }
        self
    }
}

/// The ship's way along a route.
#[derive(Clone, Debug, PartialEq)]
pub struct Travel {
    route: Route,
    /// The leg under way, where it started, and how far along it the ship is.
    leg: usize,
    start: DVec3,
    along: f64,
    speed: f64,
    braking: bool,
    halted: bool,
}

impl Travel {
    pub fn new(route: Route) -> Self {
        Self::at_speed(route, 0.0)
    }

    /// A ship already going this fast as it sets out.
    pub fn at_speed(route: Route, speed: f64) -> Self {
        let start = route.from;
        Self { route, leg: 0, start, along: 0.0, speed, braking: false, halted: false }
    }

    fn current(&self) -> Option<Leg> {
        self.route.legs.get(self.leg).copied().filter(|_| !self.halted)
    }

    pub fn tick(&mut self, dt: f64) {
        let Some(leg) = self.current() else {
            return;
        };
        let length = self.start.distance(leg.to);
        let rate = leg.pace.acceleration * dt;
        let stopping = (2.0 * leg.pace.acceleration * (length - self.along).max(0.0)).sqrt();
        self.speed = if self.braking {
            (self.speed - rate).min(stopping).max(0.0)
        } else {
            (self.speed + rate).min(leg.pace.top).min(stopping).max(CREEP)
        };
        self.along += self.speed * dt;
        if self.along >= length {
            self.start = leg.to;
            self.along = 0.0;
            self.speed = 0.0;
            self.leg += 1;
        }
        self.halted = self.braking && self.speed == 0.0;
    }

    /// Stop as soon as the ship can, wherever on its way that is.
    pub fn brake(&mut self) {
        self.braking = true;
    }

    pub fn finished(&self) -> bool {
        self.current().is_none()
    }

    pub fn position(&self) -> DVec3 {
        match self.current() {
            Some(leg) => self.start + (leg.to - self.start).normalize() * self.along,
            None => self.start + self.last_way() * self.along,
        }
    }

    /// The way the ship was going when it stopped short.
    fn last_way(&self) -> DVec3 {
        let to = self.route.legs.get(self.leg).map_or(self.start, |leg| leg.to);
        (to - self.start).normalize_or_zero()
    }

    pub fn velocity(&self) -> DVec3 {
        self.current().map_or(DVec3::ZERO, |leg| {
            (leg.to - self.start).normalize() * self.speed
        })
    }

    /// How far the ship still has to go.
    pub fn left(&self) -> f64 {
        let Some(leg) = self.current() else {
            return 0.0;
        };
        let mut left = self.start.distance(leg.to) - self.along;
        let mut at = leg.to;
        for next in &self.route.legs[self.leg + 1..] {
            left += at.distance(next.to);
            at = next.to;
        }
        left
    }
}

/// Which way the ship faces to go there. Straight up or down it stays as it is.
pub fn facing(from: &ShipPose, target: DVec3) -> DQuat {
    let along = target - from.position;
    if along.x.hypot(along.z) < 1.0 {
        from.rotation
    } else {
        DQuat::from_rotation_y(along.x.atan2(along.z))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DashError {
    Outside(NavigationError),
    /// The hull does not fit there, and the lowest height above it that does.
    Occupied { lowest: Option<f64> },
    NoRoomToTurn,
    Blocked,
    TooFar,
}

impl std::fmt::Display for DashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Outside(why) => write!(f, "{why}"),
            Self::Occupied { lowest: Some(y) } => {
                write!(f, "the hull does not fit there. The lowest height that fits is Y {y:.0}")
            }
            Self::Occupied { lowest: None } => write!(f, "the hull does not fit there at any height"),
            Self::NoRoomToTurn => write!(f, "no room to turn the ship that way"),
            Self::Blocked => write!(f, "no clear way there"),
            Self::TooFar => {
                write!(f, "too far with no open sky to cross in: at most {NO_SKY_REACH:.0} blocks here")
            }
        }
    }
}

/// Anywhere at all: the checks that need no world.
struct Anywhere;

impl CollisionQuery for Anywhere {
    fn is_clear(&self, _pose: &ShipPose, _half_extents: DVec3) -> bool {
        true
    }
}

/// Whether the target can be gone to at all, before any of it has loaded.
pub fn within_world(
    from: &ShipPose,
    target: &NavigationTarget,
    limits: &NavigationLimits,
    sky: bool,
) -> Result<(), DashError> {
    let rotation = facing(from, target.position);
    target.validate_pose(limits, rotation, &Anywhere).map_err(DashError::Outside)?;
    if !sky && from.position.distance(target.position) > NO_SKY_REACH {
        return Err(DashError::TooFar);
    }
    Ok(())
}

/// The chunk areas a dash needs loaded, as `(middle, radius)`: around both ends,
/// and with no sky to cross in, all along the way between them.
/// A dash under this length needs no far terrain window ahead of it.
pub const SHORT_DASH: f64 = 2000.0;

pub fn dash_shows_far_terrain(left: f64) -> bool {
    left > SHORT_DASH
}

/// How far off the fog stands, in chunks, in flight over the world: the path's
/// far terrain lies a long way below, well past where its distance would fog it.
pub fn fog_reach(distance: i32, light: bool) -> i32 {
    const OVER_THE_WORLD: i32 = 40;
    if light { distance.max(OVER_THE_WORLD) } else { distance }
}

/// Tiles take a moment to come, so the window reaches further ahead the faster
/// the ship goes, and its back edge stays about LEAST_RADIUS chunks behind the ship.
pub fn far_reach(speed: f64) -> (f64, u32) {
    const LEAD_SECONDS: f64 = 0.35;
    const LEAST_RADIUS: u32 = 5;
    let lead = speed.max(0.0) * LEAD_SECONDS;
    let middle = lead / 2.0;
    let radius = LEAST_RADIUS + (lead / 32.0).ceil() as u32;
    (middle, radius)
}

/// What stays loaded of a dash's areas: over the world only where it ends.
pub fn areas_in_flight(areas: &[((i32, i32), i32)], light: bool) -> &[((i32, i32), i32)] {
    if light { &areas[areas.len().saturating_sub(1)..] } else { areas }
}

pub fn dash_areas(from: DVec3, to: DVec3, radius: i32, sky: bool) -> Vec<((i32, i32), i32)> {
    let chunk = |at: DVec3| ((at.x.floor() as i32) >> 4, (at.z.floor() as i32) >> 4);
    let reach = f64::from(radius.max(1) * 16);
    let hops = if sky { 1 } else { (from.distance(to) / reach).ceil().max(1.0) as u32 };
    let mut areas = Vec::new();
    for hop in 0..=hops {
        let area = (chunk(from.lerp(to, f64::from(hop) / f64::from(hops))), radius);
        if !areas.contains(&area) {
            areas.push(area);
        }
    }
    areas
}

/// The lowest height from `at` up that the hull fits at, under `top`.
pub fn lowest_fit(
    terrain: &impl CollisionQuery,
    at: DVec3,
    rotation: DQuat,
    half: DVec3,
    top: f64,
) -> Option<f64> {
    let mut y = at.y;
    while y + half.y <= top {
        let pose = ShipPose { position: DVec3::new(at.x, y, at.z), rotation };
        if terrain.is_clear(&pose, half) {
            return Some(y);
        }
        y += ROUTE_STEP;
    }
    None
}

/// Whether the hull can go in a straight line from one point to another.
pub fn line_clear(
    from: DVec3,
    to: DVec3,
    rotation: DQuat,
    half: DVec3,
    terrain: &impl CollisionQuery,
) -> bool {
    let steps = (from.distance(to) / ROUTE_STEP).ceil().max(1.0) as u32;
    (0..=steps).all(|step| {
        let position = from.lerp(to, f64::from(step) / f64::from(steps));
        terrain.is_clear(&ShipPose { position, rotation }, half)
    })
}

fn can_turn(from: &ShipPose, rotation: DQuat, half: DVec3, terrain: &impl CollisionQuery) -> bool {
    (1..=TURN_TESTS).all(|test| {
        let turned = from.rotation.slerp(rotation, f64::from(test) / f64::from(TURN_TESTS));
        terrain.is_clear(&ShipPose { position: from.position, rotation: turned }, half)
    })
}

/// Whether the hull fits at the target. If it doesn't, the refusal names the
/// lowest height it fits at: nothing else is ever chosen for the player.
pub fn fits_at(
    target: &NavigationTarget,
    limits: &NavigationLimits,
    rotation: DQuat,
    terrain: &impl CollisionQuery,
) -> Result<(), DashError> {
    target.validate_pose(limits, rotation, terrain).map_err(|why| match why {
        NavigationError::Occupied => DashError::Occupied {
            lowest: lowest_fit(terrain, target.position, rotation, limits.half_extents, limits.max_y),
        },
        other => DashError::Outside(other),
    })
}

/// The way to `target`, with everything along it loaded. `sky` is the height to
/// cross at, in a dimension with open air above all there is. Nothing is chosen
/// but the exact point asked for: a point the hull does not fit is refused.
pub fn plan(
    from: &ShipPose,
    target: &NavigationTarget,
    limits: &NavigationLimits,
    sky: Option<f64>,
    terrain: &impl CollisionQuery,
) -> Result<Route, DashError> {
    let (to, half) = (target.position, limits.half_extents);
    let rotation = facing(from, to);
    fits_at(target, limits, rotation, terrain)?;
    if !can_turn(from, rotation, half, terrain) {
        return Err(DashError::NoRoomToTurn);
    }
    if line_clear(from.position, to, rotation, half, terrain) {
        return Ok(Route::direct(from.position, to));
    }
    let height = sky.ok_or(DashError::Blocked)?.max(from.position.y).max(to.y);
    let (up, over) = (
        DVec3::new(from.position.x, height, from.position.z),
        DVec3::new(to.x, height, to.z),
    );
    if line_clear(from.position, up, rotation, half, terrain)
        && line_clear(over, to, rotation, half, terrain)
    {
        Ok(Route::over_the_world(from.position, to, height))
    } else {
        Err(DashError::Blocked)
    }
}

/// How far a dash has got.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    Warming,
    Charging,
    Flying,
    Braking,
}

/// What a tick of a dash did with the ship.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Moment {
    Held,
    TimedOut,
    Moved { pose: ShipPose, velocity: DVec3 },
    /// It stopped, exactly where it was asked to.
    Arrived(ShipPose),
    /// It stopped short, as it was told to.
    Stopped(ShipPose),
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    Warming { waited: f64 },
    Charging { spent: f64, route: Route },
    Flying(Travel),
}

/// A dash under way: the ship waits for its destination, swings round to face
/// it while it charges, then goes.
#[derive(Clone, Debug, PartialEq)]
pub struct Dash {
    pub target: NavigationTarget,
    start: ShipPose,
    rotation: DQuat,
    state: State,
}

impl Dash {
    pub fn new(start: ShipPose, target: NavigationTarget) -> Self {
        let rotation = facing(&start, target.position);
        Self { target, start, rotation, state: State::Warming { waited: 0.0 } }
    }

    /// The destination has loaded and this is the way there.
    pub fn warmed(&mut self, route: Route) {
        self.state = State::Charging { spent: 0.0, route };
    }

    /// The seconds the swing round takes, and the whole charge with it.
    fn timing(&self) -> (f64, f64) {
        let turn = self.start.rotation.angle_between(self.rotation) / TURN_RATE;
        (turn, CHARGE_SECONDS.max(turn + 0.5))
    }

    pub fn tick(&mut self, dt: f64) -> Moment {
        let (turn, charge) = self.timing();
        match &mut self.state {
            State::Warming { waited } => {
                *waited += dt;
                if *waited > WARM_LIMIT { Moment::TimedOut } else { Moment::Held }
            }
            State::Charging { spent, route } => {
                *spent += dt;
                let share = if turn > 0.0 { (*spent / turn).min(1.0) } else { 1.0 };
                let eased = share * share * (3.0 - 2.0 * share);
                let rotation = self.start.rotation.slerp(self.rotation, eased);
                let pose = ShipPose::new(self.start.position, rotation);
                if *spent >= charge {
                    self.state = State::Flying(Travel::new(route.clone()));
                }
                Moment::Moved { pose, velocity: DVec3::ZERO }
            }
            State::Flying(travel) => {
                travel.tick(dt);
                let pose = ShipPose::new(travel.position(), self.rotation);
                if !travel.finished() {
                    Moment::Moved { pose, velocity: travel.velocity() }
                } else if travel.braking {
                    Moment::Stopped(pose)
                } else {
                    Moment::Arrived(pose)
                }
            }
        }
    }

    /// Calls it off. True if that ended it; in flight the ship first has to brake.
    pub fn cancel(&mut self) -> bool {
        match &mut self.state {
            State::Flying(travel) => {
                travel.brake();
                false
            }
            _ => true,
        }
    }

    pub fn stage(&self) -> Stage {
        match &self.state {
            State::Warming { .. } => Stage::Warming,
            State::Charging { .. } => Stage::Charging,
            State::Flying(travel) if travel.braking => Stage::Braking,
            State::Flying(_) => Stage::Flying,
        }
    }

    /// How far it is to go.
    pub fn left(&self) -> f64 {
        match &self.state {
            State::Flying(travel) => travel.left(),
            _ => self.start.position.distance(self.target.position),
        }
    }

    /// The share of the charge gathered, 0 to 1.
    pub fn charged(&self) -> f64 {
        let (_, charge) = self.timing();
        match &self.state {
            State::Warming { .. } => 0.0,
            State::Charging { spent, .. } => (spent / charge).min(1.0),
            State::Flying(_) => 1.0,
        }
    }
}

/// A coordinate as typed: a number, or `~` and an offset from where the ship is.
pub fn coordinate(word: &str, ship: f64) -> Option<f64> {
    let at = match word.strip_prefix('~') {
        Some("") => ship,
        Some(offset) => ship + offset.parse::<f64>().ok()?,
        None => word.parse::<f64>().ok()?,
    };
    at.is_finite().then_some(at)
}

/// The chunks of an area around a chunk, as `(middle, radius)`.
pub fn chunks_of(((x, z), radius): ((i32, i32), i32)) -> Vec<(i32, i32)> {
    (x - radius..=x + radius)
        .flat_map(|cx| (z - radius..=z + radius).map(move |cz| (cx, cz)))
        .collect()
}
