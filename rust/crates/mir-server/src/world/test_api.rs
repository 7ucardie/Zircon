#[cfg(test)]
use super::*;

#[cfg(test)]
impl World {
    /// Test helper: move an object to a cell without validation.
    #[cfg(test)]
    pub fn teleport(&mut self, id: ObjectId, to: Point) {
        self.move_object(id, to);
    }

    #[cfg(test)]
    pub fn test_hp(&self, id: ObjectId) -> (i32, i32, bool) {
        let o = &self.objects[&id];
        (o.hp, o.max_hp, o.dead)
    }

    #[cfg(test)]
    pub fn test_set_hp(&mut self, id: ObjectId, hp: i32) {
        if let Some(o) = self.objects.get_mut(&id) {
            o.hp = hp;
        }
    }

    #[cfg(test)]
    pub fn test_kill(&mut self, id: ObjectId) {
        self.player_die(id);
    }

    #[cfg(test)]
    pub fn test_belt(&self, id: ObjectId) -> Vec<BeltLink> {
        self.objects[&id].player().unwrap().belt.clone()
    }

    #[cfg(test)]
    pub fn test_set_gold(&mut self, id: ObjectId, gold: u64) {
        if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
            p.bag.gold = gold;
        }
    }

    #[cfg(test)]
    pub fn test_open_page(&mut self, id: ObjectId, page: i32) {
        if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
            p.npc = Some((ObjectId(0), page));
        }
    }

    #[cfg(test)]
    pub fn test_bag(&self, id: ObjectId) -> (u64, Vec<(i32, u32)>) {
        let p = self.objects[&id].player().unwrap();
        (
            p.bag.gold,
            p.bag
                .inventory
                .iter()
                .flatten()
                .map(|i| (i.info, i.count))
                .collect(),
        )
    }

    #[cfg(test)]
    pub fn test_give_item(&mut self, id: ObjectId, info: i32, count: u32) {
        if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
            let mut next = p.next_item_id;
            p.bag.gain(&self.data, info, count, &mut next);
            p.next_item_id = next;
        }
    }

    #[cfg(test)]
    pub fn test_slot_of(&self, id: ObjectId, info: i32) -> Option<u8> {
        self.objects[&id]
            .player()?
            .bag
            .inventory
            .iter()
            .position(|s| s.as_ref().map(|i| i.info == info).unwrap_or(false))
            .map(|i| i as u8)
    }

    #[cfg(test)]
    pub fn test_magics(&self, id: ObjectId) -> Vec<u16> {
        self.objects[&id]
            .player()
            .map(|p| p.magics.iter().map(|m| m.magic).collect())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub fn test_magic_exp(&self, id: ObjectId, magic: u16) -> u64 {
        self.objects[&id]
            .player()
            .and_then(|p| p.magics.iter().find(|m| m.magic == magic))
            .map(|m| m.experience + m.level as u64 * 1000)
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub fn test_movement_cells(&self, map: i32) -> Vec<Point> {
        self.maps[&map]
            .movements
            .keys()
            .map(|(x, y)| Point::new(*x, *y))
            .collect()
    }
}
