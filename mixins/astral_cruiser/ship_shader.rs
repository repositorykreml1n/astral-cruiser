// The ship's shader. Every surface detail is drawn here, not read from a texture:
// the hull's plates and windows, and inside the rooms' light, screens and materials.
pub const SHADER: &str = r#"
struct Globals {
    view_proj: mat4x4<f32>,
    fog_color: vec4<f32>,
    fog: vec4<f32>,
    sky_color: vec4<f32>,
    misc: vec4<f32>,
    inverse_view: mat4x4<f32>,
    sunrise: vec4<f32>,
    sun: vec4<f32>,
    fog_ends: vec4<f32>,
    end_portal_sprites: array<vec4<f32>, 2>,
    time: vec4<f32>,
}
struct Ship {
    rotation: mat4x4<f32>,
    // xyz: the ship's origin from the camera.
    offset: vec4<f32>,
    // x: engine power 0..1, y: seconds, for the pulsing lights, z: how much fog
    // reaches the ship, 0 while the pilot looks at it from the chase camera.
    glow: vec4<f32>,
    // x: each turret's turn, y: its barrels' elevation, in radians.
    turrets: array<vec4<f32>, 8>,
    // A portal's plane from the camera: what lies past it is in the other world.
    clip: vec4<f32>,
}
@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<uniform> ship: Ship;

struct In {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) material: u32,
    @location(3) slide: vec3<f32>,
    @location(4) door: vec3<f32>,
}
struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) local: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) relative: vec3<f32>,
    @location(3) @interpolate(flat) material: u32,
}

// Elevated about X, then turned about Y, the way the ship itself turns.
fn turned(v: vec3<f32>, turn: f32, elevation: f32) -> vec3<f32> {
    let raised = vec3<f32>(
        v.x,
        v.y * cos(elevation) + v.z * sin(elevation),
        v.z * cos(elevation) - v.y * sin(elevation),
    );
    return vec3<f32>(
        raised.x * cos(turn) + raised.z * sin(turn),
        raised.y,
        raised.z * cos(turn) - raised.x * sin(turn),
    );
}

@vertex fn vs(in: In) -> Out {
    var out: Out;
    var position = in.position;
    var normal = in.normal;
    if in.material == 23u {
        let aim = ship.turrets[u32(in.slide.x) - 1u];
        let pivot = in.door + vec3<f32>(0.0, 1.1, 0.0);
        position = pivot + turned(position - pivot, aim.x, aim.y * in.slide.y);
        normal = turned(normal, aim.x, aim.y * in.slide.y);
    } else if dot(in.slide, in.slide) > 0.0 {
        // A door opens for whoever comes near it.
        let turn = mat3x3<f32>(ship.rotation[0].xyz, ship.rotation[1].xyz, ship.rotation[2].xyz);
        let eye = transpose(turn) * (-ship.offset.xyz);
        position += in.slide * (1.0 - smoothstep(3.0, 5.0, distance(eye, in.door)));
    }
    let relative = (ship.rotation * vec4<f32>(position, 1.0)).xyz + ship.offset.xyz;
    out.clip = globals.view_proj * vec4<f32>(relative, 1.0);
    out.local = position;
    out.normal = normal;
    out.relative = relative;
    out.material = in.material;
    return out;
}

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// 1 inside a panel, 0 on the seam between panels, fading out with distance
// before the seam gets thinner than a pixel.
fn seams(uv: vec2<f32>, size: f32) -> f32 {
    let w = fwidth(uv);
    let d = abs(fract(uv / size + 0.5) - 0.5) * size;
    let line = smoothstep(vec2<f32>(0.05), vec2<f32>(0.12) + w, d);
    let fade = clamp(1.0 - max(w.x, w.y) / (size * 0.3), 0.0, 1.0);
    return mix(1.0, min(line.x, line.y), fade);
}

