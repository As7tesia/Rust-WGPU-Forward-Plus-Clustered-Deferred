//! Note that this glTF loader assumes a lot of things are always defined (textures, vertex and
//! index data, etc.), so you may run into issues loading files outside of the Sponza scene.
//!
//! In particular, it is known to not work if there is a mesh with no material.

use std::borrow::Cow;

use glam::Mat4;
use gltf::image::Format;
use gltf::texture::{MagFilter, MinFilter, WrappingMode};
use wgpu::util::DeviceExt;

use crate::renderer::{Gpu, Vertex};

/// A glTF texture: an image plus the sampler that reads it. Both are cheap handles (wgpu
/// objects are reference counted), so cloning one doesn't copy any GPU data.
#[derive(Clone)]
struct Texture {
    image: wgpu::Texture,
    sampler: wgpu::Sampler,
}

pub struct Material {
    pub material_bind_group: wgpu::BindGroup,
}

pub struct Primitive {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

/// A glTF node that has a mesh, flattened: its world transform lives in a uniform buffer that
/// the model bind group holds on to.
pub struct Node {
    pub model_bind_group: wgpu::BindGroup,
}

/// One draw call: a primitive, placed by a node, shaded with a material. The fields are indices
/// into the `Scene` vectors. Linking items by index instead of by reference is the usual Rust
/// way to keep a struct like `Scene` free of lifetime parameters.
#[derive(Clone, Copy, Debug)]
pub struct Draw {
    pub node: usize,
    pub material: usize,
    pub primitive: usize,
}

#[derive(Default)]
pub struct Scene {
    pub nodes: Vec<Node>,
    pub materials: Vec<Material>,
    pub primitives: Vec<Primitive>,
    // CHECKITOUT: renderers loop over this list to draw the scene; see NaiveRenderer::draw.
    // It is sorted by material so that consecutive draws can skip rebinding the same material.
    pub draws: Vec<Draw>,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads a binary glTF (.glb) file and adds its contents to the scene. Paths are relative
    /// to the crate root natively and to the page URL on the web.
    pub async fn load_gltf(&mut self, gpu: &Gpu, file_path: &str) {
        let bytes = load_file(file_path).await;
        // `import_slice` parses the file and decodes its images (JPEG and PNG) into pixels.
        let (document, buffers, images) = gltf::import_slice(&bytes)
            .unwrap_or_else(|err| panic!("failed to load glTF {file_path}: {err}"));

        let scene_textures: Vec<Texture> = {
            let scene_images: Vec<wgpu::Texture> = images
                .iter()
                .map(|image| create_texture(gpu, image))
                .collect();

            let scene_samplers: Vec<wgpu::Sampler> = document
                .samplers()
                .map(|gltf_sampler| create_sampler(gpu, &gltf_sampler))
                .collect();

            document
                .textures()
                .map(|gltf_texture| {
                    let gltf_sampler = gltf_texture.sampler();
                    let sampler = match gltf_sampler.index() {
                        Some(sampler_idx) => scene_samplers[sampler_idx].clone(),
                        // a texture without a sampler uses glTF's default one
                        None => create_sampler(gpu, &gltf_sampler),
                    };
                    Texture {
                        image: scene_images[gltf_texture.source().index()].clone(),
                        sampler,
                    }
                })
                .collect()
        };

        // Indices in this file start after whatever earlier load_gltf calls already added.
        let material_base_idx = self.materials.len();
        for gltf_material in document.materials() {
            let base_color_texture = gltf_material
                .pbr_metallic_roughness()
                .base_color_texture()
                .expect("material has no base color texture");
            let diffuse_texture = &scene_textures[base_color_texture.texture().index()];

            let diffuse_texture_view = diffuse_texture
                .image
                .create_view(&wgpu::TextureViewDescriptor::default());

            let material_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("material bind group"),
                layout: &gpu.material_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                    },
                ],
            });
            self.materials.push(Material {
                material_bind_group,
            });
        }

        // For each glTF mesh, its primitives as (primitive index, material index) pairs.
        let mut scene_meshes: Vec<Vec<(usize, usize)>> = Vec::new();
        for gltf_mesh in document.meshes() {
            let mut mesh = Vec::new();
            for gltf_prim in gltf_mesh.primitives() {
                let material_idx = gltf_prim
                    .material()
                    .index()
                    .expect("primitive has no material");
                mesh.push((self.primitives.len(), material_base_idx + material_idx));
                self.primitives
                    .push(create_primitive(gpu, &gltf_prim, &buffers));
            }
            scene_meshes.push(mesh);
        }

        // Nodes that aren't anyone's child are roots. Walk down from them, multiplying each
        // node's local transform onto its parent's.
        let mut is_child = vec![false; document.nodes().len()];
        for gltf_node in document.nodes() {
            for child in gltf_node.children() {
                is_child[child.index()] = true;
            }
        }
        let mut stack: Vec<(gltf::Node, Mat4)> = document
            .nodes()
            .filter(|gltf_node| !is_child[gltf_node.index()])
            .map(|gltf_node| (gltf_node, Mat4::IDENTITY))
            .collect();

        while let Some((gltf_node, parent_transform)) = stack.pop() {
            // `matrix()` handles both ways glTF stores a transform: a full matrix, or
            // translation * rotation * scale.
            let local_transform = Mat4::from_cols_array_2d(&gltf_node.transform().matrix());
            let transform = parent_transform * local_transform;

            if let Some(gltf_mesh) = gltf_node.mesh() {
                let model_mat_uniform_buffer =
                    gpu.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("model mat uniform"),
                            contents: bytemuck::bytes_of(&transform),
                            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        });

                let model_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("model bind group"),
                    layout: &gpu.model_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: model_mat_uniform_buffer.as_entire_binding(),
                    }],
                });

                let node_idx = self.nodes.len();
                self.nodes.push(Node { model_bind_group });

                for &(primitive_idx, material_idx) in &scene_meshes[gltf_mesh.index()] {
                    self.draws.push(Draw {
                        node: node_idx,
                        material: material_idx,
                        primitive: primitive_idx,
                    });
                }
            }

            for child in gltf_node.children() {
                stack.push((child, transform));
            }
        }

        self.draws.sort_by_key(|draw| (draw.material, draw.node));
    }
}

