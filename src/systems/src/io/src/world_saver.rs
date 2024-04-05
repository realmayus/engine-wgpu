// use std::error::Error;
// use std::fs;
// use std::path::Path;
//
// use log::debug;
//
// use lib::scene_serde::WorldSerde;
//
// pub fn save(path: &Path, world: WorldSerde) -> Result<(), Box<dyn Error>> {
//     debug!("Saving world to {}", path.to_str().unwrap());
//     fs::create_dir_all(path.join("images"))?;
//     for texture in world.textures.textures.as_slice() {
//         debug!(
//             "Texture has path {}",
//             texture.img_path.clone().to_str().unwrap()
//         );
//         fs::copy(
//             Path::new("run").join(texture.img_path.clone()),
//             path.join("images")
//                 .join(texture.img_path.clone().file_name().unwrap()),
//         )?;
//     }
//     let serialized = serde_json::to_string(&world)?;
//     fs::write(path.join("world.json"), serialized)?;
//
//     Ok(())
// }
