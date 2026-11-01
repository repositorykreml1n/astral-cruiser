// The way through a portal as rules: where the gate stands on both sides, where
// the far side has room for the ship, and how the ship goes through.
use super::astral_core::{CollisionQuery, Dimension, ShipPose};
use super::astral_dash::{Pace, Route, Travel, line_clear};
use glam::{DQuat, DVec3};

/// The opening's half width and half height, with room round the hull.
pub const OPENING: [f64; 2] = [76.0, 34.0];
/// How far ahead of the ship's middle a portal opens: clear of the nose.
const AHEAD: f64 = 110.0;
/// How far past the exit the ship's middle stops: far enough that the stern and
/// the pilot's camera have both come through.
const RUN_OUT: f64 = 180.0;
/// How far short of that the ship must have crossed, whatever else happens.
const LAST_CROSSING: f64 = 40.0;
/// Where along its way out the ship's middle has to have room from: by then the
/// whole hull is past the plane, and what stands behind that is never touched.
const EXIT_CLEAR_FROM: f64 = 40.0;
const PACE: Pace = Pace { top: 64.0, acceleration: 32.0 };
/// How long the far side may take to load, in seconds.
const PREPARE_LIMIT: f64 = 60.0;
/// How far ahead of the plane an eye starts to drift into the opening.
const EYE_LEAD: f64 = 160.0;
/// The share of the opening an eye keeps inside.
const EYE_ROOM: f64 = 0.6;
/// How far back the pilot's camera hangs by the time it comes through.
const CHASE_THROUGH: f64 = 120.0;
/// How far short of the plane the ship's middle is when the camera starts to close up.
const CHASE_CLOSING_FROM: f64 = 110.0;
const CHASE_CLOSING_OVER: f64 = 100.0;
/// The chunks kept at the far side: ahead of the exit, where the ship goes. What
/// lies behind the plane is never seen.
const EXIT_AREA_AHEAD: f64 = 110.0;
/// Wide enough to look through the opening from behind, too.
const EXIT_AREA_RADIUS: i32 = 14;
/// How far from an open portal the ship may go before it shuts.
const WANDER_LIMIT: f64 = 400.0;
/// Between these distances short of the plane the ship's middle has its nose in
/// the opening, and takes it as flying in.
const MOUTH: [f64; 2] = [-100.0, -60.0];
/// How far short of the plane it must be to go through at all.
const LEAST_APPROACH: f64 = 20.0;
/// How far short of the plane a ship off the axis has come onto it.
const ON_THE_AXIS: f64 = 80.0;
/// How high the Nether's caves are looked through, under its roof, before over it.
const NETHER_FLOOR: f64 = 50.0;
/// Where the ship stands on top of the Nether's roof: just over its bedrock.
const NETHER_ROOF: f64 = 146.0;
/// The heights a cave is looked for at: over the lava sea, under the bedrock.
const CAVE_FLOOR: f64 = 52.0;
const CAVE_CEILING: f64 = 108.0;
const CAVE_HEIGHT_STEP: f64 = 4.0;
/// A search loads more chunks than a plain exit: the hull's own way takes most of
/// an area, so the spots to try grow much faster than the chunks do.
const CAVE_AREA_RADIUS: i32 = 14;
/// How far from the middle of a window spots are tried, and how far apart. The
/// way out is longer than the hull is wide, so there is less room along it.
const CAVE_ACROSS: f64 = 176.0;
const CAVE_ALONG: f64 = 64.0;
const CAVE_SPOT_STEP: f64 = 16.0;
/// How far apart the windows of a search are, across the way and along it.
const WINDOW_ACROSS: f64 = 352.0;
const WINDOW_ALONG: f64 = 144.0;

/// Where the same place is in another world.
pub fn corresponding(from: Dimension, to: Dimension, at: DVec3) -> DVec3 {
    let scale = match (from, to) {
        (Dimension::Overworld, Dimension::Nether) => 1.0 / 8.0,
        (Dimension::Nether, Dimension::Overworld) => 8.0,
        _ => 1.0,
    };
    DVec3::new(at.x * scale, at.y, at.z * scale)
}

