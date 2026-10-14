//! S1 (#59): edge-detection outlines as a post-process that runs under WebGL2.
//!
//! One fullscreen pass after the main pass reads the camera's own depth texture (no prepass) and darkens pixels where
//! the depth is not locally planar. Reverse-Z NDC depth is affine in screen space across any plane, so its
//! second difference is ~0 inside a face and large at silhouettes (depth jumps) and at creases (slope changes),
//! which covers the cube edges a normal-based detector would find, from a single depth texture.
//!
//! Why not Bevy's prepass textures: under WebGL2 the depth prepass is copied into its own texture with
//! `copy_texture_to_texture`, which fails on WebGL2 (bevy_core_pipeline prepass/node.rs, bevy#9710). The main depth
//! texture is readable once the camera adds `TEXTURE_BINDING` to `Camera3d::depth_texture_usages`, but only without
//! MSAA: WebGL2 (GLSL ES 3.00) has no multisampled texture sampling. So outlined views run `Msaa::Off`, optionally
//! with FXAA after the outline pass.
//!
//! Modelled on `bevy_core_pipeline::fullscreen_material` (0.19), plus the depth binding.

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
    SpecializedRenderPipeline, SpecializedRenderPipelines,
    TextureFormat, TextureSampleType,
};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::{ExtractedView, ViewDepthTexture, ViewTarget};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};

/// On a camera: draw outlines. `width` is the sample offset in pixels (lines come out ~2x that wide),
/// `threshold` the relative second difference of depth that counts as an edge.
#[derive(Component, ExtractComponent, ShaderType, Clone, Copy)]
pub struct EdgeOutline {
    pub color: Vec4,
    pub width: f32,
    pub threshold: f32,
    /// 1 = show the raw edge mask (tuning aid).
    pub debug: f32,
    /// > 0: creases are drawn 1 px out and only edges above this relative value (silhouettes) at `width`.
    pub silhouette: f32,
}

pub struct EdgeOutlinePlugin;

impl Plugin for EdgeOutlinePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "outline.wgsl");
        app.add_plugins((ExtractComponentPlugin::<EdgeOutline>::default(), UniformComponentPlugin::<EdgeOutline>::default()));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<SpecializedRenderPipelines<OutlinePipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare_pipelines.in_set(RenderSystems::Prepare))
            .add_systems(Core3d, outline_pass.in_set(Core3dSystems::PostProcess).before(tonemapping));
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
                // rather than a shadow sampler; read through the NEAREST sampler below (see outline.wgsl).
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
    let (Some(render_pipeline), Some(settings)) = (cache.get_render_pipeline(id.0), uniforms.uniforms().binding()) else {
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
