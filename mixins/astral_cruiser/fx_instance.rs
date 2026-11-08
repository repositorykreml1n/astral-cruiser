// The effects of strikes and guns as GPU instances: one record per glow, ball of
// fire, puff of smoke, spark, beam or shell. The CPU only adds records when
// something happens; dead GPU slots stay until overwritten. Where a puff is at
// a moment is worked out from its record, here for tests and in the shader alike.
use glam::{DVec3, Vec3};
use std::ops::Range;

/// The most records alive at once: the size of the GPU buffer.
pub const FX_CAPACITY: usize = 32768;
/// The most balls of fire drawn in a frame: each costs noise on every pixel it covers.
pub const FIREBALL_CAP: usize = 64;
pub const KINDS: usize = 6;
// Prim discriminant order: Beam, Glow, Fireball, Smoke, Spark, Shell.
pub const KIND_CAPACITIES: [usize; KINDS] = [2048, 4096, FIREBALL_CAP, 8192, 12288, 1024];
pub const FX_SLOTS: usize = 27712;

pub fn kind_region(kind: usize) -> Range<usize> {
    let start: usize = KIND_CAPACITIES[..kind].iter().sum();
    start..start + KIND_CAPACITIES[kind]
}
/// The most lights from blasts that reach the ship's shader.
pub const LIGHTS: usize = 8;

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Prim {
    /// A band of light from `at` to `vec`.
    Beam = 0,
    /// A soft spot of light facing the camera.
    Glow = 1,
    /// A turning sphere of fire, white-hot to dark red as it cools.
    Fireball = 2,
    /// A soft dark puff, drawn over what is behind it rather than added.
    Smoke = 3,
    /// A short streak along its speed, falling.
    Spark = 4,
    /// A ring growing round the axis `vec`.
    Shell = 5,
}

/// Something to add to the pool, in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FxSpawn {
    pub prim: Prim,
    pub at: DVec3,
    /// Beam: where it ends. Shell: its axis. The rest: speed at birth, blocks a second.
    pub vec: DVec3,
    pub size: f32,
    /// How much `size` grows a second.
    pub grow: f32,
    pub color: [f32; 4],
    /// Seconds on the effects' clock.
    pub born: f32,
    pub life: f32,
    pub seed: u32,
}

/// A record as the GPU reads it: positions from the pool's anchor. 64 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FxInstance {
    pub pos: [f32; 3],
    pub size: f32,
    pub vec: [f32; 3],
    pub born: f32,
    pub color: [f32; 4],
    pub kind: u32,
    pub seed: u32,
    pub life: f32,
    pub grow: f32,
}

/// A blast's light on the ship, fading over its life.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FxLight {
    pub at: DVec3,
    pub radius: f32,
    pub color: [f32; 3],
    pub born: f32,
    pub life: f32,
}

#[derive(Default)]
struct Ring {
    next: usize,
    dirty_start: usize,
    dirty_len: usize,
    live: usize,
    // An upper bound between the half-second live-count scans. An overwritten
    // long-lived record can only keep a draw enabled until the next scan.
    latest_death: f32,
}

pub struct FxPool {
    pub anchor: DVec3,
    items: Vec<Option<FxSpawn>>,
    pub lights: Vec<FxLight>,
    pub cache: Vec<FxInstance>,
    rings: [Ring; KINDS],
    uploads: [Range<usize>; KINDS * 2],
    upload_count: usize,
    full_upload: bool,
    time: f32,
    next_scan: f32,
}

impl FxPool {
    pub fn new() -> Self {
        Self {
            anchor: DVec3::ZERO,
            items: vec![None; FX_SLOTS],
            lights: Vec::with_capacity(32),
            cache: vec![bytemuck::Zeroable::zeroed(); FX_SLOTS],
            rings: std::array::from_fn(|_| Ring::default()),
            uploads: std::array::from_fn(|_| 0..0),
            upload_count: 0,
            full_upload: false,
            time: 0.0,
            next_scan: 0.5,
        }
    }

    pub fn push(&mut self, s: FxSpawn) {
        if s.life <= 0.0 || s.born + s.life < self.time { return; }
        let kind = s.prim as usize;
        let cap = KIND_CAPACITIES[kind];
        let ring = &mut self.rings[kind];
        let slot = kind_region(kind).start + ring.next;
        if self.items[slot].is_none() { ring.live += 1; }
        self.items[slot] = Some(s);
        self.cache[slot] = encode_spawn(s, self.anchor);
        ring.latest_death = ring.latest_death.max(s.born + s.life);
        if ring.dirty_len == 0 { ring.dirty_start = ring.next; }
        ring.dirty_len = (ring.dirty_len + 1).min(cap);
        ring.next = (ring.next + 1) % cap;
    }

    pub fn light(&mut self, l: FxLight) {
        if self.lights.len() >= 32 {
            self.lights.remove(0);
        }
        self.lights.push(l);
    }

    pub fn expire(&mut self, now: f32) {
        self.time = now;
        // Counts are approximate for at most half a second. Expiry never changes
        // cache bytes or dirty ranges: the shader collapses dead instances.
        if now >= self.next_scan {
            self.next_scan = now + 0.5;
            for (kind, ring) in self.rings.iter_mut().enumerate() {
                ring.live = 0;
                ring.latest_death = 0.0;
                for slot in &mut self.items[kind_region(kind)] {
                    if let Some(s) = *slot {
                        if now > s.born + s.life {
                            *slot = None;
                        } else {
                            ring.live += 1;
                            ring.latest_death = ring.latest_death.max(s.born + s.life);
                        }
                    }
                }
            }
        }
        self.lights.retain(|light| now <= light.born + light.life);
    }

