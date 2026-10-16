use std::mem::{offset_of, size_of};
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use winit::window::Window;

use crate::stage::camera::Camera;
use crate::stage::lights::Lights;
use crate::stage::scene::Scene;

pub const FOV_Y_DEGREES: f32 = 45.0;

/// The GPU objects every part of the program needs: the device and queue, the window's surface
/// (the native equivalent of the canvas context), and the bind group layouts shared by all
/// renderers. In the TypeScript starter these were module-level globals in renderer.ts.
pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    /// Size and format of the surface. `surface_config.format` plays the role of the TS
    /// `canvasFormat`, and `surface_config.width`/`height` the canvas size.
    pub surface_config: wgpu::SurfaceConfiguration,

    pub model_bind_group_layout: wgpu::BindGroupLayout,
    pub material_bind_group_layout: wgpu::BindGroupLayout,
}

impl Gpu {
    // CHECKITOUT: this function initializes wgpu and also creates some bind group layouts shared by all the renderers
    pub async fn new(window: Arc<Window>) -> Self {
        // In the browser, wgpu could fall back to WebGL, which can't run this project's compute
        // shaders, so check for WebGPU first and show the same message as the TS starter.
        #[cfg(target_arch = "wasm32")]
        if !wgpu::util::is_browser_webgpu_supported().await {
            show_webgpu_unsupported_message();
            panic!("WebGPU not supported on this browser");
        }

        let size = window.inner_size();

        // `Backends::PRIMARY` is Vulkan, Metal, DX12 and browser WebGPU. Setting the
        // WGPU_BACKEND environment variable (e.g. `WGPU_BACKEND=dx12`) overrides it natively.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        // The surface keeps its own `Arc` to the window, which is why it can be `'static`.
        let surface = instance
            .create_surface(window)
            .expect("failed to create a surface for the window");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("no appropriate adapter found");

        // Check this line in the log to make sure the program runs on your main GPU and not an
        // integrated one.
        let adapter_info = adapter.get_info();
        log::info!(
            "adapter: {} ({:?})",
            adapter_info.name,
            adapter_info.backend
        );

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("device"),
                ..Default::default()
            })
            .await
            .expect("failed to create a device");

        // The TS starter's canvas format (`bgra8unorm` on most platforms) stores shader output
        // as-is. An sRGB format (`Bgra8UnormSrgb`) would gamma-encode it on write and the scene
        // would look brighter, so pick a non-sRGB one to match.
        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|format| {
                matches!(
                    format,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .expect("surface supports neither Bgra8Unorm nor Rgba8Unorm");

        let mut surface_config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("surface is not supported by the adapter");
        surface_config.format = format;
        // CHECKITOUT: `AutoVsync` caps the frame rate at the display's refresh rate, like
        // requestAnimationFrame in the browser. For performance measurements on desktop, use
        // `AutoNoVsync` so the frame time shows how long a frame really takes.
        surface_config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &surface_config);

        log::info!("wgpu init successful");

        let model_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("model bind group layout"),
                entries: &[
                    // modelMat
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("material bind group layout"),
                entries: &[
                    // diffuseTex
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // diffuseTexSampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        Self {
            device,
            queue,
            surface,
            surface_config,
            model_bind_group_layout,
            material_bind_group_layout,
        }
    }

    pub fn aspect_ratio(&self) -> f32 {
        self.surface_config.width as f32 / self.surface_config.height as f32
    }

    /// Reconfigures the surface for a new window size. Both dimensions must be nonzero.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }
}

#[cfg(target_arch = "wasm32")]
fn show_webgpu_unsupported_message() {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .expect("no document");
    if let Some(body) = document.body() {
        body.set_inner_html(
            "<h1 style=\"padding-left: 0.4em\">This browser doesn't support WebGPU! Try using Google Chrome.</h1>",
        );
    }
}

/// One vertex as the shaders see it: 32 bytes of interleaved position, normal and UV.
///
/// `#[repr(C)]` lays the fields out in declaration order like a C struct (plain Rust structs
/// may reorder fields). `Pod` and `Zeroable` let bytemuck reinterpret a `&[Vertex]` as
/// `&[u8]` for upload; deriving them fails to compile if the struct has hidden padding.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub nor: [f32; 3],
    pub uv: [f32; 2],
}

// Compile-time check that the struct is the size the vertex buffer layout below expects.
const _: () = assert!(size_of::<Vertex>() == 32);

pub const VERTEX_BUFFER_LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
    array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &[
        // pos
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: offset_of!(Vertex, pos) as wgpu::BufferAddress,
            shader_location: 0,
        },
        // nor
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: offset_of!(Vertex, nor) as wgpu::BufferAddress,
            shader_location: 1,
        },
        // uv
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: offset_of!(Vertex, uv) as wgpu::BufferAddress,
            shader_location: 2,
        },
    ],
};

/// What every render mode implements. `App` owns the scene, camera and lights and passes them
/// in each frame, so renderers don't hold references to them.
///
/// The TS `Renderer` base class also ran the frame loop (camera and light updates, timing);
/// that part is `App::render` in app.rs.
pub trait Renderer {
    /// Records and submits this renderer's passes, drawing the frame into `canvas_texture_view`.
    fn draw(
        &mut self,
        gpu: &Gpu,
        scene: &Scene,
        camera: &Camera,
        lights: &Lights,
        canvas_texture_view: &wgpu::TextureView,
    );

    /// Called after the window changes size. Recreate textures that match the window size
    /// (depth buffers, G-buffers) here. Does nothing unless a renderer overrides it.
    fn on_resize(&mut self, _gpu: &Gpu) {}
}
