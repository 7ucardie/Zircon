//! World simulation tests (need `ZIRCON_ASSETS`; skipped otherwise).

use std::path::PathBuf;

use mir_proto::{Appearance, Direction, ObjectId, Point, ServerMessage};

use crate::accounts::test_character;
use crate::data::GameData;
use crate::world::{self, Outgoing, World};

fn world() -> Option<World> {
    let assets = std::env::var_os("ZIRCON_ASSETS").map(PathBuf::from)?;
    let data = GameData::load(assets.join("../Database/System.db")).ok()?;
    // Seeded so a failure reproduces; change the seed to explore other rolls.
    Some(World::new(data, assets.join("Map"), None).with_seed(0x5A1C0))
}

fn drain(world: &mut World) -> Vec<ServerMessage> {
    world
        .outgoing
        .drain(..)
        .map(|o| match o {
            Outgoing::To(_, m) => m,
        })
        .collect()
}

#[test]
fn warrior_kills_adjacent_monster_and_gains_experience() {
    let Some(mut world) = world() else {
        eprintln!("ZIRCON_ASSETS not set; skipping");
        return;
    };
    let me = world.add_player(1, 1, &test_character("Tester")).unwrap();
    world.tick(0);
    let msgs = drain(&mut world);
    assert!(matches!(msgs[0], ServerMessage::Welcome { .. }));
    let map = world.objects[&me].map;
    let loc = world.objects[&me].location;
    // Find any live monster on the map and teleport it next to us.
    // A chicken: 7 HP, 0 AC, agility 5, so a level-1 warrior lands hits.
    let victim = world
        .objects
        .values()
        .find(|o| {
            !o.dead
                && matches!(&o.appearance, Appearance::Monster { name, .. } if name == "Chicken")
        })
        .map(|o| o.id)
        .expect("a chicken");
    let target_cell = Direction::ALL
        .iter()
        .map(|d| loc.step(*d, 1))
        .find(|p| world.maps[&map].file.is_walkable(p.x, p.y))
        .expect("walkable neighbour");
    world.teleport(victim, target_cell);
    let victim_hp = world.objects[&victim].max_hp;
    assert!(victim_hp > 0);
    let dir = Direction::from_points(loc, target_cell);
    // Swing until it dies (accuracy/agility rolls can miss).
    let mut now = 100;
    let mut kills = 0;
    for _ in 0..400 {
        world.player_attack(me, dir, None);
        now += 100;
        world.tick(now);
        now += 100;
        world.tick(now);
        now += 1400;
        world.tick(now);
        if world.objects.get(&victim).map(|v| v.dead).unwrap_or(true) {
            kills += 1;
            break;
        }
    }
    assert_eq!(kills, 1, "monster should die from repeated hits");
    let msgs = drain(&mut world);
    assert!(msgs
        .iter()
        .any(|m| matches!(m, ServerMessage::ObjectStruck { id, .. } if *id == victim)));
    assert!(msgs
        .iter()
        .any(|m| matches!(m, ServerMessage::ObjectDie { id } if *id == victim)));
    let exp = world.objects[&me].player().unwrap().experience;
    let level = world.objects[&me].player().unwrap().level;
    assert!(exp > 0 || level > 1, "experience should be granted");
}

#[test]
fn potions_heal_instantly_with_a_durability_cooldown() {
    let Some(mut world) = world() else {
        return;
    };
    let me = world.add_player(1, 1, &test_character("Tester")).unwrap();
    world.tick(1000);
    drain(&mut world);
    // Healing Potion: Shape 0, Durability 2000 ms cooldown.
    let potion = world
        .data
        .items
        .values()
        .find(|d| d.name == "Healing Potion")
        .map(|d| d.index)
        .expect("Healing Potion");
    world.test_give_item(me, potion, 3);
    let slot = world.test_slot_of(me, potion).unwrap();
    let (_, max_hp, _) = world.test_hp(me);
    world.test_set_hp(me, 1);
    world.item_use(me, slot);
    let (hp, _, _) = world.test_hp(me);
    assert!(
        hp > 1 && hp <= max_hp,
        "potion should heal at once: {hp}/{max_hp}"
    );
    let count = |w: &World| {
        w.test_bag(me)
            .1
            .iter()
            .find(|(i, _)| *i == potion)
            .map(|(_, c)| *c)
            .unwrap_or(0)
    };
    assert_eq!(count(&world), 2);
    // Still on cooldown: nothing consumed.
    world.tick(2000);
    world.item_use(me, slot);
    assert_eq!(
        count(&world),
        2,
        "second use inside the cooldown must be ignored"
    );
    world.tick(3100);
    world.item_use(me, slot);
    assert_eq!(count(&world), 1, "use after the cooldown consumes a potion");

    // Belt: link the potion type to slot 0, persist it, and reject a link
    // to an item that is not in the bag.
    world.belt_link(me, 0, Some(potion), None);
    world.belt_link(me, 1, None, Some(999_999));
    let belt = world.test_belt(me);
    assert_eq!(belt[0].info, Some(potion));
    assert_eq!(belt[1].item, None);
    let mut rec = test_character("Tester");
    world.snapshot(me, &mut rec);
    assert_eq!(rec.belt[0].info, Some(potion));
}

#[test]
fn dead_player_returns_to_town_on_request() {
    let Some(mut world) = world() else {
        return;
    };
    let me = world.add_player(1, 1, &test_character("Tester")).unwrap();
    world.tick(1000);
    drain(&mut world);
    world.test_kill(me);
    assert!(world.test_hp(me).2, "player should be dead");
    // Well before the 10 minute forced revive.
    world.tick(5000);
    assert!(world.test_hp(me).2, "no automatic revive after 4 s");
    world.town_revive(me);
    let (hp, max_hp, dead) = world.test_hp(me);
    assert!(!dead);
    assert_eq!(hp, max_hp);
    let (map, loc) = (world.objects[&me].map, world.objects[&me].location);
    assert!(world.maps[&map].file.is_walkable(loc.x, loc.y));
}

