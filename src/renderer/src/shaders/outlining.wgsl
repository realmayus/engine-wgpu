struct PushConstants {
    mesh_index: u32,
    outline: u32,
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
    @location(0) frag_pos: vec3<f32>,
    @location(1) view_pos: vec3<f32>,
    @location(2) color: vec3<f32>,
}

struct MeshInfo {
    material: u32,
    model_transform: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,  // model_transform.inverse().transpose()
    scale: vec3<f32>,
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
    let width = f32(push.outline & 0xffu) / 255.0;
    let scaling = mat4x4<f32>(
        1.0 + width, 0.0, 0.0, 0.0,
        0.0, 1.0 + width, 0.0, 0.0,
        0.0, 0.0, 1.0 + width, 0.0,
        0.0, 0.0, 0.0, 1.0,
    );
    var out: VertexOutput;
    var model_transform = mesh_infos[push.mesh_index].model_transform;
    let scale = mesh_infos[push.mesh_index].scale;
    let mesh_scale_mat = mat4x4<f32>(scale.x, 0.0, 0.0, 0.0,
                                0.0, scale.y, 0.0, 0.0,
                                0.0, 0.0, scale.z, 0.0,
                                0.0, 0.0, 0.0, 1.0);
    out.color = vec3(f32((push.outline >> 24u) & 0xFFu), f32((push.outline >> 16u) & 0xFFu), f32((push.outline >> 8u) & 0xFFu)) / 255.0;
    if push.outline > 0u {
        out.clip_position = camera.proj_view * model_transform * mesh_scale_mat * vec4<f32>(in.position + in.position * width, 1.0);
    } else {
        out.clip_position = camera.proj_view * model_transform * mesh_scale_mat * vec4<f32>(in.position, 1.0);
    }

    out.frag_pos = (model_transform * vec4<f32>(in.position, 1.0)).xyz;

    out.view_pos = camera.view_position.xyz;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color.xyz, 1.0);
}