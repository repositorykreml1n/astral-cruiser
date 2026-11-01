#[path = "../mixins/astral_cruiser/layout.rs"]
mod astral_layout;
#[path = "../mixins/astral_cruiser/furnish.rs"]
mod astral_furnish;
#[path = "../mixins/astral_cruiser/hull_mesh.rs"]
mod astral_hull;

use astral_hull::{
    BOLT_VERTICES, Bolt, HullVertex, Rect, Swirl, arrival_ring, bolt_vertices, furniture_vertices,
    hull_vertices, interior_vertices, part_vertices, prop_vertices, subtract, swirl_vertices,
    turret_vertices, warp_bolts,
};
use astral_layout::{Face, Material, Part, Prop, Shape, ShipLayout};
use glam::{DVec3, Vec3};

fn position(v: &HullVertex) -> Vec3 {
    Vec3::from_array(v.position)
}

/// The interior triangles of a kind that pass through a point.
fn covering(vertices: &[HullVertex], material: Material, at: Vec3) -> usize {
    vertices
        .chunks(3)
        .filter(|t| t[0].material == material as u32)
        .filter(|t| {
            let [a, b, c] = [position(&t[0]), position(&t[1]), position(&t[2])];
            let n = (b - a).cross(c - a);
            let inside = |p: Vec3, q: Vec3| (q - p).cross(at - p).dot(n) >= -1e-4;
            (at - a).dot(n.normalize()).abs() < 1e-3 && inside(a, b) && inside(b, c) && inside(c, a)
        })
        .count()
}

fn area(rects: &[Rect]) -> f32 {
    rects.iter().map(|r| (r.max[0] - r.min[0]) * (r.max[1] - r.min[1])).sum()
}

#[test]
fn subtracting_a_hole_keeps_the_rest_of_a_wall() {
    let wall = Rect { min: [0.0, 0.0], max: [10.0, 6.0] };
    let hole = Rect { min: [4.0, 0.0], max: [6.0, 4.0] };
    let left = subtract(wall, &[hole]);
    assert_eq!(area(&left), 60.0 - 8.0);
    assert!(left.iter().all(|r| !(r.min[0] < 5.0 && r.max[0] > 5.0 && r.min[1] < 2.0 && r.max[1] > 2.0)));
}

#[test]
fn a_hole_past_the_edge_only_takes_what_it_overlaps() {
    let wall = Rect { min: [0.0, 0.0], max: [10.0, 6.0] };
    let hole = Rect { min: [8.0, -3.0], max: [20.0, 9.0] };
    assert_eq!(area(&subtract(wall, &[hole])), 48.0);
}

#[test]
fn a_hole_elsewhere_leaves_the_wall_whole() {
    let wall = Rect { min: [0.0, 0.0], max: [10.0, 6.0] };
    let hole = Rect { min: [20.0, 0.0], max: [30.0, 6.0] };
    assert_eq!(subtract(wall, &[hole]), vec![wall]);
}

#[test]
fn two_holes_are_both_cut() {
    let wall = Rect { min: [0.0, 0.0], max: [10.0, 6.0] };
    let holes = [
        Rect { min: [1.0, 1.0], max: [2.0, 2.0] },
        Rect { min: [7.0, 1.0], max: [9.0, 3.0] },
    ];
    assert_eq!(area(&subtract(wall, &holes)), 60.0 - 1.0 - 4.0);
}

#[test]
fn a_room_has_a_floor_under_and_a_ceiling_over_its_entry() {
    let layout = ShipLayout::cruiser();
    let inside = interior_vertices(&layout);
    for room in &layout.rooms {
        let e = room.entry.as_vec3();
        assert!(covering(&inside, Material::Deck, Vec3::new(e.x, room.min.y as f32, e.z)) >= 1, "{:?}", room.kind);
        assert!(covering(&inside, Material::Ceiling, Vec3::new(e.x, room.max.y as f32, e.z)) >= 1, "{:?}", room.kind);
    }
}