/// Where a portal lets the ship out. The Nether's caves are rarely wide enough for
/// the hull, so there it is the player's choice to look for one or to go out on
/// top of the roof, which always has room but is dark and empty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExitKind {
    Usual,
    Cave,
    Roof,
}

/// The height the search for room at the exit starts from.
pub fn first_height(to: Dimension, entry: DVec3, kind: ExitKind) -> f64 {
    match (to, kind) {
        (Dimension::Nether, ExitKind::Roof) => NETHER_ROOF,
        (Dimension::Nether, _) => NETHER_FLOOR,
        _ => entry.y,
    }
}

/// What the search of one tick came to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Finding {
    /// It goes on, here or at the next window, which has to load first.
    Looking,
    /// A cave fits the ship, and the portal stands open on it.
    Opened,
    /// No cave anywhere near: the portal will go out over the roof.
    NoCave,
}

/// The chunks that hold a ship coming out at `exit`, as `(middle, radius)`.
fn area_of(exit: DVec3, rotation: DQuat, radius: i32) -> ((i32, i32), i32) {
    let at = exit + rotation * DVec3::Z * EXIT_AREA_AHEAD;
    (((at.x.floor() as i32) >> 4, (at.z.floor() as i32) >> 4), radius)
}

/// A look round the Nether for a cave the hull fits in, a window of chunks at a time.
#[derive(Clone, Debug, PartialEq)]
struct CaveSearch {
    /// The places searched from, nearest first, and which one it is at.
    windows: Vec<DVec3>,
    window: usize,
    /// The spots of this window still to try, nearest first.
    spots: Vec<DVec3>,
    next: usize,
}

impl CaveSearch {
    fn new(ideal: DVec3, rotation: DQuat) -> Self {
        let (right, forward) = (rotation * DVec3::X, rotation * DVec3::Z);
        let mut offsets: Vec<(f64, f64)> = [0.0, WINDOW_ACROSS, -WINDOW_ACROSS]
            .iter()
            .flat_map(|&across| [0.0, WINDOW_ALONG, -WINDOW_ALONG].map(|along| (across, along)))
            .collect();
        offsets.sort_by(|a, b| a.0.hypot(a.1).total_cmp(&b.0.hypot(b.1)));
        let windows: Vec<DVec3> =
            offsets.iter().map(|&(across, along)| ideal + right * across + forward * along).collect();
        let spots = spots_around(windows[0], rotation);
        Self { windows, window: 0, spots, next: 0 }
    }

}

/// The spots of the window round `middle`, nearest first.
pub fn spots_around(middle: DVec3, rotation: DQuat) -> Vec<DVec3> {
    let (right, forward) = (rotation * DVec3::X, rotation * DVec3::Z);
    let steps = |reach: f64| (reach / CAVE_SPOT_STEP) as i32;
    let (across, along) = (steps(CAVE_ACROSS), steps(CAVE_ALONG));
    let mut heights = Vec::new();
    let mut y = CAVE_FLOOR;
    while y <= CAVE_CEILING {
        heights.push(y);
        y += CAVE_HEIGHT_STEP;
    }
    let mut spots = Vec::new();
    for a in -across..=across {
        for l in -along..=along {
            let aside = right * (f64::from(a) * CAVE_SPOT_STEP) + forward * (f64::from(l) * CAVE_SPOT_STEP);
            spots.extend(heights.iter().map(|&y| DVec3::new(middle.x, y, middle.z) + aside));
        }
    }
    spots.sort_by(|a, b| {
        let off = |p: &DVec3| (p.x - middle.x).hypot(p.z - middle.z);
        off(a).total_cmp(&off(b))
    });
    spots
}

/// A portal: one plane in two worlds, facing the way the ship goes. What is past
/// the entry is shown as what is past the exit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gate {
    pub from: Dimension,
    pub to: Dimension,
    pub entry: DVec3,
    pub exit: DVec3,
    pub rotation: DQuat,
}