// The two coordinates running along a face.
fn surface(local: vec3<f32>, normal: vec3<f32>) -> vec2<f32> {
    if abs(normal.y) > 0.6 { return local.xz; }
    if abs(normal.x) > 0.6 { return local.zy; }
    return local.xy;
}

fn fog_share(d: f32, start: f32, end: f32) -> f32 {
    return clamp((d - start) / max(end - start, 0.001), 0.0, 1.0);
}

fn base_color(material: u32) -> vec3<f32> {
    switch material {
        case 0u: { return vec3<f32>(0.20, 0.22, 0.26); }
        case 1u: { return vec3<f32>(0.13, 0.15, 0.19); }
        default: { return vec3<f32>(0.30, 0.33, 0.37); }
    }
}

fn lit(color: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    // Never as dark as the sky: the End and the Nether have no sun at all, and
    // the ship carries its own running lights.
    let day = clamp(1.0 - globals.misc.x, 0.5, 1.0);
    let sun = max(dot(normal, normalize(globals.sun.xyz)), 0.0);
    let sky = (normal.y * 0.5 + 0.5) * 0.55;
    return color * ((0.34 + sky + sun * 0.75) * day);
}

// Rows of small lit windows along the hull's sides; a far side keeps their
// average glow instead of shimmering.
fn windows(local: vec3<f32>) -> f32 {
    let cell = vec2<f32>(local.z / 1.5, local.y / 1.6);
    let w = fwidth(cell);
    let spot = fract(cell);
    let on = step(0.7, hash(floor(cell))) * step(0.2, spot.x) * step(spot.x, 0.8)
        * step(0.3, spot.y) * step(spot.y, 0.7);
    return mix(on, 0.09, clamp(max(w.x, w.y) * 1.6, 0.0, 1.0));
}

fn finish(color: vec3<f32>, in: Out, emission: f32) -> vec3<f32> {
    let spherical = length(in.relative);
    let cylindrical = max(length(in.relative.xz), abs(in.relative.y));
    let f = max(fog_share(spherical, globals.fog.x, globals.fog.y),
                fog_share(cylindrical, globals.fog.z, globals.fog.w));
    return mix(color, globals.fog_color.rgb, f * globals.fog_color.a * (1.0 - emission * 0.7) * ship.glow.z);
}

// The colour of the light in each part of the ship.
fn room_light(p: vec3<f32>) -> vec3<f32> {
    let side = abs(p.x) > 2.75;
    if p.z > 39.75 { return vec3<f32>(0.72, 0.88, 1.0); }
    if p.z > 23.0 && side {
        if p.x > 0.0 { return vec3<f32>(1.0, 0.84, 0.64); }
        return vec3<f32>(1.0, 0.9, 0.72);
    }
    if p.z > 1.5 && side { return vec3<f32>(1.0, 0.52, 0.38); }
    if p.z > -7.25 && side {
        if p.x > 0.0 { return vec3<f32>(1.0, 0.82, 0.5); }
        return vec3<f32>(0.9, 1.0, 1.0);
    }
    if p.z > -30.25 && p.z < -7.75 { return vec3<f32>(0.42, 1.0, 0.86); }
    if p.z < -43.0 { return vec3<f32>(1.0, 0.86, 0.62); }
    return vec3<f32>(0.78, 0.9, 1.0);
}

// The nearest of the lamps set out along the ceilings, every six blocks.
fn lamp_near(p: vec3<f32>) -> vec3<f32> {
    let spot = round(p.xz / 6.0 + 0.001) * 6.0;
    return vec3<f32>(spot.x, 1.6, spot.y);
}

fn lamplight(p: vec3<f32>, normal: vec3<f32>) -> f32 {
    let to = lamp_near(p) - p;
    let facing = dot(normalize(to), normal) * 0.6 + 0.4;
    return max(facing, 0.0) / (1.0 + dot(to, to) * 0.028);
}

