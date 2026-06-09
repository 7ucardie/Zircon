//! Game scene state machine — Login → CharacterSelect → Game.
//!
//! Uses Bevy `States` to drive full-screen overlay panels.  The underlying
//! game world (map tiles, player, HUD) is always spawned; the overlays
//! simply sit on top until the player navigates to `GameScene::Game`.
//!
//! # Transitions
//!
//! | From              | To              | Trigger                              |
//! |-------------------|-----------------|--------------------------------------|
//! | Login             | CharacterSelect | Enter key  OR  GoodVersion from net  |
//! | CharacterSelect   | Game            | Enter / 1 / 2 / 3 keys              |
//! | CharacterSelect   | Login           | Escape key  OR  Disconnect           |
//! | Game              | Login           | Disconnect from server               |
use bevy::prelude::*;

use crate::game_state::GameScene;
use crate::network::{ConnectionStatus, NetworkHandle};

pub use crate::game_state::scene_title;

// ── Marker components ─────────────────────────────────────────────────────────

#[derive(Component)] struct LoginPanel;
#[derive(Component)] struct CharSelectPanel;

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct ScenesPlugin;

impl Plugin for ScenesPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameScene>()
            // Spawn both overlays once; initial visibilities match the Login state.
            .add_systems(Startup, setup_panels)
            // Visibility bookkeeping on transitions
            .add_systems(OnEnter(GameScene::Login),           show::<LoginPanel>)
            .add_systems(OnExit(GameScene::Login),            hide::<LoginPanel>)
            .add_systems(OnEnter(GameScene::CharacterSelect), show::<CharSelectPanel>)
            .add_systems(OnExit(GameScene::CharacterSelect),  hide::<CharSelectPanel>)
            // Per-scene input
            .add_systems(
                Update,
                (
                    login_input.run_if(in_state(GameScene::Login)),
                    char_select_input.run_if(in_state(GameScene::CharacterSelect)),
                    // Network-driven auto-advance from Login
                    auto_advance_on_ready.run_if(in_state(GameScene::Login)),
                    // Network-driven fallback to Login on disconnect
                    fallback_on_disconnect
                        .run_if(not(in_state(GameScene::Login))),
                ),
            );
    }
}

// ── Panel setup ───────────────────────────────────────────────────────────────

const OVERLAY_BG: Color = Color::srgba(0.04, 0.04, 0.08, 0.97);
const TITLE_COLOR: Color = Color::srgb(0.9, 0.75, 0.3);
const BODY_COLOR:  Color = Color::srgb(0.75, 0.75, 0.75);
const HINT_COLOR:  Color = Color::srgb(0.5, 0.5, 0.5);

fn setup_panels(mut commands: Commands) {
    // ── Login overlay ────────────────────────────────────────────────────
    commands
        .spawn((
            LoginPanel,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(OVERLAY_BG),
            GlobalZIndex(10),
            Visibility::Visible,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("ZIRCON"),
                TextFont { font_size: 52.0, ..default() },
                TextColor(TITLE_COLOR),
            ));
            p.spawn((
                Text::new("Cross-platform Mir3 Client"),
                TextFont { font_size: 16.0, ..default() },
                TextColor(BODY_COLOR),
            ));
            p.spawn((
                Text::new(" "),
                TextFont { font_size: 10.0, ..default() },
                TextColor(HINT_COLOR),
            ));
            p.spawn((
                Text::new("[Enter]  Connect to server"),
                TextFont { font_size: 15.0, ..default() },
                TextColor(HINT_COLOR),
            ));
        });

    // ── CharacterSelect overlay ──────────────────────────────────────────
    commands
        .spawn((
            CharSelectPanel,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(OVERLAY_BG),
            GlobalZIndex(10),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Select Character"),
                TextFont { font_size: 32.0, ..default() },
                TextColor(TITLE_COLOR),
            ));
            p.spawn((
                Text::new(" "),
                TextFont { font_size: 8.0, ..default() },
                TextColor(HINT_COLOR),
            ));
            // Three character slots
            for (i, name) in ["Warrior — Lv 45", "Wizard — Lv 32", "— empty —"]
                .iter()
                .enumerate()
            {
                p.spawn((
                    Node {
                        width: Val::Px(320.0),
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.12, 0.12, 0.18, 1.0)),
                    BorderColor(Color::srgb(0.3, 0.3, 0.4)),
                ))
                .with_children(|slot| {
                    slot.spawn((
                        Text::new(format!("[{}]  {name}", i + 1)),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(BODY_COLOR),
                    ));
                });
            }
            p.spawn((
                Text::new(" "),
                TextFont { font_size: 8.0, ..default() },
                TextColor(HINT_COLOR),
            ));
            p.spawn((
                Text::new("[1/2/3] or [Enter]  Play    [Esc]  Back"),
                TextFont { font_size: 13.0, ..default() },
                TextColor(HINT_COLOR),
            ));
        });
}

// ── Visibility helpers ────────────────────────────────────────────────────────

fn show<M: Component>(mut q: Query<&mut Visibility, With<M>>) {
    for mut v in &mut q {
        *v = Visibility::Visible;
    }
}

fn hide<M: Component>(mut q: Query<&mut Visibility, With<M>>) {
    for mut v in &mut q {
        *v = Visibility::Hidden;
    }
}

// ── Input systems ─────────────────────────────────────────────────────────────

fn login_input(keyboard: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<GameScene>>) {
    if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::NumpadEnter) {
        next.set(GameScene::CharacterSelect);
    }
}

fn char_select_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<GameScene>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next.set(GameScene::Login);
        return;
    }
    let advance = keyboard.just_pressed(KeyCode::Enter)
        || keyboard.just_pressed(KeyCode::NumpadEnter)
        || keyboard.just_pressed(KeyCode::Digit1)
        || keyboard.just_pressed(KeyCode::Digit2)
        || keyboard.just_pressed(KeyCode::Digit3);
    if advance {
        next.set(GameScene::Game);
    }
}

// ── Network-driven transitions ────────────────────────────────────────────────

fn auto_advance_on_ready(
    net: Option<Res<NetworkHandle>>,
    mut next: ResMut<NextState<GameScene>>,
) {
    if let Some(net) = net {
        if net.status == ConnectionStatus::Ready {
            next.set(GameScene::CharacterSelect);
        }
    }
}

fn fallback_on_disconnect(
    net: Option<Res<NetworkHandle>>,
    mut next: ResMut<NextState<GameScene>>,
) {
    if let Some(net) = net {
        if matches!(net.status, ConnectionStatus::Disconnected(_)) {
            next.set(GameScene::Login);
        }
    }
}

