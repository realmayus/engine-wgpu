#version 460
#extension GL_EXT_nonuniform_qualifier : enable

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec4 tangent;
layout(location = 3) in vec2 uv;

layout(location = 0) out vec2 tex_coords;
layout(location = 1) out vec3 normal_frag;
layout(location = 2) out vec4 tangent_frag;
layout(location = 3) flat out uint index;

layout(set = 0, binding = 0) buffer CameraUniform {
    mat4 proj_view;
    vec4 view_position;
} camera;

layout(set = 3, binding = 0) buffer DrawCallInfo {
    uint tex_id;
    mat4 model_transform;
} dci[];


void main() {
    gl_Position = camera.proj_view * dci[gl_InstanceIndex].model_transform * vec4(position, 1.0) ;
    tex_coords = uv;
    normal_frag = normal;
    tangent_frag = tangent;
    index = gl_InstanceIndex;
}