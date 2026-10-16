use std::sync::Arc;
use std::time::Duration;

use web_time::Instant;
use winit::event::{DeviceEvent, ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::PhysicalKey;
use winit::window::{CursorGrabMode, Window};

use crate::renderer::{Gpu, Renderer};
use crate::renderers::clustered_deferred::ClusteredDeferredRenderer;
use crate::renderers::forward_plus::ForwardPlusRenderer;
use crate::renderers::naive::NaiveRenderer;
use crate::stage::camera::Camera;
use crate::stage::lights::Lights;
use crate::stage::scene::Scene;

const SCENE_PATH: &str = "assets/sponza/Sponza.glb";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderMode {
    Naive,
    ForwardPlus,
    ClusteredDeferred,
}

impl RenderMode {
    const ALL: [RenderMode; 3] = [
        RenderMode::Naive,
        RenderMode::ForwardPlus,
        RenderMode::ClusteredDeferred,
    ];

    fn label(self) -> &'static str {
        match self {
            RenderMode::Naive => "naive",
            RenderMode::ForwardPlus => "forward+",
            RenderMode::ClusteredDeferred => "clustered deferred",
        }
    }
}

/// `Box<dyn Renderer>` is an owned pointer to any type that implements `Renderer`, similar to a
/// `std::unique_ptr<Renderer>` to an abstract base class in C++.
fn create_renderer(
    mode: RenderMode,
    gpu: &Gpu,
    camera: &Camera,
    lights: &Lights,
) -> Box<dyn Renderer> {
    match mode {
        RenderMode::Naive => Box::new(NaiveRenderer::new(gpu, camera, lights)),
        RenderMode::ForwardPlus => Box::new(ForwardPlusRenderer::new(gpu, camera, lights)),
        RenderMode::ClusteredDeferred => {
            Box::new(ClusteredDeferredRenderer::new(gpu, camera, lights))
        }
    }
}

/// Average frame time, updated twice a second (the TS starter used stats.js for this).
struct FrameTimer {
    window_start: Instant,
    frames: u32,
    average_ms: f32,
}

impl FrameTimer {
    const UPDATE_INTERVAL: Duration = Duration::from_millis(500);

    fn new(now: Instant) -> Self {
        Self {
            window_start: now,
            frames: 0,
            average_ms: 0.0,
        }
    }

    fn on_frame(&mut self, now: Instant) {
        self.frames += 1;
        let elapsed = now - self.window_start;
        if elapsed >= Self::UPDATE_INTERVAL {
            self.average_ms = elapsed.as_secs_f32() * 1000.0 / self.frames as f32;
            self.window_start = now;
            self.frames = 0;
        }
    }
}

/// The whole program: the GPU, the scene, camera and lights, the active renderer and the GUI.
/// `App::new` does what main.ts did at startup, and `App::render` is the per-frame loop of the
/// TS `Renderer` base class.
pub struct App {
    window: Arc<Window>,
    gpu: Gpu,

    scene: Scene,
    camera: Camera,
    lights: Lights,

    render_mode: RenderMode,
    renderer: Box<dyn Renderer>,

    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,

    /// True while a mouse button is held over the scene: the cursor is grabbed and mouse
    /// movement rotates the camera.
    mouse_look: bool,
    /// True while the window has zero size; no frames are drawn then.
    minimized: bool,

    start_time: Instant,
    prev_frame_time: Instant,
    frame_timer: FrameTimer,
}

impl App {
    pub async fn new(window: Arc<Window>) -> Self {
        // `Arc::clone` copies the pointer and bumps the reference count; the window itself is
        // shared between `App` and the GPU surface.
        let gpu = Gpu::new(Arc::clone(&window)).await;

        let mut scene = Scene::new();
        scene.load_gltf(&gpu, SCENE_PATH).await;

        let camera = Camera::new(&gpu);
        let lights = Lights::new(&gpu, &camera);

        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            Some(gpu.device.limits().max_texture_dimension_2d as usize),
        );
        let egui_renderer = egui_wgpu::Renderer::new(
            &gpu.device,
            gpu.surface_config.format,
            egui_wgpu::RendererOptions::default(),
        );

