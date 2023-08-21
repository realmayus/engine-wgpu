#version 460
#extension GL_EXT_nonuniform_qualifier : enable

layout(location = 0) in vec2 tex_coords;
layout(location = 1) in vec3 normal_frag;
layout(location = 2) flat in uint index;
layout(location = 0) out vec4 f_color;

layout(set = 1, binding = 0) uniform sampler2D[] texs;

struct MUStruct {
    uint base_texture;
    vec4 base_color;
    uint metal_roughness_texture;
    vec2 metal_roughness_factors;
    uint normal_texture;
    uint occlusion_texture;
    float occlusion_factor;
    uint emission_texture;
    vec3 emission_factors;
};

layout(set = 2, binding = 0) buffer MaterialUniform {
    MUStruct mat;
} materials[];

struct MeshStruct {
    uint mat_id;
    mat4 model_transform;
};

layout(set = 3, binding = 0) buffer MeshInfo {
    MeshStruct draw_call_infos[];
};

struct LIStruct {
    mat4 transform;
    uint light;
    float intensity;
};

layout(set = 4, binding = 0) buffer LightInfo {
    LIStruct light;
} light_info[];

void main() {
    // do sth so it's not optimized into the void
    LIStruct light = light_info[0].light;

    uint mat_id = draw_call_infos[index].mat_id;
    MUStruct material = materials[mat_id].mat;

    uint base_texture = material.base_texture;
    uint met_rough_texture = material.metal_roughness_texture;

    f_color = texture(nonuniformEXT(texs[base_texture]), tex_coords)
            * texture(nonuniformEXT(texs[met_rough_texture]), tex_coords)
    ;
}