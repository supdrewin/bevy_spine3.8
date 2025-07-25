//! Materials for Spine meshes.
//!
//! To create a custom material for Spine, see [`SpineMaterial`].

use std::marker::PhantomData;

use bevy::{
    asset::{Asset, weak_handle},
    ecs::system::{StaticSystemParam, SystemParam},
    prelude::*,
    reflect::TypePath,
    render::{
        mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef},
        render_resource::{
            AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState,
            RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError, VertexFormat,
        },
    },
    sprite::{AlphaMode2d, Material2d, Material2dKey},
};
use rusty_spine::BlendMode;

use crate::{SpineMesh, SpineMeshState, SpineSettings, SpineSystem};

/// Trait for automatically applying materials to [`SpineMesh`] entities. Used by the built-in
/// materials but can also be used to create custom materials.
///
/// Implement the trait and add it with [`SpineMaterialPlugin`].
pub trait SpineMaterial: Sized {
    type MeshMaterial: Component
        + Clone
        + Into<AssetId<Self::Material>>
        + From<Handle<Self::Material>>;
    /// The material type to apply to [`SpineMesh`]. Usually is `Self`.
    type Material: Asset + Clone;
    /// System parameters to query when updating this material.
    type Params<'w, 's>: SystemParam;

    /// Ran every frame for every material and every [`SpineMesh`].
    ///
    /// If this function returns [`Some`], then the material will be applied to the [`SpineMesh`],
    /// otherwise it will be removed. Default materials should be removed if a custom material is
    /// desired (see [`SpineSettings::default_materials`]).
    fn update(
        material: Option<Self::Material>,
        entity: Entity,
        renderable_data: SpineMaterialInfo,
        params: &StaticSystemParam<Self::Params<'_, '_>>,
    ) -> Option<Self::Material>;
}

/// Add support for a new [`SpineMaterial`].
pub struct SpineMaterialPlugin<T: SpineMaterial> {
    _marker: PhantomData<T>,
}

impl<T: SpineMaterial> Default for SpineMaterialPlugin<T> {
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T: SpineMaterial + Send + Sync + 'static> Plugin for SpineMaterialPlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_materials::<T>
                .in_set(SpineSystem::UpdateMaterials)
                .after(SpineSystem::UpdateMeshes),
        );
    }
}

