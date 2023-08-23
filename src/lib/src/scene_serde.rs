use serde::de::{DeserializeSeed, MapAccess, Visitor};
use serde::{de, Deserializer};
use std::fmt;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::format;
use vulkano::memory::allocator::StandardMemoryAllocator;

use crate::scene::Texture;
use crate::texture::create_texture;

struct TextureDeserializer<'a>(
    &'a StandardMemoryAllocator,
    &'a mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
);

impl<'a> DeserializeSeed<'a> for TextureDeserializer<'a> {
    type Value = Texture;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'a>,
    {
        deserializer.deserialize_map(TextureVisitor(self.0, self.1))
    }
}

struct TextureVisitor<'a>(
    &'a StandardMemoryAllocator,
    &'a mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
);

impl<'a> Visitor<'a> for TextureVisitor<'a> {
    type Value = Texture;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map containing Texture fields")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'a>,
    {
        let mut id = None;
        let mut name = None;
        let mut img_path: Option<Box<str>> = None;

        while let Some(key) = map.next_key()? {
            match key {
                "id" => id = Some(map.next_value()?),
                "name" => name = Some(map.next_value()?),
                "img_path" => img_path = Some(map.next_value()?),
                _ => {
                    // Consume and discard unexpected fields
                    let _: de::IgnoredAny = map.next_value()?;
                }
            }
        }

        let dyn_img = image::open(img_path.clone().expect("Missing img path").to_string())
            .expect("Could not load image");
        let width = dyn_img.width();
        let height = dyn_img.height();
        let view = create_texture(
            dyn_img.into_bytes(),
            format::Format::R8G8B8A8_UNORM,
            width,
            height,
            self.0,
            self.1,
        );

        Ok(Texture {
            id: id.ok_or_else(|| de::Error::missing_field("id"))?,
            name,
            view,
            img_path: img_path.unwrap(),
        })
    }
}
