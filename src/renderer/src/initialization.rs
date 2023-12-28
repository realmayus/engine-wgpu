use std::sync::Arc;

use log::info;
use vulkano::command_buffer::allocator::{
    StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo,
};
use vulkano::descriptor_set::allocator::{StandardDescriptorSetAllocator, StandardDescriptorSetAllocatorCreateInfo};
use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, Features, QueueCreateInfo};
use vulkano::image::ImageUsage;
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::swapchain::{Surface, Swapchain, SwapchainCreateInfo};
use vulkano::VulkanLibrary;
use vulkano_win::VkSurfaceBuild;
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

use crate::{get_render_pass, select_physical_device, RenderInitState};

pub fn init_renderer() -> RenderInitState {
    let library = VulkanLibrary::new().expect("No local Vulkan library");
    let required_extensions = vulkano_win::required_extensions(&library);
    let instance = Instance::new(
        library,
        InstanceCreateInfo {
            enabled_extensions: required_extensions,
            ..Default::default()
        },
    )
    .expect("Failed to create instance");

    let event_loop = EventLoop::new();
    let window = Arc::new(WindowBuilder::new()
        .build(&event_loop)
        .unwrap());
    let surface = Surface::from_window(instance.clone(), window.clone()).unwrap();

    window.set_title("Engine Playground - Press ESC to release controls");

    let device_extensions = DeviceExtensions {
        khr_swapchain: true,
        ext_descriptor_indexing: true, // required for textures
        ..DeviceExtensions::empty()
    };

    let (physical_device, queue_family_index) =
        select_physical_device(&instance, &surface, &device_extensions);

    info!(
        "Using device: {} (type: {:?})",
        physical_device.properties().device_name,
        physical_device.properties().device_type,
    );

    let (device, mut queues) = Device::new(
        physical_device.clone(),
        DeviceCreateInfo {
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index,
                ..Default::default()
            }],
            enabled_extensions: device_extensions, // new
            enabled_features: Features {
                descriptor_indexing: true,
                descriptor_binding_variable_descriptor_count: true,
                shader_sampled_image_array_non_uniform_indexing: true,
                shader_uniform_buffer_array_non_uniform_indexing: true,
                shader_storage_buffer_array_dynamic_indexing: true,
                shader_storage_buffer_array_non_uniform_indexing: true,
                runtime_descriptor_array: true,
                ..Features::empty()
            },
            ..Default::default()
        },
    )
    .expect("failed to create device");

    let queue = queues.next().unwrap();

    let caps = physical_device
        .surface_capabilities(&surface, Default::default())
        .expect("failed to get surface capabilities");

    info!(
        "Max swapchain images: {:?}, min: {:?}",
        caps.max_image_count, caps.min_image_count
    );

    let dimensions = window.inner_size();
    let composite_alpha = caps.supported_composite_alpha.into_iter().next().unwrap();

    // Create the swapchain
    let (swapchain, images) = {
        let image_format = device
            .physical_device()
            .surface_formats(&surface, Default::default())
            .unwrap()[0]
            .0;
        Swapchain::new(
            device.clone(),
            surface.clone(),
            SwapchainCreateInfo {
                image_format,
                min_image_count: caps.min_image_count,
                image_extent: dimensions.into(),
                image_usage: ImageUsage::COLOR_ATTACHMENT,
                composite_alpha,
                ..Default::default()
            },
        )
        .unwrap()
    };
    let cmd_buf_allocator = Arc::new(StandardCommandBufferAllocator::new(
        device.clone(),
        StandardCommandBufferAllocatorCreateInfo::default(),
    ));

    let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));
    let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(device.clone(), StandardDescriptorSetAllocatorCreateInfo::default()));
    let render_pass = get_render_pass(device.clone(), &swapchain);

    RenderInitState {
        device,
        surface,
        event_loop,
        window,
        memory_allocator: memory_allocator,
        queue,
        image_format: swapchain.image_format(),
        swapchain,
        images,
        cmd_buf_allocator,
        descriptor_set_allocator,
        render_pass,
    }
}
