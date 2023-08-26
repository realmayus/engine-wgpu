#version 460
#extension GL_EXT_nonuniform_qualifier : enable

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec4 tangent;
layout(location = 3) in vec2 uv;

layout(location = 0) flat out uint index;
layout(location = 1) out vec2 tex_coords;
layout(location = 2) out vec3 fragPos_tan;
layout(location = 3) out vec3 viewPos_tan;
layout(location = 4) out mat3 TBN;

layout(set = 0, binding = 0) buffer CameraUniform {
    mat4 proj_view;
    vec4 view_position;
} camera;

layout(set = 3, binding = 0) buffer DrawCallInfo {
    uint tex_id;
    mat4 model_transform;
} dci[];


void main() {
    mat4 model = dci[gl_InstanceIndex].model_transform;
    gl_Position = camera.proj_view * model * vec4(position, 1.0) ;
    tex_coords = uv;

    vec3 bitangent = normalize(cross(normal, tangent.xyz)) * tangent.w;
    vec3 T = normalize(vec3(model * vec4(tangent.xyz, 0.0)));
    vec3 B = normalize(vec3(model * vec4(bitangent, 0.0)));
    vec3 N = normalize(vec3(model * vec4(normal, 0.0)));
    // transpose is the same as invert for othorgonal space
    mat3 tbn = transpose(mat3(T, B, N));

    fragPos_tan = tbn * vec3(model * vec4(position, 1.0));
    viewPos_tan = tbn * camera.view_position.xyz;
    TBN = tbn;

    index = gl_InstanceIndex;
}