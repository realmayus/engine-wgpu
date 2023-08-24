#version 460
#extension GL_EXT_nonuniform_qualifier : enable

layout(location = 0) in vec2 tex_coords;
layout(location = 1) in vec3 normal_frag;
layout(location = 2) flat in uint index;
layout(location = 0) out vec4 frag_color;

layout(set = 0, binding = 0) buffer CameraUniform {
    mat4 proj_view;
    vec4 view_position;
} camera;

layout(set = 1, binding = 0) uniform sampler2D[] texs;

struct MUStruct {
    vec4 base_color;
    vec2 metal_roughness_factors;
    uint base_texture;
    uint metal_roughness_texture;
    vec3 emission_factors;
    uint emission_texture;
    uint normal_texture;
    uint ao_texture;
    float ao_factor;
};

layout(set = 2, binding = 0) buffer MaterialUniform {
    MUStruct mat;
} materials[];

struct MeshStruct {
    uint mat_id;
    mat4 model_transform;
};

layout(set = 3, binding = 0) buffer MeshInfo {
    MeshStruct meshes[];
};

struct Light {
    mat4 transform;
    vec3 color;
    uint light;
    float intensity;
    float range;
};

layout(set = 4, binding = 0) buffer LightInfo {
    Light lights[];
};

const float PI = 3.14159265359;


// Fresnel-Schlick approximation
// F0: base surface-reflectivity at 0 incidence (reflectivity when looking directly at it)
// cosTheta: result of the dot product of the surface normal and the halfway direction
// Calculates how much the surface reflects vs refracts (basically specular vs diffuse)
vec3 fresnel(float cosTheta, vec3 F0) {
    // clamp to avoid black spots
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

// Fresnel equation F of DFG which is the specular part of BRDF
vec3 reflect_ratio(vec3 half_vec, vec3 view_dir, vec3 albedo, float metallic) {
    // most dieclictric surfaces look visually correct with F0 of 0.04
    vec3 F0 = vec3(0.04);
    // metallic surfaces absorb all refraction so the F0 for them is just the albedo
    F0 = mix(F0, albedo, metallic);
    return fresnel(max(dot(half_vec, view_dir), 0.0), F0);
}


// Normal distribution function D of DFG (Trowbridge-Reitz GGX)
// (n, h, a) = a^2 / pi*((n*h)^2 (a^2 - 1) + 1)^2
// Approximates the relative surface area of microfacets exactly aligned to the (halfway) vector
float distribution(vec3 N, vec3 H, float roughness) {
    // squaring the roughness is not in the original formula but gives more accurate results
    float a = roughness*roughness;
    float a2 = a*a;
    float NdotH = max(dot(N, H), 0.0);
    float NdotH2 = NdotH * NdotH;

    float denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;

    return a2 / denom;
}

// n * v / ((n*v)(1-k) + k)
// takes the microfacets and so selfshadowing into account
float geometry_schlick(float NdotV, float roughness) {
    float r = (roughness + 1.0);
    float k = (r*r) / 8.0;

    float num   = NdotV;
    float denom = NdotV * (1.0 - k) + k;

    return num / denom;
}

// geomitry function G in DFG
// using schlicks method with both the view and light direction
float geometry_smith(vec3 N, vec3 V, vec3 L, float roughness) {
    float NdotV = max(dot(N, V), 0.0);
    float NdotL = max(dot(N, L), 0.0);
    float ggx2 = geometry_schlick(NdotV, roughness);
    float ggx1 = geometry_schlick(NdotL, roughness);

    return ggx1 * ggx2;
}

void main() {
    MeshStruct mesh = meshes[index];
    uint mat_id = mesh.mat_id;
    MUStruct material = materials[mat_id].mat;

    vec3 world_pos = mesh.model_transform[3].xyz;

    // load material values, if index 0, value will be 1 because of white default texture
    vec3 albedo = texture(nonuniformEXT(texs[material.base_texture]), tex_coords).xyz;
    vec3 normal = texture(nonuniformEXT(texs[material.normal_texture]), tex_coords).xyz;
    float metallic = texture(nonuniformEXT(texs[material.metal_roughness_texture]), tex_coords).b;
    float roughness = texture(nonuniformEXT(texs[material.metal_roughness_texture]), tex_coords).g;
    vec3 ao = texture(nonuniformEXT(texs[material.ao_texture]), tex_coords).xyz;

    normal = normalize(normal);
    vec3 view_dir = normalize(camera.view_position.xyz - world_pos);

    vec3 Lo = vec3(0.0);
    // contribution of each light,
    for (int i = 0; i < 1; i++) {
        Light light = lights[i];
        vec3 light_pos = light.transform[3].xyz;
        vec3 light_dir = normalize(light_pos - world_pos);
        vec3 half_vec = normalize(view_dir + light_dir); // \|/

        float dist = distance(light_pos, world_pos);
        float attenuation = 1.0 / (dist * dist);
        vec3 radiance = light.color * attenuation;

        vec3 refl_ratio = reflect_ratio(half_vec, view_dir, albedo, metallic);

        float normal_dist = distribution(normal, half_vec, roughness);
        float geom = geometry_smith(normal, view_dir, light_dir, roughness);

        // BRDF
        vec3 numerator = normal_dist * geom * refl_ratio;
        float denominator = 4.0 * max(dot(normal, view_dir), 0.0) * max(dot(normal, light_dir), 0.0) + 0.0001;
        vec3 specular = numerator / denominator;

        vec3 k_specular = refl_ratio;
        vec3 k_diffuse = vec3(1.0) - k_specular;

        k_diffuse *= 1.0 - metallic;

        float normal_dot_light = max(dot(normal, light_dir), 0.0);
        Lo += (k_diffuse * albedo / PI + specular) * radiance * normal_dot_light;
    }

    vec3 ambient = vec3(0.03) * albedo * ao;
    vec3 color = ambient + Lo;

    color = color / (color + vec3(1.0));
    color = pow(color, vec3(1.0/2.2));

    frag_color = vec4(color, 1.0);
}
