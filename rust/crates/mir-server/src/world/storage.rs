//! Account storage (Zircon `Storage`): `Grid::Storage`, reachable only in a
//! safe zone, capped by the account's storage size; no weight checks.

use super::*;
use crate::items::{StoredItem, STORAGE_SIZE};

impl World {
    /// Load the account's storage into the player's bag (on entry).
    pub fn set_storage(&mut self, id: ObjectId, items: &[StoredItem], size: u32) {
        let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) else {
            return;
        };
        let size = size.max(STORAGE_SIZE);
        p.storage_size = size;
        p.bag.storage = (0..size).map(|_| None).collect();
        for s in items {
            if s.grid != Grid::Storage {
                continue;
            }
            if let Some(cell) = p.bag.storage.get_mut(s.slot as usize) {
                if cell.is_none() {
                    *cell = Some(s.item.clone());
                }
            }
        }
        self.send_storage(id);
    }

    /// The account's storage as persisted.
    pub fn storage_of(&self, id: ObjectId) -> Option<(Vec<StoredItem>, u32)> {
        let p = self.objects.get(&id)?.player()?;
        let items = p
            .bag
            .storage
            .iter()
            .enumerate()
            .filter_map(|(i, it)| {
                it.as_ref().map(|item| StoredItem {
                    grid: Grid::Storage,
                    slot: i as u8,
                    item: item.clone(),
                })
            })
            .collect();
        Some((items, p.storage_size))
    }

    pub(super) fn send_storage(&mut self, id: ObjectId) {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let items = p
            .bag
            .storage
            .iter()
            .enumerate()
            .filter_map(|(i, it)| it.as_ref().map(|it| (i as u8, it.instance())))
            .collect();
        let size = p.storage_size;
        self.send_to(id, ServerMessage::Storage { size, items });
    }

    /// Zircon `InSafeZone`: inside any `SafeZoneInfo` region of the map.
    pub(super) fn in_safe_zone(&self, map: i32, loc: Point) -> bool {
        let Some(width) = self.maps.get(&map).map(|m| m.file.width as i32) else {
            return false;
        };
        self.data.safe_zones.iter().any(|sz| {
            self.data
                .regions
                .get(&sz.region)
                .filter(|r| r.map == map)
                .is_some_and(|r| r.points(width).contains(&(loc.x, loc.y)))
        })
    }
}
