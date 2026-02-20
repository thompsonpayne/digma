struct VsOut {
    @builtin(position) pos: vec4<f32>,
};

struct CameraUniform {
  pan: vec2<f32>,
  zoom: f32,
  _pad0: f32,
  canvas: vec2<f32>,
  _pad1: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> u_camera: CameraUniform;

@vertex
fn vs_main(
    @location(0) in_pos: vec2<f32>,
    @location(1) inst_pos: vec2<f32>,
    @location(2) inst_size: vec2<f32>,
    @location(3) inst_color: vec4<f32>,
) -> VsOut {
    var out: VsOut;

    let world = inst_pos + in_pos * inst_size;
    let screen = (world - u_camera.pan) * u_camera.zoom;

    let ndc = vec2<f32>(
      (screen.x / u_camera.canvas.x) * 2.0 - 1.0,
      1.0 - (screen.y / u_camera.canvas.y) * 2.0,
    );

    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.2, 0.6, 0.9, 1.0);
}
