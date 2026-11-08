#[path = "../mixins/astral_cruiser/core.rs"]
mod astral_core;
#[path = "../mixins/astral_cruiser/dash.rs"]
mod astral_dash;
#[path = "../mixins/astral_cruiser/passage.rs"]
mod astral_passage;

use astral_core::{CollisionQuery, Dimension, HULL_HALF_EXTENTS, ShipPose};
use astral_dash::chunks_of;
use astral_passage::{
    ExitKind, Finding, Gate, OPENING, PassStage, Passage, Stride, adoption_ok, corresponding,
    room_at, scene_radius, spots_around, through_opening,
};
use glam::{DQuat, DVec3};
use std::collections::HashSet;

const TICK: f64 = 0.05;
const HALF: DVec3 = DVec3::new(10.0, 10.0, 10.0);

struct Blocks(Vec<[f64; 6]>);

impl CollisionQuery for Blocks {
    fn is_clear(&self, pose: &ShipPose, half: DVec3) -> bool {
        let (low, high) = (pose.position - half, pose.position + half);
        !self.0.iter().any(|b| {
            low.x < b[3] && high.x > b[0] && low.y < b[4] && high.y > b[1] && low.z < b[5] && high.z > b[2]
        })
    }
}

fn start() -> ShipPose {
    ShipPose::new(DVec3::new(100.0, 120.0, -50.0), DQuat::from_rotation_y(0.7))
}

fn passage() -> Passage {
    Passage::new(start(), Dimension::Overworld, Dimension::Nether, ExitKind::Usual)
}

fn opened() -> Passage {
    let mut passage = passage();
    passage.opened(70.0);
    passage
}

#[test]
fn the_gate_stands_ahead_of_the_nose_square_to_the_ship() {
    let passage = passage();
    let forward = start().rotation * DVec3::Z;
    let gate = passage.gate;
    let ahead = (gate.entry - start().position).dot(forward);
    assert!(ahead > 80.0, "{ahead}");
    assert!((gate.entry - start().position - forward * ahead).length() < 1e-9);
    assert_eq!(gate.forward(), forward);
    assert_eq!(gate.from, Dimension::Overworld);
    assert_eq!(gate.to, Dimension::Nether);
}

#[test]
fn the_hull_goes_through_the_opening() {
    assert!(OPENING[0] > 48.0 && OPENING[1] > 18.0);
    let (x, y) = (48.0 / OPENING[0], 18.0 / OPENING[1]);
    assert!(x * x + y * y < 1.0, "the corners of the hull stick out");
}

#[test]
fn the_nether_is_an_eighth_of_the_overworld() {
    let at = DVec3::new(800.0, 90.0, -160.0);
    use Dimension::{End, Nether, Overworld};
    assert_eq!(corresponding(Overworld, Nether, at), DVec3::new(100.0, 90.0, -20.0));
    assert_eq!(corresponding(Nether, Overworld, at), DVec3::new(6400.0, 90.0, -1280.0));
    assert_eq!(corresponding(Overworld, End, at), at);
    assert_eq!(corresponding(End, Nether, at), at);
}

#[test]
fn the_exit_is_the_entry_in_the_other_world() {
    let mut passage = passage();
    passage.opened(70.0);
    let (gate, entry) = (passage.gate, passage.gate.entry);
    let expected = corresponding(Dimension::Overworld, Dimension::Nether, entry);
    assert_eq!(gate.exit, DVec3::new(expected.x, 70.0, expected.z));
}

#[test]
fn a_ship_is_as_far_from_the_exit_as_it_was_from_the_entry() {
    let gate = opened().gate;
    let at = gate.entry + gate.forward() * 12.5 + DVec3::Y * 3.0;
    let through = at + gate.shift();
    assert!((through - gate.exit - (at - gate.entry)).length() < 1e-9);
}

#[test]
fn past_measures_from_the_plane_along_the_way() {
    let gate = opened().gate;
    assert!((gate.past(gate.entry + gate.forward() * 5.0) - 5.0).abs() < 1e-9);
    assert!((gate.past(gate.entry - gate.forward() * 5.0) + 5.0).abs() < 1e-9);
    let across = gate.rotation * DVec3::X;
    assert!(gate.past(gate.entry + across * 50.0).abs() < 1e-9);
}

