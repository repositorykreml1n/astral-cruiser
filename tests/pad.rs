#[path = "../mixins/astral_cruiser/pad.rs"]
mod astral_pad;

use astral_pad::{pad_box, pad_for, pad_stays, Pad, PAD_HALF, PROBE};
use glam::DVec3;

#[test]
fn pad_appears_over_the_void() {
    assert!(pad_for(DVec3::new(0.5, 60., 0.5), |_, _, _| false).is_some());
}

#[test]
fn no_pad_over_ground() {
    assert!(pad_for(DVec3::new(0.5, 60., 0.5), |_, y, _| y == 50).is_none());
}

#[test]
fn ground_deeper_than_the_probe_still_gets_a_pad() {
    assert!(pad_for(DVec3::new(0.5, 60., 0.5), |_, y, _| y == 60 - PROBE - 2).is_some());
}

#[test]
fn pad_box_is_nine_by_nine_under_the_feet() {
    let b = pad_box(&Pad {
        center: DVec3::new(0.5, 60., 0.5),
        half: PAD_HALF,
    });
    assert_eq!(b, ([-4.0, 59.5, -4.0], [5.0, 60.0, 5.0]));
}

#[test]
fn pad_goes_only_when_portal_shut_and_nobody_on_it() {
    assert!(pad_stays(true, false));
    assert!(pad_stays(false, true));
    assert!(!pad_stays(false, false));
}
