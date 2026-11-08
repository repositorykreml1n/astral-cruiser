// dialog_click is closed to injections (it holds a macro). While the tablet is
// open every other caller of dialog_layout is cancelled before it gets there,
// so a call here is a click: queue it for the tablet and give the dialog nothing.
fn astral_tablet_layout(game: &crate::game::Game<'_>) -> std::ops::ControlFlow<Option<Layout>> {
    if crate::game::astral_cruiser::wheel_active(game) {
        crate::game::astral_cruiser::click_wheel(game);
        return std::ops::ControlFlow::Break(None);
    }
    if crate::game::astral_tablet_draw::queue_click(game) {
        std::ops::ControlFlow::Break(None)
    } else {
        std::ops::ControlFlow::Continue(())
    }
}
