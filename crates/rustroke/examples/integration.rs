//! rustroke inside an application that owns its event loop, window and
//! wgpu device. The app draws its own "scene" (a background whose color
//! follows the mouse), and rustroke draws a side panel over it. Clicks on
//! the panel go to the UI; clicks on the scene go to the app.
//!
//! Run with: `cargo run -p rustroke --example integration`

use std::sync::Arc;

use rustroke::winit::application::ApplicationHandler;
use rustroke::winit::event::{ElementState, WindowEvent};
use rustroke::winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use rustroke::winit::window::{Window, WindowId};
use rustroke::{Integration, Panel, PhysicalSize, Slider, wgpu};

struct Gpu {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

struct Host {
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    ui: Option<Integration>,
    /// The app's own state, changed by the UI and by the scene.
    brightness: f32,
    hue: f64,
    scene_clicks: u32,
}

impl Host {
    fn init(&mut self, event_loop: &ActiveEventLoop) {
        let attributes = Window::default_attributes()
            .with_title("Rustroke — integration")
            .with_inner_size(rustroke::winit::dpi::LogicalSize::new(720.0, 460.0));
        let window = Arc::new(event_loop.create_window(attributes).expect("window"));
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                Box::new(Arc::clone(&window)),
            ));
        let surface = instance
            .create_surface(Arc::clone(&window))
            .expect("surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("device");
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("surface config");
        // An sRGB format, so the UI's colors are right.
        let caps = surface.get_capabilities(&adapter);
        if let Some(&srgb) = caps.formats.iter().find(|f| f.is_srgb()) {
            config.format = srgb;
        }
        surface.configure(&device, &config);
        self.ui = Some(Integration::new(&device, config.format));
        self.gpu = Some(Gpu {
            surface,
            config,
            device,
            queue,
        });
        self.window = Some(window);
    }

    fn redraw(&mut self) {
        let (Some(window), Some(gpu), Some(ui)) = (&self.window, &self.gpu, &mut self.ui) else {
            return;
        };
        let frame = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            _ => {
                gpu.surface.configure(&gpu.device, &gpu.config);
                window.request_redraw();
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu.device.create_command_encoder(&Default::default());

        // 1. The app's own rendering: here just a clear color.
        let (r, g, b) = hsv(self.hue, 0.5, f64::from(self.brightness));
        drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        }));

        // 2. The UI, on top.
        let (brightness, clicks) = (&mut self.brightness, self.scene_clicks);
        let out = ui.run(window, |frame| {
            Panel::left("controls")
                .default_size(260.0)
                .show(frame, |ui| {
                    ui.heading("Scene");
                    ui.add(Slider::new(brightness, 0.0..=1.0).text("Brightness"));
                    ui.label(format!("Clicks on the scene: {clicks}"));
                    ui.label("Move the mouse over the scene to change its hue.");
                });
        });
        let size = PhysicalSize::new(gpu.config.width, gpu.config.height);
        ui.paint(&gpu.device, &gpu.queue, &mut encoder, &view, size);
        gpu.queue.submit([encoder.finish()]);
        window.pre_present_notify();
        gpu.queue.present(frame);
        if out.repaint {
            window.request_redraw();
        }
    }
}

impl ApplicationHandler for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            self.init(event_loop);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let (Some(window), Some(ui)) = (&self.window, &mut self.ui) else {
            return;
        };
        let response = ui.on_window_event(window, &event);
        if response.repaint {
            window.request_redraw();
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width = size.width.max(1);
                    gpu.config.height = size.height.max(1);
                    gpu.surface.configure(&gpu.device, &gpu.config);
                }
            }
            // The scene only reacts to what the UI didn't take.
            WindowEvent::CursorMoved { position, .. } if !response.consumed => {
                self.hue = position.x / f64::from(window.inner_size().width.max(1)) * 360.0;
                window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            } if !response.consumed => {
                self.scene_clicks += 1;
                window.request_redraw();
            }
            _ => {}
        }
    }
}

/// A color from hue (degrees), saturation and value (0..1).
fn hsv(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let (r, g, b) = match (h / 60.0) as u32 % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    (r + m, g + m, b + m)
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut host = Host {
        window: None,
        gpu: None,
        ui: None,
        brightness: 0.35,
        hue: 220.0,
        scene_clicks: 0,
    };
    event_loop.run_app(&mut host).expect("event loop");
}
