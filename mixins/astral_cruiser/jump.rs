// The jump as rules: the ship gathers itself while its destination is prepared,
// vanishes, comes out of nowhere a little short of the target with its speed
// still on it, and brakes hard onto the exact point.
use super::astral_core::{CollisionQuery, NavigationLimits, NavigationTarget, ShipPose};
use super::astral_dash::{DashError, Pace, Route, Travel, facing, fits_at, line_clear};
use glam::{DQuat, DVec3};

/// How long the ship gathers itself at the least, in seconds, and how long it
/// may wait for its destination.
const CHARGE_SECONDS: f64 = 4.0;
const WAIT_LIMIT: f64 = 60.0;
/// How fast the ship swings round to face the target, in radians a second.
const TURN_RATE: f64 = 1.0;
/// How far short of the target the ship comes out.
const APPROACH: f64 = 200.0;
const ARRIVAL: Pace = Pace { top: 500.0, acceleration: 625.0 };
/// The ways in, in radians below level, tried in this order.
const SLOPES: [f64; 4] = [0.0, 0.35, 0.8, std::f64::consts::FRAC_PI_2];
/// The chunks prepared, round the middle of the way in: the way and the hull.
const AREA_RADIUS: i32 = 11;

/// The chunk area the arrival needs, as `(middle, radius)`.
pub fn arrival_area(target: DVec3, rotation: DQuat) -> ((i32, i32), i32) {
    let middle = target - rotation * DVec3::Z * (APPROACH / 2.0);
    (((middle.x.floor() as i32) >> 4, (middle.z.floor() as i32) >> 4), AREA_RADIUS)
}

/// Where the ship comes out, with everything round the target loaded. It comes
/// in level if it can, and more steeply from above if it can't. A point the hull
/// does not fit is refused, not moved.
pub fn plan_jump(
    from: &ShipPose,
    target: &NavigationTarget,
    limits: &NavigationLimits,
    terrain: &impl CollisionQuery,
) -> Result<DVec3, DashError> {
    let (to, half) = (target.position, limits.half_extents);
    let rotation = facing(from, to);
    fits_at(target, limits, rotation, terrain)?;
    let forward = rotation * DVec3::Z;
    SLOPES
        .iter()
        .map(|slope| to - forward * (slope.cos() * APPROACH) + DVec3::Y * (slope.sin() * APPROACH))
        .find(|&exit| line_clear(exit, to, rotation, half, terrain))
        .ok_or(DashError::Blocked)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JumpStage {
    Gathering,
    Arriving,
}

/// What a tick of a jump did with the ship.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Leap {
    TimedOut,
    /// Still here, swinging round to face the target.
    Turned { pose: ShipPose },
    /// Gone from here, and there at speed.
    Jumped { pose: ShipPose, velocity: DVec3 },
    Moved { pose: ShipPose, velocity: DVec3 },
    /// It stopped, exactly where it was asked to.
    Arrived(ShipPose),
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    Gathering { spent: f64, exit: Option<DVec3> },
    Arriving(Travel),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Jump {
    pub target: NavigationTarget,
    pub rotation: DQuat,
    start: ShipPose,
    state: State,
}

impl Jump {
    pub fn new(start: ShipPose, target: NavigationTarget) -> Self {
        let rotation = facing(&start, target.position);
        Self { target, rotation, start, state: State::Gathering { spent: 0.0, exit: None } }
    }

    /// The destination has loaded, and this is where the ship comes out.
    pub fn prepared(&mut self, at: DVec3) {
        if let State::Gathering { exit, .. } = &mut self.state {
            *exit = Some(at);
        }
    }

    /// Whether it still waits to hear where it comes out.
    pub fn unprepared(&self) -> bool {
        matches!(self.state, State::Gathering { exit: None, .. })
    }

    /// The seconds the swing round takes, and the whole charge with it.
    fn timing(&self) -> (f64, f64) {
        let turn = self.start.rotation.angle_between(self.rotation) / TURN_RATE;
        (turn, CHARGE_SECONDS.max(turn + 0.5))
    }

    pub fn tick(&mut self, dt: f64) -> Leap {
        let (turn, charge) = self.timing();
        match &mut self.state {
            State::Gathering { spent, exit } => {
                *spent += dt;
                if *spent >= charge
                    && let Some(exit) = *exit
                {
                    let route = Route::paced(exit, self.target.position, ARRIVAL);
                    let travel = Travel::at_speed(route, ARRIVAL.top);
                    let velocity = (self.target.position - exit).normalize_or_zero() * ARRIVAL.top;
                    let pose = ShipPose::new(exit, self.rotation);
                    self.state = State::Arriving(travel);
                    return Leap::Jumped { pose, velocity };
                }
                if *spent > WAIT_LIMIT {
                    return Leap::TimedOut;
                }
                let share = if turn > 0.0 { (*spent / turn).min(1.0) } else { 1.0 };
                let eased = share * share * (3.0 - 2.0 * share);
                let rotation = self.start.rotation.slerp(self.rotation, eased);
                Leap::Turned { pose: ShipPose::new(self.start.position, rotation) }
            }
            State::Arriving(travel) => {
                travel.tick(dt);
                let pose = ShipPose::new(travel.position(), self.rotation);
                if travel.finished() {
                    Leap::Arrived(pose)
                } else {
                    Leap::Moved { pose, velocity: travel.velocity() }
                }
            }
        }
    }

    /// Calls it off. False once the ship has gone: it only takes a moment to arrive.
    pub fn cancel(&mut self) -> bool {
        matches!(self.state, State::Gathering { .. })
    }

    pub fn stage(&self) -> JumpStage {
        match self.state {
            State::Gathering { .. } => JumpStage::Gathering,
            State::Arriving(_) => JumpStage::Arriving,
        }
    }

    /// The share of the charge gathered, 0 to 1.
    pub fn charged(&self) -> f64 {
        let (_, charge) = self.timing();
        match self.state {
            State::Gathering { spent, .. } => (spent / charge).min(1.0),
            State::Arriving(_) => 1.0,
        }
    }

    /// Where a save puts the ship.
    pub fn safe_pose(&self) -> ShipPose {
        match &self.state {
            State::Gathering { .. } => self.start,
            State::Arriving(travel) => ShipPose::new(travel.position(), self.rotation),
        }
    }
}
