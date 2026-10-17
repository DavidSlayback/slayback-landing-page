//! The game's one material (decision Q74): a colour, Lambert-shaded by the scene's fixed key light plus ambient, or
//! at full strength when unlit, plus a glow, drawn on both faces; optionally checkered with a second colour (a box's
//! tiles, Q75). It replaces Bevy's `StandardMaterial` everywhere:
//! the look is flat-coloured cubes, and the PBR shader cost ~3 s to link on ANGLE/D3D11 each time WebGL met a new
//! variant of it, where this one links in milliseconds.

use bevy::asset::{AssetPath, embedded_asset, embedded_path};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::shader::ShaderRef;

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct Flat {
    /// Linear colour; its alpha is the opacity under [`AlphaMode::Blend`].
    #[uniform(0)]
    pub color: LinearRgba,
    /// Added on top, unshaded (a lit bomb's blink).
    #[uniform(0)]
    pub glow: LinearRgba,
    /// The checker's other colour, on alternate tiles of [`Flat::grid`].
    #[uniform(0)]
    pub check: LinearRgba,
    /// A world-space checker across whichever plane a face lies in: a tile corner (xyz) and the tile size (w); size 0
    /// is no checker.
    #[uniform(0)]
    pub grid: Vec4,
    /// 1 shaded by the light, 0 at full strength.
    #[uniform(0)]
    pub lit: f32,
    pub alpha: AlphaMode,
}

impl Flat {
    /// Shaded by the light: surfaces and fighters.
    pub fn lit(color: Color) -> Self {
        Self {
            color: color.into(),
            glow: LinearRgba::BLACK,
            check: LinearRgba::BLACK,
            grid: Vec4::ZERO,
            lit: 1.0,
            alpha: AlphaMode::Opaque,
        }
    }

    /// At full strength: marks, glows, lines.
    pub fn unlit(color: Color) -> Self {
        Self { lit: 0.0, ..Self::lit(color) }
    }

    /// See-through, by the colour's alpha.
    pub fn blend(self) -> Self {
        Self { alpha: AlphaMode::Blend, ..self }
    }
}

impl Material for Flat {
    fn fragment_shader() -> ShaderRef {
        // Embedded in the binary (`plugin`), so from the `embedded://` source, not the assets folder.
        ShaderRef::Path(AssetPath::from_path_buf(embedded_path!("flat.wgsl")).with_source("embedded"))
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha
    }

    /// Both faces: the bursts' stars are flat, and a closed cube never shows its back faces anyway.
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "flat.wgsl");
    app.add_plugins(MaterialPlugin::<Flat>::default());
}
