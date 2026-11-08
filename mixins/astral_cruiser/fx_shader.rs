// Six strike primitives, built from a single 64-byte record and six vertex IDs.
pub const SHADER: &str = r#"
struct Uniform {
    view_proj: mat4x4<f32>,
    anchor_minus_camera: vec3<f32>,
    now: f32,
    camera_right: vec3<f32>,
    pad_right: f32,
    camera_up: vec3<f32>,
    pad_up: f32,
    light_color: vec4<f32>,
}
@group(0) @binding(0) var<uniform> fx: Uniform;

struct Instance {
    @location(0) pos: vec3<f32>,
    @location(1) size: f32,
    @location(2) vec: vec3<f32>,
    @location(3) born: f32,
    @location(4) color: vec4<f32>,
    @location(5) kind: u32,
    @location(6) seed: u32,
    @location(7) life: f32,
    @location(8) grow: f32,
}
struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
    // x: seconds alive, y: fraction of life remaining.
    @location(2) @interpolate(flat) age: vec2<f32>,
    @location(3) @interpolate(flat) seed: u32,
    @location(4) @interpolate(flat) radius: f32,
}

fn unit(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let d = dot(v, v);
    if d < 0.000001 { return fallback; }
    return v * inverseSqrt(d);
}

// A stable perpendicular even when a beam points straight at the eye.
fn perpendicular(axis: vec3<f32>) -> vec3<f32> {
    let helper = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(axis.y) > 0.9);
    return normalize(cross(axis, helper));
}

// Matches fx_instance::position_at, including its clamped effects clock.
fn position_at(i: Instance, t: f32) -> vec3<f32> {
    if i.kind == 4u {
        return i.pos + i.vec * t + vec3<f32>(0.0, -4.9 * t * t, 0.0);
    }
    if i.kind == 3u {
        let drag = (1.0 - exp(-1.5 * t)) / 1.5;
        return i.pos + i.vec * drag + vec3<f32>(0.0, 1.2 * t, 0.0);
    }
    return i.pos;
}

@vertex fn vs_main(i: Instance, @builtin(vertex_index) vertex: u32) -> Out {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let uv = corners[vertex];
    let age = fx.now - i.born;
    let t = clamp(age, 0.0, max(i.life, 0.0));
    var out: Out;
    out.uv = uv;
    out.color = i.color;
    out.age = vec2<f32>(t, clamp(1.0 - t / max(i.life, 0.000001), 0.0, 1.0));
    out.seed = i.seed;
    out.radius = 0.0;
    // All six vertices coincide, including for zero/negative lifetimes.
    out.clip = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if age < 0.0 || age > i.life || i.life <= 0.0 { return out; }

    let p = position_at(i, t) + fx.anchor_minus_camera;
    var radius = max(i.size + i.grow * t, 0.0);
    if i.kind == 2u || i.kind == 3u {
        let progress = 1.0 - out.age.y;
        let ease_out = 1.0 - pow(1.0 - progress, 3.0);
        radius = max(i.size * (0.35 + 0.65 * ease_out) + i.grow * min(t, 1.5), 0.0);
    }
    out.radius = radius;
    var at = p + (fx.camera_right * uv.x + fx.camera_up * uv.y) * radius;
    if i.kind == 0u {
        // Beam's vec is an endpoint from the SAME anchor as pos, not velocity.
        let end = i.vec + fx.anchor_minus_camera;
        let axis = unit(i.vec - i.pos, fx.camera_up);
        let middle = (p + end) * 0.5;
        let side = unit(cross(axis, -middle), perpendicular(axis));
        at = mix(p, end, uv.x * 0.5 + 0.5) + side * (uv.y * i.size * 0.5);
    } else if i.kind == 4u {
        let velocity = i.vec + vec3<f32>(0.0, -9.8 * t, 0.0);
        let axis = unit(velocity, fx.camera_right);
        let side = unit(cross(axis, -p), perpendicular(axis));
        let streak = max(length(velocity) * 0.04, 0.3);
        at = p + axis * (uv.x * streak * 0.5) + side * (uv.y * i.size * 0.5);
    } else if i.kind == 5u {
        // Shell stays in its own world plane as the camera turns.
        let axis = unit(i.vec, vec3<f32>(0.0, 1.0, 0.0));
        let right = perpendicular(axis);
        let up = cross(axis, right);
        at = p + (right * uv.x + up * uv.y) * radius;
    }
    out.clip = fx.view_proj * vec4<f32>(at, 1.0);
    return out;
}