// Corners and the foot of a wall sit in shadow.
fn shadow(p: vec3<f32>, normal: vec3<f32>) -> f32 {
    let height = p.y + 4.0;
    let floor_side = mix(smoothstep(0.0, 1.3, height), 1.0, step(0.5, abs(normal.y)));
    let ceiling_side = mix(1.0 - 0.45 * smoothstep(0.7, 2.0, p.y), 1.0, step(0.5, abs(normal.y)));
    return mix(0.55, 1.0, floor_side) * ceiling_side;
}

// A working display: a wall of small panels, each showing one instrument.
fn display(uv: vec2<f32>, seed: f32, tint: vec3<f32>) -> vec3<f32> {
    let t = ship.glow.y;
    let f = fract(uv);
    let pick = hash(floor(uv) + seed * 9.0);
    if min(min(f.x, f.y), min(1.0 - f.x, 1.0 - f.y)) < 0.05 {
        return tint * 0.02;
    }
    let c = (f - 0.5) * 2.0;
    // A radar: rings, a sweep and what it has found.
    if pick < 0.25 {
        let r = length(c);
        let sweep = fract(atan2(c.y, c.x) / 6.2832 + t * 0.2);
        let rings = step(0.88, fract(r * 3.0));
        let blip = step(0.97, hash(floor(f * 11.0) + pick));
        return tint * step(r, 0.95) * (0.05 + 0.16 * rings + 0.75 * pow(sweep, 7.0) + blip * sweep);
    }
    // A trace over a grid.
    if pick < 0.5 {
        let wave = 0.5 + 0.32 * sin(f.x * 9.0 + t * (1.0 + pick * 4.0)) * sin(f.x * 3.0 - t * 0.7 + pick * 30.0);
        let trace = 1.0 - smoothstep(0.012, 0.03, abs(f.y - wave));
        let grid = max(step(0.92, fract(f.x * 5.0)), step(0.92, fract(f.y * 4.0)));
        return tint * (0.04 + 0.08 * grid + 0.85 * trace);
    }
    // Levels rising and falling.
    if pick < 0.75 {
        let column = floor(f.x * 8.0);
        let level = 0.25 + 0.65 * (0.5 + 0.5 * sin(t * (0.6 + hash(vec2<f32>(column, pick))) + column * 1.7));
        let across = fract(f.x * 8.0);
        return tint * (0.04 + 0.7 * step(f.y, level) * step(0.16, across) * step(across, 0.84));
    }
    // Lines of readout.
    let row = floor(f.y * 7.0);
    let run = 0.25 + 0.6 * hash(vec2<f32>(row, pick));
    let lit = step(f.x, run) * step(0.3, fract(f.y * 7.0)) * step(0.5, hash(vec2<f32>(floor(f.x * 16.0 + t), row)) + 0.25);
    return tint * (0.04 + 0.62 * lit);
}

// Lit keys and dials on a desk's dark front.
fn studs(uv: vec2<f32>) -> vec3<f32> {
    let cell = floor(uv * 3.3);
    let spot = fract(uv * 3.3) - 0.5;
    let on = step(0.72, hash(cell)) * step(length(spot), 0.2);
    let blink = 0.65 + 0.35 * sin(ship.glow.y * (1.0 + 2.0 * hash(cell + 3.0)));
    return mix(vec3<f32>(1.0, 0.65, 0.2), vec3<f32>(0.3, 1.0, 0.8), step(0.5, hash(cell + 9.0))) * on * blink;
}

