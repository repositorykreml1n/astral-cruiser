// Astral Cruiser core flight, navigation and pose mathematics.
//
// Pure mathematical model for cruiser navigation, rotation, analytical flight
// dynamics and bounding-hull collision checks. Timestep independent and free of
// engine/game specific side-effects.

pub mod constants {
    use glam::DVec3;

    /// Standard cruising speed in blocks (meters) per second.
    pub const CRUISE_SPEED: f64 = 64.0;

    /// Maximum boost speed in blocks (meters) per second.
    pub const BOOST_SPEED: f64 = 256.0;

    /// Bounded linear acceleration in blocks (meters) per second squared.
    pub const LINEAR_ACCELERATION: f64 = 16.0;

    /// Authoritative bounding box half-extents (X=48, Y=18, Z=80) for the cruiser hull.
    /// Full ship dimensions are 96 wide x 36 high x 160 long.
    pub const HULL_HALF_EXTENTS: DVec3 = DVec3::new(48.0, 18.0, 80.0);
}

pub mod collision {
    use glam::DVec3;
    use super::pose::ShipPose;

    /// Interface for spatial clearance queries against the surrounding world.
    pub trait CollisionQuery {
        /// Returns true if placing the ship with `half_extents` at `pose` is free of obstacles.
        fn is_clear(&self, pose: &ShipPose, half_extents: DVec3) -> bool;
    }
}

pub mod pose {
    use glam::{DQuat, DVec3};

    /// Safely normalizes a quaternion using scaled components to avoid overflow on huge finite values (e.g. 1e308).
    /// Falls back to identity if non-finite or zero-length.
    #[inline]
    pub fn safe_normalize_quat(q: DQuat) -> DQuat {
        if !q.is_finite() {
            return DQuat::IDENTITY;
        }
        let max_c = q.x.abs().max(q.y.abs()).max(q.z.abs()).max(q.w.abs());
        if max_c <= 0.0 || !max_c.is_finite() {
            return DQuat::IDENTITY;
        }
        let scaled = DQuat::from_xyzw(q.x / max_c, q.y / max_c, q.z / max_c, q.w / max_c);
        let len_sq = scaled.length_squared();
        if len_sq > 1e-24 {
            let len = len_sq.sqrt();
            let res = scaled * (1.0 / len);
            if res.is_finite() && (res.length() - 1.0).abs() < 1e-7 {
                return res;
            }
        }
        DQuat::IDENTITY
    }

    /// Safely normalizes a vector using scaled components to avoid overflow on huge finite values (e.g. 1e308).
    /// Returns the unit direction and true magnitude, or (ZERO, 0.0) if zero/invalid.
    #[inline]
    pub fn safe_normalize_vec3(v: DVec3) -> (DVec3, f64) {
        if !v.is_finite() {
            return (DVec3::ZERO, 0.0);
        }
        let max_c = v.x.abs().max(v.y.abs()).max(v.z.abs());
        if max_c <= 0.0 || !max_c.is_finite() {
            return (DVec3::ZERO, 0.0);
        }
        let scaled = v / max_c;
        let len_sq = scaled.length_squared();
        if len_sq > 1e-24 {
            let scaled_len = len_sq.sqrt();
            let dir = scaled / scaled_len;
            let len = max_c * scaled_len;
            if dir.is_finite() {
                return (dir, len);
            }
        }
        (DVec3::ZERO, 0.0)
    }

    /// Safely replaces non-finite vector components with zero.
    #[inline]
    pub fn safe_vec3(v: DVec3) -> DVec3 {
        if v.is_finite() {
            v
        } else {
            DVec3::ZERO
        }
    }

