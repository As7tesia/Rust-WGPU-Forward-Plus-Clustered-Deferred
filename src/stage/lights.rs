use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use crate::math_util::Rng;
use crate::renderer::Gpu;
use crate::shaders;
use crate::stage::camera::Camera;

/// h in [0, 1]
fn hue_to_rgb(h: f32) -> Vec3 {
    let f = |n: f32| {
        let k = (n + h * 6.0) % 6.0;
        1.0 - k.min(4.0 - k).clamp(0.0, 1.0)
    };
    Vec3::ONE.lerp(Vec3::new(f(5.0), f(3.0), f(1.0)), 0.8)
}

/// One light as laid out in the light set storage buffer, matching `Light` in common.wgsl.
/// A WGSL vec3f is aligned to 16 bytes, so each vec3 is followed by 4 bytes of padding.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Light {
    pub pos: [f32; 3],
    _padding0: f32,
    pub color: [f32; 3],
    _padding1: f32,
}

const _: () = assert!(size_of::<Light>() == Lights::NUM_FLOATS_PER_LIGHT * 4);

/// Bytes before the light array in the light set buffer: `numLights` (a u32) plus padding, since
/// the array of 16-byte-aligned `Light`s starts at the next multiple of 16.
const LIGHT_SET_HEADER_SIZE: wgpu::BufferAddress = 16;

/// Seed for the light colors.
const LIGHT_COLOR_SEED: u64 = 5650;

pub struct Lights {
    pub num_lights: u32,

    pub light_set_storage_buffer: wgpu::Buffer,

    pub time_uniform_buffer: wgpu::Buffer,
    
    /// One u32 light count per cluster, zeroed before every clustering pass.
    pub cluster_counts_buffer: wgpu::Buffer,
    /// `MAX_LIGHTS_PER_CLUSTER` light index slots per cluster.
    pub cluster_light_indices_buffer: wgpu::Buffer,

    move_lights_compute_bind_group: wgpu::BindGroup,
    move_lights_compute_pipeline: wgpu::ComputePipeline,

    clustering_compute_pipeline: wgpu::ComputePipeline,
    
    clustering_compute_bind_group: wgpu::BindGroup,
    // TODO-2: add layouts, pipelines, textures, etc. needed for light clustering here
}

impl Lights {
    pub const MAX_NUM_LIGHTS: u32 = 5000;
    pub const NUM_FLOATS_PER_LIGHT: usize = 8; // vec3f is aligned at 16 byte boundaries

    pub const LIGHT_INTENSITY: f32 = 0.1;