fn hash(cell: vec3<i32>, seed: u32) -> f32 {
    let p = vec3<u32>(cell);
    var h = (p.x * 1597334677u) ^ (p.y * 3812015801u) ^ (p.z * 2798796415u) ^ seed;
    h = (h ^ (h >> 16u)) * 2246822519u;
    h = (h ^ (h >> 13u)) * 3266489917u;
    return f32(h ^ (h >> 16u)) / 4294967295.0;
}

fn noise(p: vec3<f32>, seed: u32) -> f32 {
    let cell = vec3<i32>(floor(p));
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let z0 = mix(
        mix(hash(cell, seed), hash(cell + vec3<i32>(1, 0, 0), seed), u.x),
        mix(hash(cell + vec3<i32>(0, 1, 0), seed), hash(cell + vec3<i32>(1, 1, 0), seed), u.x), u.y);
    let z1 = mix(
        mix(hash(cell + vec3<i32>(0, 0, 1), seed), hash(cell + vec3<i32>(1, 0, 1), seed), u.x),
        mix(hash(cell + vec3<i32>(0, 1, 1), seed), hash(cell + vec3<i32>(1, 1, 1), seed), u.x), u.y);
    return mix(z0, z1, u.z);
}

fn fire_noise(p: vec3<f32>, seed: u32) -> f32 {
    // Four octaves, normalized so temperature/density stay in a useful range.
    return noise(p, seed) * (8.0 / 15.0)
        + noise(p * 2.03, seed) * (4.0 / 15.0)
        + noise(p * 4.07, seed) * (2.0 / 15.0)
        + noise(p * 8.13, seed) * (1.0 / 15.0);
}

fn temperature(heat: f32, color: vec3<f32>, coldness: f32) -> vec3<f32> {
    let smoke = vec3<f32>(0.075, 0.065, 0.06);
    let red = vec3<f32>(0.35, 0.012, 0.003);
    let orange = vec3<f32>(1.0, 0.16, 0.008);
    let yellow = vec3<f32>(1.0, 0.85, 0.10);
    let white = vec3<f32>(1.0, 1.0, 1.0);
    var fire = white;
    if heat < 0.18 {
        fire = mix(smoke, red, heat / 0.18);
    } else if heat < 0.35 {
        fire = mix(red, orange, (heat - 0.18) / 0.17);
    } else if heat < 0.7 {
        fire = mix(orange, yellow, (heat - 0.35) / 0.35);
    } else {
        fire = mix(yellow, white, (heat - 0.7) / 0.3);
    }

    let dark = color * color * 0.4;
    var energy = white;
    if heat < 0.28 {
        energy = mix(vec3<f32>(0.0, 0.0, 0.0), dark, heat / 0.28);
    } else if heat < 0.7 {
        energy = mix(dark, color, (heat - 0.28) / 0.42);
    } else {
        energy = mix(color, white, (heat - 0.7) / 0.3);
    }

    return mix(fire, energy, coldness);
}

@fragment fn fs_glow(in: Out) -> @location(0) vec4<f32> {
    let d = dot(in.uv, in.uv);
    let body = max(exp(-d * 6.0) - exp(-6.0), 0.0);
    let intensity = 1.0 / (1.0 + in.radius / 40.0);
    return vec4<f32>(in.color.rgb * body * intensity * in.color.a * in.age.y * in.age.y, 0.0);
}

@fragment fn fs_fireball(in: Out) -> @location(0) vec4<f32> {
    // Ray from the billboard through a unit sphere, in the billboard's basis.
    let origin = vec3<f32>(in.uv, 2.0);
    let ray = vec3<f32>(0.0, 0.0, -1.0);
    let b = dot(origin, ray);
    let c = dot(origin, origin) - 1.0;
    let discriminant = b * b - c;
    if discriminant <= 0.0 { discard; }
    let hit = origin + ray * (-b - sqrt(discriminant));
    let seed_phase = f32(in.seed & 1023u) * 0.013;
    let t = in.age.x * (0.7 + seed_phase * 0.04);
    let p = hit * 3.8 + vec3<f32>(seed_phase + t * 0.4, -t * 1.2, t * 0.6);
    let warp = vec3<f32>(noise(p + 11.0, in.seed), noise(p + 37.0, in.seed),
        noise(p + 73.0, in.seed)) - 0.5;
    let n = fire_noise(p + warp * 2.4, in.seed);
    let r = length(in.uv);
    let edge = 1.0 - smoothstep(0.68, 1.0, r + (n - 0.5) * 0.18);
    let heat = clamp(in.age.y * (1.12 + (n - 0.5) * 1.6)
        - r * r * 0.42 - pow(r, 4.0) * 0.65, 0.0, 1.0);
    let coldness = clamp(max((in.color.b - in.color.r) * 2.0 + (in.color.b - in.color.g),
        (in.color.b - in.color.r) * 20.0), 0.0, 1.0);
    let thermal = temperature(heat, in.color.rgb, coldness) * (0.55 + n * 0.9);
    let fire_emission = mix(thermal, thermal * in.color.rgb, 0.35);
    let emission = mix(fire_emission, thermal, coldness);
    let cold_fade = mix(1.0, smoothstep(0.0, 0.22, heat), coldness);
    let alpha = clamp(in.color.a * 0.95 * (0.82 + n * 0.3), 0.0, 0.94)
        * edge * smoothstep(0.0, 0.15, in.age.y) * cold_fade;
    return vec4<f32>(clamp(emission, vec3<f32>(0.0), vec3<f32>(1.2)) * alpha, alpha);
}

