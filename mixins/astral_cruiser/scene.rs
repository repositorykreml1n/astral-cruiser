// The far side of a portal. The server thread copies the real destination chunks
// and mobs into a mailbox; the window feeds them to a World and Streaming of its
// own, and the meshes and entity vertices go to the portal's Renderer.
use super::{Dim, Game, Mob};
use horizon_client::{
    chunk::Chunk,
    gfx::astral_portal as portal,
    mobs::MobType,
    streaming::{ChunkGenerator, Proto, Streaming},
    world::World,
};
use horizon_core::geometry::Vec3;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
};

/// How many ticks apart the far side's chunks are looked through for changes.
const MIRROR_TICKS: u64 = 5;
/// How many of them the window takes in a frame: copying a column is the one
/// slow step here, and the view of the far side must not cost a hitch.
const CHUNKS_A_FRAME: usize = 6;

/// What the server is asked to copy. `(middle, radius)` of the chunks.
#[derive(Clone, Copy)]
struct Request {
    dim: Dim,
    area: ((i32, i32), i32),
    /// A new one sends every chunk again: the window's world starts empty.
    generation: u64,
}

static GENERATION: AtomicU64 = AtomicU64::new(0);
static REQUEST: Mutex<Option<Request>> = Mutex::new(None);
static MAILBOX: Mutex<Vec<Arc<Chunk>>> = Mutex::new(Vec::new());
static MOBS: Mutex<Option<Vec<MobPose>>> = Mutex::new(None);

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the window needs to draw a mob of the far side; a Mob itself stays on its
/// thread, its AI is not Send.
struct MobPose {
    id: u32,
    kind: Arc<MobType>,
    position: Vec3,
    previous: Vec3,
    yaw: f32,
    previous_yaw: f32,
    body_yaw: f32,
    previous_body_yaw: f32,
    head_yaw: f32,
    previous_head_yaw: f32,
    head_pitch: f32,
    previous_head_pitch: f32,
    on_ground: bool,
    hurt_time: i32,
    death_time: i32,
    age: u32,
    limb_position: f32,
    limb_speed: f32,
    previous_limb_speed: f32,
    invisible: bool,
}

impl MobPose {
    fn of(m: &Mob) -> Self {
        Self {
            id: m.id,
            kind: m.kind.clone(),
            position: m.position,
            previous: m.previous,
            yaw: m.yaw,
            previous_yaw: m.previous_yaw,
            body_yaw: m.body_yaw,
            previous_body_yaw: m.previous_body_yaw,
            head_yaw: m.head_yaw,
            previous_head_yaw: m.previous_head_yaw,
            head_pitch: m.head_pitch,
            previous_head_pitch: m.previous_head_pitch,
            on_ground: m.on_ground,
            hurt_time: m.hurt_time,
            death_time: m.death_time,
            age: m.age,
            limb_position: m.limb_position,
            limb_speed: m.limb_speed,
            previous_limb_speed: m.previous_limb_speed,
            invisible: m.invisible,
        }
    }

    fn apply(self, m: &mut Mob) {
        m.position = self.position;
        m.previous = self.previous;
        m.yaw = self.yaw;
        m.previous_yaw = self.previous_yaw;
        m.body_yaw = self.body_yaw;
        m.previous_body_yaw = self.previous_body_yaw;
        m.head_yaw = self.head_yaw;
        m.previous_head_yaw = self.previous_head_yaw;
        m.head_pitch = self.head_pitch;
        m.previous_head_pitch = self.previous_head_pitch;
        m.on_ground = self.on_ground;
        m.hurt_time = self.hurt_time;
        m.death_time = self.death_time;
        m.age = self.age;
        m.limb_position = self.limb_position;
        m.limb_speed = self.limb_speed;
        m.previous_limb_speed = self.previous_limb_speed;
        m.invisible = self.invisible;
    }
}

/// The window's mobs made to match the latest snapshot.
fn apply_mobs(mobs: &mut Vec<Mob>, poses: Vec<MobPose>) {
    mobs.retain(|m| poses.iter().any(|p| p.id == m.id));
    for pose in poses {
        match mobs.iter_mut().find(|m| m.id == pose.id) {
            Some(mob) => pose.apply(mob),
            None => {
                let mut mob = Mob::new(pose.id, pose.kind.clone(), pose.position, pose.yaw);
                pose.apply(&mut mob);
                mobs.push(mob);
            }
        }
    }
}

struct NoTerrain;

impl ChunkGenerator for NoTerrain {
    type Worker = ();
    fn worker(&self) {}
    fn proto(&self, _: &mut (), _: i32, _: i32) -> Proto {
        Box::new(())
    }
    fn finish(&self, _: Proto) -> Chunk {
        Chunk::empty(0, 0)
    }
}

/// The far side as the window holds it.
struct Scene {
    dim: Dim,
    world: World,
    streaming: Streaming,
    mobs: Vec<Mob>,
    /// What has arrived and waits for its frame.
    waiting: Vec<Arc<Chunk>>,
}

thread_local! {
    // Holding the chunks makes the server's copy-on-write give an edited chunk
    // a new Arc, which is how an edit is noticed.
    static SEEN: RefCell<HashMap<(i32, i32), Arc<Chunk>>> = RefCell::new(HashMap::new());
    static SENT_GENERATION: Cell<u64> = const { Cell::new(0) };
    static SCENE: RefCell<Option<Scene>> = const { RefCell::new(None) };
}

