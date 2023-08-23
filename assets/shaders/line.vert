#version 460
#extension GL_EXT_nonuniform_qualifier : enable

layout(location = 0) in vec3 position;
layout(location = 0) flat out uint index;

layout(set = 0, binding = 0) buffer CameraUniform {
    mat4 proj_view;
    vec4 view_position;
} camera;

layout(set = 1, binding = 0) buffer LineInfo {
    mat4 model_transform;
    vec4 color;
} li[];

void main() {
    gl_Position = camera.proj_view * nonuniformEXT(li[gl_InstanceIndex].model_transform) * vec4(position, 1.0) ;
    index = gl_InstanceIndex;
}