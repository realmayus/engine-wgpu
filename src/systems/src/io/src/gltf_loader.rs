use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::{fs, io};
use base64::{engine::general_purpose, Engine as _};
use glam::{Mat4, Vec2, Vec3, Vec4};
use gltf::buffer::Data;
use gltf::image::Source;
use gltf::image::Source::View;
use gltf::{Error, Node};
use image::DynamicImage;
use image::ImageFormat::{Jpeg, Png};
use log::{debug, info};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BindGroupLayout, BufferUsages, Device, Queue};
use lib::managers::{MaterialManager, MatId, TextureManager};

use lib::scene::{PointLight};
use lib::scene::{Mesh, Model, PbrMaterial, Scene};
use lib::shader_types::{LightInfo, MaterialInfo, MeshInfo};
use lib::texture::{Texture, TextureKind};
use lib::Material;

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

fn load_image(source: Source<'_>, base: Option<&Path>, buffer_data: &[Data]) -> DynamicImage {
    let (decoded_image, ..) = match source {
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

    decoded_image
}

pub fn load_gltf(
    path: &Path,
    device: &Device,
    queue: &Queue,
    texture_bind_group_layout: &BindGroupLayout,
    material_bind_group_layout: &BindGroupLayout,
    mesh_bind_group_layout: &BindGroupLayout,
    light_bind_group_layout: &BindGroupLayout,
    texture_manager: &mut TextureManager,
    material_manager: &mut MaterialManager,
) -> Vec<Scene> {
    let (gltf, buffers, _) = gltf::import(path).unwrap(); // todo skip loading of images on gltf lib side

    info!("GLTF has {:?} scenes", gltf.scenes().len());

    let mut scenes: Vec<Scene> = vec![];
    let mut images: HashMap<u32, DynamicImage> = HashMap::with_capacity(gltf.images().len());
    for image in gltf.images() {
        images.insert(
            image.index() as u32,
            load_image(image.source(), Path::new(path).parent(), &buffers),
        );
    }
    // because gltf texture IDs need not correspond to our global texture IDs, we have to keep track of them separately at first
    let local_textures = gltf
        .textures()
        .map(|gltf_texture| {
            let img = images
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

            // let path = extract_image_to_file(img_name.as_str(), &img, file_format);  // TODO store path in texture? for serde
            let texture =
                Texture::from_image(device, queue, &img, gltf_texture.name(), TextureKind::Other)
                    .expect("Couldn't create texture");

        let global_id = texture_manager.add_texture(texture);
        (gltf_texture.index(), global_id)
    }).collect::<HashMap<_,_>>();

    let local_materials =
        gltf.materials()
            .filter(|m| m.index().is_some())
            .map(|gltf_mat| {
                let index = gltf_mat.index().unwrap();
                debug!("GLTF Material: {:?}", gltf_mat.pbr_metallic_roughness().base_color_factor());
                let mut mat = PbrMaterial {
                    // TODO only make initialization possible through material manager!
                    dirty: true, // must get updated upon start in order to prime the uniform
                    shader_id: 0, // will get overwritten by call to MaterialManager::add_material() below
                    name: gltf_mat.name().map(Box::from),
                    albedo_texture: gltf_mat
                        .pbr_metallic_roughness()
                        .base_color_texture()
                        .map(|t| t.texture().index())
                        .map(|id| {
                            *local_textures.get(&id).expect("Couldn't find base texture")
                        }),
                    albedo: gltf_mat.pbr_metallic_roughness().base_color_factor().into(),
                    metallic_roughness_texture: gltf_mat
                        .pbr_metallic_roughness()
                        .metallic_roughness_texture()
                        .map(|t| t.texture().index())
                        .map(|id| {
                                *local_textures
                                    .get(&id)
                                    .expect("Couldn't find metallic roughness texture")

                        }),
                    metallic_roughness_factors: Vec2::from((
                        gltf_mat.pbr_metallic_roughness().metallic_factor(),
                        gltf_mat.pbr_metallic_roughness().roughness_factor(),
                    )),
                    normal_texture: gltf_mat.normal_texture().map(|t| t.texture().index()).map(
                        |id| {
                                *local_textures
                                    .get(&id)
                                    .expect("Couldn't find normal texture")

                        },
                    ),
                    occlusion_texture: gltf_mat
                        .occlusion_texture()
                        .map(|t| t.texture().index())
                        .map(|id| {
                                *local_textures
                                    .get(&id)
                                    .expect("Couldn't find occlusion texture")

                        }),
                    occlusion_factor: 1.0, // TODO: Impl: try to read strength from glTF
                    emissive_texture: gltf_mat
                        .emissive_texture()
                        .map(|t| t.texture().index())
                        .map(|id| {
                                *local_textures
                                    .get(&id)
                                    .expect("Couldn't find emissive texture")

                        }),
                    emissive_factors: gltf_mat.emissive_factor().into(),
                    texture_bind_group: None,
                }; // TODO move this into a function (automatically init texture_bind_group, buffer and MaterialInfo)
                mat.create_texture_bind_group(device, texture_bind_group_layout, texture_manager);
                let global_id = material_manager.add_material(Material::Pbr(mat), device, queue, material_bind_group_layout);
                (index, global_id)
            })
            .collect::<HashMap<_, _>>();

    for scene in gltf.scenes() {
        info!("Scene has {:?} nodes", scene.nodes().len());
        let mut num_lights = 0;
        let models: Vec<Model> = scene
            .nodes()
            .map(|n| {
                load_node(
                    &n,
                    &buffers,
                    &local_materials,
                    material_manager,
                    Mat4::default(),
                    &mut num_lights,
                    device,
                )
            })
            .collect();
        scenes.push(Scene::from(device, queue, models, material_manager, scene.name().map(Box::from), mesh_bind_group_layout, light_bind_group_layout));
    }
    scenes
}

fn load_node(
    node: &Node,
    buffers: &[Data],
    materials: &HashMap<usize, MatId>,
    material_manager: &MaterialManager,
    parent_transform: Mat4,
    num_lights: &mut u32,
    device: &Device,
) -> Model {
    let mut children: Vec<Model> = vec![];
    let local_transform = Mat4::from_cols_array_2d(&node.transform().matrix());
    for child in node.children() {
        children.push(load_node(
            &child,
            buffers,
            materials,
            material_manager,
            parent_transform * local_transform,
            num_lights,
            device,
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
            // xyz is tangent, w is bi-tangent sign
            let mut tangents: Vec<Vec4> = vec![];
            let mut uvs: Vec<Vec2> = vec![];
            let reader = gltf_primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            if let Some(iter) = reader.read_tex_coords(0) {
                uvs = iter.into_f32().map(|[u, v]| Vec2::from((u, v))).collect();
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
            if let Some(iter) = reader.read_tangents() {
                tangents = iter.map(Vec4::from).collect();
            }

            let mat = gltf_primitive
                .material()
                .index()
                .map(|i| *materials.get(&i).expect("Couldn't find material"));
            meshes.push(Mesh::from(positions, indices, normals, tangents, mat.unwrap_or(material_manager.default_material), uvs, global_transform, device));
        }
    }

    let light = node.light().map(|light| PointLight::new(
        parent_transform * Mat4::from_cols_array_2d(&node.transform().matrix()),
        light.index(),
        Vec3::from(light.color()),
        light.intensity(),
        light.range(),
        device,
    ));

    if let Some(ref _light) = light {
        *num_lights += 1;
    }

    Model::from(
        meshes,
        node.name().map(Box::from),
        children,
        local_transform,
        light,
    )
}
