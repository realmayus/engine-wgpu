use base64::{engine::general_purpose, Engine as _};
use glam::{Mat4, Vec2, Vec3};
use gltf::buffer::Data;
use gltf::image::Source;
use gltf::image::Source::View;
use gltf::{Error, Node};
use image::ImageFormat::{Jpeg, Png};
use image::{guess_format, DynamicImage};
use lib::scene::{Material, Mesh, Model, Scene, Texture};
use lib::util::map_gltf_format_to_vulkano;
use lib::util::shader_types::{MaterialUniform, MeshRenderSettingsUniform};
use lib::util::texture::create_texture;
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Index;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::{fs, io};
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::format::Format;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};
use vulkano::memory::MemoryAllocateInfo;
use vulkano::sync::Sharing;
// TODO make independent of vulkano lib

fn read_to_end<P>(path: P) -> gltf::Result<Vec<u8>>
where
    P: AsRef<Path>,
{
    use io::Read;
    let file = fs::File::open(path.as_ref()).map_err(Error::Io)?;
    // Allocate one extra byte so the buffer doesn't need to grow before the
    // final `read` call at the end of the file.  Don't worry about `usize`
    // overflow because reading will fail regardless in that case.
    let length = file.metadata().map(|x| x.len() + 1).unwrap_or(0);
    let mut reader = io::BufReader::new(file);
    let mut data = Vec::with_capacity(length as usize);
    reader.read_to_end(&mut data).map_err(Error::Io)?;
    Ok(data)
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Scheme<'a> {
    /// `data:[<media type>];base64,<data>`.
    Data(Option<&'a str>, &'a str),

    /// `file:[//]<absolute file path>`.
    ///
    /// Note: The file scheme does not implement authority.
    File(&'a str),

    /// `../foo`, etc.
    Relative(Cow<'a, str>),

    /// Placeholder for an unsupported URI scheme identifier.
    Unsupported,
}

impl<'a> Scheme<'a> {
    fn parse(uri: &str) -> Scheme<'_> {
        if uri.contains(':') {
            if let Some(rest) = uri.strip_prefix("data:") {
                let mut it = rest.split(";base64,");

                match (it.next(), it.next()) {
                    (match0_opt, Some(match1)) => Scheme::Data(match0_opt, match1),
                    (Some(match0), _) => Scheme::Data(None, match0),
                    _ => Scheme::Unsupported,
                }
            } else if let Some(rest) = uri.strip_prefix("file://") {
                Scheme::File(rest)
            } else if let Some(rest) = uri.strip_prefix("file:") {
                Scheme::File(rest)
            } else {
                Scheme::Unsupported
            }
        } else {
            Scheme::Relative(urlencoding::decode(uri).unwrap())
        }
    }

    fn read(base: Option<&Path>, uri: &str) -> Vec<u8> {
        match Scheme::parse(uri) {
            // The path may be unused in the Scheme::Data case
            // Example: "uri" : "data:application/octet-stream;base64,wsVHPgA...."
            Scheme::Data(_, base64) => general_purpose::STANDARD
                .decode(base64)
                .expect("Couldn't read b64"),
            Scheme::File(path) if base.is_some() => {
                read_to_end(path).expect("Couldn't read file at path")
            }
            Scheme::Relative(path) if base.is_some() => read_to_end(base.unwrap().join(&*path))
                .expect("Couldn't read image from relative path"),
            Scheme::Unsupported => panic!("Unsupported scheme."),
            _ => panic!("External references aren't supported."),
        }
    }
}

fn load_image(
    source: Source<'_>,
    base: Option<&Path>,
    buffer_data: &[Data],
) -> (Vec<u8>, u32, u32, Format) {
    let decoded_image = match source {
        Source::Uri { uri, mime_type } if base.is_some() => match Scheme::parse(uri) {
            Scheme::Data(Some(annoying_case), base64) => {
                let encoded_image = general_purpose::STANDARD
                    .decode(base64)
                    .expect("Couldn't parse b64");

                image::load_from_memory(&encoded_image).expect("Couldn't load image")
            }
            Scheme::Unsupported => panic!("Unsupported scheme."),
            _ => {
                let encoded_image = Scheme::read(base, uri);
                image::load_from_memory(&encoded_image).expect("Couldn't load image")
            }
        },
        View { view, mime_type } => {
            let parent_buffer_data = &buffer_data[view.buffer().index()].0;
            let begin = view.offset();
            let end = begin + view.length();
            let encoded_image = &parent_buffer_data[begin..end];
            image::load_from_memory(encoded_image).expect("Couldn't load image")
        }
        _ => return panic!("External references are unsupported."),
    };

    let width = decoded_image.width();
    let height = decoded_image.height();

    match decoded_image {
        DynamicImage::ImageLuma8(_) => (
            decoded_image.into_bytes(),
            width,
            height,
            vulkano::format::Format::R8_UNORM,
        ),
        DynamicImage::ImageLumaA8(_) => (
            decoded_image.into_bytes(),
            width,
            height,
            vulkano::format::Format::R8G8_UNORM,
        ),
        DynamicImage::ImageRgb8(_) => (
            DynamicImage::from(decoded_image.to_rgba8()).into_bytes(),
            decoded_image.width(),
            decoded_image.height(),
            vulkano::format::Format::R8G8B8A8_SRGB,
        ),
        DynamicImage::ImageRgba8(_) => (
            decoded_image.into_bytes(),
            width,
            height,
            vulkano::format::Format::R8G8B8A8_SRGB,
        ),
        DynamicImage::ImageLuma16(_) => (
            decoded_image.into_bytes(),
            width,
            height,
            vulkano::format::Format::R16_UINT,
        ),
        DynamicImage::ImageLumaA16(_) => (
            decoded_image.into_bytes(),
            width,
            height,
            vulkano::format::Format::R16G16_UINT,
        ),
        DynamicImage::ImageRgb16(_) => (
            DynamicImage::from(decoded_image.to_rgba16()).into_bytes(),
            width,
            height,
            vulkano::format::Format::R16G16B16A16_UINT,
        ),
        DynamicImage::ImageRgba16(_) => (
            decoded_image.into_bytes(),
            width,
            height,
            vulkano::format::Format::R16G16B16A16_UINT,
        ),
        DynamicImage::ImageRgb32F(_) => (
            DynamicImage::from(decoded_image.to_rgba32f()).into_bytes(),
            width,
            height,
            vulkano::format::Format::R32G32B32A32_SFLOAT,
        ),
        DynamicImage::ImageRgba32F(_) => (
            decoded_image.into_bytes(),
            width,
            height,
            vulkano::format::Format::R32G32B32A32_SFLOAT,
        ),
        _ => panic!("Unsupported input format."),
    }
}

pub fn load_gltf(
    path: &str,
    allocator: &StandardMemoryAllocator,
    mut cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    default_material: Rc<Material>,
    tex_i: &mut u32,
    mat_i: &mut u32,
) -> (
    Vec<Scene>,
    HashMap<usize, Rc<Texture>>,
    HashMap<usize, Rc<Material>>,
) {
    let (gltf, buffers, _) = gltf::import(path).unwrap(); // TODO skip loading of images on gltf lib side

    println!("GLTF has {:?} scenes", gltf.scenes().len());

    let mut scenes: Vec<Scene> = vec![];
    let mut textures: HashMap<usize, Rc<Texture>> = HashMap::new();
    let mut materials: HashMap<usize, Rc<Material>> = HashMap::new();

    let mut images = Vec::new();
    for image in gltf.images() {
        images.push(load_image(
            image.source(),
            Path::new(path).parent(),
            &buffers,
        )); //TODO support relative paths
    }

    for gltfTexture in gltf.textures() {
        let image = &images[gltfTexture.source().index()];
        let vk_texture = create_texture(
            image.0.clone(), //TODO: Texture load optimization: check if this clone is bad
            image.3,
            image.1,
            image.2,
            allocator,
            cmd_buf_builder,
        );
        let texture = Texture::from(vk_texture, gltfTexture.name().map(Box::from), *tex_i);
        *tex_i += 1;
        textures.insert(gltfTexture.index(), Rc::from(texture));
    }
    for gltfMat in gltf.materials() {
        if let Some(index) = gltfMat.index() {
            let mat = Material {
                id: *mat_i,
                name: gltfMat.name().map(Box::from),
                base_texture: gltfMat
                    .pbr_metallic_roughness()
                    .base_color_texture()
                    .map(|t| t.texture().index())
                    .map(|id| {
                        textures
                            .get(&id)
                            .expect("Couldn't find base texture")
                            .clone()
                    }),
                base_color: gltfMat.pbr_metallic_roughness().base_color_factor().into(),
                metallic_roughness_texture: gltfMat
                    .pbr_metallic_roughness()
                    .metallic_roughness_texture()
                    .map(|t| t.texture().index())
                    .map(|id| {
                        textures
                            .get(&id)
                            .expect("Couldn't find metallic roughness texture")
                            .clone()
                    }),
                metallic_roughness_factors: Vec2::from((
                    gltfMat.pbr_metallic_roughness().metallic_factor(),
                    gltfMat.pbr_metallic_roughness().roughness_factor(),
                )),
                normal_texture: gltfMat
                    .normal_texture()
                    .map(|t| t.texture().index())
                    .map(|id| {
                        textures
                            .get(&id)
                            .expect("Couldn't find normal texture")
                            .clone()
                    }),
                occlusion_texture: gltfMat
                    .occlusion_texture()
                    .map(|t| t.texture().index())
                    .map(|id| {
                        textures
                            .get(&id)
                            .expect("Couldn't find occlusion texture")
                            .clone()
                    }),
                occlusion_strength: 1.0, // TODO: Impl: try to read strength from glTF
                emissive_texture: gltfMat.emissive_texture().map(|t| t.texture().index()).map(
                    |id| {
                        textures
                            .get(&id)
                            .expect("Couldn't find emissive texture")
                            .clone()
                    },
                ),
                emissive_factors: gltfMat.emissive_factor().into(),
            };
            *mat_i += 1;
            materials.insert(index, Rc::from(mat));
        }
    }

    for scene in gltf.scenes() {
        println!("Scene has {:?} nodes", scene.nodes().len());

        let models = scene
            .nodes()
            .map(|n| {
                load_node(
                    &n,
                    &buffers,
                    &materials,
                    allocator,
                    default_material.clone(),
                    Mat4::default(),
                )
            })
            .collect();
        scenes.push(Scene::from(models, scene.name().map(Box::from)));
    }

    println!("extensions???");
    for extension in gltf.extensions_used() {
        println!("Scene has {} lights", 1);
        println!("extension: {:?}", extension);
    }

    (scenes, textures, materials)
}

fn load_node(
    node: &Node,
    buffers: &Vec<Data>,
    materials: &HashMap<usize, Rc<Material>>,
    allocator: &StandardMemoryAllocator,
    default_material: Rc<Material>,
    parent_transform: Mat4,
) -> Model {
    let mut children: Vec<Model> = vec![];
    let local_transform = Mat4::from_cols_array_2d(&node.transform().matrix());
    for child in node.children() {
        println!("Iterating over children...");
        children.push(load_node(
            &child,
            buffers,
            materials,
            allocator,
            default_material.clone(),
            parent_transform * local_transform,
        ));
    }
    let mut global_transform = parent_transform * local_transform;
    global_transform.y_axis *= -1.0;

    let mut meshes: Vec<Mesh> = vec![];
    match node.mesh() {
        Some(x) => {
            for gltf_primitive in x.primitives() {
                let mut positions: Vec<Vec3> = vec![];
                let mut indices: Vec<u32> = vec![];
                let mut normals: Vec<Vec3> = vec![];
                let mut uvs: Vec<Vec2> = vec![];
                let reader = gltf_primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                if let Some(iter) = reader.read_tex_coords(0) {
                    uvs = iter.into_f32().map(|arr| Vec2::from(arr)).collect();
                }
                if let Some(iter) = reader.read_positions() {
                    positions = iter.map(|p| Vec3::from(p)).collect();
                }
                if let Some(iter) = reader.read_indices() {
                    indices = iter.into_u32().collect();
                }
                if let Some(iter) = reader.read_normals() {
                    normals = iter.map(|n| Vec3::from(n)).collect();
                }

                let mat = gltf_primitive
                    .material()
                    .index()
                    .map(|i| materials.get(&i).expect("Couldn't find material").clone());
                meshes.push(Mesh::from(
                    positions,
                    indices,
                    normals,
                    mat.unwrap_or(default_material.clone()),
                    uvs,
                    global_transform,
                ));
            }
        }
        _ => {}
    }
    Model::from(
        meshes,
        node.name().map(Box::from),
        children,
        local_transform,
    )
}
