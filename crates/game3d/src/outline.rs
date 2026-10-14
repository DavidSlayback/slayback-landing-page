//! Comic outlines (decision Q50), as one post-process pass that runs under WebGL2 (spike S1, research section 7).
//!
//! A fullscreen pass after the main pass reads the camera's own depth texture and darkens pixels where depth is not
//! locally planar. Reverse-Z NDC depth is affine in screen space across any plane, so its second difference is ~0
//! inside a face and large at silhouettes (depth jumps) and creases (slope changes): every edge of a flat-shaded
//! cube, from one depth texture and no prepass. The widths are set in [`EdgeOutline`].
//!
//! What WebGL2 forces (Bevy 0.19.1):
//! - Bevy's depth prepass is unusable: it copies depth with `copy_texture_to_texture`, which WebGL2 can't do
//!   (bevy#9710). The main depth texture is readable once the camera adds `TEXTURE_BINDING` to its usages.
//! - MSAA off: GLSL ES 3.00 can't sample a multisampled texture. No FXAA either: a second pass costs 1-1.7 ms, and the
//!   bold lines hide most of the aliasing.
//! - Depth is read through a NEAREST sampler (see `outline.wgsl`).
//!
//! [`plugin`] outlines every 3D camera. Modelled on `bevy_core_pipeline::fullscreen_material` (0.19), plus the
//! depth binding.

use bevy::asset::{embedded_asset, load_embedded_asset};
use bevy::core_pipeline::tonemapping::tonemapping;
use bevy::core_pipeline::{Core3d, Core3dSystems, FullscreenShader};
use bevy::prelude::*;
use bevy::render::extract_component::{
    ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin,
};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::{
    BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedRenderPipelineId, ColorTargetState,
    ColorWrites, FragmentState, Operations, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor,
    RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, ShaderType,
    SpecializedRenderPipeline, SpecializedRenderPipelines, TextureFormat, TextureSampleType, TextureUsages,
};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::{ExtractedView, Msaa, ViewDepthTexture, ViewTarget};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};

/// On a camera: draw outlines. Thresholds are relative second differences of depth.
#[derive(Component, ExtractComponent, ShaderType, Clone, Copy, Debug)]
pub struct EdgeOutline {
    pub color: Vec4,
    /// How far out silhouettes are sampled, whole pixels (lines come out ~2x that wide).
    pub width: f32,
    /// Edges above this are creases, drawn `crease_width` px out.
    pub crease: f32,
    /// Edges above this are silhouettes, drawn `width` px out.
    pub silhouette: f32,
    /// How far out creases are sampled, whole pixels.
    pub crease_width: f32,
}

impl Default for EdgeOutline {
    /// Rumble Box's thick black edges (Q66): ~6 px silhouettes and ~4 px creases at 720p.
    fn default() -> Self {
        Self { color: Vec4::new(0.0, 0.0, 0.0, 1.0), width: 3.0, crease: 0.0008, silhouette: 0.008, crease_width: 2.0 }
    }
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "outline.wgsl");
    app.add_plugins((
        ExtractComponentPlugin::<EdgeOutline>::default(),
        UniformComponentPlugin::<EdgeOutline>::default(),
    ))
    .add_systems(PostUpdate, outline_cameras);
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
    render_app
        .init_resource::<SpecializedRenderPipelines<OutlinePipeline>>()
        .add_systems(RenderStartup, init_pipeline)
        .add_systems(Render, prepare_pipelines.in_set(RenderSystems::Prepare))
        .add_systems(Core3d, outline_pass.in_set(Core3dSystems::PostProcess).before(tonemapping));
}

/// Gives each new 3D camera outlines, with the bindable, single-sampled depth texture the pass reads.
fn outline_cameras(mut commands: Commands, mut cameras: Query<(Entity, &mut Camera3d), Added<Camera3d>>) {
    for (e, mut camera) in &mut cameras {
        let usages = TextureUsages::from(camera.depth_texture_usages) | TextureUsages::TEXTURE_BINDING;
        camera.depth_texture_usages = usages.into();
        commands.entity(e).insert((Msaa::Off, EdgeOutline::default()));
    }
}

#[derive(Resource)]
struct OutlinePipeline {
    layout: BindGroupLayoutDescriptor,
    fullscreen: FullscreenShader,
    shader: Handle<Shader>,
    nearest: Sampler,
}

impl SpecializedRenderPipeline for OutlinePipeline {
    type Key = TextureFormat;

    fn specialize(&self, format: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("edge_outline".into()),
            layout: vec![self.layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                targets: vec![Some(ColorTargetState { format, blend: None, write_mask: ColorWrites::ALL })],
                ..default()
            }),
            ..default()
        }
    }
}

fn init_pipeline(
    mut commands: Commands,
    fullscreen: Res<FullscreenShader>,
    assets: Res<AssetServer>,
    device: Res<RenderDevice>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "edge_outline_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                // Depth32Float bound as unfilterable float (allowed by WebGPU), so WebGL2 gets a plain sampler2D
                // rather than a shadow sampler.
                texture_2d(TextureSampleType::Float { filterable: false }),
                sampler(SamplerBindingType::NonFiltering),
                uniform_buffer::<EdgeOutline>(true),
            ),
        ),
    );
    commands.insert_resource(OutlinePipeline {
        layout,
        fullscreen: fullscreen.clone(),
        shader: load_embedded_asset!(assets.as_ref(), "outline.wgsl"),
        // The default sampler filters NEAREST.
        nearest: device.create_sampler(&SamplerDescriptor::default()),
    });
}

#[derive(Component)]
struct OutlinePipelineId(CachedRenderPipelineId);

fn prepare_pipelines(
    mut commands: Commands,
    cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<OutlinePipeline>>,
    pipeline: Res<OutlinePipeline>,
    views: Query<(Entity, &ExtractedView), With<EdgeOutline>>,
) {
    for (e, view) in &views {
        let id = pipelines.specialize(&cache, &pipeline, view.target_format);
        commands.entity(e).insert(OutlinePipelineId(id));
    }
}

fn outline_pass(
    view: ViewQuery<(&ViewTarget, &ViewDepthTexture, &DynamicUniformIndex<EdgeOutline>, &OutlinePipelineId)>,
    pipeline: Res<OutlinePipeline>,
    cache: Res<PipelineCache>,
    uniforms: Res<ComponentUniforms<EdgeOutline>>,
    mut ctx: RenderContext,
) {
    let (target, depth, index, id) = view.into_inner();
    let (Some(render_pipeline), Some(settings)) = (cache.get_render_pipeline(id.0), uniforms.uniforms().binding())
    else {
        return;
    };
    let post = target.post_process_write();
    let bind_group = ctx.render_device().create_bind_group(
        "edge_outline_bind_group",
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((post.source, depth.view(), &pipeline.nearest, settings)),
    );
    let mut pass = ctx.command_encoder().begin_render_pass(&RenderPassDescriptor {
        label: Some("edge_outline"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: post.destination,
            depth_slice: None,
            resolve_target: None,
            ops: Operations::default(),
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(render_pipeline);
    pass.set_bind_group(0, &bind_group, &[index.index()]);
    pass.draw(0..3, 0..1);
}