// Rooms light themselves: the sun never reaches them.
fn interior(in: Out) -> vec3<f32> {
    let p = in.local;
    let n = in.normal;
    let uv = surface(p, n);
    let tint = room_light(p);
    let seed = hash(floor(p.xz / 7.0));
    var albedo = vec3<f32>(0.3);
    var emit = vec3<f32>(0.0);
    switch in.material {
        // Deck plates, a shade apart, and a sheen under the lamps.
        case 7u: {
            let plate = hash(floor(uv / 2.0));
            albedo = vec3<f32>(0.2, 0.215, 0.24) * (0.82 + plate * 0.28) * (0.6 + 0.4 * seams(uv, 2.0));
        }
        // Walls: pale panels above a dark band, with a lit line between.
        case 8u: {
            let upper = vec3<f32>(0.5, 0.53, 0.57) * (0.7 + 0.3 * seams(uv, 3.0));
            let lower = vec3<f32>(0.19, 0.21, 0.25) * (0.8 + 0.2 * seams(uv, 1.5));
            albedo = mix(lower, upper, step(-2.7, p.y));
            emit = tint * (1.0 - smoothstep(0.03, 0.07, abs(p.y + 2.7))) * 0.9;
        }
        // Ceiling plates with the lamps let into them.
        case 9u: {
            albedo = vec3<f32>(0.16, 0.17, 0.2) * (0.7 + 0.3 * seams(uv, 3.0));
            let from_lamp = abs(p.xz - lamp_near(p).xz);
            let panel = (1.0 - smoothstep(1.0, 1.1, from_lamp.x)) * (1.0 - smoothstep(0.3, 0.38, from_lamp.y));
            emit = tint * panel * 1.2;
        }
        // Desks: a tilted top is a display, the rest dark casing with keys.
        case 10u: {
            albedo = vec3<f32>(0.09, 0.11, 0.15) * (0.7 + 0.3 * seams(uv, 1.0));
            if abs(n.y) > 0.05 && abs(n.y) < 0.98 {
                emit = display(uv * 1.3, seed, vec3<f32>(0.3, 0.85, 1.0));
            } else if abs(n.y) < 0.05 {
                emit = studs(uv);
            }
        }
        case 11u: {
            albedo = vec3<f32>(0.02, 0.03, 0.04);
            let warm = step(0.72, seed);
            emit = display(uv * 0.85, seed, mix(vec3<f32>(0.25, 0.85, 1.0), vec3<f32>(1.0, 0.7, 0.3), warm));
        }
        case 12u: {
            albedo = vec3<f32>(0.3, 0.32, 0.35) * (0.78 + 0.22 * seams(uv, 1.0)) * (0.92 + 0.08 * hash(floor(uv * 8.0)));
        }
        case 13u: {
            let stripe = step(0.5, fract((uv.x + uv.y) * 0.8));
            albedo = mix(vec3<f32>(0.05, 0.05, 0.06), vec3<f32>(0.85, 0.62, 0.08), stripe);
        }
        // Crates: a painted shell with a pale band and a stencilled mark.
        case 14u: {
            let which = hash(floor((p.xz + vec2<f32>(0.3)) / 5.0) + floor(p.y / 2.0 + 2.0) * 3.0);
            var paint = vec3<f32>(0.32, 0.36, 0.25);
            if which > 0.66 {
                paint = vec3<f32>(0.5, 0.26, 0.12);
            } else if which > 0.33 {
                paint = vec3<f32>(0.2, 0.3, 0.42);
            }
            let band = 1.0 - smoothstep(0.1, 0.13, abs(fract(p.y / 2.0) - 0.5));
            albedo = mix(paint * (0.7 + 0.3 * seams(uv, 1.0)), vec3<f32>(0.75, 0.72, 0.62), band * 0.55);
        }
        case 15u: { albedo = vec3<f32>(0.6, 0.62, 0.62); }
        // Pipes, banded at their joints.
        case 16u: {
            let run = max(uv.x, uv.y);
            albedo = vec3<f32>(0.44, 0.3, 0.2) * (0.72 + 0.28 * step(0.08, fract(run / 3.0)));
        }
        case 17u: {
            albedo = vec3<f32>(0.2, 0.25, 0.4) * (0.8 + 0.2 * seams(uv, 0.45));
        }
        // Open floor mesh, dark under the bars.
        case 19u: {
            let bars = abs(fract(uv * 3.0) - 0.5);
            let hole = step(max(bars.x, bars.y), 0.32);
            albedo = mix(vec3<f32>(0.3, 0.32, 0.34), vec3<f32>(0.035, 0.04, 0.05), hole);
        }
        case 20u: {
            let weave = 0.92 + 0.08 * sin(uv.x * 30.0) * sin(uv.y * 30.0);
            albedo = mix(vec3<f32>(0.2, 0.3, 0.46), vec3<f32>(0.72, 0.72, 0.68), step(0.45, seed)) * weave;
        }
        case 21u: { return tint * 0.98; }
        // Doors: brushed steel leaves with a lit bar across.
        case 22u: {
            albedo = vec3<f32>(0.55, 0.57, 0.6) * (0.85 + 0.15 * seams(uv, 1.5));
            emit = tint * (1.0 - smoothstep(0.03, 0.06, abs(p.y + 2.3))) * 0.8;
            albedo = mix(albedo, vec3<f32>(0.85, 0.62, 0.08), step(abs(p.y + 3.7), 0.12) * step(0.5, fract((uv.x + uv.y) * 0.8)));
        }
        default: {}
    }
    let core = vec3<f32>(abs(p.x) - 13.0, min(p.y + 2.0, 0.0), p.z + 19.0);
    let reactor = vec3<f32>(0.1, 0.6, 0.52) * 3.0 / (1.0 + dot(core, core) * 0.03)
        * (0.8 + 0.2 * sin(ship.glow.y * 3.0)) * step(p.z, -7.75) * step(-30.25, p.z);
    let light = tint * (0.34 + 1.15 * lamplight(p, n)) * shadow(p, n) + reactor;
    return albedo * light + emit;
}

