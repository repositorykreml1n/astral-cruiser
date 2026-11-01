fn astral_main_frame(renderer: &mut crate::gfx::Renderer, frame: &crate::gfx::Frame<'_>) {
    // The far side first: its nested render arms the ship for itself.
    crate::gfx::astral_portal::before_main(renderer, frame);
    crate::gfx::astral_ship::before_main(renderer, frame);
}