#[test]
fn nothing_moves_until_the_far_side_is_ready() {
    let mut passage = passage();
    assert_eq!(passage.stage(), PassStage::Preparing);
    assert!(matches!(passage.tick(TICK), Stride::Held));
    assert!(!passage.go());
    assert_eq!(passage.stage(), PassStage::Preparing);
}

#[test]
fn a_far_side_that_never_loads_ends_it() {
    let mut passage = passage();
    assert!((0..2000).any(|_| passage.tick(TICK) == Stride::TimedOut));
}

#[test]
fn an_open_portal_waits_for_the_pilot() {
    let mut passage = opened();
    assert_eq!(passage.stage(), PassStage::Open);
    assert!((0..2000).all(|_| passage.tick(TICK) == Stride::Held));
    assert!(passage.go());
    assert_eq!(passage.stage(), PassStage::Going);
}

#[test]
fn the_ship_flies_straight_at_the_plane() {
    let mut passage = opened();
    passage.go();
    let gate = passage.gate;
    let mut before = gate.past(start().position);
    for _ in 0..40 {
        let Stride::Moved { pose, velocity } = passage.tick(TICK) else {
            panic!("the ship stopped");
        };
        let past = gate.past(pose.position);
        assert!(past > before);
        let beside = pose.position - gate.entry - gate.forward() * past;
        assert!(beside.length() < 1e-9, "it left the line: {beside}");
        assert_eq!(pose.rotation, start().rotation);
        assert!(velocity.length() > 0.0);
        before = past;
    }
}

