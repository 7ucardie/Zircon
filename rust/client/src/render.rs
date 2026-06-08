//! Map tile rendering plugin for the Bevy client.
//!
//! Spawns one `Sprite` per cell in the loaded map:
//!   - walkable cells → light colour
//!   - blocked cells  → dark colour
//!
//! Only included when the `windowed` feature is enabled.

use bevy::prelude::*;

use crate::map::MapFile;

/// Pixel size of one map tile on-screen.
pub const TILE_PX: f32 = 16.0;

/// Maximum dimension (tiles) rendered in each axis for the PoC.
/// Full maps can be thousands of tiles; we cap at 100×100 here.
pub const MAX_RENDER_DIM: u32 = 100;

/// Marker component so we can identify spawned tile entities.
#[derive(Component)]
pub struct MapTile;

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

#[derive(Resource)]
struct LoadedMap(MapFile);

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn spawn_tiles(mut commands: Commands, map: Res<LoadedMap>) {
    let map = &map.0;
    let cols = map.width.min(MAX_RENDER_DIM);
    let rows = map.height.min(MAX_RENDER_DIM);

    // Centre the tile grid around the origin.
    let offset_x = -(cols as f32 * TILE_PX) / 2.0;
    let offset_y = (rows as f32 * TILE_PX) / 2.0;

    for y in 0..rows {
        for x in 0..cols {
            let walkable = map.is_walkable(x, y);
            let color = if walkable {
                Color::srgb(0.85, 0.85, 0.85) // light grey — walkable
            } else {
                Color::srgb(0.15, 0.15, 0.2)  // near-black — blocked
            };

            commands.spawn((
                MapTile,
                Sprite {
                    color,
                    custom_size: Some(Vec2::splat(TILE_PX - 1.0)),
                    ..default()
                },
                Transform::from_xyz(
                    offset_x + x as f32 * TILE_PX,
                    offset_y - y as f32 * TILE_PX,
                    0.0,
                ),
            ));
        }
    }

    info!(
        cols,
        rows,
        walkable = map.walkable_count(),
        "map tiles spawned"
    );
}
