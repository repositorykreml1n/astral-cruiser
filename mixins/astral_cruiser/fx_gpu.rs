// Strike instances live in world space. Only their anchor, the camera and the
// effects clock change each frame; motion and quad geometry run on the GPU.
use super::astral_fx_instance::{FX_CAPACITY, FxInstance, FxLight, FxPool, FxSpawn, KINDS, LIGHTS, Prim};
use super::astral_fx_shader::SHADER;
use super::{Frame, Renderer};
use glam::DVec3;
use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::sync::OnceLock;
use std::time::Instant;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    view_proj: [[f32; 4]; 4],
    anchor_now: [f32; 4],
    camera_right: [f32; 4],
    camera_up: [f32; 4],
    light_color: [f32; 4],
}

struct Gpu {
    pipelines: [wgpu::RenderPipeline; KINDS],
    instances: wgpu::Buffer,
    uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
    ranges: [Range<u32>; KINDS],
    anchor: DVec3,
    // The portal renderer shares the device. Unlike the ship, these pipelines
    // bind no renderer globals, so both renders can reuse the same resources.
    device: wgpu::Device,
    format: wgpu::TextureFormat,
}

thread_local! {
    static POOL: RefCell<FxPool> = RefCell::new(FxPool::new());
    static GPU: RefCell<Option<Gpu>> = const { RefCell::new(None) };
    static ARMED: Cell<bool> = const { Cell::new(false) };
}

/// Seconds since the first use, shared by spawns, lights and the shader.
pub fn now() -> f32 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

pub fn spawn(s: FxSpawn) {
    POOL.with_borrow_mut(|pool| pool.push(s));
}

pub fn spawn_all(list: &[FxSpawn]) {
    POOL.with_borrow_mut(|pool| {
        for s in list { pool.push(*s); }
    });
}

pub fn light(l: FxLight) {
    POOL.with_borrow_mut(|pool| pool.light(l));
}

/// Camera-relative xyz, radius, rgb, fading strength; unused entries are zero.
pub fn lights(camera: DVec3) -> [[f32; 8]; LIGHTS] {
    POOL.with_borrow(|pool| pool.lights_at(now(), camera))
}

/// Live-slot estimate, refreshed at most every half second by the pool.
pub fn count() -> usize {
    POOL.with_borrow_mut(|pool| {
        pool.expire(now());
        pool.len()
    })
}

/// A portal's destination renderer must not reuse the source world's pool.
pub(super) fn skip_draw() {
    ARMED.set(false);
}

fn pipeline(
    main: &Renderer,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    fragment: &str,
    premultiplied: bool,
) -> wgpu::RenderPipeline {
    // Offsets 0, 12, 16, 28, 32, 48, 52, 56, 60: exactly FxInstance.
    let attributes = wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32, 2 => Float32x3, 3 => Float32,
        4 => Float32x4, 5 => Uint32, 6 => Uint32, 7 => Float32, 8 => Float32
    ];
    let blend = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: if premultiplied { wgpu::BlendFactor::OneMinusSrcAlpha } else { wgpu::BlendFactor::One },
        operation: wgpu::BlendOperation::Add,
    };
    main.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fragment),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<FxInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
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
                blend: Some(wgpu::BlendState { color: blend, alpha: blend }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn build(main: &Renderer) -> Gpu {
    let device = &main.device;
    let bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Astral effects"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Astral effects"),
        bind_group_layouts: &[Some(&bindings)],
        ..Default::default()
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Astral effects"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Astral effects camera and clock"),
        size: std::mem::size_of::<Uniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Astral effects"),
        layout: &bindings,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }],
    });
    Gpu {
        pipelines: [
            pipeline(main, &layout, &shader, "fs_beam", false),
            pipeline(main, &layout, &shader, "fs_glow", false),
            pipeline(main, &layout, &shader, "fs_fireball", true),
            pipeline(main, &layout, &shader, "fs_smoke", true),
            pipeline(main, &layout, &shader, "fs_spark", false),
            pipeline(main, &layout, &shader, "fs_shell", false),
        ],
        instances: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Astral effects instances"),
            size: (FX_CAPACITY * std::mem::size_of::<FxInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        uniform,
        group,
        ranges: std::array::from_fn(|_| 0..0),
        anchor: DVec3::ZERO,
        device: device.clone(),
        format: main.config.format.remove_srgb_suffix(),
    }
}

/// Before every world render, even when no ship is visible.
pub fn prepare(main: &mut Renderer, frame: &Frame<'_>) {
    ARMED.set(false);
    let time = now();
    POOL.with_borrow_mut(|pool| {
        pool.expire(time);
        if frame.title { return; }
        GPU.with_borrow_mut(|slot| {
            if pool.is_empty() && slot.is_none() { return; }
            if slot.as_ref().is_none_or(|gpu|
                gpu.device != main.device || gpu.format != main.config.format.remove_srgb_suffix())
            {
                *slot = Some(build(main));
                // A recreated device needs the records even if the pool did not change.
                pool.reupload();
            }
            let Some(gpu) = slot else { return; };
            if pool.encode(frame.camera).is_some() {
                gpu.anchor = pool.anchor;
                for range in pool.upload_ranges() {
                    main.queue.write_buffer(&gpu.instances,
                        (range.start * std::mem::size_of::<FxInstance>()) as u64,
                        bytemuck::cast_slice(&pool.cache[range.clone()]));
                }
            }
            // Updated even on expiry-only frames; no instance data is transferred.
            gpu.ranges = pool.draw_ranges();
            let view = main.views(frame);
            let eye = view.primary();
            let inverse = eye.view.inverse();
            let offset = gpu.anchor - frame.camera;
            let mut light_color = [0.0; 4];
            let mut brightest = 0.0;
            for l in pool.lights_at(time, frame.camera) {
                let brightness = (l[4] * 0.2126 + l[5] * 0.7152 + l[6] * 0.0722) * l[7] * l[3];
                if brightness > brightest {
                    brightest = brightness;
                    light_color = [l[4] * l[7], l[5] * l[7], l[6] * l[7], 0.0];
                }
            }
            let uniform = Uniform {
                view_proj: eye.view_proj(frame.bob * frame.spin).to_cols_array_2d(),
                anchor_now: [offset.x as f32, offset.y as f32, offset.z as f32, time],
                camera_right: inverse.x_axis.truncate().extend(0.0).to_array(),
                camera_up: inverse.y_axis.truncate().extend(0.0).to_array(),
                light_color,
            };
            main.queue.write_buffer(&gpu.uniform, 0, bytemuck::bytes_of(&uniform));
            ARMED.set(true);
        });
    });
}

/// Smoke first, then fire and light, all tested against the world's depth.
pub fn draw(pass: &mut wgpu::RenderPass<'_>) {
    if !ARMED.replace(false) { return; }
    GPU.with_borrow(|slot| {
        let Some(gpu) = slot else { return; };
        pass.set_bind_group(0, &gpu.group, &[]);
        pass.set_vertex_buffer(0, gpu.instances.slice(..));
        for kind in [Prim::Smoke, Prim::Fireball, Prim::Shell, Prim::Beam, Prim::Spark, Prim::Glow] {
            let range = gpu.ranges[kind as usize].clone();
            if !range.is_empty() {
                pass.set_pipeline(&gpu.pipelines[kind as usize]);
                pass.draw(0..6, range);
            }
        }
    });
}
