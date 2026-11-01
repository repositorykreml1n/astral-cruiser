fn astral_wheel(game: &mut crate::game::Game<'_>, steps: f32, claimed: &mut bool) {
    crate::game::astral_cruiser::wheel(game, steps, claimed);
}