#[test]
fn a_room_wall_stands_where_no_door_opens() {
    let inside = interior_vertices(&ShipLayout::cruiser());
    assert_eq!(covering(&inside, Material::Wall, Vec3::new(2.5, -1.0, 35.5)), 1);
}

#[test]
fn a_door_is_cut_through_the_wall() {
    let inside = interior_vertices(&ShipLayout::cruiser());
    // The corridor's side at the door of the gun bay, below the lintel and above it.
    assert_eq!(covering(&inside, Material::Wall, Vec3::new(2.5, -2.0, 11.0)), 0);
    assert_eq!(covering(&inside, Material::Wall, Vec3::new(2.5, 1.0, 11.0)), 1);
}

#[test]
fn the_bridge_front_is_a_window_above_its_sill() {
    let layout = ShipLayout::cruiser();
    let inside = interior_vertices(&layout);
    assert_eq!(covering(&inside, Material::Wall, Vec3::new(0.5, 0.0, 54.0)), 0);
    assert_eq!(covering(&inside, Material::Wall, Vec3::new(8.8, 0.0, 54.0)), 1);
    assert_eq!(covering(&inside, Material::Wall, Vec3::new(0.5, -3.5, 54.0)), 1);
}

#[test]
fn interior_faces_look_into_their_room() {
    let layout = ShipLayout::cruiser();
    let centers: Vec<Vec3> = layout.rooms.iter().map(|r| ((r.min + r.max) / 2.0).as_vec3()).collect();
    for t in interior_vertices(&layout).chunks(3) {
        let [a, b, c] = [position(&t[0]), position(&t[1]), position(&t[2])];
        let normal = (b - a).cross(c - a).normalize();
        assert!((Vec3::from_array(t[0].normal) - normal).length() < 1e-5);
        let middle = (a + b + c) / 3.0;
        assert!(centers.iter().any(|&center| normal.dot(center - middle) > 0.0));
        let up_or_down = normal.y.abs() > 0.5;
        if up_or_down {
            // Floors face up, ceilings down: never the other way round.
            assert_eq!(normal.y > 0.0, t[0].material == Material::Deck as u32);
        }
    }
}

fn outward_faces(part: &Part) {
    let center = ((part.min + part.max) / 2.0).as_vec3();
    let vertices = part_vertices(part);
    assert_eq!(vertices.len(), 36, "a part has six faces of two triangles");
    for t in vertices.chunks(3) {
        let [a, b, c] = [position(&t[0]), position(&t[1]), position(&t[2])];
        let geometric = (b - a).cross(c - a).normalize();
        let middle = (a + b + c) / 3.0;
        assert!(geometric.dot(middle - center) > 0.0, "triangle faces into the part: {a} {b} {c}");
        for v in t {
            assert!((Vec3::from_array(v.normal) - geometric).length() < 1e-5);
        }
    }
}

#[test]
fn every_face_of_a_box_points_outward() {
    outward_faces(&Part {
        min: DVec3::new(-2.0, -3.0, -4.0),
        max: DVec3::new(2.0, 3.0, 4.0),
        material: Material::Hull,
        front_scale: 1.0,
        top_scale: 1.0,
    });
}

#[test]
fn every_face_of_a_tapered_part_points_outward() {
    outward_faces(&Part {
        min: DVec3::new(-20.0, -5.0, -40.0),
        max: DVec3::new(20.0, 5.0, 40.0),
        material: Material::Armor,
        front_scale: 0.2,
        top_scale: 0.6,
    });
}

#[test]
fn a_tapered_nose_keeps_the_scaled_width() {
    let part = Part {
        min: DVec3::new(-10.0, 0.0, 0.0),
        max: DVec3::new(10.0, 10.0, 100.0),
        material: Material::Hull,
        front_scale: 0.5,
        top_scale: 1.0,
    };
    let widest_at_nose = part_vertices(&part)
        .iter()
        .filter(|v| v.position[2] == 100.0)
        .map(|v| v.position[0].abs())
        .fold(0.0_f32, f32::max);
    assert_eq!(widest_at_nose, 5.0);
}