    /// Authoritative 6-DOF transform of the starship in double precision coordinates.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ShipPose {
        pub position: DVec3,
        pub rotation: DQuat,
    }

    impl Default for ShipPose {
        fn default() -> Self {
            Self {
                position: DVec3::ZERO,
                rotation: DQuat::IDENTITY,
            }
        }
    }

    impl ShipPose {
        /// Creates a new pose with sanitized finite coordinates and normalized rotation.
        pub fn new(position: DVec3, rotation: DQuat) -> Self {
            Self {
                position: safe_vec3(position),
                rotation: safe_normalize_quat(rotation),
            }
        }

        /// Transforms a point from local ship coordinates to world coordinates.
        pub fn local_to_world(&self, local: DVec3) -> DVec3 {
            let rot = safe_normalize_quat(self.rotation);
            let pos = safe_vec3(self.position);
            pos + (rot * local)
        }

        /// Transforms a point from world coordinates to local ship coordinates.
        pub fn world_to_local(&self, world: DVec3) -> DVec3 {
            let rot = safe_normalize_quat(self.rotation);
            let pos = safe_vec3(self.position);
            rot.conjugate() * (world - pos)
        }

        /// Interpolates between two poses using lerp for position and shortest-path slerp for rotation.
        ///
        /// Clamps finite alpha to `[0.0, 1.0]`, handles invalid/non-finite alpha safely,
        /// and guarantees finite normalized outputs.
        pub fn interpolate(p0: &ShipPose, p1: &ShipPose, alpha: f64) -> ShipPose {
            let alpha = if alpha.is_finite() {
                alpha.clamp(0.0, 1.0)
            } else {
                0.0
            };

            let pos0 = safe_vec3(p0.position);
            let pos1 = safe_vec3(p1.position);
            let q0 = safe_normalize_quat(p0.rotation);
            let q1 = safe_normalize_quat(p1.rotation);

            if alpha <= 0.0 {
                return ShipPose { position: pos0, rotation: q0 };
            }
            if alpha >= 1.0 {
                return ShipPose { position: pos1, rotation: q1 };
            }

            let position = pos0.lerp(pos1, alpha);

            // Shortest path: if dot product < 0, negate q1 to interpolate across the acute angle
            let mut dot = q0.dot(q1);
            let target_q1 = if dot < 0.0 {
                dot = -dot;
                -q1
            } else {
                q1
            };

            let rotation = if dot > 0.999999 {
                q0.lerp(target_q1, alpha)
            } else {
                q0.slerp(target_q1, alpha)
            };

            ShipPose {
                position: safe_vec3(position),
                rotation: safe_normalize_quat(rotation),
            }
        }
    }
}

pub mod flight {
    use glam::{DQuat, DVec3};
    use super::collision::CollisionQuery;
    use super::constants::{BOOST_SPEED, CRUISE_SPEED, HULL_HALF_EXTENTS, LINEAR_ACCELERATION};
    use super::pose::{safe_normalize_quat, safe_normalize_vec3, ShipPose};