/// Info necessary for a Spine material.
#[derive(Debug, Clone)]
pub struct SpineMaterialInfo {
    pub slot_index: Option<usize>,
    pub texture: Handle<Image>,
    pub blend_mode: BlendMode,
    pub premultiplied_alpha: bool,
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update_materials<T: SpineMaterial>(
    mut commands: Commands,
    mut materials: ResMut<Assets<T::Material>>,
    mesh_query: Query<(Entity, &SpineMesh, Option<&T::MeshMaterial>)>,
    params: StaticSystemParam<T::Params<'_, '_>>,
) {
    for (mesh_entity, spine_mesh, material_handle) in mesh_query.iter() {
        let SpineMeshState::Renderable { info: data } = spine_mesh.state.clone() else {
            continue;
        };
        if let Some((material, handle)) =
            material_handle.and_then(|handle| materials.get_mut(handle.clone()).zip(Some(handle)))
        {
            if let Some(new_material) = T::update(
                Some(material.clone()),
                spine_mesh.spine_entity,
                data,
                &params,
            ) {
                *material = new_material;
            } else {
                materials.remove(handle.clone());
                if let Ok(mut entity_commands) = commands.get_entity(mesh_entity) {
                    entity_commands.remove::<T::MeshMaterial>();
                }
            }
        } else if let Some(material) = T::update(None, spine_mesh.spine_entity, data, &params) {
            let handle = materials.add(material);
            if let Ok(mut entity_commands) = commands.get_entity(mesh_entity) {
                entity_commands
                    .insert(<T::MeshMaterial as From<Handle<T::Material>>>::from(handle));
            }
        };
    }
}

pub const DARK_COLOR_SHADER_POSITION: u64 = 10;
pub const DARK_COLOR_ATTRIBUTE: MeshVertexAttribute = MeshVertexAttribute::new(
    "Vertex_DarkColor",
    DARK_COLOR_SHADER_POSITION,
    VertexFormat::Float32x4,
);

pub const SHADER_HANDLE: Handle<Shader> = weak_handle!("38b42512-1b99-43ed-ad09-6f36bb4ca3f9");

/// A [`SystemParam`] to query [`SpineSettings`].
///
/// Mostly used for the built-in materials but may be useful for implementing other materials.
#[derive(SystemParam)]
pub struct SpineSettingsQuery<'w, 's> {
    pub spine_settings_query: Query<'w, 's, &'static SpineSettings>,
}

macro_rules! material {
    ($(#[$($attrss:tt)*])* $name:ident, $blend_mode:expr, $premultiplied_alpha:expr, $blend_state:expr) => {
        $(#[$($attrss)*])*
        #[derive(Asset, Default, AsBindGroup, TypePath, Clone)]
        pub struct $name {
            #[texture(0)]
            #[sampler(1)]
            pub image: Handle<Image>,
        }

        impl $name {
            pub fn new(image: Handle<Image>) -> Self {
                Self { image }
            }
        }

        impl Material2d for $name {
            fn vertex_shader() -> ShaderRef {
                SHADER_HANDLE.into()
            }

            fn fragment_shader() -> ShaderRef {
                SHADER_HANDLE.into()
            }

            fn alpha_mode(&self) -> AlphaMode2d {
                AlphaMode2d::Blend
            }

            fn specialize(
                descriptor: &mut RenderPipelineDescriptor,
                layout: &MeshVertexBufferLayoutRef,
                _key: Material2dKey<Self>,
            ) -> Result<(), SpecializedMeshPipelineError> {
                let vertex_attributes = vec![
                    Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
                    Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
                    Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
                    Mesh::ATTRIBUTE_COLOR.at_shader_location(4),
                    DARK_COLOR_ATTRIBUTE.at_shader_location(DARK_COLOR_SHADER_POSITION as u32),
                ];
                let vertex_buffer_layout = layout.0.get_layout(&vertex_attributes)?;
                descriptor.vertex.buffers = vec![vertex_buffer_layout];
                if let Some(fragment) = &mut descriptor.fragment {
                    if let Some(target_state) = &mut fragment.targets[0] {
                        target_state.blend = Some($blend_state);
                    }
                }
                descriptor.primitive.cull_mode = None;
                Ok(())
            }
        }

        impl SpineMaterial for $name {
            type MeshMaterial = MeshMaterial2d<Self>;
            type Material = Self;
            type Params<'w, 's> = SpineSettingsQuery<'w, 's>;

            fn update(
                material: Option<Self>,
                entity: Entity,
                renderable_data: SpineMaterialInfo,
                params: &StaticSystemParam<Self::Params<'_, '_>>,
            ) -> Option<Self> {
                let spine_settings = params.spine_settings_query.get(entity).copied().unwrap_or(SpineSettings::default());
                if spine_settings.default_materials && renderable_data.blend_mode == $blend_mode && renderable_data.premultiplied_alpha == $premultiplied_alpha {
                    let mut material = material.unwrap_or_else(|| Self::default());
                    material.image = renderable_data.texture;
                    Some(material)
                } else {
                    None
                }
            }
        }
    };
}

material!(
    /// Normal blend mode material, non-premultiplied-alpha
    SpineNormalMaterial,
    BlendMode::Normal,
    false,
    BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
    }
);

material!(
    /// Additive blend mode material, non-premultiplied-alpha
    SpineAdditiveMaterial,
    BlendMode::Additive,
    false,
    BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
    }
);

material!(
    /// Multiply blend mode material, non-premultiplied-alpha
    SpineMultiplyMaterial,
    BlendMode::Multiply,
    false,
    BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::Dst,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::OneMinusSrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
    }
);

material!(
    /// Screen blend mode material, non-premultiplied-alpha
    SpineScreenMaterial,
    BlendMode::Screen,
    false,
    BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::OneMinusSrc,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
    }
);

material!(
    /// Normal blend mode material, premultiplied-alpha
    SpineNormalPmaMaterial,
    BlendMode::Normal,
    true,
    BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
    }
);

material!(
    /// Additive blend mode material, premultiplied-alpha
    SpineAdditivePmaMaterial,
    BlendMode::Additive,
    true,
    BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
    }
);

material!(
    /// Multiply blend mode material, premultiplied-alpha
    SpineMultiplyPmaMaterial,
    BlendMode::Multiply,
    true,
    BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::Dst,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::OneMinusSrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
    }
);

material!(
    /// Screen blend mode material, premultiplied-alpha
    SpineScreenPmaMaterial,
    BlendMode::Screen,
    true,
    BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::OneMinusSrc,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
    }
);
