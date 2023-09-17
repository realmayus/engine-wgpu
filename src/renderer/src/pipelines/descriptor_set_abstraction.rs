use std::sync::Arc;

use vulkano::buffer::Subbuffer;
use vulkano::image::ImageViewAbstract;
use vulkano::sampler::Sampler;

use lib::shader_types::{CameraUniform, MaterialInfo, MeshInfo};

struct DescriptorSetsAbstraction {
    camera: Subbuffer<CameraUniform>,
    textures: Vec<(Arc<dyn ImageViewAbstract<Handle = ()>>, Arc<Sampler>)>,
    material_info_buffers: Vec<Subbuffer<MaterialInfo>>,
    mesh_info_buffers: Vec<Subbuffer<MeshInfo>>,
}

impl DescriptorSetsAbstraction {}
