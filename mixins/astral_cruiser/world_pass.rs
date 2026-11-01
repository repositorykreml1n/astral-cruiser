fn astral_world_pass(pass: &mut wgpu::RenderPass<'_>) {
    crate::gfx::astral_portal::draw(pass);
    crate::gfx::astral_ship::draw(pass);
    crate::gfx::astral_portal::draw_veil(pass);
    crate::gfx::astral_ship::draw_light(pass);
}