#[test]
fn the_cruiser_mesh_stays_inside_its_advertised_size() {
    let layout = ShipLayout::cruiser();
    let half = layout.half_extents().as_vec3();
    let vertices = hull_vertices(&layout);
    assert!(vertices.len() >= layout.parts.len() * 36);
    assert_eq!(vertices.len() % 3, 0);
    for v in &vertices {
        let p = position(v);
        assert!(p.is_finite());
        assert!(p.abs().cmple(half + Vec3::splat(1e-3)).all(), "outside the hull bounds: {p}");
    }
}

fn outward_prop(prop: &Prop) {
    let center = ((prop.min + prop.max) / 2.0).as_vec3();
    let vertices = prop_vertices(prop);
    assert!(!vertices.is_empty());
    assert_eq!(vertices.len() % 3, 0);
    for t in vertices.chunks(3) {
        let [a, b, c] = [position(&t[0]), position(&t[1]), position(&t[2])];
        let geometric = (b - a).cross(c - a).normalize();
        let middle = (a + b + c) / 3.0;
        assert!(geometric.dot(middle - center) > 0.0, "{:?} faces in: {a} {b} {c}", prop.shape);
    }
}

fn prop_of(shape: Shape) -> Prop {
    Prop {
        min: DVec3::new(-2.0, -1.0, 3.0),
        max: DVec3::new(2.0, 3.0, 9.0),
        material: Material::Console,
        shape,
        solid: true,
        slide: DVec3::ZERO,
        door: DVec3::ZERO,
    }
}

#[test]
fn every_face_of_every_prop_shape_points_outward() {
    outward_prop(&prop_of(Shape::Block));
    outward_prop(&prop_of(Shape::Column));
    for face in [Face::Left, Face::Right, Face::Front, Face::Back] {
        outward_prop(&prop_of(Shape::Slope { face, low: 1.0 }));
    }
}

#[test]
fn a_sloped_top_is_low_on_the_side_it_faces() {
    let prop = prop_of(Shape::Slope { face: Face::Back, low: 1.0 });
    let height = |z: f32| {
        prop_vertices(&prop)
            .iter()
            .filter(|v| v.position[2] == z)
            .map(|v| v.position[1])
            .fold(f32::MIN, f32::max)
    };
    assert_eq!(height(3.0), 1.0);
    assert_eq!(height(9.0), 3.0);
}

#[test]
fn a_column_is_round_within_its_box() {
    let vertices = prop_vertices(&prop_of(Shape::Column));
    let center = Vec3::new(0.0, 0.0, 6.0);
    for v in vertices.iter().filter(|v| v.normal[1] == 0.0) {
        let p = Vec3::new(v.position[0], 0.0, v.position[2]) - center;
        let on_ellipse = (p.x / 2.0).powi(2) + (p.z / 3.0).powi(2);
        assert!((on_ellipse - 1.0).abs() < 1e-3, "{p}");
    }
}

#[test]
fn hidden_barriers_draw_nothing() {
    assert!(prop_vertices(&prop_of(Shape::Hidden)).is_empty());
}

#[test]
fn a_door_leaf_knows_how_it_slides() {
    let leaf = Prop {
        slide: DVec3::new(1.5, 0.0, 0.0),
        door: DVec3::new(0.0, -2.0, 40.0),
        ..prop_of(Shape::Block)
    };
    for v in prop_vertices(&leaf) {
        assert_eq!(v.slide, [1.5, 0.0, 0.0]);
        assert_eq!(v.door, [0.0, -2.0, 40.0]);
    }
    assert!(prop_vertices(&prop_of(Shape::Block)).iter().all(|v| v.slide == [0.0; 3]));
}

#[test]
fn the_furniture_is_all_in_the_mesh() {
    let layout = ShipLayout::cruiser();
    let drawn = layout.props.iter().filter(|p| p.shape != Shape::Hidden).count();
    assert!(furniture_vertices(&layout).len() >= drawn * 12);
}

#[test]
fn glowing_parts_keep_their_material_for_the_shader() {
    let vertices = hull_vertices(&ShipLayout::cruiser());
    for material in [Material::Engine, Material::Light, Material::Glass] {
        assert!(vertices.iter().any(|v| v.material == material as u32), "{material:?} lost");
    }
}

