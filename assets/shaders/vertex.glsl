#version 460

layout(location = 0) in vec3 position;
layout(location = 0) out vec2 tex_coords;

layout(set = 0, binding = 1) buffer CameraUniform {
    mat4 view_proj;
    vec4 view_position;
} camera;
layout(set = 0, binding = 2) uniform ModelUniform {
    mat4 model;
} model;

mat4 test;

void main() {
    test = model.model * camera.view_proj;
    gl_Position = camera.view_proj * vec4(position, 1.0) ;
    tex_coords = position.xy + vec2(0.5);
}