#[test]
fn the_ship_comes_out_on_the_far_side_and_stops_there() {
    let mut passage = opened();
    passage.go();
    let gate = passage.gate;
    let mut moved = 0;
    loop {
        moved += 1;
        assert!(moved < 4000, "it never ended");
        match passage.tick(TICK) {
            Stride::Moved { pose, .. } => {
                if gate.past(pose.position) > 0.0 && passage.stage() == PassStage::Going {
                    let through = passage.cross();
                    let shifted = pose.position + gate.shift();
                    assert!((through.position - shifted).length() < 1e-9);
                }
            }
            Stride::Done(pose) => {
                let beyond = (pose.position - gate.exit).dot(gate.forward());
                assert!(beyond > 120.0, "{beyond}");
                let beside = pose.position - gate.exit - gate.forward() * beyond;
                assert!(beside.length() < 1e-9);
                return;
            }
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn a_ship_never_left_on_the_wrong_side_is_told_to_cross() {
    let mut passage = opened();
    passage.go();
    let asked = (0..4000).find(|_| passage.tick(TICK) == Stride::MustCross);
    assert!(asked.is_some(), "the crossing was never forced");
    assert_eq!(passage.stage(), PassStage::Going);
    passage.cross();
    assert_eq!(passage.stage(), PassStage::Through);
}

#[test]
fn only_a_portal_not_yet_flown_into_can_be_closed() {
    assert!(passage().can_close());
    assert!(opened().can_close());
    let mut going = opened();
    going.go();
    assert!(!going.can_close());
}

#[test]
fn the_save_keeps_the_ship_where_it_was_sure_to_be() {
    let mut passage = opened();
    assert_eq!(passage.safe_pose().position, start().position);
    passage.go();
    for _ in 0..30 {
        passage.tick(TICK);
    }
    assert_eq!(passage.safe_pose().position, start().position);
    let through = passage.cross();
    assert_eq!(passage.safe_pose(), through);
    assert!((through.position - start().position - passage.gate.shift()).length() > 1.0);
}

#[test]
fn room_is_found_above_what_fills_the_exit() {
    let terrain = Blocks(vec![[-100.0, 0.0, -100.0, 100.0, 95.0, 400.0]]);
    let at = DVec3::new(0.0, 60.0, 0.0);
    assert_eq!(room_at(&terrain, at, DQuat::IDENTITY, HALF, 320.0), Some(108.0));
}

#[test]
fn room_needs_the_way_ahead_clear_too() {
    let terrain = Blocks(vec![[-100.0, 0.0, 120.0, 100.0, 150.0, 130.0]]);
    let at = DVec3::new(0.0, 60.0, 0.0);
    assert_eq!(room_at(&terrain, at, DQuat::IDENTITY, HALF, 320.0), Some(160.0));
}

#[test]
fn what_stands_behind_the_exit_plane_does_not_matter() {
    let terrain = Blocks(vec![[-100.0, 0.0, -70.0, 100.0, 150.0, 20.0]]);
    let at = DVec3::new(0.0, 60.0, 0.0);
    assert_eq!(room_at(&terrain, at, DQuat::IDENTITY, HALF, 320.0), Some(60.0));
}

#[test]
fn no_room_at_all_is_said_so() {
    let terrain = Blocks(vec![[-100.0, 0.0, -100.0, 100.0, 400.0, 400.0]]);
    assert_eq!(room_at(&terrain, DVec3::new(0.0, 60.0, 0.0), DQuat::IDENTITY, HALF, 320.0), None);
}

#[test]
fn a_ship_above_the_far_sides_ceiling_comes_out_under_it() {
    let empty = Blocks(vec![]);
    let at = DVec3::new(0.0, 300.0, 0.0);
    assert_eq!(room_at(&empty, at, DQuat::IDENTITY, HALF, 256.0), Some(246.0));
    let solid = Blocks(vec![[-100.0, 0.0, -100.0, 100.0, 256.0, 400.0]]);
    assert_eq!(room_at(&solid, at, DQuat::IDENTITY, HALF, 256.0), None);
}

fn gate() -> Gate {
    opened().gate
}

#[test]
fn a_far_eye_is_left_alone() {
    let gate = gate();
    let eye = gate.entry - gate.forward() * 400.0 + DVec3::Y * 90.0;
    assert_eq!(through_opening(&gate, eye), eye);
}

#[test]
fn an_eye_at_the_plane_goes_through_the_opening() {
    let gate = gate();
    let across = gate.rotation * DVec3::X;
    let eye = gate.entry + across * 200.0 + DVec3::Y * 90.0;
    let moved = through_opening(&gate, eye) - gate.entry;
    let (x, y) = (moved.dot(across) / OPENING[0], moved.y / OPENING[1]);
    assert!(x * x + y * y < 1.0, "{x} {y}");
}

#[test]
fn an_eye_drifts_into_the_opening_without_a_jump() {
    let gate = gate();
    let aside = DVec3::Y * 90.0;
    let at = |past: f64| through_opening(&gate, gate.entry + gate.forward() * past + aside).y;
    let mut before = at(-400.0);
    for step in 1..=400 {
        let now = at(f64::from(step) - 400.0);
        assert!((now - before).abs() < 1.0, "{before} {now}");
        before = now;
    }
}

#[test]
fn the_chunks_kept_cover_the_ships_way_out() {
    let passage = opened();
    let gate = passage.gate;
    let ((x, z), radius) = passage.exit_area();
    let forward = gate.forward();
    for along in [0.0, 100.0, 260.0] {
        let at = gate.exit + forward * along;
        let (cx, cz) = ((at.x.floor() as i32) >> 4, (at.z.floor() as i32) >> 4);
        assert!((cx - x).abs() <= radius && (cz - z).abs() <= radius, "{along}");
    }
}

/// A ship facing along Z, so that the search's windows line up with the world.
fn straight() -> ShipPose {
    ShipPose::new(DVec3::new(0.0, 120.0, 0.0), DQuat::IDENTITY)
}

fn to_the_nether(kind: ExitKind) -> Passage {
    Passage::new(straight(), Dimension::Overworld, Dimension::Nether, kind)
}

/// Solid rock everywhere but one room.
fn rock_but(room: [f64; 6]) -> Blocks {
    let far = 4000.0;
    let [x0, y0, z0, x1, y1, z1] = room;
    Blocks(vec![
        [-far, -64.0, -far, far, y0, far],
        [-far, y1, -far, far, 400.0, far],
        [-far, y0, -far, x0, y1, far],
        [x1, y0, -far, far, y1, far],
        [x0, y0, -far, x1, y1, z0],
        [x0, y0, z1, x1, y1, far],
    ])
}

fn found(passage: &mut Passage, terrain: &Blocks) -> Finding {
    for _ in 0..40 {
        let finding = passage.look_for_cave(terrain, HALF, || true);
        if finding != Finding::Looking {
            return finding;
        }
    }
    Finding::Looking
}

#[test]
fn a_cave_the_hull_fits_in_is_where_the_ship_comes_out() {
    let room = [-60.0, 60.0, -30.0, 60.0, 100.0, 300.0];
    let mut passage = to_the_nether(ExitKind::Cave);
    assert_eq!(found(&mut passage, &rock_but(room)), Finding::Opened);
    assert_eq!(passage.stage(), PassStage::Open);
    let exit = passage.gate.exit;
    assert!(exit.y >= 70.0 && exit.y <= 90.0, "{exit}");
    assert!(exit.x.abs() <= 50.0, "{exit}");
}

#[test]
fn a_cave_off_to_the_side_is_found_in_the_next_window() {
    let room = [400.0, 60.0, -30.0, 640.0, 100.0, 300.0];
    let mut passage = to_the_nether(ExitKind::Cave);
    let first = passage.exit_area();
    let terrain = rock_but(room);
    assert_eq!(passage.look_for_cave(&terrain, HALF, || true), Finding::Looking);
    assert_ne!(passage.exit_area(), first, "the search stayed where it found nothing");
    assert_eq!(found(&mut passage, &terrain), Finding::Opened);
    assert!(passage.gate.exit.x > 350.0, "{}", passage.gate.exit);
}

#[test]
fn the_ship_stays_clear_of_the_lava_sea_and_under_the_roof() {
    let room = [-300.0, 0.0, -300.0, 300.0, 127.0, 300.0];
    let mut passage = to_the_nether(ExitKind::Cave);
    assert_eq!(found(&mut passage, &rock_but(room)), Finding::Opened);
    let y = passage.gate.exit.y;
    assert!((52.0..=108.0).contains(&y), "{y}");
}

#[test]
fn with_no_cave_anywhere_the_ship_goes_out_over_the_roof() {
    let ideal = to_the_nether(ExitKind::Cave).gate.exit;
    let solid = Blocks(vec![[-4000.0, -64.0, -4000.0, 4000.0, 127.0, 4000.0]]);
    let mut passage = to_the_nether(ExitKind::Cave);
    assert_eq!(found(&mut passage, &solid), Finding::NoCave);
    assert_eq!(passage.stage(), PassStage::Preparing, "the roof still has to load");
    assert_eq!(passage.searching(), None);
    let exit = passage.gate.exit;
    assert_eq!((exit.x, exit.z), (ideal.x, ideal.z));
    assert!(exit.y >= 146.0, "{exit}");
}

#[test]
fn the_roof_can_be_asked_for_outright() {
    let passage = to_the_nether(ExitKind::Roof);
    assert_eq!(passage.searching(), None);
    assert!(passage.gate.exit.y >= 146.0, "{}", passage.gate.exit);
}

#[test]
fn a_search_says_how_far_it_has_got() {
    let mut passage = to_the_nether(ExitKind::Cave);
    let (first, windows) = passage.searching().unwrap();
    assert_eq!(first, 1);
    assert!(windows > 1);
    let solid = Blocks(vec![[-4000.0, -64.0, -4000.0, 4000.0, 127.0, 4000.0]]);
    passage.look_for_cave(&solid, HALF, || true);
    assert_eq!(passage.searching(), Some((2, windows)));
}

#[test]
fn a_search_stops_when_its_time_is_up() {
    let room = [-60.0, 60.0, -30.0, 60.0, 100.0, 300.0];
    let mut passage = to_the_nether(ExitKind::Cave);
    let mut left = 3;
    let more = || {
        left -= 1;
        left >= 0
    };
    assert_eq!(passage.look_for_cave(&rock_but(room), HALF, more), Finding::Looking);
    assert_eq!(passage.stage(), PassStage::Preparing);
}

#[test]
fn each_window_of_a_search_gets_its_own_time_to_load() {
    let mut passage = to_the_nether(ExitKind::Cave);
    for _ in 0..1100 {
        passage.tick(TICK);
    }
    let solid = Blocks(vec![[-4000.0, -64.0, -4000.0, 4000.0, 127.0, 4000.0]]);
    passage.look_for_cave(&solid, HALF, || true);
    assert!((0..1100).all(|_| passage.tick(TICK) == Stride::Held));
}

#[test]
fn a_cave_is_only_looked_for_in_the_nether() {
    let passage = Passage::new(straight(), Dimension::Overworld, Dimension::End, ExitKind::Cave);
    assert_eq!(passage.searching(), None);
}

#[test]
fn no_spot_of_a_window_needs_a_chunk_it_did_not_load() {
    let passage = to_the_nether(ExitKind::Cave);
    let chunks: HashSet<(i32, i32)> = chunks_of(passage.exit_area()).into_iter().collect();
    let half = HULL_HALF_EXTENTS;
    for spot in spots_around(passage.gate.exit, DQuat::IDENTITY) {
        let (stern, bow) = (spot.z + 40.0 - half.z, spot.z + 180.0 + half.z);
        for x in [spot.x - half.x, spot.x + half.x] {
            for z in [stern, bow] {
                let chunk = ((x.floor() as i32) >> 4, (z.floor() as i32) >> 4);
                assert!(chunks.contains(&chunk), "{spot} needs {chunk:?}");
            }
        }
    }
}

#[test]
fn a_window_tries_spots_along_the_way_as_well_as_across_it() {
    let room = [-60.0, 60.0, 90.0, 60.0, 100.0, 253.0];
    let mut passage = to_the_nether(ExitKind::Cave);
    let finding = passage.look_for_cave(&rock_but(room), HALF, || true);
    assert_eq!(finding, Finding::Opened);
}

fn open_straight() -> Passage {
    let mut passage = to_the_nether(ExitKind::Usual);
    passage.opened(60.0);
    passage
}

fn there(position: DVec3) -> ShipPose {
    ShipPose::new(position, DQuat::IDENTITY)
}

#[test]
fn an_open_portal_lets_the_ship_wander_and_shuts_when_it_goes_too_far() {
    let mut passage = open_straight();
    let entry = passage.gate.entry;
    passage.moved_to(there(entry + DVec3::new(300.0, 0.0, -100.0)));
    assert!(!passage.too_far());
    passage.moved_to(there(entry + DVec3::new(500.0, 0.0, 0.0)));
    assert!(passage.too_far());
}

#[test]
fn flying_into_the_mouth_takes_the_ship_through() {
    let mut passage = open_straight();
    let entry = passage.gate.entry;
    passage.moved_to(there(entry - DVec3::Z * 80.0));
    assert!(passage.at_the_mouth());
    passage.moved_to(there(entry + DVec3::new(120.0, 0.0, -80.0)));
    assert!(!passage.at_the_mouth(), "beside the opening");
    passage.moved_to(ShipPose::new(entry - DVec3::Z * 80.0, DQuat::from_rotation_y(3.0)));
    assert!(!passage.at_the_mouth(), "tail first");
}

#[test]
fn from_off_to_the_side_the_ship_still_goes_through_the_middle() {
    let mut passage = open_straight();
    let entry = passage.gate.entry;
    passage.moved_to(ShipPose::new(entry + DVec3::new(150.0, 40.0, -200.0), DQuat::from_rotation_y(0.5)));
    assert!(passage.go());
    let mut pose = passage.pose();
    for _ in 0..400 {
        if let Stride::Moved { pose: now, .. } = passage.tick(TICK) {
            pose = now;
        }
        if pose.position.z >= entry.z - 40.0 {
            break;
        }
    }
    assert!((pose.position.x - entry.x).abs() < 0.01, "{}", pose.position);
    assert!((pose.position.y - entry.y).abs() < 0.01, "{}", pose.position);
    assert!(pose.rotation.angle_between(DQuat::IDENTITY) < 0.01);
}

#[test]
fn from_behind_the_portal_there_is_no_going_through() {
    let mut passage = open_straight();
    let entry = passage.gate.entry;
    passage.moved_to(there(entry + DVec3::Z * 50.0));
    assert!(!passage.go());
    assert_eq!(passage.stage(), PassStage::Open);
}

#[test]
fn a_gate_flipped_twice_returns_to_itself() {
    let gate = gate();
    assert_eq!(gate.flipped().flipped(), gate);
    assert_eq!(gate.flipped().from, gate.to);
    assert_eq!(gate.flipped().to, gate.from);
    assert_eq!(gate.flipped().entry, gate.exit);
    assert_eq!(gate.flipped().exit, gate.entry);
    assert_eq!(gate.flipped().rotation, gate.rotation);
}

#[test]
fn a_point_carried_through_and_back_returns_home() {
    let gate = gate();
    let point = DVec3::new(12.34, 56.78, -90.12);
    let there_and_back = point + gate.shift() + gate.flipped().shift();
    assert!((there_and_back - point).length() < 1e-9);
}

#[test]
fn crossing_front_to_back_through_the_opening() {
    let gate = gate();
    let forward = gate.forward();
    let before = gate.entry - forward * 2.0;
    let after = gate.entry + forward * 2.0;
    assert!(gate.crossing(before, after));
}

#[test]
fn crossing_back_to_front_through_the_opening() {
    let gate = gate();
    let forward = gate.forward();
    let before = gate.entry + forward * 2.0;
    let after = gate.entry - forward * 2.0;
    assert!(gate.crossing(before, after));
}

#[test]
fn beside_the_opening_does_not_cross() {
    let gate = gate();
    let forward = gate.forward();
    let right = gate.rotation * DVec3::X;
    // Beside across the opening
    let before_wide = gate.entry - forward * 2.0 + right * (OPENING[0] + 5.0);
    let after_wide = gate.entry + forward * 2.0 + right * (OPENING[0] + 5.0);
    assert!(!gate.crossing(before_wide, after_wide));
    // Above the opening
    let before_high = gate.entry - forward * 2.0 + DVec3::Y * (OPENING[1] + 5.0);
    let after_high = gate.entry + forward * 2.0 + DVec3::Y * (OPENING[1] + 5.0);
    assert!(!gate.crossing(before_high, after_high));
}

#[test]
fn steps_on_one_side_do_not_cross() {
    let gate = gate();
    let forward = gate.forward();
    // In front
    let before_front = gate.entry - forward * 5.0;
    let after_front = gate.entry - forward * 1.0;
    assert!(!gate.crossing(before_front, after_front));
    // Behind
    let before_back = gate.entry + forward * 1.0;
    let after_back = gate.entry + forward * 5.0;
    assert!(!gate.crossing(before_back, after_back));
}

#[test]
fn entry_area_is_centred_on_the_entry() {
    let passage = opened();
    let gate = passage.gate;
    let ((cx, cz), radius) = passage.entry_area();
    assert_eq!(cx, (gate.entry.x.floor() as i32) >> 4);
    assert_eq!(cz, (gate.entry.z.floor() as i32) >> 4);
    assert_eq!(radius, 14);
}

#[test]
fn adoption_ok_when_scene_covers_render_distance() {
    assert!(adoption_ok(Dimension::End, Dimension::End, ((0, 0), 13), (2, -3), 10, false));
}

#[test]
fn adoption_refused_when_scene_does_not_cover_exit() {
    assert!(!adoption_ok(Dimension::End, Dimension::End, ((0, 0), 13), (10, 0), 10, false));
}

#[test]
fn adoption_refused_for_other_dimension() {
    assert!(!adoption_ok(Dimension::Nether, Dimension::End, ((0, 0), 13), (0, 0), 10, false));
}

#[test]
fn adoption_refused_while_closing() {
    assert!(!adoption_ok(Dimension::End, Dimension::End, ((0, 0), 13), (0, 0), 10, true));
}

#[test]
fn scene_radius_follows_render_distance() {
    assert_eq!(scene_radius(12), 14);
    assert_eq!(scene_radius(20), 21);
    assert_eq!(scene_radius(2), 14);
}