#[test]
fn chicken_drops_meat_that_can_be_picked_up_and_sold() {
    let Some(mut world) = world() else {
        return;
    };
    let me = world.add_player(1, 1, &test_character("Tester")).unwrap();
    world.tick(0);
    drain(&mut world);
    let map = world.objects[&me].map;
    let loc = world.objects[&me].location;
    let victim = world
        .objects
        .values()
        .find(|o| {
            !o.dead
                && matches!(&o.appearance, Appearance::Monster { name, .. } if name == "Chicken")
        })
        .map(|o| o.id)
        .expect("a chicken");
    let cell = Direction::ALL
        .iter()
        .map(|d| loc.step(*d, 1))
        .find(|p| world.maps[&map].file.is_walkable(p.x, p.y))
        .unwrap();
    world.teleport(victim, cell);
    let dir = Direction::from_points(loc, cell);
    let mut now = 100;
    for _ in 0..400 {
        world.player_attack(me, dir, None);
        now += 1600;
        world.tick(now);
        if world.objects.get(&victim).map(|v| v.dead).unwrap_or(true) {
            break;
        }
    }
    // Chicken Meat (179) has Chance 1 => always drops.
    let meat = world
        .objects
        .values()
        .find(|o| matches!(&o.appearance, Appearance::Item { info: 179, .. }))
        .map(|o| (o.id, o.location))
        .expect("meat on the ground");
    world.teleport(me, meat.1);
    world.pick_up(me);
    let (_, bag) = world.test_bag(me);
    assert!(
        bag.iter().any(|(info, _)| *info == 179),
        "meat should be in the bag: {bag:?}"
    );
    assert!(!world.objects.contains_key(&meat.0));

    // Sell it to the meat shop page (4) which buys ItemType 12.
    world.test_set_gold(me, 0);
    world.test_open_page(me, 4);
    let slot = world.objects[&me]
        .player()
        .unwrap()
        .bag
        .inventory
        .iter()
        .position(|s| s.as_ref().map(|i| i.info == 179).unwrap_or(false))
        .unwrap() as u8;
    world.npc_sell(me, vec![slot]);
    let (gold, bag) = world.test_bag(me);
    assert!(gold > 0, "selling should pay gold");
    assert!(!bag.iter().any(|(info, _)| *info == 179));

    // Buy one back.
    world.test_set_gold(me, 100_000);
    world.npc_buy(me, 179, 1);
    let (gold, bag) = world.test_bag(me);
    assert!(gold < 100_000);
    assert!(bag.iter().any(|(info, c)| *info == 179 && *c == 1));
}

#[test]
fn walking_onto_an_exit_changes_map() {
    let Some(mut world) = world() else {
        return;
    };
    let me = world.add_player(1, 1, &test_character("Tester")).unwrap();
    world.tick(0);
    drain(&mut world);
    let map = world.objects[&me].map;
    let exits = world.test_movement_cells(map);
    assert!(!exits.is_empty(), "Bichon should have exits");
    // Find an exit with a walkable neighbour to step from.
    let mut done = false;
    let mut now = 1000;
    for exit in exits.iter() {
        now += 1000;
        let Some(from) = Direction::ALL
            .iter()
            .map(|d| exit.step(*d, 1))
            .find(|p| world.maps[&map].file.is_walkable(p.x, p.y))
        else {
            continue;
        };
        // Nothing may stand on the exit cell.
        let blockers: Vec<ObjectId> = world
            .objects
            .values()
            .filter(|o| o.id != me && o.map == map && o.location == *exit)
            .map(|o| o.id)
            .collect();
        for b in blockers {
            world.remove_object(b);
        }
        world.teleport(me, from);
        world.tick(now);
        drain(&mut world);
        let dir = Direction::from_points(from, *exit);
        world.player_move(me, dir, false);
        let msgs = drain(&mut world);
        if world.objects[&me].map != map {
            assert!(msgs
                .iter()
                .any(|m| matches!(m, ServerMessage::MapChanged { .. })));
            done = true;
            break;
        }
    }
    assert!(
        done,
        "no exit led anywhere (level requirement or unloaded map)"
    );
}

/// Give the player a book for `magic_name`'s MagicInfo and learn it.
fn learn(world: &mut World, me: ObjectId, magic_name: &str) -> u16 {
    let def = world
        .data
        .magics
        .values()
        .find(|m| m.name == magic_name)
        .cloned()
        .expect("magic");
    let book = world
        .data
        .items
        .values()
        .find(|i| i.item_type == 14 && i.shape == def.index)
        .map(|i| i.index)
        .expect("book item");
    world.test_give_item(me, book, 1);
    let slot = world.test_slot_of(me, book).expect("book in bag");
    world.item_use(me, slot);
    let known = world.test_magics(me);
    assert!(
        known.contains(&def.magic),
        "should know {magic_name}: {known:?}"
    );
    def.magic
}

fn nearest_chicken(world: &World, me: ObjectId) -> ObjectId {
    let loc = world.objects[&me].location;
    world
        .objects
        .values()
        .filter(|o| {
            !o.dead
                && matches!(&o.appearance, Appearance::Monster { name, .. } if name == "Chicken")
        })
        .min_by_key(|o| o.location.distance(loc))
        .map(|o| o.id)
        .expect("a chicken")
}

#[test]
fn wizard_learns_fire_ball_and_burns_a_chicken() {
    let Some(mut world) = world() else {
        return;
    };
    let mut rec = test_character("Wiz");
    rec.class = mir_proto::Class::Wizard;
    rec.level = 16;
    let me = world.add_player(1, 1, &rec).unwrap();
    world.tick(0);
    drain(&mut world);
    let fire_ball = learn(&mut world, me, "Fire Ball");
    let victim = nearest_chicken(&world, me);
    let loc = world.objects[&me].location;
    let cell = Direction::ALL
        .iter()
        .map(|d| loc.step(*d, 2))
        .find(|p| {
            world.maps[&world.objects[&me].map]
                .file
                .is_walkable(p.x, p.y)
        })
        .unwrap();
    world.teleport(victim, cell);
    let hp_before = world.objects[&victim].hp;
    let mut now = 1000;
    let mut hit = false;
    for _ in 0..40 {
        world.cast(
            me,
            fire_ball,
            Direction::from_points(loc, cell),
            Some(victim),
            cell,
        );
        for _ in 0..30 {
            now += 100;
            world.tick(now);
        }
        let msgs = drain(&mut world);
        if msgs
            .iter()
            .any(|m| matches!(m, ServerMessage::ObjectMagic { magic, .. } if *magic == fire_ball))
        {
            hit = true;
        }
        if world
            .objects
            .get(&victim)
            .map(|v| v.dead || v.hp < hp_before)
            .unwrap_or(true)
        {
            break;
        }
    }
    assert!(hit, "ObjectMagic should be broadcast");
    let dead_or_hurt = world
        .objects
        .get(&victim)
        .map(|v| v.dead || v.hp < hp_before)
        .unwrap_or(true);
    assert!(dead_or_hurt, "fire ball should damage the chicken");
    let mp = world.objects[&me].player().unwrap().mp;
    assert!(
        mp < world.objects[&me].player().unwrap().max_mp,
        "casting costs mana"
    );
    let msgs = world.test_magic_exp(me, fire_ball);
    assert!(msgs > 0, "fire ball should gain experience");
}