impl Gate {
    pub fn forward(&self) -> DVec3 {
        self.rotation * DVec3::Z
    }

    /// What takes a point at the entry side to the exit side.
    pub fn shift(&self) -> DVec3 {
        self.exit - self.entry
    }

    /// How far past the entry's plane a point is, along the way through.
    pub fn past(&self, at: DVec3) -> f64 {
        (at - self.entry).dot(self.forward())
    }

    /// The same gate seen from the other world, looking back through.
    pub fn flipped(&self) -> Gate {
        Gate {
            from: self.to,
            to: self.from,
            entry: self.exit,
            exit: self.entry,
            rotation: self.rotation,
        }
    }

    /// Whether a step from before to after crosses the entry plane through the opening.
    pub fn crossing(&self, before: DVec3, after: DVec3) -> bool {
        let d1 = self.past(before);
        let d2 = self.past(after);
        let sign_changes = (d1 < 0.0 && d2 > 0.0) || (d1 > 0.0 && d2 < 0.0);
        if !sign_changes {
            return false;
        }
        let right = self.rotation * DVec3::X;
        let from_middle = after - self.entry;
        let (across, up) = (from_middle.dot(right), from_middle.y);
        across.abs() <= OPENING[0] && up.abs() <= OPENING[1]
    }
}

/// The lowest height from `at` up where the hull fits at the exit and can fly on
/// to where it stops.
pub fn room_at(
    terrain: &impl CollisionQuery,
    at: DVec3,
    rotation: DQuat,
    half: DVec3,
    top: f64,
) -> Option<f64> {
    let forward = rotation * DVec3::Z;
    let mut spot = at;
    spot.y = at.y.min(top - half.y);
    while spot.y + half.y <= top {
        let (first, last) = (spot + forward * EXIT_CLEAR_FROM, spot + forward * RUN_OUT);
        if line_clear(first, last, rotation, half, terrain) {
            return Some(spot.y);
        }
        spot.y += 4.0;
    }
    None
}

/// An eye coming up to the plane, eased into the opening: it must go through,
/// not past the rim. Far from the plane it is left alone.
pub fn through_opening(gate: &Gate, eye: DVec3) -> DVec3 {
    let near = (1.0 + gate.past(eye) / EYE_LEAD).clamp(0.0, 1.0);
    let right = gate.rotation * DVec3::X;
    let from_middle = eye - gate.entry;
    let (across, up) = (from_middle.dot(right), from_middle.y);
    let into = |value: f64, half: f64| {
        let limit = half * EYE_ROOM;
        (value.clamp(-limit, limit) - value) * near
    };
    eye + right * into(across, OPENING[0]) + DVec3::Y * into(up, OPENING[1])
}