    // The TS version stored the camera for use in light clustering. Here, take what clustering
    // needs from `camera` (e.g. `&camera.uniforms_buffer` for a bind group) while building the
    // pipelines below, and drop the leading underscore once you use it: `_camera` only tells
    // the compiler the parameter is unused on purpose for now.
    pub fn new(gpu: &Gpu, camera: &Camera) -> Self {
        let light_set_storage_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lights"),
            size: LIGHT_SET_HEADER_SIZE
                + Self::MAX_NUM_LIGHTS as wgpu::BufferAddress
                    * size_of::<Light>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let time_uniform_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("time uniform"),
            size: size_of::<f32>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let move_lights_compute_bind_group_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("move lights compute bind group layout"),
                    entries: &[
                        // lightSet
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // time
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

        let move_lights_compute_bind_group =
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("move lights compute bind group"),
                layout: &move_lights_compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: light_set_storage_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: time_uniform_buffer.as_entire_binding(),
                    },
                ],
            });

        let move_lights_compute_pipeline =
            gpu.device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("move lights compute pipeline"),
                    layout: Some(&gpu.device.create_pipeline_layout(
                        &wgpu::PipelineLayoutDescriptor {
                            label: Some("move lights compute pipeline layout"),
                            bind_group_layouts: &[Some(&move_lights_compute_bind_group_layout)],
                            immediate_size: 0,
                        },
                    )),
                    module: &gpu
                        .device
                        .create_shader_module(wgpu::ShaderModuleDescriptor {
                            label: Some("move lights compute shader"),
                            source: wgpu::ShaderSource::Wgsl(
                                shaders::MOVE_LIGHTS_COMPUTE_SRC.as_str().into(),
                            ),
                        }),
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                });

        // TODO-2: initialize layouts, pipelines, textures, etc. needed for light clustering here
        
        let cluster_counts_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cluster counts"),
            size: shaders::CLUSTER_COUNTS_SIZE,
            // COPY_DST lets `clear_buffer` zero the counts before each scatter pass
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Not cleared: slots past a cluster's count are never read.
        let cluster_light_indices_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cluster light indices"),
            size: shaders::CLUSTER_LIGHT_INDICES_SIZE,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let clustering_compute_bind_group_layout = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("clustering compute bind group layout"),
            entries: &[
                // cameraUniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, 
                        has_dynamic_offset: false, 
                        min_binding_size: None, 
                    },
                    count: None,
                },

                // lightSet
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },

                // clusterCounts
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },

                // clusterLightIndices
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let clustering_compute_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("clustering compute bind group"),
            layout: &clustering_compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera.uniforms_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_set_storage_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: cluster_counts_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: cluster_light_indices_buffer.as_entire_binding(),
                },
            ]
        });

        let clustering_compute_pipeline =
            gpu.device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("clustering compute pipeline"),
                    layout: Some(&gpu.device.create_pipeline_layout(
                        &wgpu::PipelineLayoutDescriptor {
                            label: Some("clustering compute pipeline layout"),
                            bind_group_layouts: &[Some(&clustering_compute_bind_group_layout)],
                            immediate_size: 0,
                        },
                    )),
                    module: &gpu
                        .device
                        .create_shader_module(wgpu::ShaderModuleDescriptor {
                            label: Some("clustering compute shader"),
                            source: wgpu::ShaderSource::Wgsl(
                                shaders::CLUSTERING_COMPUTE_SRC.as_str().into(),
                            ),
                        }),
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                });

        let lights = Self {
            num_lights: 500,
            light_set_storage_buffer,
            time_uniform_buffer,
            cluster_counts_buffer,
            cluster_light_indices_buffer,
            move_lights_compute_bind_group,
            move_lights_compute_pipeline,
            clustering_compute_bind_group,
            clustering_compute_pipeline,
        };
        lights.populate_lights_buffer(gpu);
        lights.update_light_set_uniform_num_lights(gpu);
        lights
    }

    fn populate_lights_buffer(&self, gpu: &Gpu) {
        let mut rng = Rng::new(LIGHT_COLOR_SEED);
        let lights_array: Vec<Light> = (0..Self::MAX_NUM_LIGHTS)
            .map(|_| {
                // light pos is set by compute shader so no need to set it here
                let light_color = hue_to_rgb(rng.next_f32()) * Self::LIGHT_INTENSITY;
                Light {
                    color: light_color.to_array(),
                    ..Light::zeroed()
                }
            })
            .collect();

        // `cast_slice` reinterprets `&[Light]` as `&[u8]` without copying, which only compiles
        // because `Light` is `Pod`.
        gpu.queue.write_buffer(
            &self.light_set_storage_buffer,
            LIGHT_SET_HEADER_SIZE,
            bytemuck::cast_slice(&lights_array),
        );
    }

    pub fn update_light_set_uniform_num_lights(&self, gpu: &Gpu) {
        gpu.queue.write_buffer(
            &self.light_set_storage_buffer,
            0,
            bytemuck::bytes_of(&self.num_lights),
        );
    }

    pub fn do_light_clustering(&self, encoder: &mut wgpu::CommandEncoder) {
        // TODO-2: run the light clustering compute pass(es) here
        // implementing clustering here allows for reusing the code in both Forward+ and Clustered Deferred

        // Only the counts need zeroing
        encoder.clear_buffer(&self.cluster_counts_buffer, 0, None);
        // not using same encoder as render pass so this doesn't interfere with measuring actual rendering performance
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("clustering compute pass"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&self.clustering_compute_pipeline);

        compute_pass.set_bind_group(0, &self.clustering_compute_bind_group, &[]);

        let workgroup_count = self
        .num_lights
        .div_ceil(shaders::CLUSTERING_WORKGROUP_SIZE);
        compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
    }

    // CHECKITOUT: this is where the light movement compute shader is dispatched from the host
    /// `time` is in milliseconds since the program started.
    pub fn on_frame(&self, time: f32, gpu: &Gpu) {
        gpu.queue
            .write_buffer(&self.time_uniform_buffer, 0, bytemuck::bytes_of(&time));

        // not using same encoder as render pass so this doesn't interfere with measuring actual rendering performance
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("move lights command encoder"),
            });

        // A pass borrows the encoder mutably until the pass is dropped, which also ends it (the
        // TS `computePass.end()`). The inner block drops it before `encoder.finish()`.
        {   
            // move lights compute pass
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("move lights compute pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.move_lights_compute_pipeline);

            compute_pass.set_bind_group(0, &self.move_lights_compute_bind_group, &[]);

            let workgroup_count = self
                .num_lights
                .div_ceil(shaders::MOVE_LIGHTS_WORKGROUP_SIZE);
            compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
        }

        self.do_light_clustering(&mut encoder);

        gpu.queue.submit([encoder.finish()]);
    }
}