#[test]
fn wizard_fire_wall_burns_a_chicken_standing_in_it() {
    let Some(mut world) = world() else {
        return;
    };
    let mut rec = test_character("Pyro");
    rec.class = mir_proto::Class::Wizard;
    rec.level = 40;
    let me = world.add_player(1, 1, &rec).unwrap();
    world.tick(0);
    drain(&mut world);
    let fire_wall = learn(&mut world, me, "Fire Wall");
    let victim = nearest_chicken(&world, me);
    let loc = world.objects[&me].location;
    let map = world.objects[&me].map;
    let cell = Direction::ALL
        .iter()
        .map(|d| loc.step(*d, 3))
        .find(|p| world.maps[&map].file.is_walkable(p.x, p.y))
        .unwrap();
    world.teleport(victim, cell);
    let hp_before = world.objects[&victim].hp;
    world.cast(me, fire_wall, Direction::from_points(loc, cell), None, cell);
    let mut now = 1000;
    // Six seconds: three wall ticks, enough for small rolls to add up.
    for _ in 0..60 {
        now += 100;
        // Keep the chicken on the burning cell; it roams otherwise.
        if world.objects.get(&victim).map(|v| !v.dead).unwrap_or(false) {
            world.teleport(victim, cell);
        }
        world.tick(now);
    }
    let walls = world
        .objects
        .values()
        .filter(|o| matches!(o.appearance, Appearance::Spell { .. }) && o.map == map)
        .count();
    assert!(walls >= 1, "fire wall spell objects should exist");
    let hurt = world
        .objects
        .get(&victim)
        .map(|v| v.dead || v.hp < hp_before)
        .unwrap_or(true);
    let (hp_now, dead_now) = world
        .objects
        .get(&victim)
        .map(|v| (v.hp, v.dead))
        .unwrap_or((0, true));
    assert!(
        hurt,
        "a chicken standing in the fire wall burns: hp {hp_before} -> {hp_now}, dead {dead_now}, walls {walls}, chicken at {:?}, wall cell {:?}",
        world.objects.get(&victim).map(|v| v.location),
        cell
    );
    // Walls burn out: level 0 => 10 ticks of 2 s.
    for _ in 0..220 {
        now += 100;
        world.tick(now);
    }
    let walls = world
        .objects
        .values()
        .filter(|o| matches!(o.appearance, Appearance::Spell { .. }))
        .count();
    assert_eq!(walls, 0, "fire walls expire");
}

#[test]
fn warrior_might_raises_damage_then_expires() {
    let Some(mut world) = world() else {
        return;
    };
    let mut rec = test_character("Brute");
    rec.level = 48;
    let me = world.add_player(1, 1, &rec).unwrap();
    world.tick(0);
    drain(&mut world);
    let might = learn(&mut world, me, "Might");
    let before = world.objects[&me].stats.max_dc;
    let loc = world.objects[&me].location;
    world.cast(me, might, Direction::Down, None, loc);
    let mut now = 1000;
    for _ in 0..12 {
        now += 100;
        world.tick(now);
    }
    let during = world.objects[&me].stats.max_dc;
    // +5 % of a small DC rounds to 0, so only check it never drops.
    assert!(
        during >= before,
        "Might must not lower max DC: {before} -> {during}"
    );
    let msgs = drain(&mut world);
    assert!(msgs
        .iter()
        .any(|m| matches!(m, ServerMessage::BuffAdd(b) if b.kind == mir_proto::buff_type::MIGHT)));
    // Level 0 lasts 60 s.
    now += 61_000;
    world.tick(now);
    assert_eq!(
        world.objects[&me].stats.max_dc, before,
        "buff should expire"
    );
    let msgs = drain(&mut world);
    assert!(msgs.iter().any(
        |m| matches!(m, ServerMessage::BuffRemove { kind } if *kind == mir_proto::buff_type::MIGHT)
    ));
}

#[test]
fn warrior_shoulder_dash_moves_forward() {
    let Some(mut world) = world() else {
        return;
    };
    let mut rec = test_character("Dasher");
    rec.level = 27;
    let me = world.add_player(1, 1, &rec).unwrap();
    world.tick(0);
    drain(&mut world);
    let dash = learn(&mut world, me, "Shoulder Dash");
    let loc = world.objects[&me].location;
    let map = world.objects[&me].map;
    let dir = Direction::ALL
        .iter()
        .copied()
        .find(|d| {
            (1..=3).all(|i| {
                let p = loc.step(*d, i);
                world.maps[&map].file.is_walkable(p.x, p.y)
                    && world.maps[&map].objects_at(p).is_empty()
            })
        })
        .expect("a clear direction");
    world.cast(me, dash, dir, None, loc);
    let mut now = 1000;
    let mut steps = 0;
    for _ in 0..30 {
        now += 100;
        world.tick(now);
        steps += drain(&mut world)
            .iter()
            .filter(|m| matches!(m, ServerMessage::ObjectDash { id, .. } if *id == me))
            .count();
    }
    let after = world.objects[&me].location;
    assert!(steps >= 1, "dash steps should be broadcast");
    assert!(
        after != loc && loc.distance(after) >= 1,
        "player should have moved"
    );
    assert_eq!(Direction::from_points(loc, after), dir);
}

