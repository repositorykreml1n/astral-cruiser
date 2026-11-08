fn astral_tablet_drag(game: &mut crate::game::Game<'_>) -> std::ops::ControlFlow<()> {
    if crate::game::astral_cruiser::wheel_active(game) {
        return std::ops::ControlFlow::Break(());
    }
    crate::game::astral_tablet_draw::drag(game)
}
