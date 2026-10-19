use crate::renderer::{Gpu, Renderer, VERTEX_BUFFER_LAYOUT};
use crate::shaders;
use crate::stage::camera::Camera;
use crate::stage::lights::Lights;
use crate::stage::scene::Scene;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const ALBEDO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
// 16-bit floats would round world positions to about 0.01 units at Sponza's scale.
const POSITION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

/// The window-sized textures the G-buffer pass writes and the fullscreen pass reads, plus the
/// bind group that points at them, since it has to be rebuilt along with them on resize.
struct GBuffer {
    albedo: wgpu::TextureView,
    normal: wgpu::TextureView,
    position: wgpu::TextureView,
    depth: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

impl GBuffer {
    fn new(gpu: &Gpu, layout: &wgpu::BindGroupLayout) -> Self {
        let albedo = create_window_sized_texture_view(gpu, "g-buffer albedo", ALBEDO_FORMAT);
        let normal = create_window_sized_texture_view(gpu, "g-buffer normal", NORMAL_FORMAT);
        let position = create_window_sized_texture_view(gpu, "g-buffer position", POSITION_FORMAT);
        let depth = create_window_sized_texture_view(gpu, "g-buffer depth", DEPTH_FORMAT);

        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("g-buffer bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&albedo),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&normal),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&position),
                },
            ],
        });

        Self {
            albedo,
            normal,
            position,
            depth,
            bind_group,
        }
    }
}

fn create_window_sized_texture_view(
    gpu: &Gpu,
    label: &str,
    format: wgpu::TextureFormat,
) -> wgpu::TextureView {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: gpu.surface_config.width,
            height: gpu.surface_config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn buffer_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    ty: wgpu::BufferBindingType,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// The fullscreen pass reads the G-buffer with `textureLoad` and no sampler, so the textures
/// don't need to be filterable (and `Rgba32Float` isn't).
fn g_buffer_texture_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn color_target(format: wgpu::TextureFormat) -> Option<wgpu::ColorTargetState> {
    Some(wgpu::ColorTargetState {
        format,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    })
}

fn cleared_color_attachment(
    view: &wgpu::TextureView,
) -> Option<wgpu::RenderPassColorAttachment<'_>> {
    Some(wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            store: wgpu::StoreOp::Store,
        },
    })
}

pub struct ClusteredDeferredRenderer {
    g_buffer_scene_bind_group: wgpu::BindGroup,
    g_buffer_pipeline: wgpu::RenderPipeline,

    fullscreen_scene_bind_group: wgpu::BindGroup,
    fullscreen_pipeline: wgpu::RenderPipeline,

    g_buffer_bind_group_layout: wgpu::BindGroupLayout,
    g_buffer: GBuffer,
}