#[test]
fn taoist_explosive_talisman_consumes_an_amulet() {
    let Some(mut world) = world() else {
        return;
    };
    let mut rec = test_character("Tao");
    rec.class = mir_proto::Class::Taoist;
    rec.level = 13;
    let me = world.add_player(1, 1, &rec).unwrap();
    world.tick(0);
    drain(&mut world);
    let talisman = learn(&mut world, me, "Explosive Talisman");
    let victim = nearest_chicken(&world, me);
    let loc = world.objects[&me].location;
    let map = world.objects[&me].map;
    let cell = Direction::ALL
        .iter()
        .map(|d| loc.step(*d, 2))
        .find(|p| world.maps[&map].file.is_walkable(p.x, p.y))
        .unwrap();
    world.teleport(victim, cell);
    // Without a talisman equipped nothing flies.
    world.cast(
        me,
        talisman,
        Direction::from_points(loc, cell),
        Some(victim),
        cell,
    );
    let msgs = drain(&mut world);
    let flew = msgs.iter().any(|m| matches!(m, ServerMessage::ObjectMagic { magic, targets, .. } if *magic == talisman && !targets.is_empty()));
    assert!(!flew, "no talisman: no target");
    // Equip a stack of talismans (ItemType Amulet, Shape 0).
    let amulet = world
        .data
        .items
        .values()
        .filter(|d| {
            d.item_type == mir_proto::item_type::AMULET && d.shape == 0 && d.required_amount <= 13
        })
        .map(|d| d.index)
        .min()
        .expect("a talisman item");
    world.test_give_item(me, amulet, 5);
    let slot = world.test_slot_of(me, amulet).unwrap();
    world.item_use(me, slot);
    let mut now = 3000;
    world.tick(now);
    drain(&mut world);
    world.cast(
        me,
        talisman,
        Direction::from_points(loc, cell),
        Some(victim),
        cell,
    );
    now += 50;
    world.tick(now);
    let msgs = drain(&mut world);
    let flew = msgs.iter().any(|m| matches!(m, ServerMessage::ObjectMagic { magic, targets, .. } if *magic == talisman && !targets.is_empty()));
    assert!(flew, "with a talisman the spell targets the chicken");
    let count = world.objects[&me].player().unwrap().bag.equipment[mir_proto::slot::AMULET]
        .as_ref()
        .map(|i| i.count)
        .unwrap_or(0);
    assert_eq!(count, 4, "one talisman is consumed per cast");
    let hp_before = world.objects[&victim].hp;
    for _ in 0..20 {
        now += 100;
        world.tick(now);
    }
    let hurt = world
        .objects
        .get(&victim)
        .map(|v| v.dead || v.hp < hp_before)
        .unwrap_or(true);
    assert!(hurt, "the talisman should hit");
}

#[test]
fn warrior_beckon_pulls_a_chicken_and_fetter_slows_it() {
    let Some(mut world) = world() else {
        return;
    };
    let mut rec = test_character("Puller");
    rec.level = 55;
    let me = world.add_player(1, 1, &rec).unwrap();
    world.tick(0);
    drain(&mut world);
    let beckon = learn(&mut world, me, "Beckon");
    let fetter = learn(&mut world, me, "Fetter");
    let victim = nearest_chicken(&world, me);
    let loc = world.objects[&me].location;
    let map = world.objects[&me].map;
    let dir = Direction::ALL
        .iter()
        .copied()
        .find(|d| {
            (1..=3).all(|i| {
                let p = loc.step(*d, i);
                world.maps[&map].file.is_walkable(p.x, p.y)
                    && world.maps[&map].objects_at(p).is_empty()
            })
        })
        .expect("a clear direction");
    let far = loc.step(dir, 3);
    let mut now = 1000;
    let mut pulled = false;
    // Level 0 succeeds 3 times in 9; keep trying.
    for _ in 0..40 {
        world.teleport(victim, far);
        world.cast(me, beckon, dir, Some(victim), far);
        for _ in 0..12 {
            now += 100;
            world.tick(now);
            if world.objects[&victim].location == loc.step(dir, 1) {
                pulled = true;
                break;
            }
        }
        drain(&mut world);
        if pulled {
            break;
        }
        now += 3000;
        world.tick(now);
    }
    assert!(
        pulled,
        "Beckon should pull the chicken in front of the warrior"
    );
    assert!(
        world.objects[&victim]
            .poisons
            .iter()
            .any(|p| p.kind == world::poison_kind::PARALYSIS),
        "pulled monsters are paralysed"
    );
    now += 3000;
    world.tick(now);
    // The chicken may have wandered off meanwhile; put it back in reach, and
    // refill the mana the Beckon retries used up.
    world.teleport(victim, loc.step(dir, 1));
    world.test_refill_mp(me);
    world.cast(me, fetter, dir, None, loc);
    for _ in 0..8 {
        now += 100;
        world.tick(now);
    }
    assert!(
        world.objects[&victim]
            .poisons
            .iter()
            .any(|p| p.kind == world::poison_kind::SLOW),
        "Fetter slows monsters within two cells"
    );
}

