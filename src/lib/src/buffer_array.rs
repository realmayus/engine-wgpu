use wgpu::{BindingResource, Buffer, BufferAddress, Device, Queue};

const PREALLOC_COUNT: usize = 16;

pub struct DynamicBufferArray<T> {
    pub(crate) buffer: Buffer,
    count: u32,
    capacity: u32,
    dirty: bool,  // if the buffer needs to be resized
    label: Option<String>,
    usages: wgpu::BufferUsages,
    phantom: std::marker::PhantomData<T>,
}

impl<T: bytemuck::Pod> DynamicBufferArray<T> {
    pub fn new(device: &Device, label: Option<String>, usages: wgpu::BufferUsages) -> Self {
        Self {
            label: label.clone(),
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: label.as_deref(),
                size: (PREALLOC_COUNT * std::mem::size_of::<T>()) as u64,
                usage: usages,
                mapped_at_creation: false,
            }),
            count: 0,
            capacity: PREALLOC_COUNT as u32,
            dirty: false,
            usages,
            phantom: std::marker::PhantomData,
        }
    }

    pub fn push(&mut self, device: &Device, queue: &Queue, data: &[T]) {
        println!("Pushing {} elements to buffer (Count: {})", data.len(), self.count);
        if self.count + data.len() as u32 > self.capacity {
            self.resize(device, queue);
        }
        queue.write_buffer(&self.buffer, (self.count as u64) * std::mem::size_of::<T>() as u64, bytemuck::cast_slice(data));
        self.count += data.len() as u32;
    }

    pub fn update(&mut self, queue: &Queue, index: u32, data: T) {
        assert!(index < self.count);
        queue.write_buffer(&self.buffer, (index as u64) * std::mem::size_of::<T>() as u64, bytemuck::cast_slice(&[data]));
    }

    fn resize(&mut self, device: &Device, queue: &Queue) {
        println!("Resizing buffer {:?}", self.label);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Buffer resize encoder"),
        });
        self.capacity *= 2;
        let new_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: self.label.as_deref(),
            size: (self.capacity * std::mem::size_of::<T>() as u32) as BufferAddress,
            usage: self.usages,
            mapped_at_creation: false,
        });
        // copy the contents of self.buffer to new_buffer
        encoder.copy_buffer_to_buffer(&self.buffer, 0, &new_buffer, 0, (self.count * std::mem::size_of::<T>() as u32) as BufferAddress);
        queue.submit(std::iter::once(encoder.finish()));

        self.dirty = true;
    }

    pub fn binding_resource(&self) -> BindingResource {
        self.buffer.as_entire_binding()
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> u32 {
        self.count
    }
}