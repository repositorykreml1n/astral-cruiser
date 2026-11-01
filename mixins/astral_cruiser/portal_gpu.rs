// The portal on the GPU. A second offscreen Renderer on the main device draws the
// far side from where a camera there stands; a GPU copy makes it sampleable, and a
// quad in the main world pass shows it by screen position, depth tested against
// the world. Nothing is ever read back to the CPU.
use super::astral_portal_shader::SHADER;
use super::astral_ship;
use super::{Frame, Renderer, View};
use crate::environment::Sky;
use crate::hud::HudBatch;
use crate::mesher::Vertex;
use glam::{DQuat, DVec3, Mat4, Vec4};
use std::cell::{Cell, RefCell};
use std::time::Instant;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    view_proj: [[f32; 4]; 4],
    corners: [[f32; 4]; 4],
    size: [f32; 4],
    state: [f32; 4],
}

/// The portal this frame.
#[derive(Clone, Copy)]
pub struct Pose {
    /// Where its plane stands on this side, in the coordinates the game draws in.
    pub entry: DVec3,
    pub rotation: DQuat,
    /// The half width and height of the opening.
    pub opening: [f64; 2],
    /// How far it has opened, 0 to 1.
    pub opened: f32,
    /// What takes the game's camera to the camera of the far side.
    pub shift: DVec3,
    /// Whether the far side has been prepared.
    pub ready: bool,
    /// Whether an opening stands there at all: a jump's arrival is prepared the
    /// same way, with nothing to see until the veil.
    pub shown: bool,
    /// The share of the screen's image taken from the far side, over everything.
    pub veil: f32,
    pub distance: f32,
    pub sky: Sky,
    pub skybox: bool,
    pub end_sky: bool,
    pub nether: bool,
    pub going: bool,
}

struct Gpu {
    renderer: Renderer,
    pipeline: wgpu::RenderPipeline,
    veil: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    texture: wgpu::Texture,
    group: wgpu::BindGroup,
    size: [u32; 2],
    pose: Option<Pose>,
}

/// What the world pass has to draw this frame.
#[derive(Clone)]
struct Draw {
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    opening: bool,
    veil: Option<wgpu::RenderPipeline>,
}

thread_local! {
    static GPU: RefCell<Option<Gpu>> = const { RefCell::new(None) };
    static DRAW: RefCell<Option<Draw>> = const { RefCell::new(None) };
    static LAST_MS: Cell<f32> = const { Cell::new(0.0) };
    static OBLIQUE: Cell<Option<[f32; 4]>> = const { Cell::new(None) };
    static ENTITIES: RefCell<Vec<Vertex>> = const { RefCell::new(Vec::new()) };
    static VIEW_CAMERA: Cell<DVec3> = const { Cell::new(DVec3::ZERO) };
}

/// The projection of the far side's view with its near plane on the exit plane
/// (Lengyel's oblique clipping, reversed Z), so nothing between the virtual
/// camera and the exit shows.
pub fn oblique(mut view: View) -> View {
    if let Some(plane) = OBLIQUE.get() {
        let plane = Vec4::from_array(plane);
        let projection = view.projection;
        let inverse = projection.inverse();
        let clip = inverse.transpose() * plane;
        let corner = inverse * Vec4::new(clip.x.signum(), clip.y.signum(), 0.0, 1.0);
        let w = projection.row(3);
        let row = w - plane * (w.dot(corner) / plane.dot(corner));
        let mut columns = projection.to_cols_array_2d();
        for (column, value) in columns.iter_mut().zip(row.to_array()) {
            column[2] = value;
        }
        view.projection = Mat4::from_cols_array_2d(&columns);
    }
    view
}

/// The camera of the last view of the far side, for camera-relative entity geometry.
pub fn view_camera() -> DVec3 {
    VIEW_CAMERA.get()
}

pub fn take_entities() -> Vec<Vertex> {
    ENTITIES.take()
}

pub fn set_entities(entities: Vec<Vertex>) {
    ENTITIES.set(entities);
}