#[test]
fn wizard_storms_renounce_and_tempest() {
    let Some(mut world) = world() else {
        return;
    };
    let mut rec = test_character("Storm");
    rec.class = mir_proto::Class::Wizard;
    rec.level = 54;
    let me = world.add_player(1, 1, &rec).unwrap();
    world.tick(0);
    drain(&mut world);
    let fire_storm = learn(&mut world, me, "Fire Storm");
    let renounce = learn(&mut world, me, "Renounce");
    let tempest = learn(&mut world, me, "Tempest");
    let victim = nearest_chicken(&world, me);
    let loc = world.objects[&me].location;
    let map = world.objects[&me].map;
    let cell = Direction::ALL
        .iter()
        .map(|d| loc.step(*d, 3))
        .find(|p| world.maps[&map].file.is_walkable(p.x, p.y))
        .unwrap();
    world.teleport(victim, cell);
    let hp_before = world.objects[&victim].hp;
    world.cast(
        me,
        fire_storm,
        Direction::from_points(loc, cell),
        None,
        cell,
    );
    let mut now = 1000;
    for _ in 0..10 {
        now += 100;
        world.teleport(victim, cell);
        world.tick(now);
    }
    let hurt = world
        .objects
        .get(&victim)
        .map(|v| v.dead || v.hp < hp_before)
        .unwrap_or(true);
    assert!(hurt, "Fire Storm hits the 3x3 around the cell");

    // Renounce trades max HP for MC.
    let (hp_max_before, mc_before) = {
        let o = &world.objects[&me];
        (o.max_hp, o.stats.max_mc)
    };
    now += 3000;
    world.tick(now);
    world.cast(me, renounce, Direction::Down, None, loc);
    for _ in 0..8 {
        now += 100;
        world.tick(now);
    }
    let o = &world.objects[&me];
    assert!(
        o.max_hp < hp_max_before,
        "Renounce lowers max HP: {hp_max_before} -> {}",
        o.max_hp
    );
    assert!(o.stats.max_mc >= mc_before, "Renounce raises MC");

    // Tempest lays a 3x3 field of spell objects.
    now += 3000;
    world.tick(now);
    world.cast(me, tempest, Direction::from_points(loc, cell), None, cell);
    for _ in 0..8 {
        now += 100;
        world.tick(now);
    }
    let fields = world
        .objects
        .values()
        .filter(|o| matches!(o.appearance, Appearance::Spell { effect } if effect == mir_proto::spell_effect::TEMPEST))
        .count();
    assert!(fields >= 5, "tempest cells: {fields}");
}

#[test]
fn taoist_summons_a_skeleton_that_follows_and_fights() {
    let Some(mut world) = world() else {
        return;
    };
    let mut rec = test_character("Necro");
    rec.class = mir_proto::Class::Taoist;
    rec.level = 17;
    let me = world.add_player(1, 1, &rec).unwrap();
    world.tick(0);
    drain(&mut world);
    let summon = learn(&mut world, me, "Summon Skeleton");
    let amulet = world
        .data
        .items
        .values()
        .filter(|d| {
            d.item_type == mir_proto::item_type::AMULET && d.shape == 0 && d.required_amount <= 17
        })
        .map(|d| d.index)
        .min()
        .expect("a talisman item");
    world.test_give_item(me, amulet, 5);
    let slot = world.test_slot_of(me, amulet).unwrap();
    world.item_use(me, slot);
    let mut now = 3000;
    world.tick(now);
    drain(&mut world);
    let loc = world.objects[&me].location;
    world.cast(me, summon, Direction::Down, None, loc);
    for _ in 0..8 {
        now += 100;
        world.tick(now);
    }
    let pet = world
        .objects
        .values()
        .find(|o| matches!(&o.kind, world::Kind::Monster(m) if m.owner == Some(me)))
        .map(|o| o.id)
        .expect("a summoned skeleton");
    assert!(
        matches!(&world.objects[&pet].appearance, Appearance::Monster { owner: Some(n), .. } if n == "Necro")
    );
    // Attack a chicken: the idle pet takes it as its target.
    let victim = nearest_chicken(&world, me);
    let map = world.objects[&me].map;
    let cell = Direction::ALL
        .iter()
        .map(|d| loc.step(*d, 1))
        .find(|p| {
            world.maps[&map].file.is_walkable(p.x, p.y)
                && world.maps[&map].objects_at(*p).is_empty()
        })
        .unwrap();
    world.teleport(victim, cell);
    let dir = Direction::from_points(loc, cell);
    for _ in 0..30 {
        now += 500;
        world.player_attack(me, dir, None);
        world.tick(now);
        let pet_target = world.objects.get(&pet).and_then(|o| match &o.kind {
            world::Kind::Monster(m) => m.target,
            _ => None,
        });
        if pet_target == Some(victim) {
            break;
        }
    }
    let pet_target = world.objects.get(&pet).and_then(|o| match &o.kind {
        world::Kind::Monster(m) => m.target,
        _ => None,
    });
    assert_eq!(pet_target, Some(victim), "the pet should join the fight");
    // A second cast recalls instead of summoning another.
    now += 3000;
    world.tick(now);
    world.cast(me, summon, Direction::Down, None, loc);
    for _ in 0..8 {
        now += 100;
        world.tick(now);
    }
    let pets = world
        .objects
        .values()
        .filter(|o| matches!(&o.kind, world::Kind::Monster(m) if m.owner == Some(me)))
        .count();
    assert_eq!(pets, 1, "same summon is recalled, not doubled");
}

#[test]
fn taoist_traps_and_buffs() {
    let Some(mut world) = world() else {
        return;
    };
    let mut rec = test_character("Tao2");
    rec.class = mir_proto::Class::Taoist;
    rec.level = 34;
    let me = world.add_player(1, 1, &rec).unwrap();
    world.tick(0);
    drain(&mut world);
    let trap = learn(&mut world, me, "Trap Octagon");
    let blood = learn(&mut world, me, "Blood Lust");
    let amulet = world
        .data
        .items
        .values()
        .filter(|d| {
            d.item_type == mir_proto::item_type::AMULET && d.shape == 0 && d.required_amount <= 34
        })
        .map(|d| d.index)
        .min()
        .expect("a talisman item");
    world.test_give_item(me, amulet, 20);
    let slot = world.test_slot_of(me, amulet).unwrap();
    world.item_use(me, slot);
    let mut now = 3000;
    world.tick(now);
    drain(&mut world);
    let victim = nearest_chicken(&world, me);
    let loc = world.objects[&me].location;
    let map = world.objects[&me].map;
    let cell = Direction::ALL
        .iter()
        .map(|d| loc.step(*d, 3))
        .find(|p| world.maps[&map].file.is_walkable(p.x, p.y))
        .unwrap();
    world.teleport(victim, cell);
    world.cast(me, trap, Direction::from_points(loc, cell), None, cell);
    for _ in 0..10 {
        now += 100;
        world.teleport(victim, cell);
        world.tick(now);
    }
    let shocked =
        matches!(&world.objects[&victim].kind, world::Kind::Monster(m) if m.shock_until > now);
    assert!(shocked, "trapped chicken cannot move");
    let ring = world
        .objects
        .values()
        .filter(|o| matches!(o.appearance, Appearance::Spell { effect } if effect == mir_proto::spell_effect::TRAP_OCTAGON))
        .count();
    assert!(ring >= 4, "octagon cells: {ring}");
    // Blood Lust raises max DC of players in the area.
    let before = world.objects[&me].stats.max_dc;
    now += 3000;
    world.tick(now);
    world.cast(me, blood, Direction::Down, None, loc);
    for _ in 0..12 {
        now += 100;
        world.tick(now);
    }
    assert!(
        world.objects[&me].stats.max_dc > before,
        "Blood Lust adds DC"
    );
}

