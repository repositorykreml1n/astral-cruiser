// The ship on the GPU. The hull stays in one vertex buffer and only a transform
// changes per frame, so the size of the ship costs no CPU. It is drawn inside the
// world pass with the terrain's depth, and reads the terrain's globals for the
// camera, sun and fog; every surface detail comes from the shader, not a texture.
use super::astral_hull::{
    BOLT_VERTICES, Bolt, BoltVertex, HullVertex, Swirl, bolt_vertices, furniture_vertices,
    hull_vertices, interior_vertices, swirl_vertices, turret_vertices,
};
use super::astral_layout::{Material, ShipLayout};
use super::astral_shader::SHADER;
use super::{Frame, Renderer};
use glam::{DQuat, DVec3, Mat4};
use std::cell::{Cell, RefCell};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    rotation: [[f32; 4]; 4],
    offset: [f32; 4],
    glow: [f32; 4],
    turrets: [[f32; 4]; 8],
    clip: [f32; 4],
}

/// Where to draw the ship this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShipView {
    pub position: DVec3,
    pub rotation: DQuat,
    /// The ship point the camera is fixed to while aboard: the ship is drawn
    /// around the camera, so it cannot jitter against the view.
    pub anchor: Option<DVec3>,
    /// Engine power, 0 to 1.
    pub power: f32,
    /// Whether the air's fog reaches the ship. The pilot's camera stands further
    /// back than a short render distance's fog, and must still see the ship.
    pub fog: bool,
    /// Each turret's turn and elevation, in radians.
    pub turrets: [[f32; 2]; 8],
}

struct Gpu {
    pipeline: wgpu::RenderPipeline,
    glass: wgpu::RenderPipeline,
    bolt: wgpu::RenderPipeline,
    swirl: wgpu::RenderPipeline,
    bolts: wgpu::Buffer,
    bolts_used: u32,
    swirl_used: u32,
    vertices: wgpu::Buffer,
    solid: u32,
    total: u32,
    uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
    /// The renderer's globals this was built on: a new renderer means a new buffer.
    globals: wgpu::Buffer,
}

/// Which of the two renderers the next draw belongs to.
#[derive(Clone, Copy, PartialEq)]
enum Armed {
    Not,
    Main,
    Second,
}

thread_local! {
    static VIEW: Cell<Option<ShipView>> = const { Cell::new(None) };
    static GPU: RefCell<Option<Gpu>> = const { RefCell::new(None) };
    /// The same for the renderer of a portal's far side, on its own globals.
    static SECOND: RefCell<Option<Gpu>> = const { RefCell::new(None) };
    /// Set just before that renderer draws: what takes the ship over there.
    static SECOND_SHIFT: Cell<Option<DVec3>> = const { Cell::new(None) };
    static SKIP_SECOND: Cell<bool> = const { Cell::new(false) };
    static ARMED: Cell<Armed> = const { Cell::new(Armed::Not) };
    static CHASE: Cell<Option<DVec3>> = const { Cell::new(None) };
    static FAR_LEAD: Cell<Option<[f64; 2]>> = const { Cell::new(None) };
    static BOLTS: RefCell<Vec<Bolt>> = const { RefCell::new(Vec::new()) };
    static SWIRL: Cell<Option<Swirl>> = const { Cell::new(None) };
    /// What the light of the frame is drawn from, once the hull is down.
    static LIGHT: Cell<Armed> = const { Cell::new(Armed::Not) };
    /// A portal the ship is flying into: a point on its plane, and the way through.
    static CLIP: Cell<Option<(DVec3, DVec3)>> = const { Cell::new(None) };
}

pub fn set_view(view: Option<ShipView>) {
    VIEW.set(view);
}

/// This frame's shots.
pub fn set_bolts(bolts: Vec<Bolt>) {
    BOLTS.set(bolts);
}

/// The portal plane the hull ends at this frame, if it is flying through one.
pub fn set_clip(clip: Option<(DVec3, DVec3)>) {
    CLIP.set(clip);
}

/// The next render is of a portal's far side, where the ship stands this far off.
pub fn set_second_shift(shift: DVec3) {
    SECOND_SHIFT.set(Some(shift));
}

