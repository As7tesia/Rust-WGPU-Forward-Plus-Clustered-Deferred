//! Everything in the rendered world: the camera, the lights and the glTF scene.
//!
//! The TS starter also had a `Stage` class bundling these three for the renderers. In this
//! version `App` owns them and passes them to `Renderer::draw` each frame instead.

pub mod camera;
pub mod lights;
pub mod scene;