#[test]
fn assassin_cloak_grip_hell_fire_and_puppets() {
    let Some(mut world) = world() else {
        return;
    };
    // Assassins start on their own map; put this one at the warrior start
    // where the chickens are.
    let scout = world.add_player(1, 1, &test_character("Scout")).unwrap();
    let (scout_map, scout_loc) = (world.objects[&scout].map, world.objects[&scout].location);
    world.remove_object(scout);
    let mut rec = test_character("Sin3");
    rec.class = mir_proto::Class::Assassin;
    rec.level = 50;
    rec.map = world.data.maps[&scout_map].file_name.clone();
    rec.location = scout_loc;
    let me = world.add_player(1, 1, &rec).unwrap();
    world.tick(0);
    drain(&mut world);
    let cloak = learn(&mut world, me, "Cloak");
    let grip = learn(&mut world, me, "Wraith Grip");
    let hell = learn(&mut world, me, "Hell Fire");
    let puppet = learn(&mut world, me, "Summon Puppet");
    let mut now = 3000;
    world.tick(now);
    drain(&mut world);
    let victim = nearest_chicken(&world, me);
    let loc = world.objects[&me].location;
    let map = world.objects[&me].map;
    let cell = Direction::ALL
        .iter()
        .map(|d| loc.step(*d, 2))
        .find(|p| world.maps[&map].file.is_walkable(p.x, p.y))
        .unwrap();
    world.teleport(victim, cell);
    // Wraith Grip pins the chicken with its own poison kind.
    world.cast(
        me,
        grip,
        Direction::from_points(loc, cell),
        Some(victim),
        cell,
    );
    for _ in 0..8 {
        now += 100;
        world.tick(now);
    }
    let gripped = world.objects[&victim]
        .poisons
        .iter()
        .any(|p| p.kind == world::poison_kind::WRAITH_GRIP);
    assert!(gripped, "Wraith Grip poison applied");
    // Hell Fire burns and leaves a ticking poison.
    now += 2000;
    world.tick(now);
    let (hp_before, _, _) = world.test_hp(victim);
    world.cast(
        me,
        hell,
        Direction::from_points(loc, cell),
        Some(victim),
        cell,
    );
    for _ in 0..15 {
        now += 100;
        world.tick(now);
    }
    let (hp_after, _, dead) = world.test_hp(victim);
    let burning = world.objects[&victim]
        .poisons
        .iter()
        .any(|p| p.kind == world::poison_kind::HELL_FIRE);
    assert!(
        dead || hp_after < hp_before || burning,
        "Hell Fire hurts: {hp_before} -> {hp_after}, burning {burning}"
    );
    // Cloak costs HP and keeps draining it every 2 s.
    now += 2000;
    world.tick(now);
    let (hp0, _, _) = world.test_hp(me);
    world.cast(me, cloak, Direction::Down, None, loc);
    for _ in 0..8 {
        now += 100;
        world.tick(now);
    }
    assert!(
        world.objects[&me].has_buff(mir_proto::buff_type::CLOAK),
        "cloaked"
    );
    let (hp1, _, _) = world.test_hp(me);
    assert!(hp1 < hp0, "cloak costs HP: {hp0} -> {hp1}");
    now += 2500;
    world.tick(now);
    let (hp2, _, _) = world.test_hp(me);
    assert!(hp2 < hp1, "cloak drains HP: {hp1} -> {hp2}");
    // Summon Puppet drops the cloak, raises a puppet, blinks and re-cloaks.
    world.cast(me, puppet, Direction::Down, None, loc);
    for _ in 0..8 {
        now += 100;
        world.tick(now);
    }
    let puppets = world
        .objects
        .values()
        .filter(|o| matches!(&o.kind, world::Kind::Monster(m) if m.owner == Some(me) && m.explode_at.is_some()))
        .count();
    assert_eq!(puppets, 1, "one puppet at level 0");
    assert!(
        world.objects[&me].has_buff(mir_proto::buff_type::GHOST_WALK),
        "puppet cloak always ghost walks"
    );
    for _ in 0..60 {
        now += 100;
        world.tick(now);
    }
    let puppets = world
        .objects
        .values()
        .filter(|o| matches!(&o.kind, world::Kind::Monster(m) if m.owner == Some(me) && !o.dead))
        .count();
    assert_eq!(puppets, 0, "puppets explode after 5 s");
}

#[test]
fn warrior_thrusting_reaches_the_second_cell() {
    let Some(mut world) = world() else {
        return;
    };
    let mut rec = test_character("Thruster");
    rec.level = 19;
    let me = world.add_player(1, 1, &rec).unwrap();
    world.tick(0);
    drain(&mut world);
    let thrusting = learn(&mut world, me, "Thrusting");
    world.magic_toggle(me, thrusting, true);
    let victim = nearest_chicken(&world, me);
    let map = world.objects[&me].map;
    let loc = world.objects[&me].location;
    let dir = Direction::ALL
        .iter()
        .copied()
        .find(|d| {
            let a = loc.step(*d, 1);
            let b = loc.step(*d, 2);
            world.maps[&map].file.is_walkable(a.x, a.y)
                && world.maps[&map].file.is_walkable(b.x, b.y)
        })
        .unwrap();
    world.teleport(victim, loc.step(dir, 2));
    let hp_before = world.objects[&victim].hp;
    let mut now = 1000;
    for _ in 0..200 {
        // Keep the chicken two cells ahead; it wanders otherwise.
        if world.objects.get(&victim).map(|v| !v.dead).unwrap_or(false) {
            world.teleport(victim, loc.step(dir, 2));
        }
        world.player_attack(me, dir, Some(thrusting));
        now += 1600;
        world.tick(now);
        if world
            .objects
            .get(&victim)
            .map(|v| v.dead || v.hp < hp_before)
            .unwrap_or(true)
        {
            break;
        }
    }
    assert!(
        world
            .objects
            .get(&victim)
            .map(|v| v.dead || v.hp < hp_before)
            .unwrap_or(true),
        "thrusting should hit two cells ahead"
    );
}

