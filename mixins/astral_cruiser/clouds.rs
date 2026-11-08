// Camera altitude steering to keep the follow view out of dense cloud layers.

/// Pushes the camera out of the cloud band toward whichever side the ship is on.
pub fn out_of_clouds(camera_y: f64, ship_y: f64, bottom: f64, top: f64, margin: f64) -> f64 {
    let low = bottom - margin;
    let high = top + margin;
    if camera_y >= low && camera_y <= high {
        if ship_y >= (bottom + top) / 2.0 {
            high
        } else {
            low
        }
    } else {
        camera_y
    }
}

/// Steps current toward target by at most step without overshooting.
pub fn ease_to(current: f64, target: f64, step: f64) -> f64 {
    let diff = target - current;
    let step = step.abs();
    if diff.abs() <= step {
        target
    } else if diff > 0.0 {
        current + step
    } else {
        current - step
    }
}
