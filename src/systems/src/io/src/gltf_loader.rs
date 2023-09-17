use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::{fs, io};

use base64::{engine::general_purpose, Engine as _};
use glam::{Mat4, Vec2, Vec3};
use gltf::buffer::Data;
use gltf::image::Source;
use gltf::image::Source::View;
use gltf::{Error, Node};
use image::ImageFormat::{Jpeg, Png};
use image::{DynamicImage, ImageFormat};
use log::info;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::format::Format;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage, StandardMemoryAllocator};

use lib::scene::{Material, MaterialManager, Mesh, Model, Scene, Texture, TextureManager};
use lib::shader_types::{MaterialInfo, MeshInfo};
use lib::texture::create_texture;

use crate::extract_image_to_file;

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
) -> (DynamicImage, u32, u32, Format, ImageFormat) {
    let (decoded_image, file_format) = match source {
        Source::Uri { uri, mime_type } if base.is_some() => match Scheme::parse(uri) {
            Scheme::Data(Some(mime), base64) => {
                let encoded_image = general_purpose::STANDARD
                    .decode(base64)
                    .expect("Couldn't parse b64");
                let encoded_format = match mime {
                    "image/png" => Png,
                    "image/jpeg" => Jpeg,
                    _ => panic!("Couldn't determine format of b64-encoded image"),
                };
                (
                    image::load_from_memory(&encoded_image).expect("Couldn't load image"),
                    encoded_format,
                )
            }
            Scheme::Unsupported => panic!("Unsupported scheme."),
            _ => {
                let encoded_image = Scheme::read(base, uri);
                let encoded_format = match mime_type {
                    Some("image/png") => Png,
                    Some("image/jpeg") => Jpeg,
                    None => match uri.rsplit('.').next() {
                        Some("png") => Png,
                        Some("jpg") | Some("jpeg") => Jpeg,
                        _ => panic!("Couldn't determine format of image"),
                    },
                    _ => panic!("Couldn't determine format of image"),
                };
                (
                    image::load_from_memory(&encoded_image).expect("Couldn't load image"),
                    encoded_format,
                )
            }
        },
        View { view, mime_type } => {
            let parent_buffer_data = &buffer_data[view.buffer().index()].0;
            let begin = view.offset();
            let end = begin + view.length();
            let encoded_image = &parent_buffer_data[begin..end];
            let encoded_format = match mime_type {
                "image/png" => Png,
                "image/jpeg" => Jpeg,
                _ => panic!("Couldn't determine format of image"),
            };
            (
                image::load_from_memory(encoded_image).expect("Couldn't load image"),
                encoded_format,
            )
        }
        _ => panic!("Unsupported source"),
    };

    let width = decoded_image.width();
    let height = decoded_image.height();

    match decoded_image {
        DynamicImage::ImageLuma8(_) => {
            (decoded_image, width, height, Format::R8_UNORM, file_format)
        }
        DynamicImage::ImageLumaA8(_) => (
            decoded_image,
            width,
            height,
            Format::R8G8_UNORM,
            file_format,
        ),
        DynamicImage::ImageRgb8(_) => (
            DynamicImage::from(decoded_image.to_rgba8()),
            decoded_image.width(),
            decoded_image.height(),
            Format::R8G8B8A8_SRGB,
            file_format,
        ),
        DynamicImage::ImageRgba8(_) => (
            decoded_image,
            width,
            height,
            Format::R8G8B8A8_SRGB,
            file_format,
        ),
        DynamicImage::ImageLuma16(_) => {
            (decoded_image, width, height, Format::R16_UINT, file_format)
        }
        DynamicImage::ImageLumaA16(_) => (
            decoded_image,
            width,
            height,
            Format::R16G16_UINT,
            file_format,
        ),
        DynamicImage::ImageRgb16(_) => (
            DynamicImage::from(decoded_image.to_rgba16()),
            width,
            height,
            Format::R16G16B16A16_UINT,
            file_format,
        ),
        DynamicImage::ImageRgba16(_) => (
            decoded_image,
            width,
            height,
            Format::R16G16B16A16_UINT,
            file_format,
        ),
        DynamicImage::ImageRgb32F(_) => (
            DynamicImage::from(decoded_image.to_rgba32f()),
            width,
            height,
            Format::R32G32B32A32_SFLOAT,
            file_format,
        ),
        DynamicImage::ImageRgba32F(_) => (
            decoded_image,
            width,
            height,
            Format::R32G32B32A32_SFLOAT,
            file_format,
        ),
        _ => panic!("Unsupported input format."),
    }
}

