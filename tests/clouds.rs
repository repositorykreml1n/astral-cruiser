#[path = "../mixins/astral_cruiser/clouds.rs"]
mod astral_clouds;

use astral_clouds::{ease_to, out_of_clouds};

#[test]
fn camera_in_clouds_moves_above_when_ship_is_above() {
    assert_eq!(out_of_clouds(193., 200., 192., 196., 2.), 198.);
}

#[test]
fn camera_in_clouds_moves_below_when_ship_is_below() {
    assert_eq!(out_of_clouds(193., 150., 192., 196., 2.), 190.);
}

#[test]
fn camera_outside_clouds_stays() {
    assert_eq!(out_of_clouds(100., 150., 192., 196., 2.), 100.);
}

#[test]
fn ease_to_never_overshoots() {
    assert_eq!(ease_to(0., 1., 5.), 1.);
    assert_eq!(ease_to(0., 10., 2.), 2.);
}