    /// Pilot input controls for flight maneuvering.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ShipInput {
        /// Desired local thrust direction vector (WASD + Space/Shift).
        pub thrust: DVec3,
        /// Desired yaw turn rate in radians per second.
        pub yaw_rate: f64,
        /// Whether boost speed is engaged.
        pub boost: bool,
    }

    impl Default for ShipInput {
        fn default() -> Self {
            Self {
                thrust: DVec3::ZERO,
                yaw_rate: 0.0,
                boost: false,
            }
        }
    }

    /// Snapshot of authoritative ship flight state at a given tick.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ShipSnapshot {
        pub ticks: u64,
        pub pose: ShipPose,
        pub velocity: DVec3,
    }

    /// Errors resulting from invalid flight simulation parameters.
    #[derive(Clone, Debug, PartialEq)]
    pub enum FlightError {
        InvalidDt(f64),
        InvalidInput(String),
    }

    impl std::fmt::Display for FlightError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::InvalidDt(dt) => write!(f, "invalid flight timestep dt: {dt} (expected 0 < dt <= 0.1)"),
                Self::InvalidInput(msg) => write!(f, "invalid flight input: {msg}"),
            }
        }
    }

    impl std::error::Error for FlightError {}

    /// Authoritative flight physics simulation with analytical timestep integration.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Flight {
        ticks: u64,
        pose: ShipPose,
        velocity: DVec3,
        cruise: f64,
    }

    impl Flight {
        /// Creates a new flight simulation instance from an initial pose.
        pub fn new(pose: ShipPose) -> Self {
            Self {
                ticks: 0,
                pose: ShipPose::new(pose.position, pose.rotation),
                velocity: DVec3::ZERO,
                cruise: CRUISE_SPEED,
            }
        }

        /// The speed the ship flies at without boost. Boost and acceleration keep
        /// their share of it, so a faster ship answers its pilot just as quickly.
        pub fn set_cruise(&mut self, cruise: f64) {
            self.cruise = cruise;
        }

        /// The ship moved by something other than its own thrust, such as a dash.
        pub fn carry(&mut self, pose: ShipPose, velocity: DVec3) {
            self.pose = ShipPose::new(pose.position, pose.rotation);
            self.velocity = velocity;
        }

        /// Returns an immutable snapshot of current flight state.
        pub fn snapshot(&self) -> ShipSnapshot {
            ShipSnapshot {
                ticks: self.ticks,
                pose: self.pose,
                velocity: self.velocity,
            }
        }

        /// Advances the flight simulation by timestep `dt` seconds under pilot `input`.
        ///
        /// - Returns `Err` without mutating state if `dt` or `input` is non-finite or invalid.
        /// - Stably normalizes thrust vectors to prevent numeric overflow on large finite inputs.
        /// - Integrates simultaneous rotation and thrust using microsteps to ensure frame-rate independence.
        /// - Performs conservative swept substeps of the full 6-DOF ShipPose to prevent tunneling and rotation clipping.
        /// - Preserves the last known clear pose and halts velocity in the event of collision.
        pub fn tick<C: CollisionQuery + ?Sized>(
            &mut self,
            input: &ShipInput,
            dt: f64,
            collision: &C,
        ) -> Result<ShipSnapshot, FlightError> {
            // Validate inputs strictly before mutating state
            if !dt.is_finite() || dt <= 0.0 || dt > 0.1 {
                return Err(FlightError::InvalidDt(dt));
            }
            if !input.thrust.is_finite() {
                return Err(FlightError::InvalidInput("non-finite thrust vector".into()));
            }
            if !input.yaw_rate.is_finite() {
                return Err(FlightError::InvalidInput("non-finite yaw_rate".into()));
            }

            // Clamp yaw rate to a sane maximum (2 full rotations/sec)
            let sane_yaw_limit = 4.0 * std::f64::consts::PI;
            let clamped_yaw = input.yaw_rate.clamp(-sane_yaw_limit, sane_yaw_limit);

            // Stably normalize local thrust direction and compute desired speed
            let (local_thrust_dir, thrust_len) = safe_normalize_vec3(input.thrust);
            let scale = self.cruise / CRUISE_SPEED;
            let max_speed = scale * if input.boost { BOOST_SPEED } else { CRUISE_SPEED };
            let has_thrust = thrust_len > 1e-6;
            let desired_speed = if has_thrust {
                max_speed * thrust_len.min(1.0)
            } else {
                0.0
            };

            // Internal math microstepping for frame-rate-independent turning + acceleration
            const BASE_HZ: f64 = 1440.0;
            let num_math_steps = ((dt * BASE_HZ).round() as usize).max(1);
            let h = dt / (num_math_steps as f64);

            let mut sim_pos = self.pose.position;
            let mut sim_rot = self.pose.rotation;
            let mut sim_vel = self.velocity;

            for _ in 0..num_math_steps {
                if clamped_yaw.abs() > 1e-12 {
                    let rot_delta = DQuat::from_rotation_y(clamped_yaw * h);
                    sim_rot = safe_normalize_quat(rot_delta * sim_rot);
                }

                let target_vel = if has_thrust {
                    let world_dir = sim_rot * local_thrust_dir;
                    world_dir * desired_speed
                } else {
                    DVec3::ZERO
                };

                let v_needed = target_vel - sim_vel;
                let dv = v_needed.length();
                if dv < 1e-9 {
                    sim_pos += sim_vel * h;
                    sim_vel = target_vel;
                } else {
                    let a_dir = v_needed / dv;
                    let a = LINEAR_ACCELERATION * scale;
                    let t_reach = dv / a;

                    if t_reach >= h {
                        let a_vec = a_dir * a;
                        sim_pos += sim_vel * h + 0.5 * a_vec * (h * h);
                        sim_vel += a_vec * h;
                    } else {
                        let a_vec = a_dir * a;
                        let t_rem = h - t_reach;
                        let d_accel = sim_vel * t_reach + 0.5 * a_vec * (t_reach * t_reach);
                        let d_cruise = target_vel * t_rem;
                        sim_pos += d_accel + d_cruise;
                        sim_vel = target_vel;
                    }
                }
            }

            let target_pose = ShipPose {
                position: sim_pos,
                rotation: sim_rot,
            };
            let target_velocity = sim_vel;

            // Check if current position is already obstructed
            if !collision.is_clear(&self.pose, HULL_HALF_EXTENTS) {
                self.velocity = DVec3::ZERO;
                self.ticks = self.ticks.saturating_add(1);
                return Ok(self.snapshot());
            }

            let start_pose = self.pose;
            let (reached_pose, reached) = furthest_clear(collision, &start_pose, &target_pose);
            if reached {
                self.pose = reached_pose;
                self.velocity = target_velocity;
            } else {
                // Only what is in the way stops: the turn and each axis go on
                // their own, so a hull against a slope slides along it and can
                // always lift away.
                let turned = ShipPose {
                    position: start_pose.position,
                    rotation: target_pose.rotation,
                };
                let (mut pose, _) = furthest_clear(collision, &start_pose, &turned);
                let mut velocity = target_velocity;
                let motion = target_pose.position - start_pose.position;
                for axis in [DVec3::Y, DVec3::X, DVec3::Z] {
                    let moved = ShipPose {
                        position: pose.position + axis * motion.dot(axis),
                        rotation: pose.rotation,
                    };
                    let (next, clear) = furthest_clear(collision, &pose, &moved);
                    pose = next;
                    if !clear {
                        velocity -= axis * velocity.dot(axis);
                    }
                }
                self.pose = pose;
                self.velocity = velocity;
            }

            self.ticks = self.ticks.saturating_add(1);
            Ok(self.snapshot())
        }
    }

    /// Follows the straight path from `start` to `target`, counting the arc of
    /// the hull's corners, as far as it stays clear. Returns where it ended and
    /// whether that is `target`.
    fn furthest_clear<C: CollisionQuery + ?Sized>(
        collision: &C,
        start: &ShipPose,
        target: &ShipPose,
    ) -> (ShipPose, bool) {
        const MAX_SUBSTEP_DIST: f64 = 1.0;
        let lin_dist = (target.position - start.position).length();
        let q_dot = start.rotation.dot(target.rotation).abs().min(1.0);
        let ang_dist = HULL_HALF_EXTENTS.length() * 2.0 * q_dot.acos();
        let max_motion = lin_dist + ang_dist;
        if max_motion <= 1e-9 {
            return (*target, true);
        }
        let substeps = ((max_motion / MAX_SUBSTEP_DIST).ceil() as usize).clamp(1, 128);
        let blocked = (1..=substeps).find(|&i| {
            let frac = i as f64 / substeps as f64;
            !collision.is_clear(&ShipPose::interpolate(start, target, frac), HULL_HALF_EXTENTS)
        });
        let Some(step) = blocked else {
            return (*target, true);
        };
        // Binary search for the furthest clear fraction along the path.
        let mut s_lo = (step - 1) as f64 / substeps as f64;
        let mut s_hi = step as f64 / substeps as f64;
        let mut last_clear_pose = ShipPose::interpolate(start, target, s_lo);
        for _ in 0..16 {
            let s_mid = 0.5 * (s_lo + s_hi);
            let candidate_pose = ShipPose::interpolate(start, target, s_mid);
            if collision.is_clear(&candidate_pose, HULL_HALF_EXTENTS) {
                s_lo = s_mid;
                last_clear_pose = candidate_pose;
            } else {
                s_hi = s_mid;
            }
        }
        (last_clear_pose, false)
    }
}

