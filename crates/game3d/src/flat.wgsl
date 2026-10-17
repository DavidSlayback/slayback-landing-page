// The game's one material (flat.rs): a colour, Lambert-shaded by the scene's fixed light plus ambient, or at full
// strength when unlit, plus a glow, optionally checkered. A tiny shader, so WebGL links it in milliseconds where Bevy's
// PBR shader took seconds on ANGLE/D3D11 (decision Q74).
#import bevy_pbr::forward_io::VertexOutput

struct Flat {
    color: vec4<f32>,
    glow: vec4<f32>,
    check: vec4<f32>,
    grid: vec4<f32>,
    lit: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: Flat;

// Toward the key light, as from (5, 14, 10) shining on the origin: above, in front and a little to the right, so the
// two side walls' faces read apart. Soft, as Rumble Box's is (Q75): a face lit from straight above shows its colour
// exactly (AMBIENT + DIFFUSE * LIGHT.y = 1), the front 0.87 of it, the right 0.71 and the faces turned away 0.55.
const LIGHT: vec3<f32> = vec3<f32>(0.2790, 0.7812, 0.5580);
const AMBIENT: f32 = 0.55;
const DIFFUSE: f32 = 0.576;

// Which of two checker colours a point on a face takes: the tiles run across the plane the face lies in.
fn alternate(p: vec3<f32>, n: vec3<f32>) -> bool {
    let a = abs(n);
    let q = (p - material.grid.xyz) / material.grid.w;
    var uv = q.xy;
    if a.y >= a.x && a.y >= a.z {
        uv = q.xz;
    } else if a.x >= a.z {
        uv = q.yz;
    }
    let t = floor(uv);
    return (i32(t.x) + i32(t.y)) % 2 != 0;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Lit meshes are closed (cubes, balls), so the face seen is the one its normal points out of; the flat meshes (the
    // bursts' stars, the ring, the shadow disc) are unlit, so no back face needs lighting.
    let n = normalize(in.world_normal);
    var base = material.color;
    if material.grid.w > 0.0 && alternate(in.world_position.xyz, n) {
        base = vec4<f32>(material.check.rgb, base.a);
    }
    let shade = AMBIENT + DIFFUSE * max(dot(n, LIGHT), 0.0);
    let rgb = base.rgb * mix(1.0, shade, material.lit) + material.glow.rgb;
    return vec4<f32>(rgb, base.a);
}
