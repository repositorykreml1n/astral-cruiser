#[path = "../mixins/astral_cruiser/layout.rs"]
mod astral_layout;
#[path = "../mixins/astral_cruiser/furnish.rs"]
mod astral_furnish;
#[path = "../mixins/astral_cruiser/hull_mesh.rs"]
mod astral_hull_mesh;
#[path = "../mixins/astral_cruiser/passenger.rs"]
mod astral_passenger;
#[path = "common/walk.rs"] mod walk;

use astral_layout::{
    ramp_step, ramp_target, Material, RoomKind, Shape, ShipLayout, WeaponMountKind, RAMP_HEIGHT,
    RAMP_SECONDS, UPPER_CEILING, UPPER_DECK,
};
use astral_passenger::{Passenger, WalkKeys};
use glam::DVec3;
use walk::reachable;

const NEW_ROOMS: [RoomKind; 6] = [
    RoomKind::Captain, RoomKind::Arsenal, RoomKind::Gym,
    RoomKind::Gallery, RoomKind::Dome, RoomKind::Stairwell,
];

fn covering(vertices: &[astral_hull_mesh::HullVertex], material: Material, at: DVec3) -> usize {
    vertices.chunks_exact(3).filter(|t| {
        if t[0].material != material as u32 { return false; }
        let [a, b, c] = [t[0].position, t[1].position, t[2].position]
            .map(|p| glam::Vec3::from_array(p).as_dvec3());
        let normal = (b - a).cross(c - a);
        let inside = |p: DVec3, q: DVec3| (q - p).cross(at - p).dot(normal) >= -1e-4;
        (at - a).dot(normal.normalize()).abs() < 1e-3
            && inside(a, b) && inside(b, c) && inside(c, a)
    }).count()
}

#[test]
fn fixed_points_unchanged() {
    let l = ShipLayout::cruiser();
    assert_eq!(l.pilot_seat(), DVec3::new(0.0, -3.6, 50.0));
    assert_eq!(l.bridge_stand(), DVec3::new(0.0, -4.0, 48.2));
    assert_eq!(l.airlock_exit(), DVec3::new(27.0, -4.0, -3.0));
    assert!(l.capsule_clear(DVec3::new(0.0, -4.0, 20.0), 0.3, 1.8),
        "the lower spine must remain clear beside the forward stair");
    for x in [20.9, 21.0, 21.4] {
        for z in [-4.5, -3.0, -1.5] {
            assert!(l.in_outer_door(DVec3::new(x, -4.0, z)));
        }
    }
    for (x, z) in [(20.89, -3.0), (21.41, -3.0), (21.0, -4.51), (21.0, -1.49)] {
        assert!(!l.in_outer_door(DVec3::new(x, -4.0, z)));
    }
    let positions = [
        [5.5,11.5,30.0], [-5.5,11.5,30.0], [10.5,11.5,0.0], [-10.5,11.5,0.0],
        [15.5,11.5,-30.0], [-15.5,11.5,-30.0], [10.0,14.5,-40.0], [-10.0,14.5,-40.0],
        [3.0,-1.0,78.0], [-3.0,-1.0,78.0], [0.0,-16.0,66.5], [0.0,-1.0,80.0],
    ];
    assert_eq!(l.hardpoints.len(), positions.len());
    for (id, (h, position)) in l.hardpoints.iter().zip(positions).enumerate() {
        assert_eq!(h.id, id as u32);
        assert_eq!(h.position, DVec3::from_array(position));
        assert_eq!(h.forward, DVec3::Z);
        assert_eq!(h.kind, match id {
            0..=7 => WeaponMountKind::Turret,
            8..=9 => WeaponMountKind::NosePulse,
            10 => WeaponMountKind::BellyBeam,
            _ => WeaponMountKind::MainBeam,
        });
    }
    for h in l.hardpoints.iter().take(8) {
        assert!(l.parts.iter().any(|p| p.material == Material::Trim
            && p.min == h.position + DVec3::new(-2.0, -1.5, -2.0)
            && p.max == h.position + DVec3::new(2.0, 0.0, 2.0)));
    }
}