pub mod navigation {
    use glam::{DQuat, DVec3};
    use super::collision::CollisionQuery;
    use super::pose::{safe_normalize_quat, ShipPose};

    /// Supported game dimensions for travel and portals.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum Dimension {
        Overworld,
        Nether,
        End,
    }

    /// Spatial and boundary limits for destination coordinate validation.
    #[allow(dead_code)] // For the coordinate dash.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct NavigationLimits {
        pub min_y: f64,
        pub max_y: f64,
        pub border: f64,
        pub half_extents: DVec3,
    }

    /// Navigation destination coordinate request.
    #[allow(dead_code)] // For the coordinate dash.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct NavigationTarget {
        pub dimension: Dimension,
        pub position: DVec3,
    }

    /// Errors resulting from invalid navigation coordinates, orientation, or obstacles.
    #[allow(dead_code)] // For the coordinate dash.
    #[derive(Clone, Debug, PartialEq)]
    pub enum NavigationError {
        InvalidPosition,
        InvalidLimits,
        InvalidOrientation,
        OutOfBounds(String),
        Occupied,
    }

    impl std::fmt::Display for NavigationError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::InvalidPosition => write!(f, "navigation target position is not finite"),
                Self::InvalidLimits => write!(f, "navigation limits are invalid or inconsistent"),
                Self::InvalidOrientation => write!(f, "navigation arrival orientation is invalid"),
                Self::OutOfBounds(reason) => write!(f, "navigation target out of bounds: {reason}"),
                Self::Occupied => write!(f, "navigation target destination is obstructed"),
            }
        }
    }

    impl std::error::Error for NavigationError {}

    #[allow(dead_code)] // For the coordinate dash.
    impl NavigationTarget {
        /// Validates destination coordinates and arrival orientation against bounds and collision clearance.
        ///
        /// Computes rotated world AABB extents for world border and altitude bounds checks.
        /// Passes the exact candidate pose with local hull half-extents to the collision query.
        pub fn validate_pose<C: CollisionQuery + ?Sized>(
            &self,
            limits: &NavigationLimits,
            rotation: DQuat,
            collision: &C,
        ) -> Result<(), NavigationError> {
            if !self.position.is_finite() {
                return Err(NavigationError::InvalidPosition);
            }

            if !rotation.is_finite() {
                return Err(NavigationError::InvalidOrientation);
            }

            let max_q = rotation.x.abs().max(rotation.y.abs()).max(rotation.z.abs()).max(rotation.w.abs());
            if max_q <= 0.0 || !max_q.is_finite() {
                return Err(NavigationError::InvalidOrientation);
            }
            let safe_rot = safe_normalize_quat(rotation);
            if (safe_rot.length() - 1.0).abs() > 1e-4 {
                return Err(NavigationError::InvalidOrientation);
            }

            if !limits.min_y.is_finite()
                || !limits.max_y.is_finite()
                || !limits.border.is_finite()
                || !limits.half_extents.is_finite()
                || limits.min_y >= limits.max_y
                || limits.border < 0.0
                || limits.half_extents.x < 0.0
                || limits.half_extents.y < 0.0
                || limits.half_extents.z < 0.0
            {
                return Err(NavigationError::InvalidLimits);
            }

            // Rotated world AABB extents = abs(rotation axes) * local half_extents
            let axis_x = safe_rot * DVec3::X;
            let axis_y = safe_rot * DVec3::Y;
            let axis_z = safe_rot * DVec3::Z;

            let world_extent_x = axis_x.x.abs() * limits.half_extents.x
                + axis_y.x.abs() * limits.half_extents.y
                + axis_z.x.abs() * limits.half_extents.z;

            let world_extent_y = axis_x.y.abs() * limits.half_extents.x
                + axis_y.y.abs() * limits.half_extents.y
                + axis_z.y.abs() * limits.half_extents.z;

            let world_extent_z = axis_x.z.abs() * limits.half_extents.x
                + axis_y.z.abs() * limits.half_extents.y
                + axis_z.z.abs() * limits.half_extents.z;

            // Altitude bounds for rotated hull
            let hull_bottom = self.position.y - world_extent_y;
            let hull_top = self.position.y + world_extent_y;
            if hull_bottom < limits.min_y {
                return Err(NavigationError::OutOfBounds(format!(
                    "hull bottom {hull_bottom} is below min altitude {}",
                    limits.min_y
                )));
            }
            if hull_top > limits.max_y {
                return Err(NavigationError::OutOfBounds(format!(
                    "hull top {hull_top} is above max altitude {}",
                    limits.max_y
                )));
            }

            // World border bounds for rotated hull
            if (self.position.x.abs() + world_extent_x) > limits.border {
                return Err(NavigationError::OutOfBounds(format!(
                    "hull X extent exceeds border {}",
                    limits.border
                )));
            }
            if (self.position.z.abs() + world_extent_z) > limits.border {
                return Err(NavigationError::OutOfBounds(format!(
                    "hull Z extent exceeds border {}",
                    limits.border
                )));
            }

            // Spatial clearance query at destination with candidate pose and local half_extents
            let dest_pose = ShipPose {
                position: self.position,
                rotation: safe_rot,
            };
            if !collision.is_clear(&dest_pose, limits.half_extents) {
                return Err(NavigationError::Occupied);
            }

            Ok(())
        }

        /// Validates destination coordinates using default axis-aligned identity orientation.
        pub fn validate<C: CollisionQuery + ?Sized>(
            &self,
            limits: &NavigationLimits,
            collision: &C,
        ) -> Result<(), NavigationError> {
            self.validate_pose(limits, DQuat::IDENTITY, collision)
        }
    }
}

