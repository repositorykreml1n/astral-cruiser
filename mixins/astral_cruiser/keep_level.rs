// The server is about to choose the levels it ticks.
fn astral_keep_level(game: &crate::game::Game<'_>) -> std::ops::ControlFlow<Vec<Dim>> {
    crate::game::astral_cruiser::levels_to_tick(game)
}
