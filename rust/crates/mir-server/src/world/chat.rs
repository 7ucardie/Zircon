//! Player chat (Zircon `PlayerObject.Chat`): `/name text` whispers, `!!`
//! group, `!@` global (level 33, 30 s), `!` shout (level 2, 10 s, whole
//! map), otherwise local talk to players in view range with an overhead
//! bubble.

use super::*;
use mir_proto::ChatKind;

/// Zircon `Config.ShoutDelay`.
const SHOUT_DELAY: u64 = 10_000;
const GLOBAL_SHOUT_DELAY: u64 = 30_000;

impl World {
    pub fn chat(&mut self, id: ObjectId, text: String) {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let Some(p) = o.player() else {
            return;
        };
        let (name, level, map, loc) = (p.name.clone(), p.level, o.map, o.location);
        tracing::info!(target: "chat", "{name}: {text}");

        if let Some(rest) = text.strip_prefix('/') {
            // Whisper: "/name message".
            let mut parts = rest.splitn(2, ' ');
            let target_name = parts.next().unwrap_or("").to_string();
            let message = parts.next().unwrap_or("").trim().to_string();
            if target_name.is_empty() {
                return;
            }
            let target = self
                .players()
                .find(|o| {
                    o.player()
                        .is_some_and(|p| p.name.eq_ignore_ascii_case(&target_name))
                })
                .map(|o| o.id);
            match target {
                Some(t) => {
                    self.send_to(id, say(None, ChatKind::WhisperOut, format!("/{rest}")));
                    self.send_to(
                        t,
                        say(None, ChatKind::WhisperIn, format!("{name}=> {message}")),
                    );
                }
                None => self.send_to(
                    id,
                    say(
                        None,
                        ChatKind::System,
                        format!("Could not find {target_name}."),
                    ),
                ),
            }
        } else if let Some(rest) = text.strip_prefix("!!") {
            let members = self.group_members(id);
            if members.is_empty() {
                return;
            }
            let line = format!("{name}: {}", rest.trim_start());
            for m in members {
                self.send_to(m, say(None, ChatKind::Group, line.clone()));
            }
        } else if let Some(rest) = text.strip_prefix("!@") {
            let p = self.objects[&id].player().unwrap();
            if self.now < p.global_shout_time {
                let wait = p.global_shout_time.saturating_sub(self.now).div_ceil(1000);
                self.send_to(
                    id,
                    say(
                        None,
                        ChatKind::System,
                        format!("You cannot global shout for another {wait} seconds."),
                    ),
                );
                return;
            }
            if level < 33 {
                self.send_to(
                    id,
                    say(
                        None,
                        ChatKind::System,
                        "You need to be level 33 to global shout.".into(),
                    ),
                );
                return;
            }
            self.objects
                .get_mut(&id)
                .and_then(|o| o.player_mut())
                .unwrap()
                .global_shout_time = self.now + GLOBAL_SHOUT_DELAY;
            let line = format!("(!@){name}: {}", rest.trim_start());
            let ids: Vec<ObjectId> = self.players().map(|o| o.id).collect();
            for t in ids {
                self.send_to(t, say(None, ChatKind::Global, line.clone()));
            }
        } else if let Some(rest) = text.strip_prefix('!') {
            let p = self.objects[&id].player().unwrap();
            if self.now < p.shout_time {
                let wait = p.shout_time.saturating_sub(self.now).div_ceil(1000);
                self.send_to(
                    id,
                    say(
                        None,
                        ChatKind::System,
                        format!("You cannot shout for another {wait} seconds."),
                    ),
                );
                return;
            }
            if level < 2 {
                self.send_to(
                    id,
                    say(
                        None,
                        ChatKind::System,
                        "You need to be level 2 to shout.".into(),
                    ),
                );
                return;
            }
            self.objects
                .get_mut(&id)
                .and_then(|o| o.player_mut())
                .unwrap()
                .shout_time = self.now + SHOUT_DELAY;
            let line = format!("(!){name}: {}", rest.trim_start());
            let ids: Vec<ObjectId> = self
                .on_map(map)
                .filter(|o| o.player().is_some())
                .map(|o| o.id)
                .collect();
            for t in ids {
                self.send_to(t, say(None, ChatKind::Shout, line.clone()));
            }
        } else {
            let line = format!("{name}: {text}");
            let ids: Vec<ObjectId> = self
                .on_map(map)
                .filter(|o| o.player().is_some() && o.location.distance(loc) <= MAX_VIEW_RANGE)
                .map(|o| o.id)
                .collect();
            for t in ids {
                self.send_to(t, say(Some(id), ChatKind::Normal, line.clone()));
            }
        }
    }
}

fn say(id: Option<ObjectId>, kind: ChatKind, text: String) -> ServerMessage {
    ServerMessage::Say { id, kind, text }
}