fn view_target(
    device: &wgpu::Device,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    sampler: &wgpu::Sampler,
) -> (wgpu::Texture, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Astral portal view"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Astral portal"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    (texture, group)
}

pub fn installed() -> bool {
    GPU.with_borrow(Option::is_some)
}

pub fn install(main: &Renderer, second: Renderer) {
    let device = &main.device;
    let format = main.config.format.remove_srgb_suffix();
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Astral portal"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Astral portal"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Astral portal"),
        bind_group_layouts: &[Some(&layout)],
        ..Default::default()
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Astral portal"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    let veil = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Astral portal veil"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_veil"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_veil"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Astral portal"),
        size: std::mem::size_of::<Uniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Astral portal"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let size = [second.config.width, second.config.height];
    let (texture, group) = view_target(device, size, format, &layout, &uniform, &sampler);
    GPU.with_borrow_mut(|gpu| {
        *gpu = Some(Gpu {
            renderer: second,
            pipeline,
            veil,
            layout,
            uniform,
            sampler,
            texture,
            group,
            size,
            pose: None,
        });
    });
}

pub fn remove() {
    GPU.with_borrow_mut(|gpu| *gpu = None);
    DRAW.with_borrow_mut(|draw| *draw = None);
    astral_ship::forget_second();
}

pub fn set_pose(pose: Option<Pose>) {
    GPU.with_borrow_mut(|gpu| {
        if let Some(gpu) = gpu {
            gpu.pose = pose;
        }
    });
}

pub fn with_renderer(f: impl FnOnce(&mut Renderer)) {
    GPU.with_borrow_mut(|gpu| {
        if let Some(gpu) = gpu {
            f(&mut gpu.renderer);
        }
    });
}

pub fn last_ms() -> f32 {
    LAST_MS.get()
}

/// The far side drawn from where the eye would stand there, on the image the
/// portal shows. False if nothing came of it.
fn draw_far_side(gpu: &mut Gpu, main: &Renderer, frame: &Frame<'_>, pose: &Pose, size: [u32; 2]) -> bool {
    // From behind, the same opening looks the other way out of the far side.
    let plane_forward = pose.rotation * DVec3::Z;
    let behind = (frame.camera - pose.entry).dot(plane_forward) >= 0.0;
    let camera = frame.camera + pose.shift;
    VIEW_CAMERA.set(camera);
    // With the eye still on this side, nothing short of the exit plane may show.
    let forward = if behind { -plane_forward } else { plane_forward };
    if pose.veil <= 0.0 {
        let plane_at = pose.entry + pose.shift - camera;
        let plane = Vec4::new(
            forward.x as f32,
            forward.y as f32,
            forward.z as f32,
            -forward.dot(plane_at) as f32,
        );
        let eye = frame.bob * frame.spin * crate::camera::view_matrix(frame.yaw, frame.pitch);
        OBLIQUE.set(Some((eye.inverse().transpose() * plane).to_array()));
    }
    let entities = ENTITIES.take();
    let hud = HudBatch::new(size);
    let view = Frame {
        camera,
        sky: pose.sky,
        distance: pose.distance,
        detail_distance: pose.distance,
        underwater: false,
        cave_culling: false,
        sky_blocked: false,
        skybox: pose.skybox,
        end_sky: pose.end_sky,
        environment_fog: pose.nether.then_some((pose.sky.fog_start, pose.sky.fog_end)),
        fog: None,
        dark_disc: false,
        outline: Vec::new(),
        debug_lines: &[],
        debug_lines_on_top: &[],
        debug_crosshair: &[],
        hud: &hud,
        nether_lighting: pose.nether,
        entities: &entities,
        glowing: &[],
        entity_spans: &[],
        weather: &[],
        dynamic_lights: &[],
        clouds: None,
        lightning: Default::default(),
        additive: &[],
        see_through: &[],
        celestial: if pose.skybox { frame.celestial } else { &[] },
        stars: if pose.skybox { frame.stars } else { None },
        hand: &[],
        screen: &[],
        border: None,
        menu_blur: None,
        post_effects: crate::post_chain::Request::none(),
        ..*frame
    };
    if pose.going && !behind {
        astral_ship::set_second_shift(pose.shift);
    } else {
        astral_ship::skip_second();
    }
    let rendered = gpu.renderer.render(&view).is_ok();
    astral_ship::clear_skip_second();
    drop(view);
    OBLIQUE.set(None);
    ENTITIES.set(entities);
    if !rendered {
        return false;
    }
    let Some(source) = gpu.renderer.offscreen_frame() else {
        return false;
    };
    let mut encoder = main.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Astral portal copy"),
    });
    encoder.copy_texture_to_texture(
        source.as_image_copy(),
        gpu.texture.as_image_copy(),
        wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
    );
    main.queue.submit([encoder.finish()]);
    true
}