fn create_primitive(
    gpu: &Gpu,
    gltf_prim: &gltf::Primitive,
    buffers: &[gltf::buffer::Data],
) -> Primitive {
    // The reader decodes accessors into plain values whatever their storage type, e.g. u16 or
    // u32 indices both come out as u32 here.
    let reader = gltf_prim.reader(|buffer| Some(&buffers[buffer.index()]));

    let indices: Vec<u32> = reader
        .read_indices()
        .expect("primitive has no indices")
        .into_u32()
        .collect();

    let positions = reader.read_positions().expect("primitive has no positions");
    let normals = reader.read_normals().expect("primitive has no normals");
    let uvs = reader
        .read_tex_coords(0)
        .expect("primitive has no UVs")
        .into_f32();

    // Interleave the three attribute streams into one array of 32-byte vertices.
    let vertices: Vec<Vertex> = positions
        .zip(normals)
        .zip(uvs)
        .map(|((pos, nor), uv)| Vertex { pos, nor, uv })
        .collect();

    let index_buffer = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });

    let vertex_buffer = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

    Primitive {
        vertex_buffer,
        index_buffer,
        num_indices: indices.len() as u32,
    }
}

fn create_texture(gpu: &Gpu, image: &gltf::image::Data) -> wgpu::Texture {
    // The textures are RGBA8, so expand images with fewer channels. `Cow` holds either a
    // borrowed slice (RGBA input, no copy) or a newly built Vec.
    let rgba: Cow<[u8]> = match image.format {
        Format::R8G8B8A8 => Cow::Borrowed(&image.pixels),
        Format::R8G8B8 => Cow::Owned(
            // `as_chunks` views the bytes as `[u8; 3]` arrays (plus a leftover slice, unused)
            image
                .pixels
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|&[r, g, b]| [r, g, b, 255])
                .collect(),
        ),
        Format::R8G8 => Cow::Owned(
            image
                .pixels
                .as_chunks::<2>()
                .0
                .iter()
                .flat_map(|&[l, a]| [l, l, l, a])
                .collect(),
        ),
        Format::R8 => Cow::Owned(image.pixels.iter().flat_map(|&l| [l, l, l, 255]).collect()),
        other => panic!("unsupported image format: {other:?}"),
    };

    // `create_texture_with_data` creates the texture and queues the upload of its pixels.
    gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some("glTF image"),
            size: wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &rgba,
    )
}