#[test]
fn every_turret_has_a_model_that_knows_which_it_is() {
    let layout = ShipLayout::cruiser();
    let vertices = turret_vertices(&layout);
    assert!(!vertices.is_empty());
    assert_eq!(vertices.len() % 3, 0);
    assert!(vertices.iter().all(|v| v.material == Material::Turret as u32));
    for index in 0..8 {
        let mount = layout.hardpoints[index].position.as_vec3();
        let own: Vec<&HullVertex> =
            vertices.iter().filter(|v| v.slide[0] == index as f32 + 1.0).collect();
        assert!(!own.is_empty(), "turret {index} has no model");
        assert!(own.iter().all(|v| v.door == mount.to_array()));
        assert!(own.iter().any(|v| v.slide[1] == 1.0), "turret {index} has no barrels");
        assert!(own.iter().all(|v| position(v).distance(mount) < 7.0));
    }
}

#[test]
fn turret_barrels_point_along_the_nose_at_rest() {
    let layout = ShipLayout::cruiser();
    let vertices = turret_vertices(&layout);
    let mount = layout.hardpoints[0].position.as_vec3();
    let reach = vertices
        .iter()
        .filter(|v| v.slide[0] == 1.0 && v.slide[1] == 1.0)
        .map(|v| v.position[2] - mount.z)
        .fold(f32::MIN, f32::max);
    assert!(reach > 3.0, "{reach}");
}

fn bolt(width: f32, flash: f32) -> Bolt {
    Bolt {
        from: DVec3::new(0.0, 0.0, 10.0),
        to: DVec3::new(0.0, 0.0, 50.0),
        width,
        flash,
        color: [0.2, 1.0, 0.7, 1.0],
    }
}

#[test]
fn a_bolt_is_one_quad_and_a_flash_another() {
    assert_eq!(bolt_vertices(&[bolt(0.5, 0.0)], DVec3::new(5.0, 0.0, 0.0)).len(), 6);
    assert_eq!(bolt_vertices(&[bolt(0.0, 2.0)], DVec3::new(5.0, 0.0, 0.0)).len(), 6);
    assert_eq!(bolt_vertices(&[bolt(0.5, 2.0)], DVec3::new(5.0, 0.0, 0.0)).len(), 12);
}

#[test]
fn a_bolt_keeps_its_width_turned_to_the_camera() {
    let camera = DVec3::new(5.0, 0.0, 0.0);
    let vertices = bolt_vertices(&[bolt(0.5, 0.0)], camera);
    let ys: Vec<f32> = vertices.iter().map(|v| v.position[1]).collect();
    let (low, high) = (ys.iter().cloned().fold(f32::MAX, f32::min), ys.iter().cloned().fold(f32::MIN, f32::max));
    assert!((high - low - 1.0).abs() < 1e-5, "{low} {high}");
    assert!(vertices.iter().all(|v| (v.position[0] + 5.0).abs() < 1e-4), "stays in the bolt's plane");
}

#[test]
fn a_far_bolt_stays_wide_enough_to_see() {
    let camera = DVec3::new(400.0, 0.0, 30.0);
    let vertices = bolt_vertices(&[bolt(0.01, 0.0)], camera);
    let ys: Vec<f32> = vertices.iter().map(|v| v.position[1]).collect();
    let high = ys.iter().cloned().fold(f32::MIN, f32::max);
    assert!(high > 400.0 * 0.0012, "{high}");
}

#[test]
fn bolts_are_placed_from_the_camera_so_far_worlds_stay_exact() {
    let camera = DVec3::new(30_000_000.25, 90.0, -30_000_000.5);
    let shot = Bolt {
        from: camera + DVec3::new(1.0, 0.0, 2.0),
        to: camera + DVec3::new(1.0, 0.0, 12.0),
        width: 0.25,
        flash: 0.0,
        color: [1.0; 4],
    };
    let vertices = bolt_vertices(&[shot], camera);
    assert!(vertices.iter().all(|v| (v.position[2] - 2.0).abs() < 1e-5 || (v.position[2] - 12.0).abs() < 1e-5));
}

