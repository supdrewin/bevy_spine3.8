use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use bevy::{
    prelude::*,
    render::{RenderPlugin, settings::WgpuSettings},
    winit::WinitPlugin,
};

use crate::{SpineSet, prelude::*};

pub fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(RenderPlugin {
                render_creation: WgpuSettings {
                    backends: None,
                    ..default()
                }
                .into(),
                ..default()
            })
            .build()
            .disable::<WinitPlugin>(),
        SpinePlugin,
    ));
    app
}

pub fn test_app_with_spineboy() -> App {
    let mut app = test_app();
    app.add_systems(
        Startup,
        |mut commands: Commands,
         mut skeletons: ResMut<Assets<SkeletonData>>,
         asset_server: Res<AssetServer>| {
            let skeleton = SkeletonData::new_from_json(
                asset_server.load("spineboy/export/spineboy-pro.json"),
                asset_server.load("spineboy/export/spineboy.atlas"),
            );
            let skeleton_handle = skeletons.add(skeleton);
            commands.spawn(SpineBundle {
                skeleton: skeleton_handle.into(),
                transform: Transform::from_xyz(0., -200., 0.).with_scale(Vec3::ONE * 0.5),
                ..Default::default()
            });
        },
    );
    let ready = Arc::new(AtomicBool::new(false));
    let ready_inside = ready.clone();
    app.add_systems(
        Update,
        (move |mut spine_ready_events: EventReader<SpineReadyEvent>| {
            for _ in spine_ready_events.read() {
                ready_inside.store(true, Ordering::SeqCst);
            }
        })
        .in_set(SpineSet::OnReady),
    );
    while !ready.load(Ordering::SeqCst) {
        app.update();
    }
    app
}

#[test]
fn spawn() {
    let mut app = test_app_with_spineboy();
    app.add_systems(Update, |spine: Single<&Spine>| {
        assert_eq!(spine.skeleton.data().hash(), "PAYbOkc7GRcWIZQTjcDV7U+dhD8");
    });
    app.update();
}
