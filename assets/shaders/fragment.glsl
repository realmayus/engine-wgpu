#version 460

layout(location = 0) in vec3 fragColor;
layout(location = 0) out vec4 f_color;

void main() {

    f_color = vec4(fragColor, 1.0);
}
