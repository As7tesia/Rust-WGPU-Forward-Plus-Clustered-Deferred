//! One renderer per render mode. Each implements the `Renderer` trait from renderer.rs.

pub mod clustered_deferred;
pub mod forward_plus;
pub mod naive;
