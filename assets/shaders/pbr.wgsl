struct PushConstants {
    mesh_index: u32,
}
var<push_constant> push: PushConstants;


struct VertexInput {
    @location(0) position: vec3<f32>, // 3*4 = 12
    @location(1) normal: vec3<f32>, // 12 + 3*4 = 24
    @location(2) tangent: vec4<f32>,    // 24 + 4*4 = 40
    @location(3) uv: vec2<f32> // 40 + 2*4 = 48
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(flat) index: u32,
    @location(1) tex_coords: vec2<f32>,
    @location(2) frag_pos: vec3<f32>,
    @location(3) view_pos: vec3<f32>,
    @location(4) t: vec3<f32>,
    @location(5) b: vec3<f32>,
    @location(6) n: vec3<f32>,
    @location(7) @interpolate(flat) num_lights: u32,
}

struct MeshInfo {
    material: u32,
    model_transform: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,  // model_transform.inverse().transpose()
}
@group(2) @binding(0)
var<storage, read> mesh_infos: array<MeshInfo>;

struct Camera {
    proj_view: mat4x4<f32>,
    view_position: vec4<f32>,
    num_lights: u32,
};
@group(3) @binding(0)
var<uniform> camera: Camera;

@vertex
fn vs_main(
    in: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    let model_transform = mesh_infos[push.mesh_index].model_transform;

    out.clip_position = camera.proj_view * model_transform * vec4<f32>(in.position, 1.0);
    out.index = push.mesh_index;
    out.tex_coords = in.uv;
    out.frag_pos = (model_transform * vec4<f32>(in.position, 1.0)).xyz;

    out.t = normalize((model_transform * vec4(in.tangent.xyz, 0.0)).xyz);
    out.n = normalize((model_transform * vec4(in.normal, 0.0)).xyz);
    let bitangent = cross(in.normal, in.tangent.xyz) * in.tangent.w;
    out.b = normalize((model_transform * vec4(bitangent, 0.0)).xyz);

    // we can't just pass the matrix to the fragment shader due to size limitations
    out.view_pos = camera.view_position.xyz;
    out.num_lights = camera.num_lights;  // camera is only accessible in vertex shader
    return out;
}

@group(0) @binding(0)
var t_albedo: texture_2d<f32>;
@group(0) @binding(1)
var s_albedo: sampler;

@group(0) @binding(2)
var t_normal: texture_2d<f32>;
@group(0) @binding(3)
var s_normal: sampler;

@group(0) @binding(4)
var t_metallic: texture_2d<f32>;
@group(0) @binding(5)
var s_metallic: sampler;

@group(0) @binding(6)
var t_occlusion: texture_2d<f32>;
@group(0) @binding(7)
var s_occlusion: sampler;

@group(0) @binding(8)
var t_emissive: texture_2d<f32>;
@group(0) @binding(9)
var s_emissive: sampler;

struct Material {
    albedo: vec4<f32>, // 4*4 = 16
    emission_factors: vec3<f32>, // 16 + 3*4 = 28
    occlusion_factor: f32, // 28 + 4 = 32
    metal_roughness_factors: vec2<f32>, // 32 + 2*4 = 40
};

@group(1) @binding(0)
var<storage, read> materials: array<Material>;


struct LightInfo {
    transform: mat4x4<f32>,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
};
@group(4) @binding(0)
var<storage, read> lights: array<LightInfo>;

