#version 460
#extension GL_EXT_nonuniform_qualifier : enable

layout(location = 0) flat in uint index;
layout(location = 0) out vec4 f_color;

layout(set = 1, binding = 0) buffer LineInfo {
    mat4 model_transform;
    vec4 color;
} li[];

void main() {
    f_color = nonuniformEXT(li[index].color);
}