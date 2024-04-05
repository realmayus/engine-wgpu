pub enum Geometry {
    Cube { width: f32, height: f32, depth: f32 },
    Plane { width: f32, depth: f32 },
}

// impl Geometry {
//     pub fn new_mesh(&self) -> Mesh {
//         let vertex_data = match self {
//             Geometry::Cube { width, height, depth } => {
//                 let width = *width;
//                 let height = *height;
//                 let depth = *depth;
//                 vec![
//                     Vec3::new(width, height, -depth),
//                     Vec3::new(width, height, -depth),
//                     Vec3::new(width, height, -depth),
//                     Vec3::new(width, -height, -depth),
//                     Vec3::new(width, -height, -depth),
//                     Vec3::new(width, -height, -depth),
//                     Vec3::new(width, height, depth),
//                     Vec3::new(width, height, depth),
//                     Vec3::new(width, height, depth),
//                     Vec3::new(width, -height, depth),
//                     Vec3::new(width, -height, depth),
//                     Vec3::new(width, -height, depth),
//                     Vec3::new(-width, height, -depth),
//                     Vec3::new(-width, height, -depth),
//                     Vec3::new(-width, height, -depth),
//                     Vec3::new(-width, -height, -depth),
//                     Vec3::new(-width, -height, -depth),
//                     Vec3::new(-width, -height, -depth),
//                     Vec3::new(-width, height, depth),
//                     Vec3::new(-width, height, depth),
//                     Vec3::new(-width, height, depth),
//                     Vec3::new(-width, -height, depth),
//                     Vec3::new(-width, -height, depth),
//                     Vec3::new(-width, -height, depth),
//                 ]
//             }
//             Geometry::Plane { .. } => {}
//         }
//         Mesh::from()
//     }
// }
