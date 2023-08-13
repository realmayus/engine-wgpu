use crate::playground::init;
use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::allocator::{
    StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo,
};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferInfo};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage};
use vulkano::sync;
use vulkano::sync::GpuFuture;

#[derive(BufferContents, Debug, Default)]
#[repr(C)]
struct MyDataStruct {
    a: u32,
    b: u128,
}

pub fn buffer_copy() {
    let (queue_family_index, device, queue, memory_allocator) = init(None);

    let data = MyDataStruct { a: 42, b: 69 };

    let src = Buffer::from_data(
        &memory_allocator,
        BufferCreateInfo {
            usage: BufferUsage::TRANSFER_SRC,
            ..Default::default()
        },
        AllocationCreateInfo {
            usage: MemoryUsage::Upload,
            ..Default::default()
        },
        data,
    )
    .expect("Failed to create src buffer");

    let dest = Buffer::from_data(
        &memory_allocator,
        BufferCreateInfo {
            usage: BufferUsage::TRANSFER_DST,
            ..Default::default()
        },
        AllocationCreateInfo {
            usage: MemoryUsage::Download,
            ..Default::default()
        },
        MyDataStruct::default(),
    )
    .expect("Failed to create dest buffer");

    // We need allocator to allocate several command buffers
    let cmd_buf_allocator = StandardCommandBufferAllocator::new(
        device.clone(),
        StandardCommandBufferAllocatorCreateInfo::default(),
    );
    // command buffer contains list of commands to execute -> much more efficient than executing the cmds one-by-one

    // Create a builder
    let mut builder = AutoCommandBufferBuilder::primary(
        &cmd_buf_allocator,
        queue_family_index,
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    // Add a copy command to the builder
    builder
        .copy_buffer(CopyBufferInfo::buffers(src.clone(), dest.clone()))
        .unwrap();

    // Turn the builder into an actual command buffer
    let command_buffer = builder.build().unwrap();

    // No function in vulkano immediately sends an operation to the GPU, instead we need to use sync::now()
    let future = sync::now(device.clone()) // creates a future (which keeps alive all the resources used by the GPU and represents execution in time of actual operations)
        .then_execute(queue.clone(), command_buffer) // The returned future is in a pending state and makes it possible to append the execution of other command buffers and operations
        .unwrap()
        .then_signal_fence_and_flush() // ...only by calling .flush() the operations are submitted all at once and start executing on the GPU
        .unwrap();

    // Can't (or shouldn't) immediately read from the dest buffer as CPU and GPU have been operating in parallel. Thus, we need to await the GPU's special signal that it's done (called 'fence' -> tells us when the GPU has reached a certain point of execution)

    future.wait(None).unwrap();

    let src_content = src.read().unwrap();
    let dest_content = dest.read().unwrap();
    println!("Src: {:?}, Dest: {:?}", &*src_content, &*dest_content);
}