/// How far back the pilot's camera hangs, with the ship's middle `past` the plane:
/// close enough to come through before the ship has to stop.
pub fn chase_behind(past: f64, usual: f64) -> f64 {
    let share = ((past + CHASE_CLOSING_FROM) / CHASE_CLOSING_OVER).clamp(0.0, 1.0);
    usual + (CHASE_THROUGH - usual) * share
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PassStage {
    /// The far side is loading.
    Preparing,
    /// The view is live and the portal waits for the pilot.
    Open,
    /// The ship flies in, on this side still.
    Going,
    /// The ship has crossed and flies clear of the exit.
    Through,
}

/// What a tick of a passage did with the ship.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stride {
    Held,
    TimedOut,
    Moved { pose: ShipPose, velocity: DVec3 },
    /// It is as far as it can go on this side.
    MustCross,
    /// It stopped clear of the exit.
    Done(ShipPose),
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    Preparing { waited: f64 },
    Open,
    Going(Travel),
    Through(Travel),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Passage {
    pub gate: Gate,
    start: ShipPose,
    state: State,
    area: ((i32, i32), i32),
    search: Option<CaveSearch>,
    /// Going in: how far off the portal's axis it set out, and how far short.
    aside: DVec3,
    approach: f64,
}

impl Passage {
    /// A portal ahead of the ship, to `to`. Until it is prepared, the height of
    /// its exit is only a first guess.
    pub fn new(start: ShipPose, from: Dimension, to: Dimension, kind: ExitKind) -> Self {
        let entry = start.position + start.rotation * DVec3::Z * AHEAD;
        let mut exit = corresponding(from, to, entry);
        exit.y = first_height(to, entry, kind);
        let rotation = start.rotation;
        let gate = Gate { from, to, entry, exit, rotation };
        let looks = to == Dimension::Nether && kind == ExitKind::Cave;
        let radius = if looks { CAVE_AREA_RADIUS } else { EXIT_AREA_RADIUS };
        Self {
            gate,
            start,
            state: State::Preparing { waited: 0.0 },
            area: area_of(exit, rotation, radius),
            search: looks.then(|| CaveSearch::new(exit, rotation)),
            aside: DVec3::ZERO,
            approach: 0.0,
        }
    }

    /// Which of how many windows a search for a cave is at, if one is under way.
    pub fn searching(&self) -> Option<(usize, usize)> {
        self.search.as_ref().map(|search| (search.window + 1, search.windows.len()))
    }

    /// The search for a cave, with this window's chunks loaded, for as long as
    /// `more` says. Only a ship with no cave left to look for ends up on the roof.
    pub fn look_for_cave(
        &mut self,
        terrain: &impl CollisionQuery,
        half: DVec3,
        mut more: impl FnMut() -> bool,
    ) -> Finding {
        let rotation = self.gate.rotation;
        let forward = rotation * DVec3::Z;
        let Some(search) = &mut self.search else {
            return Finding::NoCave;
        };
        while more() {
            let Some(&spot) = search.spots.get(search.next) else {
                search.window += 1;
                let Some(&middle) = search.windows.get(search.window) else {
                    self.search = None;
                    self.gate.exit.y = NETHER_ROOF;
                    self.area = area_of(self.gate.exit, rotation, EXIT_AREA_RADIUS);
                    self.state = State::Preparing { waited: 0.0 };
                    return Finding::NoCave;
                };
                search.spots = spots_around(middle, rotation);
                search.next = 0;
                self.area = area_of(middle, rotation, CAVE_AREA_RADIUS);
                self.state = State::Preparing { waited: 0.0 };
                return Finding::Looking;
            };
            search.next += 1;
            let (first, last) = (spot + forward * EXIT_CLEAR_FROM, spot + forward * RUN_OUT);
            if line_clear(first, last, rotation, half, terrain) {
                self.gate.exit = spot;
                self.search = None;
                self.state = State::Open;
                return Finding::Opened;
            }
        }
        Finding::Looking
    }

    /// The far side has loaded, and this is the height the ship can come out at.
    pub fn opened(&mut self, height: f64) {
        self.gate.exit.y = height;
        self.state = State::Open;
    }

    /// The ship goes for it, from wherever it is on the near side. False unless
    /// the portal stands open and the ship is in front of it.
    pub fn go(&mut self) -> bool {
        let past = self.gate.past(self.start.position);
        if self.state != State::Open || past > -LEAST_APPROACH {
            return false;
        }
        let forward = self.gate.forward();
        let on_axis = self.gate.entry + forward * past;
        self.aside = self.start.position - on_axis;
        self.approach = -past;
        let end = self.gate.entry + forward * RUN_OUT;
        self.state = State::Going(Travel::new(Route::paced(on_axis, end, PACE)));
        true
    }

    /// While it stands, the ship goes where it likes: this is where it is now.
    pub fn moved_to(&mut self, pose: ShipPose) {
        if self.can_close() {
            self.start = pose;
        }
    }

    /// Whether the ship has gone so far that the portal shuts.
    pub fn too_far(&self) -> bool {
        self.start.position.distance(self.gate.entry) > WANDER_LIMIT
    }

    /// Whether the ship is flying nose first into the open portal.
    pub fn at_the_mouth(&self) -> bool {
        let forward = self.gate.forward();
        let from_middle = self.start.position - self.gate.entry;
        let across = from_middle.dot(self.gate.rotation * DVec3::X);
        self.state == State::Open
            && (MOUTH[0]..=MOUTH[1]).contains(&from_middle.dot(forward))
            && across.abs() <= OPENING[0]
            && from_middle.y.abs() <= OPENING[1]
            && (self.start.rotation * DVec3::Z).dot(forward) > 0.7
    }

    /// The share of its way off the axis a ship going in still has, 1 to 0.
    fn off_the_axis(&self, at: DVec3) -> f64 {
        let near = ON_THE_AXIS.min(self.approach / 2.0);
        ((-self.gate.past(at) - near) / (self.approach - near).max(1.0)).clamp(0.0, 1.0)
    }

    /// The ship goes over to the far side, as it stands. This is the pose it has there.
    pub fn cross(&mut self) -> ShipPose {
        if let State::Going(travel) = &self.state {
            self.state = State::Through(travel.clone());
        }
        self.pose()
    }

    /// Where the ship is, in the world it is in.
    pub fn pose(&self) -> ShipPose {
        let rotation = |off: f64| match off {
            0.0 => self.gate.rotation,
            _ if self.start.rotation == self.gate.rotation => self.start.rotation,
            _ => self.gate.rotation.slerp(self.start.rotation, off),
        };
        match &self.state {
            State::Going(travel) => {
                let off = self.off_the_axis(travel.position());
                ShipPose { position: travel.position() + self.aside * off, rotation: rotation(off) }
            }
            State::Through(travel) => {
                let off = self.off_the_axis(travel.position());
                let position = travel.position() + self.aside * off + self.gate.shift();
                ShipPose { position, rotation: rotation(off) }
            }
            _ => self.start,
        }
    }

    pub fn tick(&mut self, dt: f64) -> Stride {
        match &mut self.state {
            State::Preparing { waited } => {
                *waited += dt;
                if *waited > PREPARE_LIMIT { Stride::TimedOut } else { Stride::Held }
            }
            State::Open => Stride::Held,
            State::Going(travel) => {
                travel.tick(dt);
                if travel.left() <= LAST_CROSSING {
                    return Stride::MustCross;
                }
                Stride::Moved { pose: self.pose(), velocity: self.velocity() }
            }
            State::Through(travel) => {
                travel.tick(dt);
                if travel.finished() {
                    return Stride::Done(self.pose());
                }
                Stride::Moved { pose: self.pose(), velocity: self.velocity() }
            }
        }
    }

    fn velocity(&self) -> DVec3 {
        match &self.state {
            State::Going(travel) | State::Through(travel) => travel.velocity(),
            _ => DVec3::ZERO,
        }
    }

    pub fn stage(&self) -> PassStage {
        match self.state {
            State::Preparing { .. } => PassStage::Preparing,
            State::Open => PassStage::Open,
            State::Going(_) => PassStage::Going,
            State::Through(_) => PassStage::Through,
        }
    }

    /// Whether the portal may still shut: not once the ship is flying into it.
    pub fn can_close(&self) -> bool {
        matches!(self.state, State::Preparing { .. } | State::Open)
    }

    /// Where a save puts the ship. Part of it past the plane is in neither world,
    /// so until it has crossed that is where it set out from.
    pub fn safe_pose(&self) -> ShipPose {
        match self.state {
            State::Through(_) => self.pose(),
            _ => self.start,
        }
    }

    /// The chunks the far side needs, as `(middle, radius)`.
    pub fn exit_area(&self) -> ((i32, i32), i32) {
        self.area
    }

    /// The chunks around the entry, as `(middle, radius)`.
    pub fn entry_area(&self) -> ((i32, i32), i32) {
        (
            ((self.gate.entry.x.floor() as i32) >> 4, (self.gate.entry.z.floor() as i32) >> 4),
            EXIT_AREA_RADIUS,
        )
    }
}
