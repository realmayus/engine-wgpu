
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}
@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(1 - i32(in_vertex_index)) * 0.5;
    let y = f32(i32(in_vertex_index & 1u) * 2 - 1) * 0.5;
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

@group(0) @binding(0)
var t_albedo: texture_2d<f32>;
@group(0) @binding(1)
var s_albedo: sampler;

@group(0) @binding(2)
var t_normal: texture_2d<f32>;
@group(0) @binding(3)
var s_normal: sampler;

@group(0) @binding(4)
var t_metallic: texture_2d<f32>;
@group(0) @binding(5)
var s_metallic: sampler;

@group(0) @binding(6)
var t_occlusion: texture_2d<f32>;
@group(0) @binding(7)
var s_occlusion: sampler;

@group(0) @binding(8)
var t_emissive: texture_2d<f32>;
@group(0) @binding(9)
var s_emissive: sampler;

struct Material {
    albedo: vec4<f32>,
    emission_factors: vec3<f32>,
    occlusion_factor: f32,
    metal_roughness_factors: vec2<f32>,
    albedo_texture: u32, // index of texture
    normal_texture: u32,
    metal_roughness_texture: u32,
    occlusion_texture: u32,
    emission_texture: u32,
};

@group(1) @binding(0)
var<storage, read> materials: array<Material>;


struct MeshInfo {
    material: u32,
    model_transform: mat4x4<f32>,
}
@group(2) @binding(0)
var<storage, read> mesh_infos: array<MeshInfo>;

struct Camera {
    proj_view: mat4x4<f32>,
    view_position: mat4x4<f32>,
};
@group(3) @binding(0)
var<uniform> camera: Camera;

struct LightInfo {
    transform: mat4x4<f32>,
    color: vec4<f32>,
    light: u32,
    intensity: f32,
    range: f32,
    amount: u32,
};
@group(4) @binding(0)
var<storage, read> lights: array<LightInfo>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}