fn smoke_fbm(p: vec3<f32>, seed: u32) -> f32 {
    return noise(p, seed) * (4.0 / 7.0)
        + noise(p * 2.08, seed) * (2.0 / 7.0)
        + noise(p * 4.23, seed) * (1.0 / 7.0);
}

@fragment fn fs_smoke(in: Out) -> @location(0) vec4<f32> {
    let r = length(in.uv);
    if r >= 1.0 { discard; }

    let seed_phase = f32(in.seed & 1023u) * 0.007;
    let drift = vec3<f32>(in.age.x * 0.08, in.age.x * 0.22, in.age.x * 0.14);
    let p = vec3<f32>(in.uv * 2.6, seed_phase) + drift;
    let n = smoke_fbm(p, in.seed);

    // Billowy edges with smoothstep thresholding.
    let billow_edge = 1.0 - smoothstep(0.28, 0.95, r + (n - 0.5) * 0.32);
    if billow_edge <= 0.0 { discard; }

    // Internal structure and life fading.
    // in.age.y is fraction of life remaining (1.0 - t / life).
    let life_remaining = in.age.y;
    let life_fade = pow(life_remaining, 1.5);
    let birth_fade = smoothstep(0.0, 0.08, 1.0 - life_remaining);
    let structure = 0.7 + n * 0.5;
    let center_alpha = min(in.color.a, 0.45);
    let alpha = min(center_alpha * billow_edge * structure * life_fade * birth_fade, 0.45);

    // World-down on a billboard, also correct when the camera looks up or down.
    let height = (fx.camera_right * in.uv.x + fx.camera_up * in.uv.y).y;
    let below = clamp(0.5 - height * 0.5, 0.0, 1.0);

    // Lighter base: never output rgb below 0.12 * light (grey-brown tint).
    let grey_brown_floor = vec3<f32>(0.13, 0.115, 0.10);
    let base = max(mix(grey_brown_floor, in.color.rgb * 0.35, 0.3), grey_brown_floor);

    // Lit from below by the uniform light colour for the first third of life.
    let first_third = smoothstep(0.3333, 0.0, 1.0 - life_remaining);
    let bottom_light = min(fx.light_color.rgb, vec3<f32>(0.8)) * (below * below * 0.5) * first_third;

    let rgb = max(base * (0.65 + n * 0.6) + bottom_light, grey_brown_floor);
    return vec4<f32>(rgb * alpha, alpha);
}

// The bolt's exponential body and white core, with UV.x along the band.
fn band(in: Out) -> vec4<f32> {
    let d = in.uv.y * in.uv.y;
    let body = max(exp(-d * 5.0) - exp(-5.0), 0.0);
    let core = exp(-d * 45.0);
    return vec4<f32>((in.color.rgb * body + vec3<f32>(core)) * in.color.a * in.age.y, 0.0);
}

@fragment fn fs_spark(in: Out) -> @location(0) vec4<f32> {
    return band(in) * (1.0 - smoothstep(0.7, 1.0, abs(in.uv.x)));
}

@fragment fn fs_beam(in: Out) -> @location(0) vec4<f32> {
    return band(in);
}

@fragment fn fs_shell(in: Out) -> @location(0) vec4<f32> {
    let radius = length(in.uv);
    if radius < 0.88 || radius > 1.0 { discard; }
    let ring = smoothstep(0.88, 0.94, radius) * (1.0 - smoothstep(0.94, 1.0, radius));
    return vec4<f32>(in.color.rgb * ring * in.color.a * in.age.y * in.age.y, 0.0);
}
"#;
