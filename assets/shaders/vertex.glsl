#version 460

layout(location = 0) in vec2 position;
layout(location = 0) out vec2 tex_coords;

layout(set = 0, binding = 1) buffer CameraUniform {
    mat4 view_proj;
    vec4 view_position;
} camera;
layout(set = 0, binding = 2) uniform ModelUniform {
    mat4 model;
} model;


void main() {
    gl_Position = vec4(position, 0.0, 1.0) * camera.view_proj * model.model;
    tex_coords = position + vec2(0.5);
}