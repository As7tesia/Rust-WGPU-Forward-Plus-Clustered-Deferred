use std::collections::HashSet;
use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use glam::camera::rh::{proj, view};
use glam::{Mat4, Vec3};
use winit::keyboard::KeyCode;

use crate::renderer::{FOV_Y_DEGREES, Gpu};

/// Host-side copy of the camera uniform buffer.
///
/// `#[repr(C)]` keeps the fields in declaration order so the bytes line up with the WGSL
/// `CameraUniforms` struct in common.wgsl, and `Pod` lets `bytemuck::bytes_of` view the whole
/// struct as the `&[u8]` that `queue.write_buffer()` takes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CameraUniforms {
    pub view_proj_mat: Mat4,
    // TODO-2: add extra fields needed for light clustering here. Follow WGSL's alignment rules
    // (e.g. a vec3f is 16-byte aligned) and add explicit padding fields where WGSL would pad,
    // since `Pod` refuses structs with hidden padding.
}

// Compile-time check that the struct has the size the shader expects. Update it when you add
// fields.
const _: () = assert!(size_of::<CameraUniforms>() == 16 * 4);

impl CameraUniforms {
    pub fn set_view_proj_mat(&mut self, mat: Mat4) {
        // TODO-1.1: set `self.view_proj_mat` to the input `mat`
    }

    // TODO-2: add extra functions to set values needed for light clustering here
}

pub struct Camera {
    pub uniforms: CameraUniforms,
    pub uniforms_buffer: wgpu::Buffer,

    pub proj_mat: Mat4,
    pub camera_pos: Vec3,
    pub camera_front: Vec3,
    pub camera_up: Vec3,
    pub camera_right: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub move_speed: f32,
    pub sensitivity: f32,

    /// Keys currently held down, by physical position on the keyboard.
    keys: HashSet<KeyCode>,
}

impl Camera {
    pub const NEAR_PLANE: f32 = 0.1;
    pub const FAR_PLANE: f32 = 1000.0;

    pub fn new(gpu: &Gpu) -> Self {
        // TODO-1.1: set `uniforms_buffer` to a new buffer of size `size_of::<CameraUniforms>()`
        // make sure the usage is `wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST`
        // since we will be copying to this buffer
        // check lights.rs for examples of using `gpu.device.create_buffer()`
        //
        // note that you can add more fields (e.g. inverse proj matrix) to `CameraUniforms` in
        // later parts of the assignment
        //
        // `todo!()` stops the program with this message when it runs. It also makes the rest
        // of this function unreachable, so the compiler warns about that until you replace it.
        let uniforms_buffer: wgpu::Buffer = todo!(
            "TODO-1.1: create the camera uniforms buffer in Camera::new (src/stage/camera.rs)"
        );

        let mut camera = Self {
            uniforms: CameraUniforms::zeroed(),
            uniforms_buffer,
            proj_mat: Self::proj_mat_for(gpu.aspect_ratio()),
            camera_pos: Vec3::new(-7.0, 2.0, 0.0),
            camera_front: Vec3::new(0.0, 0.0, -1.0),
            camera_up: Vec3::new(0.0, 1.0, 0.0),
            camera_right: Vec3::new(1.0, 0.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            move_speed: 0.004,
            sensitivity: 0.15,
            keys: HashSet::new(),
        };

        camera.rotate_camera(0.0, 0.0); // set initial camera vectors
        camera
    }

    /// WebGPU's clip space is the same as Direct3D's (depth in [0, 1], y up), hence
    /// `proj::directx`. `rh` is a right-handed view space looking down -z, as with the TS
    /// starter's `mat4.perspective` and `mat4.lookAt`. (`proj::opengl` maps depth to [-1, 1].)
    fn proj_mat_for(aspect_ratio: f32) -> Mat4 {
        proj::directx::perspective(
            FOV_Y_DEGREES.to_radians(),
            aspect_ratio,
            Self::NEAR_PLANE,
            Self::FAR_PLANE,
        )
    }

    /// Called when the window changes size. The TS starter's canvas never changed size, so it
    /// computed the projection only once.
    pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
        self.proj_mat = Self::proj_mat_for(aspect_ratio);
    }

    pub fn on_key_event(&mut self, key: KeyCode, pressed: bool) {
        if pressed {
            self.keys.insert(key);
        } else {
            self.keys.remove(&key);
        }
    }

    /// Releases all keys so they don't get stuck when the window loses focus (e.g. on alt + tab).
    pub fn clear_keys(&mut self) {
        self.keys.clear();
    }

    fn is_key_down(&self, key: KeyCode) -> bool {
        self.keys.contains(&key)
    }

    fn rotate_camera(&mut self, dx: f32, dy: f32) {
        self.yaw += dx;
        self.pitch -= dy;

        self.pitch = self.pitch.clamp(-89.0, 89.0);

        let (yaw, pitch) = (self.yaw.to_radians(), self.pitch.to_radians());
        let front = Vec3::new(
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos(),
        );

        self.camera_front = front.normalize();
        self.camera_right = self.camera_front.cross(Vec3::Y).normalize();
        self.camera_up = self.camera_right.cross(self.camera_front).normalize();
    }

    /// Mouse movement in pixels. `App` only calls this while a mouse button is held.
    pub fn on_mouse_move(&mut self, dx: f32, dy: f32) {
        self.rotate_camera(dx * self.sensitivity, dy * self.sensitivity);
    }

    fn process_input(&mut self, delta_time: f32) {
        let mut move_dir = Vec3::ZERO;
        if self.is_key_down(KeyCode::KeyW) {
            move_dir += self.camera_front;
        }
        if self.is_key_down(KeyCode::KeyS) {
            move_dir -= self.camera_front;
        }
        if self.is_key_down(KeyCode::KeyA) {
            move_dir -= self.camera_right;
        }
        if self.is_key_down(KeyCode::KeyD) {
            move_dir += self.camera_right;
        }
        if self.is_key_down(KeyCode::KeyQ) {
            move_dir -= self.camera_up;
        }
        if self.is_key_down(KeyCode::KeyE) {
            move_dir += self.camera_up;
        }

        let mut move_speed = self.move_speed * delta_time;
        const MOVE_SPEED_MULTIPLIER: f32 = 3.0;
        if self.is_key_down(KeyCode::ShiftLeft) || self.is_key_down(KeyCode::ShiftRight) {
            move_speed *= MOVE_SPEED_MULTIPLIER;
        }
        if self.is_key_down(KeyCode::AltLeft) || self.is_key_down(KeyCode::AltRight) {
            move_speed /= MOVE_SPEED_MULTIPLIER;
        }

        if move_dir.length() > 0.0 {
            self.camera_pos += move_dir.normalize() * move_speed;
        }
    }

    /// `delta_time` is in milliseconds.
    pub fn on_frame(&mut self, delta_time: f32, gpu: &Gpu) {
        self.process_input(delta_time);

        let look_pos = self.camera_pos + self.camera_front * 1.0;
        let view_mat = view::look_at_mat4(self.camera_pos, look_pos, Vec3::Y);
        let view_proj_mat = self.proj_mat * view_mat;
        // TODO-1.1: set `self.uniforms`'s view proj mat to the newly calculated `view_proj_mat`

        // TODO-2: write to extra buffers needed for light clustering here

        // TODO-1.1: upload `self.uniforms` (host side) to `self.uniforms_buffer` (device side)
        // check lights.rs for examples of using `gpu.queue.write_buffer()`
    }
}
