mod map;
mod lib_asset;
mod network;

#[cfg(feature = "windowed")]
mod render;

#[cfg(feature = "windowed")]
use bevy::prelude::*;

fn main() {
    let mut args = std::env::args().skip(1);
    let first = args.next();
    let second = args.next();

    match (first.as_deref(), second.as_deref()) {
        // --lib <path> [index]  — display a single sprite from a .zl file
        (Some("--lib"), Some(lib_path)) => {
            let index: usize = args
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            #[cfg(feature = "windowed")]
            {
                let lib = lib_asset::LibFile::load(std::path::Path::new(lib_path))
                    .expect("failed to load lib file");
                App::new()
                    .add_plugins(DefaultPlugins.set(WindowPlugin {
                        primary_window: Some(Window {
                            title: format!("Zircon — {lib_path}[{index}]"),
                            ..default()
                        }),
                        ..default()
                    }))
                    .add_plugins(render::LibRenderPlugin { lib, image_index: index })
                    .run();
            }
            #[cfg(not(feature = "windowed"))]
            {
                let lib = lib_asset::LibFile::load(std::path::Path::new(lib_path))
                    .expect("failed to load lib file");
                println!("LibFile: version={} images={}", lib.version, lib.len());
                if let Some(img) = lib.decode_image(index) {
                    println!("  [{}] {}×{} offset ({},{})", index, img.width, img.height, img.offset_x, img.offset_y);
                } else {
                    println!("  [{}] disabled or out of range", index);
                }
            }
        }

        // <map.json> [server_addr]  — render a map with player + optional network
        (Some(map_path), _) => {
            let server_addr = second.as_deref().map(str::to_owned);
            let map = map::MapFile::load(std::path::Path::new(map_path))
                .expect("failed to load map file");

            #[cfg(feature = "windowed")]
            {
                let mut app = App::new();
                app.add_plugins(DefaultPlugins.set(WindowPlugin {
                    primary_window: Some(Window {
                        title: format!("Zircon — {map_path} (WASD/arrows to move)"),
                        resolution: (1024.0, 768.0).into(),
                        ..default()
                    }),
                    ..default()
                }))
                .add_plugins(render::MapRenderPlugin { map })
                .add_plugins(render::PlayerPlugin);

                if let Some(addr) = server_addr {
                    app.add_plugins(network::NetworkPlugin { server_addr: addr });
                }

                app.run();
            }
            #[cfg(not(feature = "windowed"))]
            {
                println!(
                    "Map {}×{}: {} walkable tiles",
                    map.width,
                    map.height,
                    map.walkable_count()
                );
            }
        }

        // no args — print usage
        _ => {
            eprintln!("Usage:");
            eprintln!("  zircon-client <map.json> [server_addr]");
            eprintln!("  zircon-client --lib <file.zl> [image_index]");
        }
    }
}
