// Detail makes whole decorated chunks in batches, gigabytes on a long flight,
// and no one sees it from up there.
fn astral_far_detail() -> std::ops::ControlFlow<()> {
    if crate::gfx::astral_ship::far_lead().is_some() {
        std::ops::ControlFlow::Break(())
    } else {
        std::ops::ControlFlow::Continue(())
    }
}
