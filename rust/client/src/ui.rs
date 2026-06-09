//! Bevy UI — HUD bars, inventory grid, NPC dialog, and connection status.
//!
//! Compiled only when the `windowed` feature is active.
use bevy::prelude::*;

use crate::network::{bar_pct, status_label, ConnectionStatus, NetworkHandle};

// ── Resources ─────────────────────────────────────────────────────────────────

/// Game stat values; update this resource to drive the HUD bars.
#[derive(Resource)]
pub struct PlayerStats {
    pub hp: u32,
    pub max_hp: u32,
    pub mp: u32,
    pub max_mp: u32,
}

impl Default for PlayerStats {
    fn default() -> Self {
        Self { hp: 75, max_hp: 100, mp: 40, max_mp: 100 }
    }
}

// ── Marker components ─────────────────────────────────────────────────────────

#[derive(Component)] struct StatusText;
#[derive(Component)] struct HealthFill;
#[derive(Component)] struct ManaFill;
#[derive(Component)] struct HealthText;
#[derive(Component)] struct ManaText;
#[derive(Component)] struct InventoryRoot;
#[derive(Component)] struct DialogRoot;

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerStats>()
            .add_systems(Startup, setup_ui)
            .add_systems(
                Update,
                (update_status_hud, update_bars, toggle_inventory, toggle_dialog),
            );
    }
}

// ── Setup ─────────────────────────────────────────────────────────────────────

fn setup_ui(mut commands: Commands) {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            ..default()
        })
        .with_children(|root| {
            // ── Status overlay (top-right) ───────────────────────────────
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(8.0),
                    top: Val::Px(8.0),
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
            ))
            .with_children(|p| {
                p.spawn((
                    StatusText,
                    Text::new("Status: Connecting…"),
                    TextFont { font_size: 13.0, ..default() },
                    TextColor(Color::srgb(0.75, 0.75, 0.75)),
                ));
            });

            // ── Bottom HUD bars ──────────────────────────────────────────
            root.spawn(Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(8.0),
                left: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|hud| {
                spawn_bar(hud, HealthFill, HealthText, "HP",
                          Color::srgb(0.7, 0.1, 0.1), Color::srgb(0.22, 0.05, 0.05),
                          75, 100);
                spawn_bar(hud, ManaFill, ManaText, "MP",
                          Color::srgb(0.1, 0.2, 0.8), Color::srgb(0.05, 0.07, 0.25),
                          40, 100);
            });

            // ── Inventory panel (press I) ────────────────────────────────
            root.spawn((
                InventoryRoot,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Percent(50.0),
                    margin: UiRect {
                        left: Val::Px(-100.0),
                        top: Val::Px(-118.0),
                        ..default()
                    },
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(8.0)),
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.08, 0.13, 0.95)),
                Visibility::Hidden,
            ))
            .with_children(|inv| {
                inv.spawn((
                    Text::new("Inventory   [I] close"),
                    TextFont { font_size: 13.0, ..default() },
                    TextColor(Color::srgb(0.9, 0.9, 0.9)),
                ));
                // 5 rows × 6 columns
                inv.spawn(Node {
                    display: Display::Grid,
                    grid_template_columns: RepeatedGridTrack::px(6, 28.0),
                    column_gap: Val::Px(2.0),
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|grid| {
                    for _ in 0..30 {
                        grid.spawn((
                            Node {
                                width: Val::Px(28.0),
                                height: Val::Px(28.0),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.13, 0.13, 0.18)),
                            BorderColor(Color::srgb(0.32, 0.32, 0.38)),
                        ));
                    }
                });
            });

            // ── NPC dialog panel (press E) ───────────────────────────────
            root.spawn((
                DialogRoot,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(62.0),
                    left: Val::Percent(50.0),
                    margin: UiRect { left: Val::Px(-200.0), ..default() },
                    width: Val::Px(400.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(12.0)),
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.04, 0.1, 0.96)),
                Visibility::Hidden,
            ))
            .with_children(|dlg| {
                dlg.spawn((
                    Text::new("Elder Vorn"),
                    TextFont { font_size: 14.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.85, 0.4)),
                ));
                dlg.spawn((
                    Text::new("Greetings, adventurer. The lands beyond the\neastern ridge grow dangerous. Tread carefully."),
                    TextFont { font_size: 13.0, ..default() },
                    TextColor(Color::srgb(0.85, 0.85, 0.85)),
                ));
                dlg.spawn((
                    Text::new("[E] Close"),
                    TextFont { font_size: 12.0, ..default() },
                    TextColor(Color::srgb(0.55, 0.55, 0.55)),
                ));
            });
        });
}

// ── Bar helper ────────────────────────────────────────────────────────────────

fn spawn_bar<F: Component, T: Component>(
    parent: &mut ChildBuilder,
    fill_marker: F,
    text_marker: T,
    label: &str,
    fill_color: Color,
    bg_color: Color,
    current: u32,
    max: u32,
) {
    parent
        .spawn((
            Node {
                width: Val::Px(160.0),
                height: Val::Px(16.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(bg_color),
        ))
        .with_children(|bar| {
            bar.spawn((
                fill_marker,
                Node {
                    width: Val::Percent(bar_pct(current, max)),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(fill_color),
            ));
            bar.spawn((
                text_marker,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                Text::new(format!("{label}: {current}/{max}")),
                TextFont { font_size: 11.0, ..default() },
                TextColor(Color::WHITE),
            ));
        });
}

// ── Update systems ────────────────────────────────────────────────────────────

fn update_status_hud(
    net: Option<Res<NetworkHandle>>,
    mut q: Query<&mut Text, With<StatusText>>,
) {
    let label = net
        .as_ref()
        .map(|h| status_label(&h.status))
        .unwrap_or_else(|| "Offline".to_string());
    for mut text in &mut q {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
}

fn update_bars(
    stats: Res<PlayerStats>,
    mut hp_fill: Query<&mut Node, With<HealthFill>>,
    mut mp_fill: Query<&mut Node, (With<ManaFill>, Without<HealthFill>)>,
    mut hp_text: Query<&mut Text, (With<HealthText>, Without<ManaText>)>,
    mut mp_text: Query<&mut Text, (With<ManaText>, Without<HealthText>)>,
) {
    if !stats.is_changed() {
        return;
    }
    for mut node in &mut hp_fill {
        node.width = Val::Percent(bar_pct(stats.hp, stats.max_hp));
    }
    for mut node in &mut mp_fill {
        node.width = Val::Percent(bar_pct(stats.mp, stats.max_mp));
    }
    for mut text in &mut hp_text {
        text.0 = format!("HP: {}/{}", stats.hp, stats.max_hp);
    }
    for mut text in &mut mp_text {
        text.0 = format!("MP: {}/{}", stats.mp, stats.max_mp);
    }
}

fn toggle_inventory(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut q: Query<&mut Visibility, With<InventoryRoot>>,
) {
    if keyboard.just_pressed(KeyCode::KeyI) {
        for mut vis in &mut q {
            *vis = if *vis == Visibility::Hidden {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

fn toggle_dialog(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut q: Query<&mut Visibility, With<DialogRoot>>,
) {
    if keyboard.just_pressed(KeyCode::KeyE) {
        for mut vis in &mut q {
            *vis = if *vis == Visibility::Hidden {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

