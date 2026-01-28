#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod asset_tracking;

use bevy::{asset::AssetMetaCheck, prelude::*};
use crate::asset_tracking::{LoadResource, ResourceHandles};

fn main() -> AppExit {
    App::new().add_plugins(AppPlugin).run()
}

pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        // Add Bevy plugins.
        app.add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    // Wasm builds will check for meta files (that don't exist) if this isn't set.
                    // This causes errors and even panics on web build on itch.
                    // See https://github.com/bevyengine/bevy_github_ci_template/issues/48.
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Window {
                        title: "Pc Case Visualizer".to_string(),
                        fit_canvas_to_parent: true,
                        ..default()
                    }
                        .into(),
                    ..default()
                }),
        );

        // Add other plugins.
        app.add_plugins(
            asset_tracking::plugin
        );
        app.load_resource::<LevelAssets>();
        app.init_state::<Screen>();

        app.add_systems(Update, enter_gameplay_screen.run_if(in_state(Screen::Loading).and(all_assets_loaded)));
        app.add_systems(OnEnter(Screen::Game), (init_spawn, spawn_text_in_ui, sync_orbit_camera_on_spawn).chain());
        app.add_systems(Update, (orbit_camera_system, aim_camera_light).chain().run_if(in_state(Screen::Game)));
    }
}

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, PartialOrd, Ord, Default)]
enum Screen {
    #[default]
    Loading,
    Game
}

fn enter_gameplay_screen(mut next_screen: ResMut<NextState<Screen>>) {
    next_screen.set(Screen::Game);
}

fn all_assets_loaded(resource_handles: Res<ResourceHandles>) -> bool {
    resource_handles.is_all_done()
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct LevelAssets {
    #[dependency]
    pc_case: Handle<Scene>
}

impl FromWorld for LevelAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            pc_case: assets.load(GltfAssetLabel::Scene(0).from_asset("models/pc_case.glb")),
        }
    }
}

#[derive(Component)]
pub struct OrbitCamera {
    pub radius: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
    pub target: Vec3,
}

fn init_spawn(mut commands: Commands, level_assets: Res<LevelAssets>) {
    commands.spawn((
        Name::new("Camera"),
        Camera3d::default(),
        OrbitCamera {
            radius: 900.0,
            yaw: 0.7,
            pitch: 0.4,
            speed: 1.5,
            target: Vec3::new(0.0, 200.0, 0.0),
        },
        Transform::default(),
        children![
        (
            Name::new("Camera Light"),
            SpotLight {
                intensity: 500_000.0,
                range: 5000.0,
                inner_angle: 0.35,
                outer_angle: 0.6,
                shadows_enabled: true,
                ..default()
            },
            Transform::from_xyz(0.0, 50.0, 0.0), // smaller offset
        )
    ]
    ));

    commands.spawn((
        Name::new("Level"),
        Transform::default(),
        Visibility::default(),
        children![
            SceneRoot(level_assets.pc_case.clone()),
        ],
    ));
}

fn spawn_text_in_ui(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(5.0),
            left: px(5.0),
            ..default()
        },
        Text::new("Use 'A' and 'D' to rotate the object."),
        TextColor(Color::WHITE),
        TextLayout::new_with_justify(Justify::Center),
    ));
}

fn sync_orbit_camera_on_spawn(
    mut query: Query<(&OrbitCamera, &mut Transform)>,
) {
    for (orbit, mut transform) in &mut query {
        let x = orbit.radius * orbit.yaw.cos() * orbit.pitch.cos();
        let z = orbit.radius * orbit.yaw.sin() * orbit.pitch.cos();
        let y = orbit.radius * orbit.pitch.sin();

        transform.translation = orbit.target + Vec3::new(x, y, z);
        transform.look_at(orbit.target, Vec3::Y);
    }
}

fn orbit_camera_system(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut OrbitCamera, &mut Transform)>,
) {
    for (mut orbit, mut transform) in &mut query {
        // Input
        let mut direction = 0.0;
        if keys.pressed(KeyCode::KeyA) {
            direction += 1.0;
        }
        if keys.pressed(KeyCode::KeyD) {
            direction -= 1.0;
        }

        // Update yaw
        orbit.yaw += direction * orbit.speed * time.delta_secs();

        // Clamp pitch so we never flip
        orbit.pitch = orbit.pitch.clamp(0.05, 1.2);

        // Spherical → Cartesian
        let cos_pitch = orbit.pitch.cos();
        let sin_pitch = orbit.pitch.sin();

        let x = orbit.radius * orbit.yaw.cos() * cos_pitch;
        let z = orbit.radius * orbit.yaw.sin() * cos_pitch;
        let y = orbit.radius * sin_pitch;

        // Apply transform
        transform.translation = orbit.target + Vec3::new(x, y, z);
        transform.look_at(orbit.target, Vec3::Y);
    }
}

fn aim_camera_light(
    camera_query: Query<(&GlobalTransform, &OrbitCamera)>,
    mut light_query: Query<(&mut Transform, &GlobalTransform), With<SpotLight>>,
) {
    if let Ok((camera_global, orbit)) = camera_query.single() {
        for (mut local_transform, light_global) in &mut light_query {
            let light_pos = light_global.translation();
            let dir = orbit.target - light_pos;

            if dir.length_squared() > 0.0001 {
                let rotation = Quat::from_rotation_arc(Vec3::NEG_Z, dir.normalize());
                local_transform.rotation =
                    camera_global.rotation().inverse() * rotation;
            }
        }
    }
}
