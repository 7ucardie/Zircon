//! Zircon Bevy client — Phase 3 proof-of-concept.
//!
//! # Usage
//!
//!   zircon-client [map.map.json]
//!
//! Loads the given `.map.json` (or `Map/default.map.json` if omitted) and
//! renders its tile layout in a Bevy window.
//!
//! Build with the default `windowed` feature for an on-screen window.
//! Build with `--no-default-features` for a headless/CI compile check.

mod map;
#[cfg(feature = "windowed")]
mod render;

use std::path::PathBuf;

#[cfg(feature = "windowed")]
use bevy::prelude::*;

fn main() -> anyhow::Result<()> {
    let map_path: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("Map/default.map.json"));

    let map = map::MapFile::load(&map_path)?;

    println!(
        "Loaded map {:?}: {}×{}, {} walkable cells",
        map_path,
        map.width,
        map.height,
        map.walkable_count()
    );

    #[cfg(feature = "windowed")]
    {
        App::new()
            .add_plugins(DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: format!(
                        "Zircon — {}×{} map",
                        map.width, map.height
                    ),
                    resolution: (1024.0, 768.0).into(),
                    ..default()
                }),
                ..default()
            }))
            .add_plugins(render::MapRenderPlugin { map })
            .run();
    }

    #[cfg(not(feature = "windowed"))]
    {
        println!("(headless build — windowed feature disabled, skipping Bevy app)");
    }

    Ok(())
}