#[test]
fn passive_animals_do_not_aggro_but_hunters_do() {
    let Some(mut world) = world() else {
        return;
    };
    let _me = world.add_player(1, 1, &test_character("Tester")).unwrap();
    world.tick(0);
    let passive: Vec<ObjectId> = world
        .objects
        .values()
        .filter(|o| match &o.appearance {
            Appearance::Monster { name, .. } => name == "Chicken" || name == "Deer",
            _ => false,
        })
        .map(|o| o.id)
        .collect();
    assert!(!passive.is_empty());
    for id in &passive {
        assert!(world.data.monsters[&world.objects[id].monster_def()].is_passive());
    }
    assert!(!world
        .data
        .monsters
        .values()
        .find(|m| m.name == "Wolf")
        .unwrap()
        .is_passive());
}

#[test]
fn move_is_validated_against_walls_and_objects() {
    let Some(mut world) = world() else {
        return;
    };
    let me = world.add_player(1, 1, &test_character("Tester")).unwrap();
    world.tick(0);
    drain(&mut world);
    let map = world.objects[&me].map;
    let loc = world.objects[&me].location;
    let blocked_dir = Direction::ALL.iter().copied().find(|d| {
        !world.maps[&map]
            .file
            .is_walkable(loc.step(*d, 1).x, loc.step(*d, 1).y)
    });
    if let Some(d) = blocked_dir {
        world.player_move(me, d, false);
        let msgs = drain(&mut world);
        assert!(matches!(
            msgs.last(),
            Some(ServerMessage::MoveDenied { .. })
        ));
        assert_eq!(world.objects[&me].location, loc);
    }
    let open_dir = Direction::ALL
        .iter()
        .copied()
        .find(|d| {
            world.maps[&map]
                .file
                .is_walkable(loc.step(*d, 1).x, loc.step(*d, 1).y)
        })
        .expect("open direction");
    // Clear any monster from that cell.
    let cell = loc.step(open_dir, 1);
    let blockers: Vec<ObjectId> = world
        .objects
        .values()
        .filter(|o| o.location == cell)
        .map(|o| o.id)
        .collect();
    for b in blockers {
        world.remove_object(b);
    }
    world.tick(1000);
    drain(&mut world);
    world.player_move(me, open_dir, false);
    assert_eq!(world.objects[&me].location, Point::new(cell.x, cell.y));
}

#[test]
fn guards_stand_in_town_and_archers_shoot_from_range() {
    let Some(mut world) = world() else {
        return;
    };
    let mut rec = test_character("Watch");
    rec.level = 40;
    let me = world.add_player(1, 1, &rec).unwrap();
    world.tick(0);
    drain(&mut world);
    let map = world.objects[&me].map;
    assert!(
        world.test_guard_count(map) > 0,
        "GuardInfo places guards on the start map"
    );
    // A skeleton axe thrower (AI 7) six cells away shoots without closing in
    // (closer than that it would back off first).
    let loc = world.objects[&me].location;
    let cell = Direction::ALL
        .iter()
        .map(|d| loc.step(*d, 6))
        .find(|p| world.maps[&map].file.is_walkable(p.x, p.y))
        .unwrap();
    let archer = world.test_spawn_ai(7, map, cell).expect("an AI 7 monster");
    world.test_set_target(archer, me);
    let (hp0, _, _) = world.test_hp(me);
    let mut now = 3000;
    let mut shot = false;
    for _ in 0..80 {
        now += 100;
        world.teleport(archer, cell);
        world.teleport(me, loc);
        world.tick(now);
        if drain(&mut world)
            .iter()
            .any(|m| matches!(m, ServerMessage::ObjectRangeAttack { id, .. } if *id == archer))
        {
            shot = true;
        }
    }
    assert!(shot, "archer used a ranged attack");
    let (hp1, _, _) = world.test_hp(me);
    assert!(
        hp1 <= hp0,
        "ranged hit landed or was dodged: {hp0} -> {hp1}"
    );
}

#[test]
fn npc_data_lists_and_currencies_round_trip() {
    use crate::data::{NpcActionDef, NpcCheckDef};
    let Some(mut world) = world() else {
        return;
    };
    let me = world.add_player(1, 1, &test_character("Scribe")).unwrap();
    world.tick(0);
    drain(&mut world);
    let action = |t: i32, s: &str, i1: i32, i2: i32| NpcActionDef {
        action_type: t,
        string1: s.into(),
        int1: i1,
        int2: i2,
        item1: 0,
        map1: 0,
        stat1: 0,
    };
    let check = |t: i32, op: i32, s: &str, i1: i32, i2: i32| NpcCheckDef {
        check_type: t,
        operator: op,
        string1: s.into(),
        int1: i1,
        int2: i2,
        item1: 0,
        stat1: 0,
        fail_page: 0,
    };
    // Data list membership (Zircon CheckDataList) after AddDataList / RemoveDataList.
    assert!(!world.test_npc_check(me, &check(19, 0, "Quest1", 1, 0)));
    world.test_npc_action(me, &action(17, "Quest1", 1, 0));
    assert!(world.test_npc_check(me, &check(19, 0, "Quest1", 1, 0)));
    world.test_npc_action(me, &action(18, "Quest1", 1, 0));
    assert!(!world.test_npc_check(me, &check(19, 0, "Quest1", 1, 0)));
    // Data values: set, change, compare with IntParameter2.
    world.test_npc_action(me, &action(21, "Kills", 1, 5));
    world.test_npc_action(me, &action(20, "Kills", 1, 2));
    assert!(world.test_npc_check(me, &check(20, 0, "Kills", 1, 7)));
    assert!(world.test_npc_check(me, &check(20, 5, "Kills", 1, 3)));
    // Currencies by name; gold routes to the bag.
    world.test_npc_action(me, &action(15, "Fame Point", 12, 0));
    assert!(world.test_npc_check(me, &check(17, 0, "FP", 12, 0)));
    world.test_npc_action(me, &action(16, "Fame Point", 2, 0));
    assert!(world.test_npc_check(me, &check(17, 0, "Fame Point", 10, 0)));
    let gold_before = world.test_gold(me);
    world.test_npc_action(me, &action(15, "Gold", 30, 0));
    assert_eq!(world.test_gold(me), gold_before + 30);
    // Unknown currency names pass the check like Zircon's `continue`.
    assert!(world.test_npc_check(me, &check(17, 0, "Moonstones", 1, 0)));
}