@fragment fn fs(in: Out) -> @location(0) vec4<f32> {
    if dot(ship.clip.xyz, in.relative) > ship.clip.w {
        discard;
    }
    let cold = vec3<f32>(0.45, 0.95, 1.0);
    let normal = normalize((ship.rotation * vec4<f32>(in.normal, 0.0)).xyz);
    if in.material == 23u {
        let plates = vec3<f32>(0.25, 0.28, 0.32) * (0.75 + 0.25 * seams(surface(in.local, in.normal), 0.8));
        return vec4<f32>(finish(lit(plates, normal), in, 0.0), 1.0);
    }
    if in.material >= 7u {
        return vec4<f32>(interior(in), 1.0);
    }
    if in.material == 3u {
        let pulse = 0.9 + 0.1 * sin(ship.glow.y * 2.0 + in.local.z * 0.08);
        return vec4<f32>(finish(cold * pulse, in, 1.0), 1.0);
    }
    if in.material == 4u {
        // Bands of energy climbing a ribbed core.
        let flow = 0.5 + 0.5 * sin(in.local.y * 2.2 - ship.glow.y * 4.0);
        let ribs = 0.45 + 0.55 * seams(surface(in.local, in.normal), 2.0);
        let pulse = 0.8 + 0.2 * sin(ship.glow.y * 3.0);
        let energy = mix(vec3<f32>(0.02, 0.3, 0.28), vec3<f32>(0.6, 1.0, 0.92), flow * flow);
        return vec4<f32>(energy * ribs * pulse, 1.0);
    }
    if in.material == 5u {
        let power = 0.72 + 0.28 * ship.glow.x;
        let flicker = 0.96 + 0.04 * sin(ship.glow.y * 37.0);
        let bands = 0.9 + 0.1 * sin(surface(in.local, in.normal).x * 2.2);
        let heart = mix(vec3<f32>(0.3, 0.72, 1.0), vec3<f32>(0.9, 0.98, 1.0), ship.glow.x);
        return vec4<f32>(finish(heart * power * flicker * bands * 1.25, in, 1.0), 1.0);
    }
    let uv = surface(in.local, in.normal);
    let big = hash(floor(uv / 9.0));
    let small = hash(floor(uv / 3.0) + 7.0);
    var color = base_color(in.material) * (0.82 + big * 0.26 + small * 0.1);
    color *= 0.55 + 0.45 * seams(uv, 9.0);
    color *= 0.8 + 0.2 * seams(uv, 3.0);
    var shaded = lit(color, normal);
    var emission = 0.0;
    if abs(in.normal.x) > 0.5 && in.material == 0u {
        let window = windows(in.local) * (0.55 + 0.45 * hash(floor(in.local.zz / 20.0)));
        shaded += vec3<f32>(0.75, 0.9, 1.0) * window * 0.8;
        emission = window;
        let seam = 1.0 - smoothstep(0.08, 0.2 + fwidth(in.local.y), abs(in.local.y + 7.0));
        shaded += cold * seam;
        emission = max(emission, seam);
    }
    return vec4<f32>(finish(shaded, in, emission), 1.0);
}

