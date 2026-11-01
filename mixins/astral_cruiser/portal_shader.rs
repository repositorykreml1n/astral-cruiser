// The portal's shader. The opening shows the far side's image by screen position,
// so what is seen through it is what a camera there sees; the rim is light. The
// veil is the same image over the whole screen, for the moments when the eye has
// gone through and the game's own scene is still arriving.
pub const SHADER: &str = r#"
struct Uniform {
    view_proj: mat4x4<f32>,
    corners: array<vec4<f32>, 4>,
    // xy: the screen's size. z: seconds. w: how far the portal has opened, 0 to 1.
    size: vec4<f32>,
    // x: the veil's share over the scene. y: whether the far side's image is ready.
    state: vec4<f32>,
}
@group(0) @binding(0) var<uniform> u: Uniform;
@group(0) @binding(1) var view_texture: texture_2d<f32>;
@group(0) @binding(2) var view_sampler: sampler;

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) quad: vec2<f32>,
}

@vertex fn vs(@builtin(vertex_index) index: u32) -> Out {
    var order = array<u32, 6>(0u, 1u, 2u, 2u, 1u, 3u);
    var quads = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, 1.0));
    let corner = order[index];
    var out: Out;
    out.position = u.view_proj * vec4<f32>(u.corners[corner].xyz, 1.0);
    out.quad = quads[corner];
    return out;
}

fn image(position: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(view_texture, view_sampler, position / u.size.xy, 0.0).rgb;
}

// Light running round the rim, so that it is plainly alive.
fn rim(angle: f32, seconds: f32) -> f32 {
    let waves = sin(angle * 5.0 + seconds * 2.3) + sin(angle * 11.0 - seconds * 3.7) * 0.5;
    return 0.75 + 0.25 * waves;
}

@fragment fn fs(in: Out) -> @location(0) vec4<f32> {
    let opened = u.size.w;
    let reach = length(in.quad) / max(opened, 0.001);
    if reach > 1.0 {
        discard;
    }
    let edge = 1.0 - reach;
    let width = fwidth(reach);
    let band = 0.035 + width * 2.0;
    let angle = atan2(in.quad.y, in.quad.x);
    let glow = vec3<f32>(0.25, 0.9, 1.0) * rim(angle, u.size.z);
    var seen = vec3<f32>(0.01, 0.03, 0.05) + glow * 0.04 * rim(angle * 3.0, u.size.z * 0.5);
    if u.state.y > 0.5 {
        seen = image(in.position.xy);
    }
    // A thin bright line, and a soft halo over the view inside it.
    let line = 1.0 - smoothstep(band * 0.3, band, edge);
    let halo = (1.0 - smoothstep(band, band * 5.0, edge)) * 0.35;
    return vec4<f32>(seen + glow * halo + glow * line, 1.0);
}

@vertex fn vs_veil(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return vec4<f32>(corners[index], 0.0, 1.0);
}

@fragment fn fs_veil(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(image(position.xy), u.state.x);
}
"#;
