//! Bevy plugins for map tile and sprite rendering.
//!
//! Both plugins are compiled only when the `windowed` feature is active.
use bevy::prelude::*;

use crate::map::MapFile;
use crate::lib_asset::LibFile;

/// Pixel size of one map tile on-screen.
pub const TILE_PX: f32 = 16.0;

/// Maximum dimension (tiles) rendered in each axis for the PoC.
/// Full maps can be thousands of tiles; we cap at 100×100 here.
pub const MAX_RENDER_DIM: u32 = 100;

/// Marker component for spawned tile entities.
#[derive(Component)]
pub struct MapTile;

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

#[derive(Resource)]
struct LoadedMap(MapFile);

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn spawn_tiles(mut commands: Commands, map: Res<LoadedMap>) {
    let map = &map.0;
    let cols = map.width.min(MAX_RENDER_DIM);
    let rows = map.height.min(MAX_RENDER_DIM);

    let offset_x = -(cols as f32 * TILE_PX) / 2.0 + TILE_PX / 2.0;
    let offset_y = (rows as f32 * TILE_PX) / 2.0 - TILE_PX / 2.0;

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