const PI = 3.14159265359;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let mat_id = mesh_infos[in.index].material;
    let material = materials[mat_id];
    let tbn = mat3x3<f32>(in.t, in.b, in.n);


    // load material values, if index 0, value will be 1 because of white default texture
    var albedo = textureSample(t_albedo, s_albedo, in.tex_coords) * material.albedo;
    var normal = textureSample(t_normal, s_normal, in.tex_coords).rgb * 2.0;
    normal = normal - vec3(1.0);
    normal = normalize(tbn * normal);

    let metallic = textureSample(t_metallic, s_metallic, in.tex_coords).b * material.metal_roughness_factors.x;
    let roughness = textureSample(t_metallic, s_metallic, in.tex_coords).g * material.metal_roughness_factors.y;
    var occlusion = textureSample(t_occlusion, s_occlusion, in.tex_coords).r;
    var emission = textureSample(t_emissive, s_emissive, in.tex_coords).rgb;
    // convert to linear space
    albedo = pow(albedo, vec4(2.2));
    emission = pow(emission, vec3(2.2));
    occlusion = pow(occlusion, 2.2);
    let view_dir = normalize(in.view_pos - in.frag_pos);
    // most dielectric surfaces look visually correct with f0 of 0.04
    var f0 = vec3(0.04);
    f0 = mix(f0, albedo.rgb, metallic);
    var lo = vec3(0.0);

    // contribution of each light
    for (var i = 0u; i < in.num_lights; i++) {
        let light = lights[i];
        let light_pos = light.transform[3].xyz;
        let light_dir = normalize(light_pos - in.frag_pos);
        let half_vec = normalize(view_dir + light_dir);

        let dist = length(light_pos - in.frag_pos);
        let attenuation = 1.0 / (dist * dist);
        let radiance: vec3<f32> = light.color * 5.0 * attenuation;
        // Fresnel equation F of DFG which is the specular part of BRDF
        let reflect_ratio = fresnel(max(dot(half_vec, view_dir), 0.0), f0);
        let normal_dist = distribution(normal, half_vec, roughness);
        let geom = geometry_smith(normal, view_dir, light_dir, roughness);

        // BRDF
        let numerator = normal_dist * geom * reflect_ratio;
        let denominator = 4.0 * max(dot(normal, view_dir), 0.0) * max(dot(normal, light_dir), 0.0) + 0.0001;
        let specular: vec3<f32> = numerator / denominator;

        let k_specular = reflect_ratio;
        var k_diffuse = vec3(1.0) - k_specular;
        k_diffuse *= 1.0 - metallic;

        let normal_dot_light: f32 = max(dot(normal, light_dir), 0.0);

        let diffuse_albedo: vec3<f32> = k_diffuse * albedo.rgb;
        let diffuse_albedo_by_pi: vec3<f32> = diffuse_albedo / PI;
        lo += (diffuse_albedo_by_pi + specular) * radiance * normal_dot_light;
    }

    let ambient = vec3(0.001) * albedo.rgb * occlusion;
    var color = ambient + lo + emission * material.emission_factors;
    // reinhard tone mapping
    color = color / (color + vec3(1.0));
    // gamma correction
    color = pow(color, vec3(1.0 / 2.2));
    return vec4<f32>(color, 1.0);

}

// Fresnel-Schlick approximation
// F0: base surface-reflectivity at 0 incidence (reflectivity when looking directly at it)
// cosTheta: result of the dot product of the view direction and the halfway direction
// Calculates how much the surface reflects vs refracts (basically specular vs diffuse)
fn fresnel(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// Normal distribution function D of DFG (Trowbridge-Reitz GGX)
// (n, h, a) = a^2 / pi*((n*h)^2 (a^2 - 1) + 1)^2
// Approximates the relative surface area of microfacets exactly aligned to the (halfway) vector
fn distribution(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    // squaring the roughness is not in the original formula but gives more accurate results
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h = max(dot(n, h), 0.0);
    let n_dot_h2 = n_dot_h * n_dot_h;
    let denom = n_dot_h2 * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

// n * v / ((n*v)(1-k) + k)
// takes the microfacets and so selfshadowing into account
fn geometry_schlick(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k);
}

// geometry function G in DFG
// using schlicks method with both the view and light direction
fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_l = max(dot(n, l), 0.0);
    let ggx1 = geometry_schlick(n_dot_v, roughness);
    let ggx2 = geometry_schlick(n_dot_l, roughness);
    return ggx1 * ggx2;
}