fn convert_wrap_mode(wrap_mode: WrappingMode) -> wgpu::AddressMode {
    match wrap_mode {
        WrappingMode::Repeat => wgpu::AddressMode::Repeat,
        WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
    }
}

fn create_sampler(gpu: &Gpu, gltf_sampler: &gltf::texture::Sampler) -> wgpu::Sampler {
    // glTF leaves the filters up to the renderer when a sampler doesn't set them; use linear.
    let mag_filter = match gltf_sampler.mag_filter() {
        Some(MagFilter::Nearest) => wgpu::FilterMode::Nearest,
        Some(MagFilter::Linear) | None => wgpu::FilterMode::Linear,
    };

    let (min_filter, mipmap_filter) = match gltf_sampler.min_filter() {
        Some(MinFilter::Nearest) => (wgpu::FilterMode::Nearest, wgpu::MipmapFilterMode::Nearest),
        Some(MinFilter::Linear) | None => {
            (wgpu::FilterMode::Linear, wgpu::MipmapFilterMode::Nearest)
        }
        Some(MinFilter::NearestMipmapNearest) => {
            (wgpu::FilterMode::Nearest, wgpu::MipmapFilterMode::Nearest)
        }
        Some(MinFilter::LinearMipmapNearest) => {
            (wgpu::FilterMode::Linear, wgpu::MipmapFilterMode::Nearest)
        }
        Some(MinFilter::NearestMipmapLinear) => {
            (wgpu::FilterMode::Nearest, wgpu::MipmapFilterMode::Linear)
        }
        Some(MinFilter::LinearMipmapLinear) => {
            (wgpu::FilterMode::Linear, wgpu::MipmapFilterMode::Linear)
        }
    };

    gpu.device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("glTF sampler"),
        address_mode_u: convert_wrap_mode(gltf_sampler.wrap_s()),
        address_mode_v: convert_wrap_mode(gltf_sampler.wrap_t()),
        mag_filter,
        min_filter,
        mipmap_filter,
        ..Default::default()
    })
}

/// Reads a whole file. Natively the path is relative to the crate root (the folder with
/// Cargo.toml), whichever folder the program is started from.
#[cfg(not(target_arch = "wasm32"))]
async fn load_file(path: &str) -> Vec<u8> {
    // `env!` reads an environment variable at compile time; Cargo sets this one.
    let full_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read(&full_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", full_path.display()))
}

/// Reads a whole file. On the web the path is relative to the page URL; trunk copies the
/// assets/ folder next to index.html (see the `copy-dir` line in index.html).
#[cfg(target_arch = "wasm32")]
async fn load_file(path: &str) -> Vec<u8> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    // `JsFuture` turns a JavaScript Promise into a Rust future we can `.await`.
    let window = web_sys::window().expect("no window");
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str(path))
        .await
        .unwrap_or_else(|err| panic!("failed to fetch {path}: {err:?}"))
        .dyn_into()
        .expect("fetch did not return a Response");
    if !response.ok() {
        panic!("failed to fetch {path}: HTTP {}", response.status());
    }
    let array_buffer = JsFuture::from(response.array_buffer().expect("no response body"))
        .await
        .unwrap_or_else(|err| panic!("failed to read {path}: {err:?}"));
    js_sys::Uint8Array::new(&array_buffer).to_vec()
}
