//! Shared game-state types compiled in all build configurations
//! (headless CI and windowed).  Bevy's `States` derive is only available
//! when the `windowed` feature is active; the enum and pure helpers here
//! work without it.

/// The three top-level game scenes.
///
/// Bevy state registration happens in `scenes::ScenesPlugin` (windowed only).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "windowed", derive(bevy::prelude::States))]
pub enum GameScene {
    #[default]
    Login,
    CharacterSelect,
    Game,
}

/// Short display title for each scene.
pub fn scene_title(scene: &GameScene) -> &'static str {
    match scene {
        GameScene::Login           => "Login",
        GameScene::CharacterSelect => "Select Character",
        GameScene::Game            => "Game",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_scene_is_login() {
        assert_eq!(GameScene::default(), GameScene::Login);
    }

    #[test]
    fn scene_titles_are_distinct() {
        let titles = [
            scene_title(&GameScene::Login),
            scene_title(&GameScene::CharacterSelect),
            scene_title(&GameScene::Game),
        ];
        let unique: std::collections::HashSet<_> = titles.iter().collect();
        assert_eq!(unique.len(), titles.len());
    }

    #[test]
    fn scene_title_values() {
        assert_eq!(scene_title(&GameScene::Login), "Login");
        assert!(scene_title(&GameScene::CharacterSelect).contains("Character"));
        assert_eq!(scene_title(&GameScene::Game), "Game");
    }

    #[test]
    fn game_scene_eq_and_hash() {
        use std::collections::HashMap;
        let mut m = HashMap::new();
        m.insert(GameScene::Login, 0u8);
        m.insert(GameScene::CharacterSelect, 1);
        m.insert(GameScene::Game, 2);
        assert_eq!(m[&GameScene::CharacterSelect], 1);
    }
}