/// The next nested render must not draw the ship at all.
pub fn skip_second() {
    SKIP_SECOND.set(true);
}

pub fn clear_skip_second() {
    SKIP_SECOND.set(false);
}

/// The renderer of the far side has gone, and what was built on it.
pub fn forget_second() {
    SECOND.with_borrow_mut(|slot| *slot = None);
    SECOND_SHIFT.set(None);
    SKIP_SECOND.set(false);
}

/// The spiral behind a ship gathering a jump, if it is.
pub fn set_swirl(swirl: Option<Swirl>) {
    SWIRL.set(swirl);
}

/// Where the pilot's camera stands this frame, if it is on.
pub fn set_chase(camera: Option<DVec3>) {
    CHASE.set(camera);
}

/// The x and z offset of the far terrain's window from the player, on while a light dash runs.
pub fn set_far_lead(lead: Option<[f64; 2]>) {
    FAR_LEAD.set(lead);
}

pub fn far_lead() -> Option<[f64; 2]> {
    FAR_LEAD.get()
}

/// The game's own camera, moved out to the pilot's. False if that is not on.
pub fn place_camera(position: &mut [f64; 3]) -> bool {
    let camera = CHASE.get();
    if let Some(camera) = camera {
        *position = camera.to_array();
    }
    camera.is_some()
}

fn pipeline(
    main: &Renderer,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    glass: bool,
) -> wgpu::RenderPipeline {
    let attributes = wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Uint32, 3 => Float32x3, 4 => Float32x3
    ];
    main.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Astral ship"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<HullVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(!glass),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(if glass { "fs_glass" } else { "fs" }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: main.config.format.remove_srgb_suffix(),
                    blend: glass.then_some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
}

/// Shots are light: they add to what is behind them and hide nothing.
fn bolt_pipeline(
    main: &Renderer,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    fragment: &str,
) -> wgpu::RenderPipeline {
    let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x4];
    let add = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    };
    main.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Astral bolts"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_bolt"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<BoltVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(fragment),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: main.config.format.remove_srgb_suffix(),
                    blend: Some(wgpu::BlendState { color: add, alpha: add }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
}

fn build(main: &Renderer) -> Gpu {
    let device = &main.device;
    let uniform_entry = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    let bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Astral ship"),
        entries: &[uniform_entry(0), uniform_entry(1)],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Astral ship"),
        bind_group_layouts: &[Some(&bindings)],
        ..Default::default()
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Astral ship"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let ship_layout = ShipLayout::cruiser();
    let mut mesh = hull_vertices(&ship_layout);
    mesh.extend(interior_vertices(&ship_layout));
    mesh.extend(furniture_vertices(&ship_layout));
    mesh.extend(turret_vertices(&ship_layout));
    let clear = |v: &HullVertex| {
        v.material == Material::Glass as u32 || v.material == Material::Holo as u32
    };
    mesh.sort_by_key(clear);
    let solid = mesh.iter().take_while(|v| !clear(v)).count() as u32;
    let vertices = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Astral ship hull"),
        size: std::mem::size_of_val(mesh.as_slice()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    main.queue
        .write_buffer(&vertices, 0, bytemuck::cast_slice(&mesh));
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Astral ship"),
        size: std::mem::size_of::<Uniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Astral ship"),
        layout: &bindings,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: main.globals.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: uniform.as_entire_binding(),
            },
        ],
    });
    Gpu {
        pipeline: pipeline(main, &layout, &shader, false),
        glass: pipeline(main, &layout, &shader, true),
        bolt: bolt_pipeline(main, &layout, &shader, "fs_bolt"),
        swirl: bolt_pipeline(main, &layout, &shader, "fs_swirl"),
        bolts: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Astral bolts"),
            size: (BOLT_VERTICES * std::mem::size_of::<BoltVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        bolts_used: 0,
        swirl_used: 0,
        vertices,
        solid,
        total: mesh.len() as u32,
        uniform,
        group,
        globals: main.globals.clone(),
    }
}

