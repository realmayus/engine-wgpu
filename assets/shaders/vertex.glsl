#version 460

layout(location = 0) in vec2 position;
// declares that each vertex has an attribute named `position` and of type vec2 -> corresponds to our MyVertex struct
layout(location = 1) in vec3 color;

layout(location = 0) out vec3 fragColor;


void main() { // called once for each vertex
    gl_Position = vec4(position, 0.0, 1.0);
    fragColor = color;
}
