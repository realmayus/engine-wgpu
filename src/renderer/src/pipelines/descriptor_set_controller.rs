use std::sync::Arc;

use vulkano::buffer::Subbuffer;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::image::ImageViewAbstract;
use vulkano::pipeline::{PipelineBindPoint, PipelineLayout};
use vulkano::sampler::Sampler;

use lib::shader_types::{CameraUniform, MaterialInfo, MeshInfo};

pub struct DescriptorSetController {
    camera: Subbuffer<CameraUniform>,
    textures: Vec<(Arc<dyn ImageViewAbstract>, Arc<Sampler>)>,
    material_info_buffers: Vec<Subbuffer<MaterialInfo>>,
    mesh_info_buffers: Vec<Subbuffer<MeshInfo>>,
    descriptor_sets: [Arc<PersistentDescriptorSet>; 4],
    // 0: camera, 1: textures, 2: material_info, 3: mesh_info
    pipeline_layout: Arc<PipelineLayout>,
}

impl DescriptorSetController {
    pub fn init(
        camera: Subbuffer<CameraUniform>,
        textures: Vec<(Arc<dyn ImageViewAbstract>, Arc<Sampler>)>,
        material_info_buffers: Vec<Subbuffer<MaterialInfo>>,
        mesh_info_buffers: Vec<Subbuffer<MeshInfo>>,
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
        pipeline_layout: Arc<PipelineLayout>,
    ) -> Self {
        Self {
            camera: camera.clone(),
            textures: textures.clone(),
            material_info_buffers: material_info_buffers.clone(),
            mesh_info_buffers: mesh_info_buffers.clone(),
            descriptor_sets: [
                Self::get_camera_descriptor_set(
                    descriptor_set_allocator,
                    pipeline_layout.clone(),
                    camera,
                ),
                Self::get_textures_descriptor_set(
                    descriptor_set_allocator,
                    pipeline_layout.clone(),
                    textures,
                ),
                Self::get_materials_descriptor_set(
                    descriptor_set_allocator,
                    pipeline_layout.clone(),
                    material_info_buffers,
                ),
                Self::get_meshes_descriptor_set(
                    descriptor_set_allocator,
                    pipeline_layout.clone(),
                    mesh_info_buffers,
                ),
            ],
            pipeline_layout,
        }
    }
    pub fn bind(&self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        for (i, descriptor_set) in self.descriptor_sets.iter().enumerate() {
            builder.bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline_layout.clone(),
                i as u32,
                descriptor_set.clone(),
            );
        }
    }

    fn get_camera_descriptor_set(
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
        pipeline_layout: Arc<PipelineLayout>,
        buffer: Subbuffer<CameraUniform>,
    ) -> Arc<PersistentDescriptorSet> {
        PersistentDescriptorSet::new(
            descriptor_set_allocator,
            pipeline_layout.set_layouts().get(0).unwrap().clone(),
            [WriteDescriptorSet::buffer(0, buffer)],
        )
        .unwrap()
    }
    pub fn update_camera(
        &mut self,
        update_fn: Box<dyn Fn(&mut Subbuffer<CameraUniform>)>,
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
    ) {
        update_fn(&mut self.camera);
        self.descriptor_sets[0] = Self::get_camera_descriptor_set(
            descriptor_set_allocator,
            self.pipeline_layout.clone(),
            self.camera.clone(),
        );
    }

    fn get_textures_descriptor_set(
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
        pipeline_layout: Arc<PipelineLayout>,
        array: Vec<(Arc<dyn ImageViewAbstract>, Arc<Sampler>)>,
    ) -> Arc<PersistentDescriptorSet> {
        PersistentDescriptorSet::new_variable(
            descriptor_set_allocator,
            pipeline_layout.set_layouts().get(1).unwrap().clone(),
            array.len() as u32,
            [WriteDescriptorSet::image_view_sampler_array(0, 0, array)],
        )
        .unwrap()
    }
    pub fn update_textures(
        &mut self,
        update_fn: Box<dyn Fn(&mut Vec<(Arc<dyn ImageViewAbstract>, Arc<Sampler>)>)>,
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
    ) {
        update_fn(&mut self.textures);
        self.descriptor_sets[1] = Self::get_textures_descriptor_set(
            descriptor_set_allocator,
            self.pipeline_layout.clone(),
            self.textures.clone(),
        );
    }

    fn get_materials_descriptor_set(
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
        pipeline_layout: Arc<PipelineLayout>,
        array: Vec<Subbuffer<MaterialInfo>>,
    ) -> Arc<PersistentDescriptorSet> {
        PersistentDescriptorSet::new_variable(
            descriptor_set_allocator,
            pipeline_layout.set_layouts().get(2).unwrap().clone(),
            array.len() as u32,
            [WriteDescriptorSet::buffer_array(0, 0, array)],
        )
        .unwrap()
    }
    pub fn update_material_infos(
        &mut self,
        update_fn: Box<dyn Fn(&mut Vec<Subbuffer<MaterialInfo>>)>,
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
    ) {
        update_fn(&mut self.material_info_buffers);
        self.descriptor_sets[2] = Self::get_materials_descriptor_set(
            descriptor_set_allocator,
            self.pipeline_layout.clone(),
            self.material_info_buffers.clone(),
        )
    }

    fn get_meshes_descriptor_set(
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
        pipeline_layout: Arc<PipelineLayout>,
        array: Vec<Subbuffer<MeshInfo>>,
    ) -> Arc<PersistentDescriptorSet> {
        PersistentDescriptorSet::new_variable(
            descriptor_set_allocator,
            pipeline_layout.set_layouts().get(3).unwrap().clone(),
            array.len() as u32,
            [WriteDescriptorSet::buffer_array(0, 0, array)],
        )
        .unwrap()
    }
    pub fn update_mesh_infos<F>(
        &mut self,
        update_fn: F,
        descriptor_set_allocator: &StandardDescriptorSetAllocator,
    ) where
        F: Fn(&mut Vec<Subbuffer<MeshInfo>>) -> (),
    {
        update_fn(&mut self.mesh_info_buffers);
        self.descriptor_sets[3] = Self::get_meshes_descriptor_set(
            descriptor_set_allocator,
            self.pipeline_layout.clone(),
            self.mesh_info_buffers.clone(),
        );
    }
}
