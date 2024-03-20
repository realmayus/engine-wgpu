// largely based on http://asliceofrendering.com/scene%20helper/2020/01/05/InfiniteGrid/

struct VertexInput {
    @location(0) position: vec3<f32>, // 3*4 = 12
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) near_point: vec3<f32>,
    @location(1) far_point: vec3<f32>,
    @location(2) proj_view_x: vec4<f32>,
    @location(3) proj_view_y: vec4<f32>,
    @location(4) proj_view_z: vec4<f32>,
    @location(5) proj_view_w: vec4<f32>,
}

struct Camera {
    proj_view: mat4x4<f32>,
    unproj_view: mat4x4<f32>,  // inverse of proj_view; e.g. used for rendering the grid
    view_position: vec4<f32>,
    num_lights: u32,
};
@group(0) @binding(0)
var<uniform> camera: Camera;


fn unproject_point(x: f32, y: f32, z: f32, unproj: mat4x4<f32>) -> vec3<f32> {
    let unproj_point = unproj * vec4(x, y, z, 1.0);
    return unproj_point.xyz / unproj_point.w;
}

@vertex
fn vs_main(
    in: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.near_point = unproject_point(in.position.x, in.position.y, 0.0, camera.unproj_view);
    out.far_point = unproject_point(in.position.x, in.position.y, 1.0, camera.unproj_view);
    let t = -out.near_point.y / (out.far_point.y - out.near_point.y);
    let fragPos3D = out.near_point - t * (out.far_point - out.near_point);
    let clip_space_pos = camera.proj_view * vec4<f32>(fragPos3D, 1.0);
    let depth = clip_space_pos.z / clip_space_pos.w;
    out.clip_position = vec4<f32>(in.position.xy, depth, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = -in.near_point.y / (in.far_point.y - in.near_point.y);
    var flag = 0.0;
    if (t > 0.0) {
        flag = 1.0;
    }
    let fragPos3D = in.near_point + t * (in.far_point - in.near_point);
    let scale = 20.0;
    let coord = fragPos3D.xz * scale; // use the scale variable to set the distance between the lines
    let derivative = fwidth(coord);
    let grid = abs(fract(coord - 0.5) - 0.5) / derivative;
    let grid_line = min(grid.x, grid.y);
    let minimumz = min(derivative.y, 1.0);
    let minimumx = min(derivative.x, 1.0);
    var color = vec4(0.2, 0.2, 0.2, 1.0 - min(grid_line, 1.0));
    // z axis
    if (fragPos3D.x > -0.1 * minimumx && fragPos3D.x < 0.1 * minimumx) {
        color.z = 1.0;
    }
    // x axis
    if (fragPos3D.z > -0.1 * minimumz && fragPos3D.z < 0.1 * minimumz) {
        color.x = 1.0;
    }


    return color;
//    return vec4<f32>(in.clip_position.zzz, 1.0);
}