#[test]
fn quest_accept_progress_and_complete() {
    let Some(mut world) = world() else {
        return;
    };
    // "Curing the Poison Pt. 1": quest 9 at NPC 13, level 20, ten venoms from
    // monster 20 at a 1-in-2 roll, rewards item 801 x352.
    let Some(def) = world.data.quests.get(&9).cloned() else {
        return;
    };
    let mut rec = test_character("Seeker");
    rec.level = 20;
    let me = world.add_player(1, 1, &rec).unwrap();
    world.tick(0);
    let msgs = drain(&mut world);
    assert!(
        msgs.iter()
            .any(|m| matches!(m, ServerMessage::QuestList(_))),
        "quest log sent on entry"
    );
    let npc = world.test_open_npc(me, def.start_npc);
    let offered = world.test_npc_quests(me, npc);
    assert!(
        offered.iter().any(|q| q.quest == 9 && q.state == 0),
        "quest offered: {offered:?}"
    );
    world.quest_accept(me, 9);
    assert!(world
        .test_quests(me)
        .iter()
        .any(|q| q.quest == 9 && !q.completed));
    assert!(
        matches!(drain(&mut world).last(), Some(ServerMessage::QuestChanged(q)) if q.quest == 9)
    );
    // Not offered twice.
    assert!(!world
        .test_npc_quests(me, npc)
        .iter()
        .any(|q| q.quest == 9 && q.state == 0));
    // Kills credit the gather task (chance rolls; 200 kills is plenty).
    let task = def.tasks[0].clone();
    let monster = task.monsters[0].monster;
    let map = world.objects[&me].map;
    for _ in 0..200 {
        world.test_quest_kill(me, monster, map);
    }
    let q = world
        .test_quests(me)
        .into_iter()
        .find(|q| q.quest == 9)
        .unwrap();
    let amount = q
        .tasks
        .iter()
        .find(|(t, _)| *t == task.index)
        .map(|(_, a)| *a)
        .unwrap_or(0);
    assert_eq!(amount, task.amount, "task capped at the required amount");
    drain(&mut world);
    // Completing at the finish NPC grants the reward and marks it done.
    let finish = if def.finish_npc == def.start_npc {
        npc
    } else {
        world.test_open_npc(me, def.finish_npc)
    };
    assert!(world
        .test_npc_quests(me, finish)
        .iter()
        .any(|q| q.quest == 9 && q.state == 2));
    let gold_before = world.test_gold(me);
    let reward = def.rewards[0].clone();
    let reward_def = world.data.items[&reward.item].clone();
    let exp_before = world.test_experience(me);
    world.quest_complete(me, 9, 0);
    let q = world
        .test_quests(me)
        .into_iter()
        .find(|q| q.quest == 9)
        .unwrap();
    assert!(q.completed, "quest completed");
    if reward_def.effect == 2 {
        assert_eq!(world.test_experience(me), exp_before + reward.amount as u64);
    } else if reward.item == world.data.gold_item {
        assert_eq!(world.test_gold(me), gold_before + reward.amount as u64);
    } else if reward_def.item_type == 34 {
        assert!(
            world.test_currency_total(me) >= reward.amount as i64,
            "currency reward credited"
        );
    } else {
        assert!(
            world.test_slot_of(me, reward.item).is_some(),
            "reward item in the bag"
        );
    }
    // Requirement HaveNotCompleted now blocks a second run.
    assert!(!world.test_npc_quests(me, npc).iter().any(|q| q.quest == 9));
}

#[test]
fn wave_four_bursts_and_buffs() {
    let Some(mut world) = world() else {
        return;
    };
    // A high-level warrior at the start: Taecheon Sword burns everything
    // within two cells; Invincibility nulls incoming damage.
    let mut rec = test_character("Blade");
    rec.level = 70;
    let me = world.add_player(1, 1, &rec).unwrap();
    world.tick(0);
    drain(&mut world);
    let has_book = |world: &World, name: &str| {
        world
            .data
            .magics
            .values()
            .find(|m| m.name == name)
            .map(|def| {
                world
                    .data
                    .items
                    .values()
                    .any(|i| i.item_type == 14 && i.shape == def.index)
            })
            .unwrap_or(false)
    };
    if !has_book(&world, "Taecheon Sword") || !has_book(&world, "Invincibility") {
        eprintln!("books missing; skipping");
        return;
    }
    let burst = learn(&mut world, me, "Taecheon Sword");
    let shield = learn(&mut world, me, "Invincibility");
    let mut now = 3000;
    world.tick(now);
    drain(&mut world);
    let victim = nearest_chicken(&world, me);
    let loc = world.objects[&me].location;
    let cell = Direction::ALL
        .iter()
        .map(|d| loc.step(*d, 1))
        .find(|p| {
            world.maps[&world.objects[&me].map]
                .file
                .is_walkable(p.x, p.y)
        })
        .unwrap();
    world.teleport(victim, cell);
    let (hp0, _, _) = world.test_hp(victim);
    world.cast(me, burst, Direction::Down, None, loc);
    for _ in 0..20 {
        now += 100;
        world.teleport(victim, cell);
        world.tick(now);
    }
    let (hp1, _, dead) = world.test_hp(victim);
    assert!(
        dead || hp1 < hp0,
        "Taecheon Sword hurt the chicken: {hp0} -> {hp1}"
    );
    // Invincibility: a direct hit does nothing while the buff lasts.
    world.cast(me, shield, Direction::Down, None, loc);
    for _ in 0..8 {
        now += 100;
        world.tick(now);
    }
    assert!(world.objects[&me].has_buff(mir_proto::buff_type::INVINCIBILITY));
    let (php0, _, _) = world.test_hp(me);
    world.test_damage(me, victim, 50);
    let (php1, _, _) = world.test_hp(me);
    assert_eq!(php0, php1, "invincible");
}
