use std::sync::Arc;
use vulkano::command_buffer::allocator::{
    StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo,
};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
};
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{
    Device, DeviceCreateInfo, DeviceExtensions, Queue, QueueCreateInfo, QueueFlags,
};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::swapchain::Surface;
use vulkano::VulkanLibrary;

// pub mod buffer_copy;
// pub mod compute;
// pub(crate) mod graphics;
// pub mod image;
pub(crate) mod window;

fn init(
    device_extensions: Option<DeviceExtensions>,
) -> (
    u32,
    Arc<Device>,
    Arc<Queue>,
    StandardMemoryAllocator,
    Arc<PhysicalDevice>,
) {
    let library = VulkanLibrary::new().expect("no local Vulkan library/DLL");
    let instance =
        Instance::new(library, InstanceCreateInfo::default()).expect("failed to create instance");

    let physical_device = instance
        .enumerate_physical_devices()
        .expect("could not enumerate devices")
        .next()
        .expect("no devices available");

    for family_property in physical_device.queue_family_properties() {
        println!(
            "Found a queue family with {:?} queue(s)",
            family_property.queue_count
        );
    }

    // get index of viable queue family (= one that supports graphics)
    let queue_family_index = physical_device
        .queue_family_properties()
        .iter()
        .enumerate()
        .position(|(_queue_family_index, queue_family_properties)| {
            queue_family_properties
                .queue_flags
                .contains(QueueFlags::GRAPHICS)
        })
        .expect("Couldn't find a graphical queue family") as u32;

    // create device using queue family index
    let (device, mut queues) = Device::new(
        physical_device.clone(),
        DeviceCreateInfo {
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index,
                ..Default::default()
            }],
            enabled_extensions: device_extensions.unwrap_or(DeviceExtensions::default()),
            ..Default::default()
        },
    )
    .expect("Failed to create device");

    // get queue
    let queue = queues.next().unwrap();

    let memory_allocator = StandardMemoryAllocator::new_default(device.clone());
    // device actually is Arc<Device>, thus calling .clone() is pretty cheap as it only clones the Arc

    return (
        queue_family_index,
        device,
        queue,
        memory_allocator,
        physical_device,
    );
}

fn init_cmd_buf(
    device: Arc<Device>,
    queue: Arc<Queue>,
) -> (
    StandardCommandBufferAllocator,
    AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) {
    let cmd_buf_allocator = StandardCommandBufferAllocator::new(
        device.clone(),
        StandardCommandBufferAllocatorCreateInfo::default(),
    );
    let mut builder = AutoCommandBufferBuilder::primary(
        &cmd_buf_allocator,
        queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();
    return (cmd_buf_allocator, builder);
}