/// Before the main renderer draws: this frame's transform.
pub fn before_main(main: &mut Renderer, frame: &Frame<'_>) {
    ARMED.set(Armed::Not);
    if SKIP_SECOND.replace(false) {
        SECOND_SHIFT.take();
        return;
    }
    let shift = SECOND_SHIFT.take();
    let view = VIEW.get();
    let Some(view) = view.filter(|_| !frame.title) else {
        return;
    };
    let slots = if shift.is_some() { &SECOND } else { &GPU };
    slots.with_borrow_mut(|slot| {
        if slot.as_ref().is_none_or(|gpu| gpu.globals != main.globals) {
            *slot = Some(build(main));
        }
        let Some(gpu) = slot else {
            return;
        };
        let offset = match view.anchor {
            Some(anchor) => -(view.rotation * anchor),
            None => view.position + shift.unwrap_or(DVec3::ZERO) - frame.camera,
        };
        // The far side's view has its own near plane on the exit.
        let clip = CLIP.get().filter(|_| shift.is_none()).map_or([0.0, 0.0, 0.0, 1.0], |(at, way)| {
            let distance = way.dot(at - frame.camera);
            [way.x as f32, way.y as f32, way.z as f32, distance as f32]
        });
        let uniform = Uniform {
            rotation: Mat4::from_quat(view.rotation.as_quat()).to_cols_array_2d(),
            offset: [offset.x as f32, offset.y as f32, offset.z as f32, 0.0],
            glow: [
                view.power,
                frame.game_time / 20.0,
                if view.fog { 1.0 } else { 0.0 },
                0.0,
            ],
            turrets: view.turrets.map(|[turn, elevation]| [turn, elevation, 0.0, 0.0]),
            clip,
        };
        main.queue
            .write_buffer(&gpu.uniform, 0, bytemuck::bytes_of(&uniform));
        let mut light = if shift.is_some() {
            Vec::new()
        } else {
            BOLTS.with_borrow(|bolts| bolt_vertices(bolts, frame.camera))
        };
        gpu.bolts_used = light.len() as u32;
        if let Some(swirl) = SWIRL.get().filter(|_| shift.is_none()) {
            light.extend(swirl_vertices(&swirl, frame.camera));
        }
        gpu.swirl_used = light.len() as u32 - gpu.bolts_used;
        main.queue
            .write_buffer(&gpu.bolts, 0, bytemuck::cast_slice(&light));
        ARMED.set(if shift.is_some() { Armed::Second } else { Armed::Main });
    });
}

/// In the world pass, after the opaque terrain and entities.
pub fn draw(pass: &mut wgpu::RenderPass<'_>) {
    let armed = ARMED.replace(Armed::Not);
    LIGHT.set(armed);
    let slots = match armed {
        Armed::Not => return,
        Armed::Main => &GPU,
        Armed::Second => &SECOND,
    };
    slots.with_borrow(|slot| {
        if let Some(gpu) = slot {
            pass.set_bind_group(0, &gpu.group, &[]);
            pass.set_vertex_buffer(0, gpu.vertices.slice(..));
            pass.set_pipeline(&gpu.pipeline);
            pass.draw(0..gpu.solid, 0..1);
            pass.set_pipeline(&gpu.glass);
            pass.draw(gpu.solid..gpu.total, 0..1);
        }
    });
}

/// Last in the world pass: the light of the guns and of a jump, over a veil too.
pub fn draw_light(pass: &mut wgpu::RenderPass<'_>) {
    let slots = match LIGHT.replace(Armed::Not) {
        Armed::Not => return,
        Armed::Main => &GPU,
        Armed::Second => &SECOND,
    };
    slots.with_borrow(|slot| {
        if let Some(gpu) = slot {
            if gpu.bolts_used + gpu.swirl_used == 0 {
                return;
            }
            pass.set_bind_group(0, &gpu.group, &[]);
            pass.set_vertex_buffer(0, gpu.bolts.slice(..));
            pass.set_pipeline(&gpu.bolt);
            pass.draw(0..gpu.bolts_used, 0..1);
            pass.set_pipeline(&gpu.swirl);
            pass.draw(gpu.bolts_used..gpu.bolts_used + gpu.swirl_used, 0..1);
        }
    });
}
