# engine-wgpu [![Rust](https://github.com/realmayus/engine-vk/actions/workflows/rust.yml/badge.svg)](https://github.com/realmayus/engine-vk/actions/workflows/rust.yml)
wgpu game engine written in Rust. The goal is to have the entire engine as a separate library to ensure low coupling.

This crate contains the following subcrates:
- `lib`: contains structs and helpers that are shared across the entire engine, such as the world, scene, model, mesh, texture, and material structs
- `renderer`: contains rendering logic
- `system::io`: contains various I/O functions such as loading/saving features and a glTF importer
- `system::particle`: particle system, TBA
- `system::physics`: physics engine, TBA
- `system::sound`: sound engine, TBA

## I/O
The program creates a directory `run` in the current working directory (cwd) where all textures and other resources are expanded at runtime. This directory is cleared upon exit.

World files (`world.json`) contain relative paths to resources like images. Thus, the scene file must be contained in the same directory as the other resource directories. 

The working directory can be set using the `WORKING_DIR` environment variable.

## Roadmap
### Renderer
- [x] egui [17.08.23]
- [x] dynamic asset loading [17.09.23]
- [x] point lights
- [ ] gamma correction
- [ ] Instancing
- [x] Physically based Rendering
- [x] Normal/Bump maps
- [ ] Shadows
- [ ] Anti-Aliasing
- [ ] face culling
- [ ] frustum culling
- [x] object outlines
- [ ] SSAO
- [ ] spotlights, directional lights 
- [ ] Skybox
- [ ] reflections (both skybox and reflections require cubemaps)
- [ ] Billboarding
- [ ] Decals
- [ ] Tessellation
- [ ] text rendering
- [ ] transparency
- [ ] cascaded shadow mapping
- [ ] percentage-closer filtering (shadows)
- [ ] good bloom
- [ ] deferred shading
- [ ] area lights
- [ ] Image-based lighting
- [ ] HDR, Tone Mapping
- [ ] mip maps

### I/O
- [ ] world (de-)serialization ~~[25.08.23]~~
- [ ] investigate https://github.com/google/flatbuffers -> maybe https://lib.rs/crates/prost?
- [x] GLTF import
- [ ] OBJ import / export, perhaps store vertex/normal/uv data in OBJs to reduce clutter in world.json
- [ ] multi-threaded asset loading

### ECS
- [ ] Implement
### Physics
- [ ] Implement Rigid Body physics
### Sound
- [ ] Implement basic sound system
### Particles
- [ ] Implement basic particle system
### UI
- [ ] Asset drag & drop
- [x] Object picking