// Flat re-exports at root for seamless usage by contracts and child modules
pub use collision::*;
pub use constants::*;
pub use flight::*;
pub use navigation::*;
pub use pose::*;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DQuat, DVec3};

    struct AlwaysClear;
    impl CollisionQuery for AlwaysClear {
        fn is_clear(&self, _pose: &ShipPose, _half_extents: DVec3) -> bool {
            true
        }
    }

    #[test]
    fn test_pose_invalid_quaternion_sanitization() {
        let nan_rot = DQuat::from_xyzw(f64::NAN, 0.0, 0.0, 1.0);
        let pose = ShipPose::new(DVec3::ZERO, nan_rot);
        assert!(pose.rotation.is_finite());
        assert_eq!(pose.rotation, DQuat::IDENTITY);

        let zero_rot = DQuat::from_xyzw(0.0, 0.0, 0.0, 0.0);
        let pose_zero = ShipPose::new(DVec3::new(1.0, 2.0, 3.0), zero_rot);
        assert!(pose_zero.rotation.is_finite());
        assert_eq!(pose_zero.rotation, DQuat::IDENTITY);

        let huge_rot = DQuat::from_xyzw(1e308, -1e308, 1e308, 1e308);
        let pose_huge = ShipPose::new(DVec3::ZERO, huge_rot);
        assert!(pose_huge.rotation.is_finite());
        assert!((pose_huge.rotation.length() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_pose_interpolate_invalid_alpha() {
        let p0 = ShipPose::new(DVec3::ZERO, DQuat::IDENTITY);
        let p1 = ShipPose::new(DVec3::new(10.0, 20.0, 30.0), DQuat::IDENTITY);

        let at_nan = ShipPose::interpolate(&p0, &p1, f64::NAN);
        assert_eq!(at_nan.position, p0.position);
        assert!(at_nan.rotation.is_finite());

        let at_neg = ShipPose::interpolate(&p0, &p1, -5.0);
        assert_eq!(at_neg.position, p0.position);

        let at_inf = ShipPose::interpolate(&p0, &p1, f64::INFINITY);
        assert_eq!(at_inf.position, p0.position);

        let at_large = ShipPose::interpolate(&p0, &p1, 5.0);
        assert_eq!(at_large.position, p1.position);
    }

    #[test]
    fn test_navigation_invalid_limits_rejection() {
        let target = NavigationTarget {
            dimension: Dimension::Overworld,
            position: DVec3::new(0.0, 100.0, 0.0),
        };

        let inverted_y = NavigationLimits {
            min_y: 320.0,
            max_y: -64.0,
            border: 30_000_000.0,
            half_extents: DVec3::new(48.0, 18.0, 80.0),
        };
        assert_eq!(target.validate(&inverted_y, &AlwaysClear), Err(NavigationError::InvalidLimits));

        let neg_border = NavigationLimits {
            min_y: -64.0,
            max_y: 320.0,
            border: -100.0,
            half_extents: DVec3::new(48.0, 18.0, 80.0),
        };
        assert_eq!(target.validate(&neg_border, &AlwaysClear), Err(NavigationError::InvalidLimits));

        let neg_extent = NavigationLimits {
            min_y: -64.0,
            max_y: 320.0,
            border: 30_000_000.0,
            half_extents: DVec3::new(-48.0, 18.0, 80.0),
        };
        assert_eq!(target.validate(&neg_extent, &AlwaysClear), Err(NavigationError::InvalidLimits));
    }

    #[test]
    fn test_flight_analytical_transition_within_step() {
        let mut f = Flight::new(ShipPose::default());
        let input = ShipInput { thrust: DVec3::Z, ..Default::default() };
        for _ in 0..39 {
            f.tick(&input, 0.1, &AlwaysClear).unwrap();
        }
        let v_pre = f.snapshot().velocity;
        assert!((v_pre.z - 62.4).abs() <= 1e-6);

        f.tick(&input, 0.05, &AlwaysClear).unwrap();
        let v_mid = f.snapshot().velocity;
        assert!((v_mid.z - 63.2).abs() <= 1e-6);

        f.tick(&input, 0.1, &AlwaysClear).unwrap();
        let v_post = f.snapshot().velocity;
        assert!((v_post.z - 64.0).abs() <= 1e-6);
    }
}
