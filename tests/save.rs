#[path = "../mixins/astral_cruiser/core.rs"]
mod astral_core;
#[path = "../mixins/astral_cruiser/save.rs"]
mod astral_save;

use astral_core::{Dimension, ShipPose};
use astral_save::{SaveError, SaveState, Speeds};
use glam::{DQuat, DVec3};
use std::{fs, path::PathBuf};

fn state() -> SaveState {
    SaveState {
        dimension: Dimension::Nether,
        pose: ShipPose::new(DVec3::new(1.5, 80.0, -2.25), DQuat::from_rotation_y(0.5)),
        aboard: false,
        energy: 42.5,
        heat: 0.25,
        speeds: Speeds::default(),
    }
}

/// JSON may move a float's last digit; a pose only has to come back in place.
fn assert_same(loaded: Option<SaveState>, saved: SaveState) {
    let loaded = loaded.expect("a saved ship");
    assert_eq!(loaded.dimension, saved.dimension);
    assert_eq!(loaded.aboard, saved.aboard);
    assert_eq!((loaded.energy, loaded.heat), (saved.energy, saved.heat));
    assert!((loaded.pose.position - saved.pose.position).length() < 1e-9);
    assert!((loaded.pose.rotation.dot(saved.pose.rotation).abs() - 1.0).abs() < 1e-9);
}

/// A folder of its own under the target directory, empty for each test.
fn folder(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn a_saved_ship_loads_back_the_same() {
    let path = folder("round_trip").join("ship.json");
    state().save_atomic(&path).unwrap();
    assert_same(SaveState::load(&path).unwrap(), state());
}

#[test]
fn a_player_left_aboard_is_remembered() {
    let path = folder("aboard").join("ship.json");
    let fast = SaveState { speeds: Speeds::limited(128.0, 2000.0), ..state() };
    assert_eq!(SaveState::from_json(&fast.to_json()).unwrap().speeds, fast.speeds);
    let aboard = SaveState { aboard: true, ..state() };
    aboard.save_atomic(&path).unwrap();
    assert_same(SaveState::load(&path).unwrap(), aboard);
}

#[test]
fn a_save_from_before_the_crew_was_kept_has_nobody_aboard() {
    let text = r#"{"version":1,"dimension":"end","position":[1,2,3],"rotation":[0,0,0,1]}"#;
    assert!(!SaveState::from_json(text).unwrap().aboard);
}

#[test]
fn a_save_from_before_the_guns_has_a_full_cold_battery() {
    let text = r#"{"version":1,"dimension":"end","position":[1,2,3],"rotation":[0,0,0,1]}"#;
    let loaded = SaveState::from_json(text).unwrap();
    assert_eq!((loaded.energy, loaded.heat), (100.0, 0.0));
}

#[test]
fn a_world_without_a_ship_loads_none() {
    let path = folder("missing").join("ship.json");
    assert_eq!(SaveState::load(&path).unwrap(), None);
}

#[test]
fn a_later_format_is_refused() {
    let text = r#"{"version":99,"dimension":"overworld","position":[0,0,0],"rotation":[0,0,0,1]}"#;
    assert_eq!(SaveState::from_json(text), Err(SaveError::UnsupportedVersion(99)));
}

#[test]
fn a_truncated_file_is_an_error_not_a_lost_ship() {
    let text = r#"{"version":1,"dimension":"overworld","posi"#;
    assert!(matches!(SaveState::from_json(text), Err(SaveError::Malformed(_))));
}

#[test]
fn an_unknown_dimension_is_refused() {
    let text = r#"{"version":1,"dimension":"aether","position":[0,0,0],"rotation":[0,0,0,1]}"#;
    assert!(matches!(SaveState::from_json(text), Err(SaveError::Malformed(_))));
}

#[test]
fn a_zero_rotation_loads_as_a_valid_pose() {
    let text = r#"{"version":1,"dimension":"end","position":[1,2,3],"rotation":[0,0,0,0]}"#;
    let loaded = SaveState::from_json(text).unwrap();
    assert_eq!(loaded.dimension, Dimension::End);
    assert_eq!(loaded.pose.rotation, DQuat::IDENTITY);
}

#[test]
fn a_leftover_temporary_file_does_not_disturb_the_save() {
    let dir = folder("leftover");
    let path = dir.join("ship.json");
    state().save_atomic(&path).unwrap();
    fs::write(dir.join("ship.json.tmp"), "half written").unwrap();
    assert_same(SaveState::load(&path).unwrap(), state());
    state().save_atomic(&path).unwrap();
    assert_same(SaveState::load(&path).unwrap(), state());
}

#[test]
fn saving_over_a_ship_replaces_it() {
    let path = folder("replace").join("ship.json");
    state().save_atomic(&path).unwrap();
    let moved = SaveState { dimension: Dimension::End, ..state() };
    moved.save_atomic(&path).unwrap();
    assert_same(SaveState::load(&path).unwrap(), moved);
}

#[test]
fn speeds_stay_within_what_the_ship_can_do() {
    let low = Speeds::limited(1.0, 1.0);
    let high = Speeds::limited(1e9, 1e9);
    assert!(low.cruise > 0.0 && low.dash > low.cruise);
    assert!(high.cruise <= 256.0 && high.dash <= 4000.0);
}

#[test]
fn speeds_that_are_no_numbers_fall_back() {
    let speeds = Speeds::limited(f64::NAN, f64::INFINITY);
    assert_eq!(speeds.cruise, Speeds::default().cruise);
    assert!(speeds.dash <= 4000.0);
}

#[test]
fn a_save_from_before_speeds_has_the_usual_ones() {
    let old = r#"{"version":1,"dimension":"overworld","position":[0,70,0],"rotation":[0,0,0,1]}"#;
    assert_eq!(SaveState::from_json(old).unwrap().speeds, Speeds::default());
}
