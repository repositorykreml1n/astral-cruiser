// The frame's view is about to be worked out.
fn astral_camera(game: &mut crate::game::Game<'_>, partial: f32) {
    crate::game::astral_cruiser::camera(game, partial);
}