impl ClusteredDeferredRenderer {
    pub fn new(gpu: &Gpu, camera: &Camera, lights: &Lights) -> Self {
        // G-buffer pass: only naive.vs.wgsl reads the scene group, for cameraUniforms.
        let g_buffer_scene_bind_group_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("clustered deferred g-buffer scene bind group layout"),
                    entries: &[buffer_layout_entry(
                        0,
                        wgpu::ShaderStages::VERTEX,
                        wgpu::BufferBindingType::Uniform,
                    )],
                });

        let g_buffer_scene_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("clustered deferred g-buffer scene bind group"),
            layout: &g_buffer_scene_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.uniforms_buffer.as_entire_binding(),
            }],
        });

        let g_buffer_pipeline =
            gpu.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("clustered deferred g-buffer pipeline"),
                    layout: Some(&gpu.device.create_pipeline_layout(
                        &wgpu::PipelineLayoutDescriptor {
                            label: Some("clustered deferred g-buffer pipeline layout"),
                            bind_group_layouts: &[
                                Some(&g_buffer_scene_bind_group_layout),
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
                                label: Some("clustered deferred naive vert shader"),
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
                                label: Some("clustered deferred g-buffer frag shader"),
                                source: wgpu::ShaderSource::Wgsl(
                                    shaders::CLUSTERED_DEFERRED_FRAG_SRC.as_str().into(),
                                ),
                            }),
                        entry_point: Some("main"),
                        compilation_options: Default::default(),
                        // same order as the @location()s in clustered_deferred.fs.wgsl
                        targets: &[
                            color_target(ALBEDO_FORMAT),
                            color_target(NORMAL_FORMAT),
                            color_target(POSITION_FORMAT),
                        ],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                });

        // Fullscreen pass: the same scene bindings as Forward+ (cluster_lighting.wgsl), plus the
        // G-buffer in its own group so a resize only rebuilds that one.
        let read_only_storage = wgpu::BufferBindingType::Storage { read_only: true };
        let fullscreen_scene_bind_group_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("clustered deferred fullscreen scene bind group layout"),
                    entries: &[
                        buffer_layout_entry(
                            0,
                            wgpu::ShaderStages::FRAGMENT,
                            wgpu::BufferBindingType::Uniform,
                        ),
                        buffer_layout_entry(1, wgpu::ShaderStages::FRAGMENT, read_only_storage),
                        buffer_layout_entry(2, wgpu::ShaderStages::FRAGMENT, read_only_storage),
                        buffer_layout_entry(3, wgpu::ShaderStages::FRAGMENT, read_only_storage),
                    ],
                });

        let fullscreen_scene_bind_group =
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("clustered deferred fullscreen scene bind group"),
                layout: &fullscreen_scene_bind_group_layout,
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

        let g_buffer_bind_group_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("g-buffer bind group layout"),
                    entries: &[
                        g_buffer_texture_layout_entry(0),
                        g_buffer_texture_layout_entry(1),
                        g_buffer_texture_layout_entry(2),
                    ],
                });

        let fullscreen_pipeline =
            gpu.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("clustered deferred fullscreen pipeline"),
                    layout: Some(&gpu.device.create_pipeline_layout(
                        &wgpu::PipelineLayoutDescriptor {
                            label: Some("clustered deferred fullscreen pipeline layout"),
                            // indices shaders::BIND_GROUP_SCENE and shaders::BIND_GROUP_GBUFFER
                            bind_group_layouts: &[
                                Some(&fullscreen_scene_bind_group_layout),
                                Some(&g_buffer_bind_group_layout),
                            ],
                            immediate_size: 0,
                        },
                    )),
                    depth_stencil: None,
                    vertex: wgpu::VertexState {
                        module: &gpu
                            .device
                            .create_shader_module(wgpu::ShaderModuleDescriptor {
                                label: Some("clustered deferred fullscreen vert shader"),
                                source: wgpu::ShaderSource::Wgsl(
                                    shaders::CLUSTERED_DEFERRED_FULLSCREEN_VERT_SRC
                                        .as_str()
                                        .into(),
                                ),
                            }),
                        entry_point: Some("main"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &gpu
                            .device
                            .create_shader_module(wgpu::ShaderModuleDescriptor {
                                label: Some("clustered deferred fullscreen frag shader"),
                                source: wgpu::ShaderSource::Wgsl(
                                    shaders::CLUSTERED_DEFERRED_FULLSCREEN_FRAG_SRC
                                        .as_str()
                                        .into(),
                                ),
                            }),
                        entry_point: Some("main"),
                        compilation_options: Default::default(),
                        targets: &[color_target(gpu.surface_config.format)],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                });

        let g_buffer = GBuffer::new(gpu, &g_buffer_bind_group_layout);

        Self {
            g_buffer_scene_bind_group,
            g_buffer_pipeline,
            fullscreen_scene_bind_group,
            fullscreen_pipeline,
            g_buffer_bind_group_layout,
            g_buffer,
        }
    }
}

impl Renderer for ClusteredDeferredRenderer {
    fn draw(
        &mut self,
        gpu: &Gpu,
        scene: &Scene,
        _camera: &Camera,
        lights: &Lights,
        canvas_texture_view: &wgpu::TextureView,
    ) {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clustered deferred command encoder"),
            });

        lights.do_light_clustering(&mut encoder);

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clustered deferred g-buffer pass"),
                color_attachments: &[
                    cleared_color_attachment(&self.g_buffer.albedo),
                    cleared_color_attachment(&self.g_buffer.normal),
                    cleared_color_attachment(&self.g_buffer.position),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.g_buffer.depth,
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
            render_pass.set_pipeline(&self.g_buffer_pipeline);

            render_pass.set_bind_group(
                shaders::BIND_GROUP_SCENE,
                &self.g_buffer_scene_bind_group,
                &[],
            );

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

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clustered deferred fullscreen pass"),
                color_attachments: &[cleared_color_attachment(canvas_texture_view)],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            render_pass.set_pipeline(&self.fullscreen_pipeline);
            render_pass.set_bind_group(
                shaders::BIND_GROUP_SCENE,
                &self.fullscreen_scene_bind_group,
                &[],
            );
            render_pass.set_bind_group(shaders::BIND_GROUP_GBUFFER, &self.g_buffer.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        gpu.queue.submit([encoder.finish()]);
    }

    fn on_resize(&mut self, gpu: &Gpu) {
        self.g_buffer = GBuffer::new(gpu, &self.g_buffer_bind_group_layout);
    }
}