pub fn load_gltf(
    path: &Path,
    allocator: &StandardMemoryAllocator,
    cmd_buf_builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    texture_manager: &mut TextureManager,
    material_manager: &mut MaterialManager,
) -> Vec<Scene> {
    let (gltf, buffers, _) = gltf::import(path).unwrap(); // todo skip loading of images on gltf lib side

    info!("GLTF has {:?} scenes", gltf.scenes().len());

    let mut scenes: Vec<Scene> = vec![];
    let mut local_textures: Vec<Rc<Texture>> = vec![]; // because gltf texture IDs need not correspond to our global texture IDs, we have to keep track of them separately at first
    let mut local_materials: Vec<Rc<RefCell<Material>>> = vec![]; // same thing with materials
    let mut images: HashMap<u32, (DynamicImage, u32, u32, Format, ImageFormat)> =
        HashMap::with_capacity(gltf.images().len());
    for image in gltf.images() {
        images.insert(
            image.index() as u32,
            load_image(image.source(), Path::new(path).parent(), &buffers),
        ); //TODO support relative paths
    }

    for gltf_texture in gltf.textures() {
        let (img, width, height, format, file_format) = images
            .remove(&(gltf_texture.source().index() as u32))
            .unwrap();

        let img_name = if let Some(name) = gltf_texture.name() {
            format!(
                "{}_{}",
                Path::new(path)
                    .file_name()
                    .map(|s| s.to_str())
                    .unwrap_or(Some(""))
                    .unwrap_or("")
                    .split('.')
                    .next()
                    .unwrap(),
                name
            ) //TODO fix this abomination
        } else {
            Path::new(path)
                .file_name()
                .map(|s| s.to_str())
                .unwrap_or(Some(""))
                .unwrap_or("")
                .split('.')
                .next()
                .unwrap()
                .to_string()
        };

        let path = extract_image_to_file(img_name.as_str(), &img, file_format);
        let vk_texture = create_texture(
            img.into_bytes(), //TODO: Texture load optimization: check if this clone is bad
            format,
            width,
            height,
            allocator,
            cmd_buf_builder,
        );

        let texture = Texture::from(
            vk_texture,
            gltf_texture.name().map(Box::from),
            gltf_texture.index() as u32,
            path,
        );
        let global_id = texture_manager.add_texture(texture);
        local_textures.insert(gltf_texture.index(), texture_manager.get_texture(global_id));
    }
    for gltf_mat in gltf.materials() {
        if let Some(index) = gltf_mat.index() {
            let mat = Material {
                dirty: true, // must get updated upon start in order to prime the uniform
                id: 0, // will get overwritten by call to MaterialManager::add_material() below
                name: gltf_mat.name().map(Box::from),
                base_texture: gltf_mat
                    .pbr_metallic_roughness()
                    .base_color_texture()
                    .map(|t| t.texture().index())
                    .map(|id| {
                        local_textures
                            .get(id)
                            .expect("Couldn't find base texture")
                            .clone()
                    }),
                base_color: gltf_mat.pbr_metallic_roughness().base_color_factor().into(),
                metallic_roughness_texture: gltf_mat
                    .pbr_metallic_roughness()
                    .metallic_roughness_texture()
                    .map(|t| t.texture().index())
                    .map(|id| {
                        local_textures
                            .get(id)
                            .expect("Couldn't find metallic roughness texture")
                            .clone()
                    }),
                metallic_roughness_factors: Vec2::from((
                    gltf_mat.pbr_metallic_roughness().metallic_factor(),
                    gltf_mat.pbr_metallic_roughness().roughness_factor(),
                )),
                normal_texture: gltf_mat
                    .normal_texture()
                    .map(|t| t.texture().index())
                    .map(|id| {
                        local_textures
                            .get(id)
                            .expect("Couldn't find normal texture")
                            .clone()
                    }),
                occlusion_texture: gltf_mat
                    .occlusion_texture()
                    .map(|t| t.texture().index())
                    .map(|id| {
                        local_textures
                            .get(id)
                            .expect("Couldn't find occlusion texture")
                            .clone()
                    }),
                occlusion_strength: 1.0, // TODO: Impl: try to read strength from glTF
                emissive_texture: gltf_mat
                    .emissive_texture()
                    .map(|t| t.texture().index())
                    .map(|id| {
                        local_textures
                            .get(id)
                            .expect("Couldn't find emissive texture")
                            .clone()
                    }),
                emissive_factors: gltf_mat.emissive_factor().into(),
                buffer: Buffer::from_data(
                    allocator,
                    BufferCreateInfo {
                        usage: BufferUsage::STORAGE_BUFFER,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        usage: MemoryUsage::Upload,
                        ..Default::default()
                    },
                    MaterialInfo::default(),
                )
                .expect("Couldn't allocate MaterialInfo uniform"),
            };
            let global_id = material_manager.add_material(mat);
            local_materials.insert(index, material_manager.get_material(global_id));
        }
    }

    for scene in gltf.scenes() {
        info!("Scene has {:?} nodes", scene.nodes().len());

        let models = scene
            .nodes()
            .map(|n| {
                load_node(
                    &n,
                    &buffers,
                    &local_materials,
                    allocator,
                    material_manager.get_default_material(),
                    Mat4::default(),
                )
            })
            .collect();
        scenes.push(Scene::from(models, scene.name().map(Box::from)));
    }
    scenes
}