#[test]
fn half_extents_unchanged() {
    let l = ShipLayout::cruiser();
    let half = DVec3::new(48.0, 18.0, 80.0);
    assert_eq!(l.half_extents(), half);
    for p in &l.parts {
        assert!(p.min.is_finite() && p.max.is_finite() && p.min.cmplt(p.max).all());
        assert!(p.min.cmpge(-half).all() && p.max.cmple(half).all(), "{p:?}");
    }
}

#[test]
fn upper_deck_and_dome_reachable_from_airlock() {
    let l = ShipLayout::cruiser();
    let mesh = astral_hull_mesh::interior_vertices(&l);
    for (material, at) in [
        (Material::Ceiling, DVec3::new(-3.5, 2.0, 20.0)),
        (Material::Deck, DVec3::new(-3.5, 3.0, 20.0)),
        (Material::Ceiling, DVec3::new(0.0, 9.0, -10.0)),
        (Material::Deck, DVec3::new(0.0, 10.0, -18.2)),
        (Material::Ceiling, DVec3::new(2.0, 15.0, -21.0)),
        (Material::Deck, DVec3::new(6.0, 3.0, -19.0)),
    ] {
        assert_eq!(covering(&mesh, material, at), 0, "{material:?} blocks the opening at {at}");
    }
    assert!(covering(&mesh, Material::Deck, DVec3::new(19.0, 3.0, -19.0)) > 0);
    let hull = astral_hull_mesh::hull_vertices(&l);
    for (material, at) in [
        (Material::Hull, DVec3::new(-3.5, 2.0, 20.0)),
        (Material::Armor, DVec3::new(7.0, 6.0, 4.0)),
        (Material::Armor, DVec3::new(2.0, 13.0, -21.0)),
        (Material::Hull, DVec3::new(2.0, 15.0, -21.0)),
    ] {
        assert_eq!(covering(&hull, material, at), 0, "opaque {material:?} crosses room air at {at}");
    }
    let from = l.room(RoomKind::Airlock).unwrap().entry;
    for room in l.rooms.iter().filter(|r| r.kind != RoomKind::Doorway
        && (NEW_ROOMS.contains(&r.kind) || r.min.y == UPPER_DECK)) {
        assert!(l.capsule_clear(room.entry, 0.3, 1.8), "{:?} has no headroom", room.kind);
        assert!(reachable(&l, from, room.entry), "no route up to {:?} at {}", room.kind, room.entry);
        assert!(reachable(&l, room.entry, from), "no route down from {:?}", room.kind);
    }
}

#[test]
fn stairs_steps_at_most_0_6() {
    let l = ShipLayout::cruiser();
    assert!(!l.capsule_clear(DVec3::new(-4.0, -5.0, -13.05), 0.3, 1.8),
        "the pit rail opposite the gallery stair must stay closed");
    let wells: Vec<_> = l.rooms.iter().filter(|r| r.kind == RoomKind::Stairwell).collect();
    assert!(wells.iter().filter(|r| r.min.y == -4.0 && r.max.y >= UPPER_CEILING).count() >= 2);
    for well in wells {
        let mut treads: Vec<_> = l.props.iter().filter(|p| {
            let center = (p.min + p.max) / 2.0;
            p.solid && p.shape == Shape::Block && p.material == Material::Grate
                && center.cmpge(well.min).all() && center.cmple(well.max).all()
        }).collect();
        treads.sort_by(|a, b| a.max.y.total_cmp(&b.max.y));
        assert!(treads.len() >= 14, "too few steps in {}", well.entry);
        let mut top = well.min.y;
        for tread in &treads {
            assert!((tread.max.y - top - 0.5).abs() < 1e-9, "uneven rise at {tread:?}");
            top = tread.max.y;
            let center = (tread.min + tread.max) / 2.0;
            let low = DVec3::new(center.x - 0.3, top + 0.05, center.z - 0.3);
            let high = DVec3::new(center.x + 0.3, top + 1.8, center.z + 0.3);
            for prop in l.props.iter().filter(|p| !p.solid && p.shape != Shape::Hidden) {
                assert!(!(prop.min.cmplt(high).all() && prop.max.cmpgt(low).all()),
                    "decoration crosses the walker above {tread:?}: {prop:?}");
            }
        }
        // Drive the actual walker up and down the flight, including its body radius.
        let low = (treads[0].min + treads[0].max) / 2.0;
        let high = (treads.last().unwrap().min + treads.last().unwrap().max) / 2.0;
        let sign = (high.z - low.z).signum();
        let start = DVec3::new(low.x, well.min.y, low.z - sign);
        let end = DVec3::new(high.x, top, high.z + sign * 0.4);
        let mut walker = Passenger::at(start);
        for _ in 0..450 {
            walker.tick(&WalkKeys { forward: true, ..WalkKeys::default() },
                if sign > 0.0 { 0.0 } else { std::f64::consts::PI }, &l, 1.0 / 60.0);
        }
        assert!(walker.local.y >= top - 0.01, "walker stuck going up: {}", walker.local);
        let mut walker = Passenger::at(end);
        for _ in 0..230 {
            walker.tick(&WalkKeys { forward: true, ..WalkKeys::default() },
                if sign < 0.0 { 0.0 } else { std::f64::consts::PI }, &l, 1.0 / 60.0);
        }
        assert!(walker.local.y <= well.min.y + 0.01, "walker stuck going down: {}", walker.local);
    }
}

