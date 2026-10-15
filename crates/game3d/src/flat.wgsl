// The game's one material (flat.rs): a colour, Lambert-shaded by the scene's fixed light plus ambient, or at full
// strength when unlit, plus a glow. A tiny shader, so WebGL links it in milliseconds where Bevy's PBR shader took
// seconds on ANGLE/D3D11 (decision Q74).
#import bevy_pbr::forward_io::VertexOutput

struct Flat {
    color: vec4<f32>,
    glow: vec4<f32>,
    lit: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: Flat;

// Toward the key light, as from (5, 14, 10) shining on the origin: above, in front and a little to the right, so the
// two side walls' faces read apart. Ambient and diffuse were fitted to how
// StandardMaterial lit the box under that light (linear, per channel), so the look stays the same.
const LIGHT: vec3<f32> = vec3<f32>(0.2790, 0.7812, 0.5580);
const AMBIENT: vec3<f32> = vec3<f32>(0.243, 0.287, 0.48);
const DIFFUSE: f32 = 3.1;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Lit meshes are closed (cubes, balls), so the face seen is the one its normal points out of; the flat meshes (the
    // bursts' stars, the ring, the shadow disc) are unlit, so no back face needs lighting.
    let n = normalize(in.world_normal);
    let shade = AMBIENT + vec3<f32>(DIFFUSE * max(dot(n, LIGHT), 0.0));
    let rgb = material.color.rgb * mix(vec3<f32>(1.0), shade, material.lit) + material.glow.rgb;
    return vec4<f32>(rgb, material.color.a);
}
