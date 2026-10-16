use crate::renderer::{Gpu, Renderer};
use crate::stage::camera::Camera;
use crate::stage::lights::Lights;
use crate::stage::scene::Scene;

// Parameter names starting with `_` tell the compiler they are unused on purpose. Drop the
// underscore when you start using one.

pub struct ForwardPlusRenderer {
    // TODO-2: add layouts, pipelines, textures, etc. needed for Forward+ here
    // you may need extra uniforms such as the camera view matrix and the canvas resolution
}

impl ForwardPlusRenderer {
    pub fn new(_gpu: &Gpu, _camera: &Camera, _lights: &Lights) -> Self {
        // TODO-2: initialize layouts, pipelines, textures, etc. needed for Forward+ here
        // (shader sources are in `crate::shaders`)

        Self {}
    }
}

impl Renderer for ForwardPlusRenderer {
    fn draw(
        &mut self,
        _gpu: &Gpu,
        _scene: &Scene,
        _camera: &Camera,
        _lights: &Lights,
        _canvas_texture_view: &wgpu::TextureView,
    ) {
        // TODO-2: run the Forward+ rendering pass:
        // - run the clustering compute shader
        // - run the main rendering pass, using the computed clusters for efficient lighting
    }

    // TODO-2: override `on_resize` to recreate any textures that match the window size
}
