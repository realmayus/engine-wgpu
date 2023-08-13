#version 460


layout(local_size_x = 64, local_size_y = 1, local_size_z = 1) in;
// We want to invoke the shader 65536 times in total, but in practice we divide the work into 1024 work groups with a local size of 64 each
// always try to aim for a local size of at least 32 to 64

layout(set = 0, binding = 0) buffer Data {
    uint data[];
} buf;

void main() {
    uint idx = gl_GlobalInvocationID.x;
    buf.data[idx] *= 12;
}