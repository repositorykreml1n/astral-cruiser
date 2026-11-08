extern crate self as horizon_client;
#[path = "../mixins/astral_cruiser/lang.rs"]
pub mod astral_lang;
#[path = "../mixins/astral_cruiser/arsenal.rs"]
pub mod astral_arsenal;
#[path = "../mixins/astral_cruiser/guns.rs"]
pub mod astral_guns;
#[path = "../mixins/astral_cruiser/layout.rs"]
pub mod astral_layout;
#[path = "../mixins/astral_cruiser/furnish.rs"]
pub mod astral_furnish;
#[path = "../mixins/astral_cruiser/hull_mesh.rs"]
pub mod astral_hull;
pub mod gfx { pub use crate::{astral_hull, astral_layout}; }
#[path = "../mixins/astral_cruiser/gun_mesh.rs"]
mod gun_mesh;
use glam::{DQuat, DVec3};

#[test]
fn deployed_mesh_fits_buffer_and_tracks_muzzle() {
    let mut guns = astral_guns::Guns::default();
    for gun in &mut guns.0 { gun.tick(2.0, None, true); }
    let vertices = gun_mesh::mesh(&guns);
    assert!(vertices.len() < 8190);
    assert!(vertices.iter().all(|v| v.position.iter().all(|x| x.is_finite())));
    for gun in &guns.0 {
        let p = gun.pose();
        assert!(vertices.iter().any(|v| DVec3::from_array(v.position.map(f64::from)).distance(p.muzzle) < gun.battery.width() + 0.5));
    }
    let max_x = vertices.iter().map(|v| v.position[0]).fold(f32::NEG_INFINITY, f32::max);
    let min_x = vertices.iter().map(|v| v.position[0]).fold(f32::INFINITY, f32::min);
    assert!((max_x + min_x).abs() < 1e-5);
}

#[test]
fn cube_budget_and_camera_relative_positions() {
    let cube = astral_hull::LightCube { center: DVec3::splat(1000.0), rotation: DQuat::IDENTITY, half: 0.5, color: [1.0; 4] };
    let mesh = astral_hull::cube_vertices(&vec![cube; 5000], DVec3::splat(1000.0));
    assert_eq!(mesh.len(), astral_hull::CUBE_VERTICES);
    assert!(mesh.iter().all(|v| v.position.iter().all(|x| x.abs() == 0.5)));
}