fn load_node(
    node: &Node,
    buffers: &Vec<Data>,
    materials: &Vec<Rc<RefCell<Material>>>,
    allocator: &StandardMemoryAllocator,
    default_material: Rc<RefCell<Material>>,
    parent_transform: Mat4,
) -> Model {
    let mut children: Vec<Model> = vec![];
    let local_transform = Mat4::from_cols_array_2d(&node.transform().matrix());
    for child in node.children() {
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
    if let Some(x) = node.mesh() {
        for gltf_primitive in x.primitives() {
            let mut positions: Vec<Vec3> = vec![];
            let mut indices: Vec<u32> = vec![];
            let mut normals: Vec<Vec3> = vec![];
            let mut uvs: Vec<Vec2> = vec![];
            let reader = gltf_primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            if let Some(iter) = reader.read_tex_coords(0) {
                uvs = iter.into_f32().map(Vec2::from).collect();
            }
            if let Some(iter) = reader.read_positions() {
                positions = iter.map(Vec3::from).collect();
            }
            if let Some(iter) = reader.read_indices() {
                indices = iter.into_u32().collect();
            }
            if let Some(iter) = reader.read_normals() {
                normals = iter.map(Vec3::from).collect();
            }

            let mat = gltf_primitive
                .material()
                .index()
                .map(|i| materials.get(i).expect("Couldn't find material").clone());
            meshes.push(Mesh::from(
                positions,
                indices,
                normals,
                mat.unwrap_or(default_material.clone()),
                uvs,
                global_transform,
                Buffer::from_data(
                    allocator,
                    BufferCreateInfo {
                        usage: BufferUsage::STORAGE_BUFFER,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        usage: MemoryUsage::Upload,
                        ..Default::default()
                    },
                    MeshInfo::default(),
                )
                .expect("Couldn't allocate MeshInfo uniform"),
            ));
        }
    }
    Model::from(
        meshes,
        node.name().map(Box::from),
        children,
        local_transform,
    )
}
