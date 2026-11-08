fn astral_main_frame(renderer: &mut crate::gfx::Renderer, frame: &crate::gfx::Frame<'_>) {
    // The far side first: its nested render arms the ship for itself.
    crate::gfx::astral_portal::before_main(renderer, frame);
    // Effects have their own GPU and exist even without a ship view or hull.
    crate::gfx::astral_fx_gpu::prepare(renderer, frame);
    crate::gfx::astral_ship::before_main(renderer, frame);
}
