// Rain splashes on the ground: none in the dry under the hull.
fn astral_splash_height(value: Option<i32>, x: i32, z: i32) -> Option<i32> {
    crate::game::astral_cruiser::splash_height(value, x, z)
}