    pub fn len(&self) -> usize {
        self.rings.iter().filter(|r| r.latest_death >= self.time).map(|r| r.live).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// A recreated device must receive the retained records, including dead slots
    /// that may still be inside a kind's draw region.
    pub fn reupload(&mut self) { self.full_upload = true; }

    pub fn upload_ranges(&self) -> &[Range<usize>] { &self.uploads[..self.upload_count] }

    pub fn draw_ranges(&self) -> [Range<u32>; KINDS] {
        std::array::from_fn(|kind| {
            let ring = &self.rings[kind];
            if ring.live == 0 || self.time > ring.latest_death { return 0..0; }
            let region = kind_region(kind);
            region.start as u32..region.end as u32
        })
    }

    /// Encode only at insertion or an anchor move. The returned slice mirrors
    /// fixed GPU slots; only upload_ranges() need transferring, then become clean.
    pub fn encode(&mut self, camera: DVec3) -> Option<(DVec3, &[FxInstance])> {
        self.upload_count = 0;
        let far = (camera - self.anchor).length_squared() > 2048.0 * 2048.0;
        if far {
            self.anchor = DVec3::new(
                (camera.x / 1024.0).round() * 1024.0,
                (camera.y / 1024.0).round() * 1024.0,
                (camera.z / 1024.0).round() * 1024.0,
            );
            for (item, encoded) in self.items.iter().zip(&mut self.cache) {
                *encoded = match item {
                    Some(s) if s.born + s.life >= self.time => encode_spawn(*s, self.anchor),
                    _ => bytemuck::Zeroable::zeroed(),
                };
            }
            self.full_upload = true;
        }
        if self.full_upload {
            self.uploads[0] = 0..FX_SLOTS;
            self.upload_count = 1;
            self.full_upload = false;
        } else {
            for (kind, ring) in self.rings.iter().enumerate() {
                let cap = KIND_CAPACITIES[kind];
                let start = kind_region(kind).start;
                let len = ring.dirty_len;
                if len == 0 { continue; }
                if len == cap {
                    self.uploads[self.upload_count] = start..start + cap;
                    self.upload_count += 1;
                } else {
                    let end = ring.dirty_start + len;
                    self.uploads[self.upload_count] = start + ring.dirty_start..start + end.min(cap);
                    self.upload_count += 1;
                    if end > cap {
                        self.uploads[self.upload_count] = start..start + end - cap;
                        self.upload_count += 1;
                    }
                }
            }
        }
        for ring in &mut self.rings { ring.dirty_len = 0; }
        if self.upload_count == 0 { None } else { Some((self.anchor, &self.cache)) }
    }

    pub fn lights_at(&self, now: f32, camera: DVec3) -> [[f32; 8]; LIGHTS] {
        let mut entries = [[0.0_f32; 8]; 32];
        let mut scores = [0.0_f32; 32];
        let mut count = 0;
        for light in &self.lights {
            if light.life <= 0.0 {
                continue;
            }
            let age = now - light.born;
            if age < 0.0 || age > light.life {
                continue;
            }
            let mut strength = 1.0 - age / light.life;
            if strength < 0.0 {
                strength = 0.0;
            } else if strength > 1.0 {
                strength = 1.0;
            }
            if strength <= 0.0 {
                continue;
            }
            let score = strength * light.radius;
            let entry = [
                (light.at.x - camera.x) as f32,
                (light.at.y - camera.y) as f32,
                (light.at.z - camera.z) as f32,
                light.radius,
                light.color[0],
                light.color[1],
                light.color[2],
                strength,
            ];
            if count < 32 {
                entries[count] = entry;
                scores[count] = score;
                count += 1;
            }
        }

        let mut i = 1;
        while i < count {
            let mut j = i;
            while j > 0 && scores[j - 1] < scores[j] {
                let tmp_score = scores[j - 1];
                scores[j - 1] = scores[j];
                scores[j] = tmp_score;

                let tmp_entry = entries[j - 1];
                entries[j - 1] = entries[j];
                entries[j] = tmp_entry;

                j -= 1;
            }
            i += 1;
        }

        let mut result = [[0.0; 8]; LIGHTS];
        let mut k = 0;
        while k < LIGHTS && k < count {
            result[k] = entries[k];
            k += 1;
        }
        result
    }
}

fn encode_spawn(s: FxSpawn, anchor: DVec3) -> FxInstance {
    FxInstance {
        pos: (s.at - anchor).as_vec3().to_array(),
        size: s.size,
        vec: if s.prim == Prim::Beam { (s.vec - anchor).as_vec3().to_array() }
            else { s.vec.as_vec3().to_array() },
        born: s.born,
        color: s.color,
        kind: s.prim as u32,
        seed: s.seed,
        life: s.life,
        grow: s.grow,
    }
}

impl Default for FxPool {
    fn default() -> Self {
        Self::new()
    }
}

pub fn position_at(i: &FxInstance, now: f32) -> Vec3 {
    let t = if now <= i.born {
        0.0
    } else if now >= i.born + i.life {
        i.life
    } else {
        now - i.born
    };
    let pos = Vec3::new(i.pos[0], i.pos[1], i.pos[2]);
    let vec = Vec3::new(i.vec[0], i.vec[1], i.vec[2]);
    if i.kind == Prim::Spark as u32 {
        pos + vec * t + Vec3::new(0.0, -4.9 * t * t, 0.0)
    } else if i.kind == Prim::Smoke as u32 {
        let drag = (1.0 - (-1.5 * t).exp()) / 1.5;
        pos + vec * drag + Vec3::new(0.0, 1.2 * t, 0.0)
    } else {
        pos
    }
}
