struct PushConstants {
    mesh_index: u32,
    color: vec4<f32>,
}
var<push_constant> push: PushConstants;


struct VertexInput {
    @location(0) position: vec3<f32>, // 3*4 = 12
    @location(1) normal: vec3<f32>, // 12 + 3*4 = 24
    @location(2) tangent: vec4<f32>,    // 24 + 4*4 = 40
    @location(3) uv: vec2<f32> // 40 + 2*4 = 48
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) frag_pos: vec3<f32>,
    @location(2) view_pos: vec3<f32>,
}

struct MeshInfo {
    material: u32,
    model_transform: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,  // model_transform.inverse().transpose()
}
@group(0) @binding(0)
var<storage, read> mesh_infos: array<MeshInfo>;

struct Camera {
    proj_view: mat4x4<f32>,
    unproj_view: mat4x4<f32>,
    view_position: vec4<f32>,
    num_lights: u32,
};
@group(1) @binding(0)
var<uniform> camera: Camera;

@vertex
fn vs_main(
    in: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    let model_transform = mesh_infos[push.mesh_index].model_transform;

    out.clip_position = camera.proj_view * model_transform * vec4<f32>(in.position, 1.0);
    out.color = push.color;
    out.frag_pos = (model_transform * vec4<f32>(in.position, 1.0)).xyz;

    out.view_pos = camera.view_position.xyz;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}