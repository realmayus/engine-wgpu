# engine-vk [![Rust](https://github.com/realmayus/engine-vk/actions/workflows/rust.yml/badge.svg)](https://github.com/realmayus/engine-vk/actions/workflows/rust.yml)
Vulkan game engine written in Rust. The goal is to have the entire engine as a separate library to ensure low coupling.

This crate contains the following subcrates:
- `lib`: contains structs and helpers that are shared across the entire engine, such as the world, scene, model, mesh, texture, and material structs
- `renderer`: contains the abstraction that interfaces with vulkan through vulkano
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
- [ ] dynamic asset loading
- [x] point lights
- [x] gamma correction (for pbr)
- [ ] Instancing
- [x] Physically based Rendering
- [x] Normal/Bump maps
- [ ] Shadows
- [ ] Anti-Aliasing
- [ ] face culling
- [ ] frustum culling
- [ ] object outlines
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
- [x] HDR, Tone Mapping (for pbr)
- [ ] mip maps

### I/O
- [x] world (de-)serialization [25.08.23]
- [ ] investigate https://github.com/google/flatbuffers
- [ ] multithreaded asset loading

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