#[test]
fn too_many_bolts_are_cut_off_not_a_crash() {
    assert!(bolt_vertices(&vec![bolt(0.5, 2.0); 5000], DVec3::ZERO).len() <= astral_hull::BOLT_VERTICES);
}

fn stern() -> (DVec3, glam::DQuat) {
    (DVec3::new(100.0, 70.0, -50.0), glam::DQuat::from_rotation_y(0.6))
}

#[test]
fn the_warp_rings_stand_behind_the_stern() {
    let (at, rotation) = stern();
    let aft = rotation * DVec3::NEG_Z;
    for bolt in warp_bolts(at, rotation, 1.0, 1.0) {
        assert!((bolt.from - at).dot(aft) > 0.0, "{}", bolt.from);
    }
}

#[test]
fn the_warp_rings_are_many_parts_and_all_purple_light() {
    let (at, rotation) = stern();
    let parts = warp_bolts(at, rotation, 1.0, 1.0);
    assert!(parts.len() > 60, "{}", parts.len());
    for bolt in &parts {
        let [r, g, b, _] = bolt.color;
        assert!(b > g && r > g, "{r} {g} {b}");
    }
}

#[test]
fn the_warp_rings_turn() {
    let (at, rotation) = stern();
    let (first, later) = (warp_bolts(at, rotation, 1.0, 1.0), warp_bolts(at, rotation, 1.5, 1.0));
    assert_ne!(first[0].from, later[0].from);
}

#[test]
fn the_warp_rings_grow_with_the_charge() {
    let (at, rotation) = stern();
    let reach = |power: f32| {
        warp_bolts(at, rotation, 1.0, power)
            .iter()
            .map(|bolt| ((bolt.from - at) - rotation * DVec3::NEG_Z * (bolt.from - at).dot(rotation * DVec3::NEG_Z)).length())
            .fold(0.0, f64::max)
    };
    assert!(reach(1.0) > reach(0.2) * 1.5, "{} {}", reach(1.0), reach(0.2));
}

#[test]
fn no_charge_shows_nothing() {
    let (at, rotation) = stern();
    assert!(warp_bolts(at, rotation, 1.0, 0.0).is_empty());
}

#[test]
fn the_warp_fits_what_a_frame_can_draw() {
    let (at, rotation) = stern();
    let parts = warp_bolts(at, rotation, 1.0, 1.0);
    assert!(bolt_vertices(&parts, DVec3::ZERO).len() < BOLT_VERTICES / 2);
}

#[test]
fn the_arrival_ring_spreads_and_fades() {
    let (at, rotation) = stern();
    let (young, old) = (arrival_ring(at, rotation, 0.1), arrival_ring(at, rotation, 0.8));
    assert!(young[0].from.distance(at) < old[0].from.distance(at));
    assert!(young[0].color[3] > old[0].color[3]);
    assert!(arrival_ring(at, rotation, 1.0).is_empty());
}

#[test]
fn a_swirl_is_one_quad_from_the_camera() {
    let camera = DVec3::new(90.0, 70.0, -60.0);
    let swirl = Swirl { middle: DVec3::new(100.0, 70.0, -50.0), rotation: glam::DQuat::IDENTITY, radius: 40.0, spin: 0.3, power: 1.0 };
    let vertices = swirl_vertices(&swirl, camera);
    assert_eq!(vertices.len(), 6);
    for vertex in &vertices {
        let at = DVec3::new(f64::from(vertex.position[0]), f64::from(vertex.position[1]), f64::from(vertex.position[2]));
        assert!((at + camera).distance(swirl.middle) < 60.0, "{at}");
    }
}

#[test]
fn a_swirl_with_no_power_draws_nothing() {
    let swirl = Swirl { middle: DVec3::ZERO, rotation: glam::DQuat::IDENTITY, radius: 40.0, spin: 0.3, power: 0.0 };
    assert!(swirl_vertices(&swirl, DVec3::ZERO).is_empty());
}
