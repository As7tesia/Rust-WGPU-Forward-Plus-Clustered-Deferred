use crate::renderer::{Gpu, Renderer, VERTEX_BUFFER_LAYOUT};
use crate::shaders;
use crate::stage::camera::Camera;
use crate::stage::lights::Lights;
use crate::stage::scene::Scene;

// Parameter names starting with `_` tell the compiler they are unused on purpose. Drop the
// underscore when you start using one.

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

pub struct ForwardPlusRenderer {
    // TODO-2: add layouts, pipelines, textures, etc. needed for Forward+ here
    // you may need extra uniforms such as the camera view matrix and the canvas resolution
    scene_uniforms_bind_group: wgpu::BindGroup,

    depth_texture_view: wgpu::TextureView,

    pipeline: wgpu::RenderPipeline,
}

impl ForwardPlusRenderer {
    pub fn new(gpu: &Gpu, camera: &Camera, lights: &Lights) -> Self {
        // TODO-2: initialize layouts, pipelines, textures, etc. needed for Forward+ here
        // (shader sources are in `crate::shaders`)
        let scene_uniforms_bind_group_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("forward+ scene uniforms bind group layout"),
                    entries: &[
                        // cameraUniforms
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // lightSet
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
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
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // clusterLightIndices
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

        let scene_uniforms_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("forward+ scene uniforms bind group"),
            layout: &scene_uniforms_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera.uniforms_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lights.light_set_storage_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: lights.cluster_counts_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: lights.cluster_light_indices_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline =
            gpu.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("forward+ pipeline"),
                    layout: Some(&gpu.device.create_pipeline_layout(
                        &wgpu::PipelineLayoutDescriptor {
                            label: Some("forward+ pipeline layout"),
                            bind_group_layouts: &[
                                Some(&scene_uniforms_bind_group_layout),
                                Some(&gpu.model_bind_group_layout),
                                Some(&gpu.material_bind_group_layout),
                            ],
                            immediate_size: 0,
                        },
                    )),
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: DEPTH_FORMAT,
                        depth_write_enabled: Some(true),
                        depth_compare: Some(wgpu::CompareFunction::Less),
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    vertex: wgpu::VertexState {
                        module: &gpu
                            .device
                            .create_shader_module(wgpu::ShaderModuleDescriptor {
                                label: Some("forward+ but still uses naive vert shader"),
                                source: wgpu::ShaderSource::Wgsl(
                                    shaders::NAIVE_VERT_SRC.as_str().into(),
                                ),
                            }),
                        entry_point: Some("main"),
                        compilation_options: Default::default(),
                        buffers: &[Some(VERTEX_BUFFER_LAYOUT)],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &gpu
                            .device
                            .create_shader_module(wgpu::ShaderModuleDescriptor {
                                label: Some("forward+ frag shader"),
                                source: wgpu::ShaderSource::Wgsl(
                                    shaders::FORWARD_PLUS_FRAG_SRC.as_str().into(),
                                ),
                            }),
                        entry_point: Some("main"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: gpu.surface_config.format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                });

        Self {
            scene_uniforms_bind_group,
            depth_texture_view: create_depth_texture_view(gpu),
            pipeline,
        }
    }
}

/// The depth texture has to match the window size, so it is created here and again on resize.
fn create_depth_texture_view(gpu: &Gpu) -> wgpu::TextureView {
    let depth_texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("forward+ depth texture"),
        size: wgpu::Extent3d {
            width: gpu.surface_config.width,
            height: gpu.surface_config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    // The view keeps its texture alive, so only the view needs to be stored.
    depth_texture.create_view(&wgpu::TextureViewDescriptor::default())
}

impl Renderer for ForwardPlusRenderer {
    fn draw(
        &mut self,
        gpu: &Gpu,
        scene: &Scene,
        _camera: &Camera,
        lights: &Lights, // in forward +, lights need to run the compute pass so can't be unused.
        canvas_texture_view: &wgpu::TextureView,
    ) {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("forward+ command encoder"),
            });

        // The pass borrows `encoder` until it is dropped at the end of this block, which ends
        // the pass (the TS `renderPass.end()`).
        {
            // call lights compuite pass
            lights.do_light_clustering(&mut encoder);

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("forward+ render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: canvas_texture_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            render_pass.set_pipeline(&self.pipeline);

            render_pass.set_bind_group(
                shaders::BIND_GROUP_SCENE,
                &self.scene_uniforms_bind_group,
                &[],
            );

            // Bind each node's model matrix and each material only when it changes from the
            // previous draw; `scene.draws` is sorted by material to make that rare.
            let mut current_node = None;
            let mut current_material = None;
            for draw in &scene.draws {
                if current_node != Some(draw.node) {
                    render_pass.set_bind_group(
                        shaders::BIND_GROUP_MODEL,
                        &scene.nodes[draw.node].model_bind_group,
                        &[],
                    );
                    current_node = Some(draw.node);
                }
                if current_material != Some(draw.material) {
                    render_pass.set_bind_group(
                        shaders::BIND_GROUP_MATERIAL,
                        &scene.materials[draw.material].material_bind_group,
                        &[],
                    );
                    current_material = Some(draw.material);
                }

                let primitive = &scene.primitives[draw.primitive];
                render_pass.set_vertex_buffer(0, primitive.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(primitive.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..primitive.num_indices, 0, 0..1);
            }
        }

        gpu.queue.submit([encoder.finish()]);
    }

    // TODO-2: override `on_resize` to recreate any textures that match the window size
    fn on_resize(&mut self, gpu: &Gpu) {
        self.depth_texture_view = create_depth_texture_view(gpu);
    }
}
