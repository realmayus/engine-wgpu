#version 460

layout(location = 0) in vec2 positionA;
// declares that each vertex has an attribute named `position` and of type vec2 -> corresponds to our MyVertex struct

void main() { // called once for each vertex
    gl_Position = vec4(positiydtgjaonA, 0.0, 1.0);
}