        let render_mode = RenderMode::Naive;
        let renderer = create_renderer(render_mode, &gpu, &camera, &lights);

        let now = Instant::now();
        window.request_redraw();

        Self {
            window,
            gpu,
            scene,
            camera,
            lights,
            render_mode,
            renderer,
            egui_ctx,
            egui_state,
            egui_renderer,
            mouse_look: false,
            minimized: false,
            start_time: now,
            prev_frame_time: now,
            frame_timer: FrameTimer::new(now),
        }
    }

    fn set_renderer(&mut self, mode: RenderMode) {
        self.render_mode = mode;
        // Assigning drops the previous renderer, which frees its GPU resources.
        self.renderer = create_renderer(mode, &self.gpu, &self.camera, &self.lights);
    }

    /// Matches the surface to the window's current size, in case it changed while `App` was
    /// still being created (on the web, setup is async and resize events can arrive meanwhile).
    pub fn resize_to_window(&mut self) {
        let size = self.window.inner_size();
        self.resize(size.width, size.height);
    }

    fn resize(&mut self, width: u32, height: u32) {
        // A minimized window has zero size, which a surface can't have.
        self.minimized = width == 0 || height == 0;
        if self.minimized {
            return;
        }

        self.gpu.resize(width, height);
        self.camera.set_aspect_ratio(self.gpu.aspect_ratio());
        self.renderer.on_resize(&self.gpu);
        self.window.request_redraw();
    }

    /// Starts or stops mouse look. The cursor is grabbed so it doesn't leave the window while
    /// dragging, like the browser's pointer lock in the TS starter.
    fn set_mouse_look(&mut self, enabled: bool) {
        if self.mouse_look == enabled {
            return;
        }
        self.mouse_look = enabled;

        if enabled {
            // `Locked` keeps the cursor in place, `Confined` keeps it inside the window. Windows
            // only supports `Confined` and macOS only `Locked`, so try one, then the other.
            let grab = self
                .window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| self.window.set_cursor_grab(CursorGrabMode::Confined));
            if let Err(err) = grab {
                log::warn!("could not grab the cursor: {err}");
            }
            self.window.set_cursor_visible(false);
        } else {
            if let Err(err) = self.window.set_cursor_grab(CursorGrabMode::None) {
                log::warn!("could not release the cursor: {err}");
            }
            self.window.set_cursor_visible(true);
        }
    }

    pub fn on_window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        // egui sees every event first and reports whether it used it (e.g. a click on the GUI).
        let egui_response = self.egui_state.on_window_event(&self.window, &event);

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.resize(size.width, size.height),
            WindowEvent::Focused(false) => {
                // reset keys when the window loses focus so they don't get stuck (e.g. on alt + tab)
                self.camera.clear_keys();
                self.set_mouse_look(false);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    let pressed = event.state == ElementState::Pressed;
                    // Releases always reach the camera, so a key can't stay stuck down.
                    if !pressed || !egui_response.consumed {
                        self.camera.on_key_event(key, pressed);
                    }
                }
            }
            WindowEvent::MouseInput { state, .. } => match state {
                ElementState::Pressed if !egui_response.consumed => self.set_mouse_look(true),
                ElementState::Released => self.set_mouse_look(false),
                _ => {}
            },
            WindowEvent::RedrawRequested => self.render(),
            _ => {}
        }
    }

    /// Device events carry raw mouse movement, which keeps coming while the cursor is grabbed.
    pub fn on_device_event(&mut self, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event
            && self.mouse_look
        {
            self.camera.on_mouse_move(dx as f32, dy as f32);
        }
    }

    // CHECKITOUT: this is the main rendering loop
    fn render(&mut self) {
        if self.minimized {
            return;
        }

        let now = Instant::now();
        let delta_time = (now - self.prev_frame_time).as_secs_f32() * 1000.0;
        let time = (now - self.start_time).as_secs_f32() * 1000.0;
        self.prev_frame_time = now;
        self.frame_timer.on_frame(now);

        self.camera.on_frame(delta_time, &self.gpu);
        self.lights.on_frame(time, &self.gpu);

        let frame = match self.gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.gpu
                    .surface
                    .configure(&self.gpu.device, &self.gpu.surface_config);
                self.window.request_redraw();
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                self.window.request_redraw();
                return;
            }
        };
        let canvas_texture_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let gui_output = self.run_gui();

        self.renderer.draw(
            &self.gpu,
            &self.scene,
            &self.camera,
            &self.lights,
            &canvas_texture_view,
        );

        self.paint_gui(gui_output, &canvas_texture_view);

        self.window.pre_present_notify();
        self.gpu.queue.present(frame);

        // ask for the next frame right away (the TS `requestAnimationFrame`)
        self.window.request_redraw();
    }

    /// Builds this frame's GUI and applies any changes made in it.
    fn run_gui(&mut self) -> egui::FullOutput {
        let raw_input = self.egui_state.take_egui_input(&self.window);

        // The GUI edits copies; changes are applied below, once egui is done with them.
        let mut num_lights = self.lights.num_lights;
        let mut render_mode = self.render_mode;
        let frame_time_ms = self.frame_timer.average_ms;

        let mut full_output = self.egui_ctx.run_pass(raw_input, |ctx| {
            egui::Area::new(egui::Id::unique("frame time"))
                .anchor(egui::Align2::LEFT_TOP, [8.0, 8.0])
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.monospace(format!("{frame_time_ms:6.2} ms"));
                    });
                });

            egui::Window::new("controls")
                .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
                .resizable(false)
                .show(ctx, |ui| {
                    ui.add(
                        egui::Slider::new(&mut num_lights, 1..=Lights::MAX_NUM_LIGHTS)
                            .text("numLights"),
                    );
                    egui::ComboBox::from_label("mode")
                        .selected_text(render_mode.label())
                        .show_ui(ui, |ui| {
                            for mode in RenderMode::ALL {
                                ui.selectable_value(&mut render_mode, mode, mode.label());
                            }
                        });
                });
        });

        if num_lights != self.lights.num_lights {
            self.lights.num_lights = num_lights;
            self.lights.update_light_set_uniform_num_lights(&self.gpu);
        }
        if render_mode != self.render_mode {
            self.set_renderer(render_mode);
        }

        // `std::mem::take` moves the value out and leaves an empty one in its place.
        let platform_output = std::mem::take(&mut full_output.platform_output);
        self.egui_state
            .handle_platform_output(&self.window, platform_output);
        full_output
    }

    /// Draws the GUI over the frame. egui_wgpu does the work: upload changed font and image
    /// textures, upload vertex data, then record a render pass that loads (keeps) the scene.
    fn paint_gui(&mut self, full_output: egui::FullOutput, view: &wgpu::TextureView) {
        let mut textures_delta = full_output.textures_delta;
        let paint_jobs = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [
                self.gpu.surface_config.width,
                self.gpu.surface_config.height,
            ],
            pixels_per_point: full_output.pixels_per_point,
        };

        for (id, image_deltas) in textures_delta.set.drain() {
            for image_delta in image_deltas {
                self.egui_renderer.update_texture(
                    &self.gpu.device,
                    &self.gpu.queue,
                    id,
                    &image_delta,
                );
            }
        }

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gui command encoder"),
            });
        let extra_command_buffers = self.egui_renderer.update_buffers(
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gui render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // egui_wgpu wants a pass not tied to the encoder's lifetime; `forget_lifetime`
            // moves that check from compile time to run time.
            self.egui_renderer.render(
                &mut render_pass.forget_lifetime(),
                &paint_jobs,
                &screen_descriptor,
            );
        }

        self.gpu
            .queue
            .submit(extra_command_buffers.into_iter().chain([encoder.finish()]));

        for id in textures_delta.free.drain() {
            self.egui_renderer.free_texture(&id);
        }
    }
}
