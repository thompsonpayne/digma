struct VsOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(@location(0) in_pos: vec2<f32>) -> VsOut {
    var out: VsOut;
    out.pos = vec4<f32>(in_pos, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.2, 0.6, 0.9, 1.0);
}