/// The server's side, on its own thread: new and edited chunks of the far side.
pub(super) fn mirror(game: &mut Game<'_>) {
    let request = *lock(&REQUEST);
    let Some(request) = request else {
        SEEN.with_borrow_mut(HashMap::clear);
        return;
    };
    if SENT_GENERATION.replace(request.generation) != request.generation {
        SEEN.with_borrow_mut(HashMap::clear);
    }
    let ticks = game.ticks;
    let level = if game.dim == request.dim {
        Some((&game.world, &game.mobs))
    } else {
        game.levels.parked_mut(request.dim).map(|parked| {
            let parked = &*parked;
            (parked.__horizon_mixins_astral_cruiser_parked_world(), &parked.mobs)
        })
    };
    let Some((world, mobs)) = level else {
        return;
    };
    let ((cx, cz), radius) = request.area;
    let reach = f64::from((radius + 1) * 16);
    let near = |m: &&Mob| {
        (m.position.x - f64::from(cx * 16 + 8)).abs() < reach
            && (m.position.z - f64::from(cz * 16 + 8)).abs() < reach
    };
    *lock(&MOBS) = Some(mobs.iter().filter(near).map(MobPose::of).collect());
    if !ticks.is_multiple_of(MIRROR_TICKS) {
        return;
    }
    let mut fresh = Vec::new();
    SEEN.with_borrow_mut(|seen| {
        for z in cz - radius..=cz + radius {
            for x in cx - radius..=cx + radius {
                let Some(chunk) = world.chunks.get(&(x, z)) else {
                    continue;
                };
                if !chunk.lit || seen.get(&(x, z)).is_some_and(|old| Arc::ptr_eq(old, chunk)) {
                    continue;
                }
                seen.insert((x, z), chunk.clone());
                fresh.push(chunk.clone());
            }
        }
    });
    if !fresh.is_empty() {
        lock(&MAILBOX).append(&mut fresh);
    }
}

/// Starts showing a far side: the server starts copying it, and a world for it
/// is made here on the window.
pub(super) fn open(game: &mut Game<'_>, dim: Dim, area: ((i32, i32), i32)) {
    close();
    let (middle, radius) = area;
    let mut streaming = Streaming::new(Arc::new(NoTerrain), game.pack.context.clone(), 2);
    streaming.remote = true;
    streaming.radius = radius + 2;
    streaming.center = middle;
    SCENE.with_borrow_mut(|slot| {
        *slot = Some(Scene {
            dim,
            world: World::new(game.res.facts().air, 0),
            streaming,
            mobs: Vec::new(),
            waiting: Vec::new(),
        });
    });
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    *lock(&REQUEST) = Some(Request { dim, area, generation });
}

pub(super) fn close() {
    *lock(&REQUEST) = None;
    lock(&MAILBOX).clear();
    *lock(&MOBS) = None;
    SCENE.with_borrow_mut(|slot| *slot = None);
    portal::remove();
}

/// The window's frame for the far side: what arrived goes to its own world and
/// renderer, and `pose` tells the renderer where the portal stands.
pub(super) fn update(game: &mut Game<'_>, pose: impl FnOnce(&Game<'_>, Dim) -> portal::Pose) {
    SCENE.with_borrow_mut(|slot| {
        let Some(scene) = slot else {
            return;
        };
        if !portal::installed() {
            let (Some(window), Some(main)) = (game.window.clone(), game.renderer.as_ref()) else {
                return;
            };
            let second = super::build_renderer(
                window,
                true,
                &game.pack,
                &game.gui,
                Some(main),
                None,
                game.settings.horizon.api,
            );
            match second {
                Ok(second) => portal::install(main, second),
                Err(e) => {
                    log::error!("astral cruiser: no renderer for the portal: {e}");
                    *slot = None;
                    return;
                }
            }
        }
        scene.waiting.append(&mut lock(&MAILBOX));
        let taken = scene.waiting.len().min(CHUNKS_A_FRAME);
        for chunk in scene.waiting.drain(..taken) {
            let chunk = Arc::try_unwrap(chunk).unwrap_or_else(|shared| (*shared).clone());
            scene.streaming.insert_remote(&mut scene.world, chunk);
        }
        let (uploads, unloaded) = scene.streaming.update(&mut scene.world);
        portal::with_renderer(|renderer| {
            for (key, mesh) in &uploads {
                renderer.upload(*key, mesh);
            }
            renderer.remove_columns(unloaded.iter().map(|c| (c.x, c.z)));
        });
        scene.streaming.free(uploads);
        if let Some(poses) = lock(&MOBS).take() {
            apply_mobs(&mut scene.mobs, poses);
        }
        // The game's own entity drawing, with the far side's mobs and world in place.
        let mut vertices = portal::take_entities();
        vertices.clear();
        std::mem::swap(&mut game.world, &mut scene.world);
        std::mem::swap(&mut game.mobs, &mut scene.mobs);
        let vertices = game.entity_geometry(portal::view_camera(), 1.0, vertices);
        std::mem::swap(&mut game.world, &mut scene.world);
        std::mem::swap(&mut game.mobs, &mut scene.mobs);
        portal::set_entities(vertices);
        portal::set_pose(Some(pose(game, scene.dim)));
    });
}
