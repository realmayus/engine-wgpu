use log::debug;
use std::fmt::{Debug, Formatter};
use std::hash::Hash;
use wgpu::{BindGroupLayout, Buffer, BufferAddress, Device, Queue};

const PREALLOC_COUNT: usize = 16; // how many elements we want to have space for initially

/**
A dynamic buffer array on the GPU that auto-resizes and can be updated.
*/
pub struct DynamicBufferArray<T> {
    buffer: Buffer,
    pub bind_group: wgpu::BindGroup,
    count: u64,
    capacity: u64,
    dirty: bool, // if the buffer needs to be resized
    label: Option<String>,
    usages: wgpu::BufferUsages,
    phantom: std::marker::PhantomData<T>,
}

impl<T: bytemuck::Pod> DynamicBufferArray<T> {
    pub fn new(
        device: &Device,
        label: Option<String>,
        usages: wgpu::BufferUsages,
        bind_group_layout: &BindGroupLayout,
    ) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: label.as_deref(),
            size: (PREALLOC_COUNT * std::mem::size_of::<T>()) as u64,
            usage: usages | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: label.as_deref(),
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        Self {
            label: label.clone(),
            buffer,
            bind_group,
            count: 0,
            capacity: PREALLOC_COUNT as u64,
            dirty: false,
            usages,
            phantom: std::marker::PhantomData,
        }
    }

    pub fn push(&mut self, device: &Device, queue: &Queue, data: &[T], bind_group_layout: &BindGroupLayout) {
        debug!("Pushing {} elements to buffer (Count: {})", data.len(), self.count);
        if self.count + data.len() as u64 > self.capacity {
            self.resize(device, queue, bind_group_layout);
        }
        queue.write_buffer(
            &self.buffer,
            self.count * std::mem::size_of::<T>() as u64,
            bytemuck::cast_slice(data),
        );
        self.count += data.len() as u64;
    }

    pub fn update(&mut self, queue: &Queue, index: u64, data: T) {
        assert!(index < self.count);
        println!(
            "Updating buffer {:?} at index {} (offset {})",
            self.label,
            index,
            index * std::mem::size_of::<T>() as u64
        );
        queue.write_buffer(
            &self.buffer,
            index * std::mem::size_of::<T>() as u64,
            bytemuck::cast_slice(&[data]),
        );
    }

    fn resize(&mut self, device: &Device, queue: &Queue, bind_group_layout: &BindGroupLayout) {
        debug!("Resizing buffer {:?}", self.label);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Buffer resize encoder"),
        });
        self.capacity *= 2;
        let new_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: self.label.as_deref(),
            size: (self.capacity * std::mem::size_of::<T>() as u64) as BufferAddress,
            usage: self.usages | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        // copy the contents of self.buffer to new_buffer
        encoder.copy_buffer_to_buffer(
            &self.buffer,
            0,
            &new_buffer,
            0,
            (self.count * std::mem::size_of::<T>() as u64) as BufferAddress,
        );
        queue.submit(std::iter::once(encoder.finish()));

        self.dirty = true;
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: self.label.as_deref(),
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.buffer.as_entire_binding(),
            }],
        });
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> u64 {
        self.count
    }
}

/**
A dynamic buffer array that also stores a map of keys to memory offsets within the buffer.
*/
pub struct DynamicBufferMap<T, K> {
    array: DynamicBufferArray<T>,
    map: std::collections::HashMap<K, u64>,
}

impl<T, K> DynamicBufferMap<T, K>
where
    T: bytemuck::Pod,
    K: Eq + Hash + Debug,
{
    pub fn new(
        device: &Device,
        label: Option<String>,
        usages: wgpu::BufferUsages,
        bind_group_layout: &BindGroupLayout,
    ) -> Self
    where
        T: bytemuck::Pod,
    {
        Self {
            array: DynamicBufferArray::new(device, label, usages, bind_group_layout),
            map: std::collections::HashMap::new(),
        }
    }

    pub fn push(&mut self, device: &Device, queue: &Queue, key: K, data: &[T], bind_group_layout: &BindGroupLayout) {
        self.map.insert(key, self.array.len());
        self.array.push(device, queue, data, bind_group_layout);
        println!(
            "Pushed to buffer, now length is {}; map: {:?}",
            self.array.len(),
            self.map
        );
    }

    pub fn update(&mut self, queue: &Queue, key: &K, data: T) {
        let index = *self.map.get(key).unwrap();
        println!("Mesh {key:?} located at {index}, array len {}", self.array.len());
        self.array.update(queue, index, data);
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<K, u64> {
        self.map.iter()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.array.bind_group
    }

    pub fn get(&self, key: &K) -> Option<&u64> {
        self.map.get(key)
    }
}

impl<T, K> Debug for DynamicBufferMap<T, K>
where
    K: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "DynamicBufferMap: ")?;
        writeln!(f, "Map: {:?}", self.map)?;
        Ok(())
    }
}