@fragment fn fs_glass(in: Out) -> @location(0) vec4<f32> {
    if dot(ship.clip.xyz, in.relative) > ship.clip.w {
        discard;
    }
    if in.material == 18u {
        // A hologram: bright slices of light with air between them.
        let slice = 0.35 + 0.65 * step(0.5, fract(in.local.y * 6.0 - ship.glow.y * 0.8));
        return vec4<f32>(vec3<f32>(0.35, 0.9, 1.0) * slice, 0.28 * slice);
    }
    let normal = normalize((ship.rotation * vec4<f32>(in.normal, 0.0)).xyz);
    let view = normalize(in.relative);
    let edge = pow(1.0 - abs(dot(view, normal)), 3.0);
    let sky = mix(vec3<f32>(0.05, 0.1, 0.14), globals.sky_color.rgb, 0.35 + edge * 0.5);
    return vec4<f32>(finish(sky + vec3<f32>(0.1, 0.3, 0.35), in, 0.4), 0.55 + edge * 0.4);
}

struct BoltIn {
    @location(0) position: vec3<f32>,
    @location(1) across: vec2<f32>,
    @location(2) color: vec4<f32>,
}
struct BoltOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) across: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex fn vs_bolt(in: BoltIn) -> BoltOut {
    var out: BoltOut;
    out.clip = globals.view_proj * vec4<f32>(in.position, 1.0);
    out.across = in.across;
    out.color = in.color;
    return out;
}

// A spiral with a burning eye. It is turned by turning its quad.
@fragment fn fs_swirl(in: BoltOut) -> @location(0) vec4<f32> {
    let reach = length(in.across);
    if reach >= 1.0 {
        discard;
    }
    let angle = atan2(in.across.y, in.across.x);
    let arms = 0.5 + 0.5 * sin(angle * 3.0 + log(reach + 0.03) * 7.0);
    let fine = 0.5 + 0.5 * sin(angle * 11.0 - log(reach + 0.03) * 13.0);
    let edge = 1.0 - smoothstep(0.55, 1.0, reach);
    let eye = exp(-reach * reach * 40.0);
    let body = pow(arms, 3.0) * (0.6 + 0.4 * fine) * edge;
    let light = in.color.rgb * body * 0.9 + vec3<f32>(0.5, 0.25, 0.8) * eye;
    return vec4<f32>(light * in.color.a, 0.0);
}

// Light only adds: a soft body round a white-hot core.
@fragment fn fs_bolt(in: BoltOut) -> @location(0) vec4<f32> {
    let d = dot(in.across, in.across);
    let body = exp(-d * 5.0) - exp(-5.0);
    let core = exp(-d * 45.0);
    return vec4<f32>((in.color.rgb * body + vec3<f32>(core)) * in.color.a, 0.0);
}
"#;