#[test]
fn every_new_room_kind_present_and_furnished() {
    let layout = ShipLayout::cruiser();
    for kind in NEW_ROOMS {
        let rooms: Vec<_> = layout.rooms.iter().filter(|r| r.kind == kind).collect();
        assert!(!rooms.is_empty(), "missing {kind:?}");
        for room in rooms {
            let count = layout.props.iter().filter(|p| {
                let at = (p.min + p.max) / 2.0;
                at.cmpge(room.min).all() && at.cmple(room.max).all()
            }).count();
            assert!(count >= 12, "{kind:?} at {} has only {count} props", room.entry);
        }
    }
}

#[test]
fn vertex_count_at_most_twice_baseline() {
    let l = ShipLayout::cruiser();
    let hull = astral_hull_mesh::hull_vertices(&l).len();
    let furniture = astral_hull_mesh::furniture_vertices(&l).len();
    println!("hull: {hull}, furniture: {furniture}, total: {}", hull + furniture);
    assert!(hull + furniture <= 93_384);
}

#[test]
fn upper_rooms_inside_hull() {
    let l = ShipLayout::cruiser();
    assert_eq!((UPPER_DECK, UPPER_CEILING), (3.0, 9.0));
    for room in l.rooms.iter().filter(|r| r.min.y >= UPPER_DECK) {
        if room.kind != RoomKind::Dome && room.kind != RoomKind::Stairwell {
            assert_eq!(room.min.y, 3.0);
            if room.kind != RoomKind::Doorway { assert_eq!(room.max.y, 9.0); }
            assert!(room.min.z >= -42.0 && room.max.z <= 30.0);
        }
        for x in [room.min.x, room.max.x] { for y in [room.min.y, room.max.y] {
            for z in [room.min.z, room.max.z] {
                let at = DVec3::new(x, y, z);
                assert!(l.parts.iter().any(|p| p.material == Material::Hull && p.contains(at)),
                    "{:?} outside hull at {at}", room.kind);
            }
        }}
    }
    // The gallery must have an open view down, rather than a floor across the hall.
    assert_eq!(l.floor_height(6.0, -19.0, 3.1), Some(-9.0));
    assert!(l.props.iter().any(|p| p.material == Material::Rail && p.solid && p.min.y == 3.0));
}

