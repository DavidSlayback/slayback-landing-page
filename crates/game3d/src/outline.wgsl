// Edge outlines (outline.rs): the relative second difference of reverse-Z depth, in four directions. NDC depth is
// affine in screen space across a plane, so it is ~0 inside a flat face and large at creases and silhouettes.
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct EdgeOutline {
    color: vec4<f32>,
    width: f32,
    crease: f32,
    silhouette: f32,
    crease_width: f32,
}

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var depth: texture_2d<f32>;
// Depth is read through a NEAREST sampler, not textureLoad: on WebGL2 a texture read without a sampler object uses
// the texture's own filter (LINEAR by default), which leaves a Depth32Float texture incomplete, so every read is 0.
@group(0) @binding(2) var nearest: sampler;
@group(0) @binding(3) var<uniform> o: EdgeOutline;

fn d(uv: vec2<f32>) -> f32 {
    return textureSampleLevel(depth, nearest, uv, 0.0).r;
}

// Largest relative second difference of depth at `uv`, sampling `w` pixels out in 4 directions.
fn rel_lap(uv: vec2<f32>, c: f32, px: vec2<f32>, w: f32) -> f32 {
    let x = vec2(w * px.x, 0.0);
    let y = vec2(0.0, w * px.y);
    let h = d(uv - x) + d(uv + x);
    let v = d(uv - y) + d(uv + y);
    let a = d(uv - x - y) + d(uv + x + y);
    let b = d(uv - x + y) + d(uv + x - y);
    let lap = max(max(abs(h - 2.0 * c), abs(v - 2.0 * c)), 0.5 * max(abs(a - 2.0 * c), abs(b - 2.0 * c)));
    return lap / max(c, 1e-6);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let px = 1.0 / vec2<f32>(textureDimensions(depth));
    let uv = in.position.xy * px;
    let col = textureLoad(screen, vec2<i32>(in.position.xy), 0);
    let c = d(uv);
    // Creases `crease_width` px out; only the stronger silhouette edges `width` px out (lines ~2 x width).
    let thin = smoothstep(o.crease, 2.0 * o.crease, rel_lap(uv, c, px, o.crease_width));
    let thick = smoothstep(o.silhouette, 2.0 * o.silhouette, rel_lap(uv, c, px, o.width));
    return vec4(mix(col.rgb, o.color.rgb, max(thin, thick) * o.color.a), col.a);
}
