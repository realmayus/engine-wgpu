#version 460
#extension GL_EXT_nonuniform_qualifier : enable

layout(location = 0) in vec2 tex_coords;
layout(location = 1) in vec3 normal_frag;
layout(location = 2) flat in uint index;
layout(location = 0) out vec4 f_color;

layout(set = 1, binding = 0) uniform sampler2D[] texs;

layout(set = 2, binding = 0) buffer MaterialUniform {
    vec4 base_color;
    uint base_texture;
} materials[];

layout(set = 3, binding = 0) buffer DrawCallInfo {
    uint mat_id;
    mat4 model_transform;
} draw_call_infos[];

void main() {
    uint mat_id = draw_call_infos[index].mat_id;
    uint base_texture = materials[mat_id].base_texture;
    f_color = texture(nonuniformEXT(texs[base_texture]), tex_coords);
}