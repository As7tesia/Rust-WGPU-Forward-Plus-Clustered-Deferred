use crate::renderer::{Gpu, Renderer};
use crate::stage::camera::Camera;
use crate::stage::lights::Lights;
use crate::stage::scene::Scene;

// Parameter names starting with `_` tell the compiler they are unused on purpose. Drop the
// underscore when you start using one.

pub struct ClusteredDeferredRenderer {
    // TODO-3: add layouts, pipelines, textures, etc. needed for Clustered Deferred here
    // you may need extra uniforms such as the camera view matrix and the canvas resolution
}

impl ClusteredDeferredRenderer {
    pub fn new(_gpu: &Gpu, _camera: &Camera, _lights: &Lights) -> Self {
        // TODO-3: initialize layouts, pipelines, textures, etc. needed for Clustered Deferred here
        // you'll need two pipelines: one for the G-buffer pass and one for the fullscreen pass
        // (shader sources are in `crate::shaders`)

        Self {}
    }
}

impl Renderer for ClusteredDeferredRenderer {
    fn draw(
        &mut self,
        _gpu: &Gpu,
        _scene: &Scene,
        _camera: &Camera,
        _lights: &Lights,
        _canvas_texture_view: &wgpu::TextureView,
    ) {
        // TODO-3: run the Clustered Deferred rendering pass:
        // - run the clustering compute shader
        // - run the G-buffer pass, outputting position, albedo, and normals
        // - run the fullscreen pass, which reads from the G-buffer and performs lighting calculations
    }

    // TODO-3: override `on_resize` to recreate the G-buffer and any other textures that match
    // the window size
}