#[test]
fn beacons_red_left_green_right() {
    let l = ShipLayout::cruiser();
    assert_eq!((Material::Stripe as u32, Material::Beacon as u32, Material::Panel as u32), (24,25,26));
    let beacons: Vec<_> = l.parts.iter().filter(|p| p.material == Material::Beacon).collect();
    // Beacon colours are selected by ship coordinates in the hull shader (Task 5).
    let left: Vec<_> = beacons.iter().filter(|p| p.min.x >= 46.0).collect();
    let right: Vec<_> = beacons.iter().filter(|p| p.max.x <= -46.0).collect();
    assert!(!left.is_empty(), "red navigation light belongs at the +X tip");
    assert_eq!(left.len(), right.len(), "green navigation light belongs at the -X tip");
    for p in left {
        assert!(right.iter().any(|q| q.min == DVec3::new(-p.max.x, p.min.y, p.min.z)
            && q.max == DVec3::new(-p.min.x, p.max.y, p.max.z)));
    }
    assert!(beacons.iter().any(|p| p.min.x < 0.0 && p.max.x > 0.0 && p.max.z >= 79.0));
    assert!(beacons.iter().any(|p| p.min.x < 0.0 && p.max.x > 0.0 && p.min.z <= -79.0));
}

#[test]
fn ramp_box_under_hangar_and_outside_rooms() {
    let l = ShipLayout::cruiser();
    let hangar = l.room(RoomKind::Hangar).unwrap();
    let (min, max) = l.ramp();
    assert!(min.is_finite() && max.is_finite() && min.cmplt(max).all());
    assert!(min.cmpge(-l.half_extents()).all() && max.cmple(l.half_extents()).all());
    assert!(min.x >= hangar.min.x && max.x <= hangar.max.x);
    assert!(min.z >= hangar.min.z && max.z <= hangar.max.z && max.y < hangar.min.y);
    assert!(!l.rooms.iter().any(|r| min.cmplt(r.max).all() && max.cmpgt(r.min).all()));
    assert!(l.parts.iter().any(|p| p.min == min && p.max == max), "closed ramp has no mesh");
    let center = (min + max) / 2.0;
    assert_eq!(l.hull_spans(center.x, center.z).first().unwrap().0, min.y,
        "the closed ramp is buried above the belly armor");
}

#[test]
fn ramp_open_below_12_blocks_closed_above() {
    assert_eq!(RAMP_HEIGHT, 12.0);
    assert_eq!(ramp_target(0.0), 1.0);
    assert_eq!(ramp_target(5.0), 1.0);
    assert_eq!(ramp_target(11.99), 1.0);
    assert_eq!(ramp_target(12.0), 0.0);
    assert_eq!(ramp_target(12.01), 0.0);
    assert_eq!(ramp_target(30.0), 0.0);
    assert_eq!(ramp_target(f64::INFINITY), 0.0);
}

#[test]
fn ramp_step_takes_two_seconds() {
    assert_eq!(RAMP_SECONDS, 2.0);
    let mut open = 0.0;
    open = ramp_step(open, 1.0, 1.0);
    assert!((open - 0.5).abs() < 1e-9);
    open = ramp_step(open, 1.0, 1.0);
    assert!((open - 1.0).abs() < 1e-9);
    open = ramp_step(open, 1.0, 0.5);
    assert!((open - 1.0).abs() < 1e-9);

    open = ramp_step(open, 0.0, 1.0);
    assert!((open - 0.5).abs() < 1e-9);
    open = ramp_step(open, 0.0, 1.0);
    assert!((open - 0.0).abs() < 1e-9);
    open = ramp_step(open, 0.0, 0.5);
    assert!((open - 0.0).abs() < 1e-9);

    let dt = 1.0 / 60.0;
    let mut o = 0.0;
    for _ in 0..60 {
        o = ramp_step(o, 1.0, dt);
    }
    assert!((o - 0.5).abs() < 1e-3);
    for _ in 0..60 {
        o = ramp_step(o, 1.0, dt);
    }
    assert!((o - 1.0).abs() < 1e-9);
}