pub fn before_main(main: &mut Renderer, frame: &Frame<'_>) {
    if frame.title {
        return;
    }
    // Taken out, so the nested render below finds nothing and does not recurse.
    let Some(mut gpu) = GPU.with_borrow_mut(Option::take) else {
        return;
    };
    let started = Instant::now();
    gpu.renderer.gamma = main.gamma;
    gpu.renderer.fullbright = main.fullbright;
    let size = [main.config.width, main.config.height];
    if gpu.size != size {
        gpu.renderer.resize(size[0], size[1]);
        let format = main.config.format.remove_srgb_suffix();
        let (texture, group) =
            view_target(&main.device, size, format, &gpu.layout, &gpu.uniform, &gpu.sampler);
        gpu.texture = texture;
        gpu.group = group;
        gpu.size = size;
    }
    if let Some(pose) = gpu.pose {
        let facing = pose.shown;
        let seen = pose.ready && (facing || pose.veil > 0.0);
        let drawn = seen && draw_far_side(&mut gpu, main, frame, &pose, size);
        let right = pose.rotation * DVec3::X;
        let corner = |side: f64, up: f64| {
            let at = pose.entry + right * (side * pose.opening[0]) + DVec3::Y * (up * pose.opening[1])
                - frame.camera;
            [at.x as f32, at.y as f32, at.z as f32, 1.0]
        };
        let uniform = Uniform {
            view_proj: main.views(frame).primary().view_proj(frame.bob * frame.spin).to_cols_array_2d(),
            corners: [
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
                corner(-1.0, 1.0),
                corner(1.0, 1.0),
            ],
            size: [size[0] as f32, size[1] as f32, frame.game_time / 20.0, pose.opened],
            state: [pose.veil, if drawn { 1.0 } else { 0.0 }, 0.0, 0.0],
        };
        main.queue.write_buffer(&gpu.uniform, 0, bytemuck::bytes_of(&uniform));
        if facing || pose.veil > 0.0 {
            DRAW.with_borrow_mut(|draw| {
                *draw = Some(Draw {
                    pipeline: gpu.pipeline.clone(),
                    group: gpu.group.clone(),
                    opening: facing,
                    veil: (drawn && pose.veil > 0.0).then(|| gpu.veil.clone()),
                });
            });
        }
    }
    LAST_MS.set(started.elapsed().as_secs_f32() * 1000.0);
    GPU.with_borrow_mut(|slot| *slot = Some(gpu));
}

/// In the main world pass: the opening, among the world by its depth.
pub fn draw(pass: &mut wgpu::RenderPass<'_>) {
    DRAW.with_borrow(|draw| {
        if let Some(draw) = draw.as_ref().filter(|draw| draw.opening) {
            pass.set_pipeline(&draw.pipeline);
            pass.set_bind_group(0, &draw.group, &[]);
            pass.draw(0..6, 0..1);
        }
    });
}

/// Last in the main world pass, over the ship too: the far side over everything.
pub fn draw_veil(pass: &mut wgpu::RenderPass<'_>) {
    DRAW.with_borrow_mut(|draw| {
        if let Some(draw) = draw.take()
            && let Some(veil) = draw.veil
        {
            pass.set_pipeline(&veil);
            pass.set_bind_group(0, &draw.group, &[]);
            pass.draw(0..3, 0..1);
        }
    });
}
