#[path = "../mixins/astral_cruiser/ship_shader.rs"]
mod astral_shader;

#[path = "../mixins/astral_cruiser/portal_shader.rs"]
mod astral_portal_shader;

use naga::valid::{Capabilities, ValidationFlags, Validator};

#[test]
fn the_portal_shader_compiles() {
    let module = naga::front::wgsl::parse_str(astral_portal_shader::SHADER)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(astral_portal_shader::SHADER)));
    Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .unwrap_or_else(|e| panic!("{e:?}"));
}

#[test]
fn the_ship_shader_compiles() {
    let module = naga::front::wgsl::parse_str(astral_shader::SHADER)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(astral_shader::SHADER)));
    Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .unwrap_or_else(|e| panic!("{e:?}"));
}
