//! Your own wgpu drawing inside the UI with a paint callback: a spinning
//! triangle drawn by a custom shader in a card of the central panel, with
//! UI widgets around (and on top of) it.
//!
//! For a 3D viewport with its own depth buffer, render into your own
//! texture instead and show it with `Frame::register_native_texture`.
//!
//! Run with: `cargo run -p rustroke --example custom_wgpu`

use std::sync::Arc;

use rustroke::{App, CallbackFn, Frame, Slider, WindowOptions, vec2, wgpu};

const SHADER: &str = "
struct Params { angle: f32, aspect: f32, _pad: vec2<f32> };
@group(0) @binding(0) var<uniform> params: Params;

struct Out { @builtin(position) pos: vec4<f32>, @location(0) color: vec3<f32> };

@vertex fn vs(@builtin(vertex_index) i: u32) -> Out {
    let corners = array(vec2(0.0, 0.8), vec2(-0.7, -0.5), vec2(0.7, -0.5));
    let colors = array(vec3(0.95, 0.55, 0.66), vec3(0.54, 0.71, 0.98), vec3(0.65, 0.89, 0.63));
    let c = cos(params.angle);
    let s = sin(params.angle);
    let p = corners[i];
    var out: Out;
    out.pos = vec4((c * p.x - s * p.y) / params.aspect, s * p.x + c * p.y, 0.0, 1.0);
    out.color = colors[i];
    return out;
}

@fragment fn fs(in: Out) -> @location(0) vec4<f32> { return vec4(in.color, 1.0); }
";

/// GPU resources, created once on the first frame.
struct Gpu {
    pipeline: wgpu::RenderPipeline,
    params: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl Gpu {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("triangle"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("triangle"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("triangle params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("triangle params"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params.as_entire_binding(),
            }],
        });
        Self {
            pipeline,
            params,
            bind_group,
        }
    }
}

struct Demo {
    gpu: Option<Arc<Gpu>>,
    speed: f32,
    angle: f32,
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;
        if self.gpu.is_none()
            && let (Some((device, _)), Some(format)) = (frame.wgpu(), frame.wgpu_target_format())
        {
            self.gpu = Some(Arc::new(Gpu::new(device, format)));
        }
        self.angle += self.speed / 60.0;
        frame.request_repaint = self.speed != 0.0;
        let angle = self.angle;
        let gpu = self.gpu.clone();

        frame.ui(|ui| {
            ui.heading("Custom wgpu drawing");
            ui.add(Slider::new(&mut self.speed, 0.0..=4.0).text("Speed"));
            let rect = ui.allocate_rect(vec2(ui.available_width(), 300.0));
            let visuals = ui.style().visuals.clone();
            ui.painter()
                .rect(rect, 8.0, visuals.panel_fill, visuals.window_stroke);
            if let Some(gpu) = gpu {
                let for_prepare = Arc::clone(&gpu);
                let callback = CallbackFn::new(move |_info, pass| {
                    pass.set_pipeline(&gpu.pipeline);
                    pass.set_bind_group(0, &gpu.bind_group, &[]);
                    pass.draw(0..3, 0..1);
                })
                .prepare(move |_device, queue, _encoder, info| {
                    let aspect = info.viewport[2] / info.viewport[3];
                    let params = [angle, aspect, 0.0, 0.0];
                    let bytes: Vec<u8> = params.iter().flat_map(|f| f.to_le_bytes()).collect();
                    queue.write_buffer(&for_prepare.params, 0, &bytes);
                });
                ui.painter().add(callback.into_shape(rect.expand(-1.0)));
            }
            // Widgets added after the callback are drawn on top of it.
            ui.label("A label below the drawing");
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    rustroke::run(
        WindowOptions {
            title: "Rustroke — custom wgpu drawing".into(),
            inner_size: (640.0, 480.0),
            ..Default::default()
        },
        Demo {
            gpu: None,
            speed: 1.0,
            angle: 0.0,
        },
    )
}