#[test]
fn ramp_closing_moves_rider_inside() {
    let l = ShipLayout::cruiser();
    let mut open = 1.0;
    let (top, bottom, _) = l.ramp_pose(open);
    let rider_pos = (top + bottom) / 2.0 + DVec3::new(0.0, 0.1, 0.0);
    assert!(l.on_ramp(open, rider_pos), "rider should be on open ramp");

    let target = ramp_target(15.0);
    assert_eq!(target, 0.0);

    for _ in 0..25 {
        open = ramp_step(open, target, 0.1);
    }
    assert_eq!(open, 0.0);
    assert!(!l.on_ramp(open, rider_pos), "rider should not be on closed ramp");

    let inside = l.ramp_rider_inside();
    let hangar = l.room(RoomKind::Hangar).expect("hangar room");
    assert!(hangar.min.cmple(inside).all() && hangar.max.cmpge(inside).all(),
        "rider inside point {inside} must lie inside Hangar [{}, {}]", hangar.min, hangar.max);
    assert!(l.capsule_clear(inside, 0.3, 1.8), "rider inside entry must have clear headroom");
}

#[test]
fn ramp_exit_only_at_lowered_end() {
    let l = ShipLayout::cruiser();
    let open = 1.0;
    let (top, bottom, half_w) = l.ramp_pose(open);

    let plank_point = |x: f64, z: f64| {
        let t = (z - top.z) / (bottom.z - top.z);
        let y = top.y + t * (bottom.y - top.y);
        DVec3::new(x, y + 0.1, z)
    };

    let exit_point = plank_point(0.0, bottom.z + 0.5);
    assert!(l.ramp_exit(open, exit_point), "should exit on last 1.5 blocks when open");

    let at_edge = plank_point(0.0, bottom.z);
    assert!(l.ramp_exit(open, at_edge), "should exit at edge of lowered end");

    let near_top = plank_point(0.0, top.z - 0.5);
    assert!(!l.ramp_exit(open, near_top), "cannot exit near top end");

    let middle = plank_point(0.0, (top.z + bottom.z) / 2.0);
    assert!(!l.ramp_exit(open, middle), "cannot exit in middle of ramp");

    let off_side = plank_point(half_w.x + 1.0, bottom.z + 0.5);
    assert!(!l.ramp_exit(open, off_side), "cannot exit off the side");

    let too_high = exit_point + DVec3::new(0.0, 1.0, 0.0);
    assert!(!l.ramp_exit(open, too_high), "cannot exit high above ramp");

    assert!(!l.ramp_exit(0.9, exit_point), "cannot exit when open <= 0.9");
    assert!(!l.ramp_exit(0.85, exit_point), "cannot exit when open <= 0.9");
    assert!(!l.ramp_exit(0.0, exit_point), "cannot exit when closed");

    let (top_95, bottom_95, _) = l.ramp_pose(0.95);
    let t_95 = (bottom_95.z + 0.5 - top_95.z) / (bottom_95.z - top_95.z);
    let y_95 = top_95.y + t_95 * (bottom_95.y - top_95.y);
    let exit_95 = DVec3::new(0.0, y_95 + 0.1, bottom_95.z + 0.5);
    assert!(l.ramp_exit(0.95, exit_95), "can exit when open > 0.9");
}

#[test]
fn ramp_rejects_nan() {
    let l = ShipLayout::cruiser();
    assert_eq!(ramp_target(f64::NAN), 0.0);
    assert_eq!(ramp_step(f64::NAN, 1.0, 0.1), 0.0);
    assert_eq!(ramp_step(0.5, f64::NAN, 0.1), 0.0);
    assert_eq!(ramp_step(0.5, 1.0, f64::NAN), 0.0);

    let (top, bottom, half_w) = l.ramp_pose(f64::NAN);
    assert!(top.is_finite() && bottom.is_finite() && half_w.is_finite());

    let point = DVec3::new(0.0, -10.0, -55.0);
    assert!(!l.on_ramp(f64::NAN, point));
    assert!(!l.on_ramp(1.0, DVec3::NAN));
    assert!(!l.on_ramp(1.0, DVec3::new(f64::NAN, -10.0, -55.0)));
    assert!(!l.ramp_exit(f64::NAN, point));
    assert!(!l.ramp_exit(1.0, DVec3::NAN));
    assert!(!l.ramp_exit(1.0, DVec3::new(0.0, f64::NAN, -60.0)));
}
