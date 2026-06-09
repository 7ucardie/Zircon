//! Bevy plugins for map tile, player, and sprite rendering.
//!
//! Compiled only when the `windowed` feature is active.
use bevy::prelude::*;

use crate::map::MapFile;
use crate::lib_asset::LibFile;

/// Pixel size of one map tile on-screen.
pub const TILE_PX: f32 = 16.0;

/// Maximum dimension (tiles) rendered in each axis for the PoC.
pub const MAX_RENDER_DIM: u32 = 100;

/// Marker component for spawned tile entities.
#[derive(Component)]
pub struct MapTile;

// ── Shared resource ──────────────────────────────────────────────────────────

#[derive(Resource)]
pub(crate) struct LoadedMap(pub MapFile);

// ── World-space helpers ──────────────────────────────────────────────────────

/// Convert a grid coordinate to Bevy world-space (origin = map centre).
pub fn grid_to_world(gx: u32, gy: u32, cols: u32, rows: u32) -> Vec2 {
    let ox = -(cols as f32 * TILE_PX) / 2.0 + TILE_PX / 2.0;
    let oy = (rows as f32 * TILE_PX) / 2.0 - TILE_PX / 2.0;
    Vec2::new(ox + gx as f32 * TILE_PX, oy - gy as f32 * TILE_PX)
}

// ── Map render ───────────────────────────────────────────────────────────────

/// Renders a [`MapFile`] as a flat grid of coloured tiles centred on the origin.
pub struct MapRenderPlugin {
    pub map: MapFile,
}

impl Plugin for MapRenderPlugin {
    fn build(&self, app: &mut App) {
        let map = self.map.clone();
        app.insert_resource(LoadedMap(map))
            .add_systems(Startup, (spawn_camera, spawn_tiles).chain());
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn spawn_tiles(mut commands: Commands, map: Res<LoadedMap>) {
    let map = &map.0;
    let cols = map.width.min(MAX_RENDER_DIM);
    let rows = map.height.min(MAX_RENDER_DIM);

    for y in 0..rows {
        for x in 0..cols {
            let walkable = map.is_walkable(x, y);
            let color = if walkable {
                Color::srgb(0.85, 0.85, 0.85)
            } else {
                Color::srgb(0.15, 0.15, 0.2)
            };
            let pos = grid_to_world(x, y, cols, rows);
            commands.spawn((
                MapTile,
                Sprite {
                    color,
                    custom_size: Some(Vec2::splat(TILE_PX - 1.0)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, 0.0),
            ));
        }
    }
    info!(cols, rows, walkable = map.walkable_count(), "map tiles spawned");
}

// ── Player ───────────────────────────────────────────────────────────────────

/// Player entity component storing grid coordinates.
#[derive(Component)]
pub struct Player {
    pub grid_x: u32,
    pub grid_y: u32,
}

/// Seconds between repeated movement steps while a key is held.
const MOVE_REPEAT_SECS: f32 = 0.15;

#[derive(Resource)]
struct MoveTimer(Timer);

/// Adds a player sprite with WASD/arrow-key movement and a camera that follows.
///
/// Requires `MapRenderPlugin` to be added first (needs `LoadedMap` resource).
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MoveTimer(Timer::from_seconds(
            MOVE_REPEAT_SECS,
            TimerMode::Repeating,
        )))
        .add_systems(Startup, spawn_player.after(spawn_tiles))
        .add_systems(Update, (handle_movement, camera_follow).chain());
    }
}

fn first_walkable(map: &MapFile) -> (u32, u32) {
    for y in 0..map.height.min(MAX_RENDER_DIM) {
        for x in 0..map.width.min(MAX_RENDER_DIM) {
            if map.is_walkable(x, y) {
                return (x, y);
            }
        }
    }
    (0, 0)
}

fn spawn_player(mut commands: Commands, map: Res<LoadedMap>) {
    let cols = map.0.width.min(MAX_RENDER_DIM);
    let rows = map.0.height.min(MAX_RENDER_DIM);
    let (sx, sy) = first_walkable(&map.0);
    let pos = grid_to_world(sx, sy, cols, rows);
    commands.spawn((
        Player { grid_x: sx, grid_y: sy },
        Sprite {
            color: Color::srgb(0.2, 0.5, 1.0),
            custom_size: Some(Vec2::splat(TILE_PX - 2.0)),
            ..default()
        },
        Transform::from_xyz(pos.x, pos.y, 1.0),
    ));
}

fn input_direction(keyboard: &ButtonInput<KeyCode>) -> (i32, i32) {
    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        return (0, -1);
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        return (0, 1);
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        return (-1, 0);
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        return (1, 0);
    }
    (0, 0)
}

fn handle_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut timer: ResMut<MoveTimer>,
    mut player_q: Query<(&mut Player, &mut Transform)>,
    map: Res<LoadedMap>,
) {
    let (dx, dy) = input_direction(&keyboard);
    if dx == 0 && dy == 0 {
        timer.0.reset();
        return;
    }

    // Move on first press; thereafter repeat on timer.
    let just_pressed = keyboard.any_just_pressed([
        KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD,
        KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight,
    ]);
    timer.0.tick(time.delta());
    if !just_pressed && !timer.0.just_finished() {
        return;
    }

    let Ok((mut player, mut transform)) = player_q.get_single_mut() else { return };

    let nx = player.grid_x as i32 + dx;
    let ny = player.grid_y as i32 + dy;
    if nx < 0 || ny < 0 {
        return;
    }
    let nx = nx as u32;
    let ny = ny as u32;
    if !map.0.is_walkable(nx, ny) {
        return;
    }

    player.grid_x = nx;
    player.grid_y = ny;

    let cols = map.0.width.min(MAX_RENDER_DIM);
    let rows = map.0.height.min(MAX_RENDER_DIM);
    let pos = grid_to_world(nx, ny, cols, rows);
    transform.translation.x = pos.x;
    transform.translation.y = pos.y;
}

fn camera_follow(
    player_q: Query<&Transform, With<Player>>,
    mut cam_q: Query<&mut Transform, (With<Camera2d>, Without<Player>)>,
) {
    let Ok(pt) = player_q.get_single() else { return };
    let Ok(mut ct) = cam_q.get_single_mut() else { return };
    ct.translation.x = pt.translation.x;
    ct.translation.y = pt.translation.y;
}

// ── Lib-asset sprite render ──────────────────────────────────────────────────

/// Renders a single decoded image from a [`LibFile`] as a Bevy sprite.
pub struct LibRenderPlugin {
    pub lib: LibFile,
    pub image_index: usize,
}

impl Plugin for LibRenderPlugin {
    fn build(&self, app: &mut App) {
        let decoded = self
            .lib
            .decode_image(self.image_index)
            .expect("image index out of range or slot disabled");
        app.insert_resource(LibImageResource { decoded })
            .add_systems(Startup, (spawn_camera, spawn_lib_sprite).chain());
    }
}

#[derive(Resource)]
struct LibImageResource {
    decoded: crate::lib_asset::DecodedImage,
}

fn spawn_lib_sprite(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    res: Res<LibImageResource>,
) {
    let d = &res.decoded;
    let bevy_image = Image::new(
        bevy::render::render_resource::Extent3d {
            width: d.width,
            height: d.height,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        d.rgba.clone(),
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
    );
    let handle = images.add(bevy_image);
    commands.spawn((
        Sprite {
            image: handle,
            ..default()
        },
        Transform::from_xyz(d.offset_x as f32, -(d.offset_y as f32), 0.0),
    ));
}
