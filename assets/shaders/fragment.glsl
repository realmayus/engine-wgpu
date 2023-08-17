#version 460

layout(location = 0) in vec2 tex_coords;
layout(location = 1) in vec3 normal_frag;
layout(location = 0) out vec4 f_color;

//layout(set = 0, binding = 0) uniform sampler2D tex;

void main() {
//    f_color = texture(tex, tex_coords);
    f_color = vec4(normal_frag, 1.0);
}