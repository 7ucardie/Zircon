//! Rust Zircon game server.
//!
//! ```text
//! mir-server [--assets DIR] [--db FILE] [--port N] [--map FILE] [--inspect]
//!   ZIRCON_ASSETS  client root containing Map/ (default ~/zircon-assets/Client)
//!   ZIRCON_DB      System.db path (default <assets>/../Database/System.db)
//! ```

mod accounts;
mod data;
mod items;
mod magic;
mod net;
mod world;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use mir_proto::{
    ClientMessage, LoginResult, NewAccountResult, NewCharacterResult, ServerMessage,
    PROTOCOL_VERSION,
};

use crate::accounts::{now_secs, Accounts, CharacterRecord};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use crate::data::GameData;
use crate::net::Inbound;
use crate::world::{ConnId, Outgoing, World};

const TICK: Duration = Duration::from_millis(50);

struct Args {
    assets: PathBuf,
    db: PathBuf,
    data: PathBuf,
    port: u16,
    map: Option<String>,
    inspect: bool,
}

fn parse_args() -> Args {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let mut assets = std::env::var_os("ZIRCON_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("zircon-assets/Client"));
    let mut db = std::env::var_os("ZIRCON_DB").map(PathBuf::from);
    let mut data = std::env::var_os("ZIRCON_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"));
    let mut port = std::env::var("ZIRCON_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(mir_proto::DEFAULT_PORT);
    let mut map = None;
    let mut inspect = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--assets" => assets = PathBuf::from(it.next().expect("--assets DIR")),
            "--db" => db = Some(PathBuf::from(it.next().expect("--db FILE"))),
            "--data" => data = PathBuf::from(it.next().expect("--data DIR")),
            "--port" => port = it.next().expect("--port N").parse().expect("port"),
            "--map" => map = Some(it.next().expect("--map FILE")),
            "--inspect" => inspect = true,
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    let db = db.unwrap_or_else(|| assets.join("../Database/System.db"));
    Args {
        assets,
        db,
        data,
        port,
        map,
        inspect,
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let args = parse_args();
    tracing::info!(assets = %args.assets.display(), db = %args.db.display(), "loading game data");
    let data = GameData::load(&args.db)?;
    tracing::info!(
        maps = data.maps.len(),
        regions = data.regions.len(),
        respawns = data.respawns.len(),
        monsters = data.monsters.len(),
        safe_zones = data.safe_zones.len(),
        base_stats = data.base_stats.len(),
        "game data loaded"
    );
    let mut world = World::new(data, args.assets.join("Map"), args.map.clone());

    if args.inspect {
        return inspect(&mut world);
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let (in_tx, mut in_rx) = mpsc::unbounded_channel::<Inbound>();
    let addr: SocketAddr = format!("0.0.0.0:{}", args.port).parse()?;
    runtime.spawn(async move {
        if let Err(e) = net::listen(addr, in_tx).await {
            tracing::error!("listener failed: {e}");
            std::process::exit(1);
        }
    });

    let mut accounts = Accounts::load(args.data.join("accounts.json"))?;
    let mut sessions = Sessions {
        conns: HashMap::new(),
        state: HashMap::new(),
    };
    let start = Instant::now();
    let mut next_tick = Instant::now();
    let mut next_save = Instant::now() + SAVE_INTERVAL;
    loop {
        // Drain input.
        while let Ok(ev) = in_rx.try_recv() {
            handle_inbound(&mut world, &mut accounts, &mut sessions, ev);
        }
        let now = start.elapsed().as_millis() as u64;
        world.tick(now);
        for out in world.outgoing.drain(..) {
            match out {
                Outgoing::To(conn, msg) => sessions.send(conn, msg),
            }
        }
        if Instant::now() >= next_save {
            next_save = Instant::now() + SAVE_INTERVAL;
            sessions.snapshot_all(&world, &mut accounts);
            if let Err(e) = accounts.save_if_dirty() {
                tracing::error!("saving accounts failed: {e:#}");
            }
        }
        next_tick += TICK;
        let sleep = next_tick.saturating_duration_since(Instant::now());
        if sleep.is_zero() {
            next_tick = Instant::now();
        } else {
            std::thread::sleep(sleep);
        }
    }
}

const SAVE_INTERVAL: Duration = Duration::from_secs(30);

/// Where a connection is in the login flow.
#[derive(Debug, Clone, Copy)]
enum Stage {
    /// TCP connected, no `Hello` yet.
    New,
    Connected,
    LoggedIn {
        account: u32,
    },
    InGame {
        account: u32,
        character: u32,
        object: mir_proto::ObjectId,
    },
}

struct Sessions {
    conns: HashMap<ConnId, mpsc::UnboundedSender<ServerMessage>>,
    state: HashMap<ConnId, Stage>,
}

impl Sessions {
    fn send(&self, conn: ConnId, msg: ServerMessage) {
        if let Some(tx) = self.conns.get(&conn) {
            let _ = tx.send(msg);
        }
    }

    fn snapshot_all(&self, world: &World, accounts: &mut Accounts) {
        for stage in self.state.values() {
            if let Stage::InGame {
                account,
                character,
                object,
            } = stage
            {
                if let Some(rec) = accounts.character_mut(*account, *character) {
                    world.snapshot(*object, rec);
                    accounts.mark_dirty();
                }
            }
        }
    }

    /// Is this account already logged in on another connection?
    fn account_in_use(&self, account: u32, except: ConnId) -> bool {
        self.state.iter().any(|(c, s)| {
            *c != except
                && matches!(s, Stage::LoggedIn { account: a } | Stage::InGame { account: a, .. } if *a == account)
        })
    }
}

/// Take a player out of the world and persist it.
fn leave_world(world: &mut World, accounts: &mut Accounts, stage: Stage) {
    if let Stage::InGame {
        account,
        character,
        object,
    } = stage
    {
        if let Some(rec) = accounts.character_mut(account, character) {
            world.snapshot(object, rec);
            accounts.mark_dirty();
        }
        world.remove_object(object);
    }
}

fn handle_inbound(
    world: &mut World,
    accounts: &mut Accounts,
    sessions: &mut Sessions,
    ev: Inbound,
) {
    match ev {
        Inbound::Connected { conn, addr, tx } => {
            tracing::info!(conn, %addr, "connected");
            sessions.conns.insert(conn, tx);
            sessions.state.insert(conn, Stage::New);
        }
        Inbound::Disconnected { conn } => {
            tracing::info!(conn, "disconnected");
            sessions.conns.remove(&conn);
            if let Some(stage) = sessions.state.remove(&conn) {
                leave_world(world, accounts, stage);
                if let Err(e) = accounts.save_if_dirty() {
                    tracing::error!("saving accounts failed: {e:#}");
                }
            }
        }
        Inbound::Message { conn, msg } => {
            let stage = sessions.state.get(&conn).copied().unwrap_or(Stage::New);
            handle_message(world, accounts, sessions, conn, stage, msg);
        }
    }
}

/// Character summaries with the map name filled in from game data.
fn summaries(world: &World, accounts: &Accounts, account: u32) -> Vec<mir_proto::CharacterSummary> {
    let mut list = accounts.summaries(account);
    if let Some(acc) = accounts.account(account) {
        for s in &mut list {
            if let Some(rec) = acc.characters.iter().find(|c| c.id == s.id) {
                s.location = world
                    .data
                    .map_by_file(&rec.map)
                    .map(|m| m.description.clone())
                    .unwrap_or_default();
            }
        }
    }
    list
}

fn handle_message(
    world: &mut World,
    accounts: &mut Accounts,
    sessions: &mut Sessions,
    conn: ConnId,
    stage: Stage,
    msg: ClientMessage,
) {
    match (stage, msg) {
        (_, ClientMessage::Ping { nonce }) => sessions.send(conn, ServerMessage::Pong { nonce }),
        (Stage::New, ClientMessage::Hello { version }) => {
            if version != PROTOCOL_VERSION {
                sessions.send(
                    conn,
                    ServerMessage::Rejected {
                        reason: format!("protocol {version} != {PROTOCOL_VERSION}"),
                    },
                );
                return;
            }
            sessions.state.insert(conn, Stage::Connected);
            sessions.send(conn, ServerMessage::Connected);
        }
        (Stage::Connected, ClientMessage::NewAccount { email, password }) => {
            let result = match accounts.create(&email, &password) {
                Ok(id) => {
                    tracing::info!(conn, id, email, "account created");
                    NewAccountResult::Success
                }
                Err(reason) => NewAccountResult::Failed { reason },
            };
            let _ = accounts.save_if_dirty();
            sessions.send(conn, ServerMessage::NewAccountResult(result));
        }
        (Stage::Connected, ClientMessage::Login { email, password }) => {
            let result = match accounts.login(&email, &password) {
                Ok(account) if sessions.account_in_use(account, conn) => LoginResult::Failed {
                    reason: "Account already logged in".into(),
                },
                Ok(account) => {
                    tracing::info!(conn, account, email, "logged in");
                    sessions.state.insert(conn, Stage::LoggedIn { account });
                    LoginResult::Success {
                        characters: summaries(world, accounts, account),
                    }
                }
                Err(reason) => LoginResult::Failed { reason },
            };
            sessions.send(conn, ServerMessage::LoginResult(result));
        }
        (
            Stage::LoggedIn { account },
            ClientMessage::NewCharacter {
                name,
                class,
                gender,
                hair,
            },
        ) => {
            let result = match accounts.new_character(account, &name, class, gender, hair) {
                Ok(rec) => {
                    tracing::info!(conn, account, name = rec.name, ?class, "character created");
                    NewCharacterResult::Success {
                        character: rec.summary(),
                    }
                }
                Err(reason) => NewCharacterResult::Failed { reason },
            };
            let _ = accounts.save_if_dirty();
            sessions.send(conn, ServerMessage::NewCharacterResult(result));
        }
        (Stage::LoggedIn { account }, ClientMessage::DeleteCharacter { id }) => {
            let (ok, reason) = match accounts.delete_character(account, id) {
                Ok(()) => (true, String::new()),
                Err(r) => (false, r),
            };
            let _ = accounts.save_if_dirty();
            sessions.send(
                conn,
                ServerMessage::DeleteCharacterResult { id, ok, reason },
            );
        }
        (Stage::LoggedIn { account }, ClientMessage::StartGame { id }) => {
            let Some(rec) = accounts.character(account, id).cloned() else {
                sessions.send(
                    conn,
                    ServerMessage::Rejected {
                        reason: "No such character".into(),
                    },
                );
                return;
            };
            match world.add_player(conn, account, &rec) {
                Ok(object) => {
                    tracing::info!(conn, account, name = rec.name, ?object, "entered world");
                    if let Some(r) = accounts.character_mut(account, id) {
                        r.last_login = now_secs();
                        accounts.mark_dirty();
                    }
                    sessions.state.insert(
                        conn,
                        Stage::InGame {
                            account,
                            character: id,
                            object,
                        },
                    );
                }
                Err(e) => {
                    tracing::warn!(conn, "start game failed: {e:#}");
                    sessions.send(
                        conn,
                        ServerMessage::Rejected {
                            reason: format!("{e:#}"),
                        },
                    );
                }
            }
        }
        (Stage::InGame { account, .. }, ClientMessage::Logout) => {
            leave_world(world, accounts, stage);
            let _ = accounts.save_if_dirty();
            sessions.state.insert(conn, Stage::LoggedIn { account });
            sessions.send(
                conn,
                ServerMessage::LoggedOut {
                    characters: summaries(world, accounts, account),
                },
            );
        }
        (Stage::InGame { object, .. }, ClientMessage::Turn { direction }) => {
            world.player_turn(object, direction)
        }
        (Stage::InGame { object, .. }, ClientMessage::Move { direction, run }) => {
            world.player_move(object, direction, run)
        }
        (
            Stage::InGame { object, .. },
            ClientMessage::Attack {
                direction,
                attack_magic,
            },
        ) => world.player_attack(object, direction, attack_magic),
        (
            Stage::InGame { object, .. },
            ClientMessage::Magic {
                magic,
                direction,
                target,
                location,
            },
        ) => world.cast(object, magic, direction, target, location),
        (Stage::InGame { object, .. }, ClientMessage::MagicKey { magic, key }) => {
            world.magic_key(object, magic, key)
        }
        (Stage::InGame { object, .. }, ClientMessage::MagicToggle { magic, on }) => {
            world.magic_toggle(object, magic, on)
        }
        (
            Stage::InGame { object, .. },
            ClientMessage::ItemMove {
                from,
                from_slot,
                to,
                to_slot,
            },
        ) => world.item_move(object, from, from_slot, to, to_slot),
        (Stage::InGame { object, .. }, ClientMessage::ItemUse { slot }) => {
            world.item_use(object, slot)
        }
        (Stage::InGame { object, .. }, ClientMessage::BeltLink { slot, info, item }) => {
            world.belt_link(object, slot, info, item)
        }
        (Stage::InGame { object, .. }, ClientMessage::TownRevive) => world.town_revive(object),
        (Stage::InGame { object, .. }, ClientMessage::ItemDrop { slot, count }) => {
            world.item_drop(object, slot, count)
        }
        (Stage::InGame { object, .. }, ClientMessage::PickUp) => world.pick_up(object),
        (Stage::InGame { object, .. }, ClientMessage::NpcCall { id }) => world.npc_call(object, id),
        (Stage::InGame { object, .. }, ClientMessage::NpcButton { button }) => {
            world.npc_button(object, button)
        }
        (Stage::InGame { object, .. }, ClientMessage::NpcBuy { info, count }) => {
            world.npc_buy(object, info, count)
        }
        (Stage::InGame { object, .. }, ClientMessage::NpcSell { slots }) => {
            world.npc_sell(object, slots)
        }
        (Stage::InGame { object, .. }, ClientMessage::NpcClose) => world.npc_close(object),
        (stage, msg) => {
            tracing::debug!(conn, ?stage, ?msg, "message ignored in this stage");
        }
    }
}

/// A fresh level-1 warrior record, for `--inspect` and tests.
fn test_character(name: &str) -> CharacterRecord {
    CharacterRecord {
        id: 0,
        name: name.to_string(),
        class: mir_proto::Class::Warrior,
        gender: mir_proto::Gender::Male,
        hair: 1,
        level: 1,
        experience: 0,
        hp: 0,
        mp: 0,
        map: String::new(),
        location: mir_proto::Point::default(),
        direction: mir_proto::Direction::Down,
        created: 0,
        last_login: 0,
        deleted: false,
        items: Vec::new(),
        gold: 0,
        next_item_id: 0,
        magics: Vec::new(),
        belt: Vec::new(),
    }
}

/// Print what a new warrior would get, then exit. Useful without a client.
fn inspect(world: &mut World) -> anyhow::Result<()> {
    let id = world.add_player(0, 0, &test_character("Inspector"))?;
    let o = &world.objects[&id];
    let map = &world.maps[&o.map];
    println!(
        "start: map {} \"{}\" ({}) at ({}, {}), {}x{} cells, {} walkable",
        map.index,
        map.descriptor.name,
        map.descriptor.file,
        o.location.x,
        o.location.y,
        map.file.width,
        map.file.height,
        map.file.walkable_count()
    );
    println!(
        "player: hp {} dc {}-{} ac {}-{} acc {} agi {}",
        o.hp,
        o.stats.min_dc,
        o.stats.max_dc,
        o.stats.min_ac,
        o.stats.max_ac,
        o.stats.accuracy,
        o.stats.agility
    );
    println!("spawn groups on map: {}", map.spawns.len());
    let mut counts: HashMap<String, (i32, i32)> = HashMap::new();
    for g in &map.spawns {
        let m = &world.data.monsters[&g.def.monster];
        let e = counts.entry(m.name.clone()).or_insert((0, 0));
        e.0 += g.def.count;
        e.1 += g.alive;
    }
    let mut v: Vec<_> = counts.into_iter().collect();
    v.sort_by_key(|(_, (want, _))| std::cmp::Reverse(*want));
    for (name, (want, alive)) in v.iter().take(25) {
        let def = world
            .data
            .monsters
            .values()
            .find(|m| &m.name == name)
            .unwrap();
        println!(
            "  {name:24} want {want:4} alive {alive:4}  image {:3} lvl {:3} hp {:6} dc {}-{} ac {}-{} acc {} agi {} view {} atk {}ms move {}ms exp {}",
            def.image, def.level, def.health(), def.stat(8), def.stat(9), def.stat(4), def.stat(5), def.stat(14), def.stat(15), def.view_range, def.attack_delay, def.move_delay, def.experience
        );
    }
    let near: Vec<_> = world
        .objects
        .values()
        .filter(|m| !m.is_player() && m.location.distance(o.location) <= 18)
        .map(|m| match &m.appearance {
            mir_proto::Appearance::Monster { name, .. } => {
                format!("{name}@({},{})", m.location.x, m.location.y)
            }
            _ => String::new(),
        })
        .collect();
    println!(
        "monsters within view of start ({}): {}",
        near.len(),
        near.join(", ")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir_proto::{Appearance, Direction, ObjectId, Point};

    fn world() -> Option<World> {
        let assets = std::env::var_os("ZIRCON_ASSETS").map(PathBuf::from)?;
        let data = GameData::load(assets.join("../Database/System.db")).ok()?;
        Some(World::new(data, assets.join("Map"), None))
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
            .find(|o| !o.dead && matches!(&o.appearance, Appearance::Monster { name, .. } if name == "Chicken"))
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
            .find(|o| !o.dead && matches!(&o.appearance, Appearance::Monster { name, .. } if name == "Chicken"))
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
            .filter(|o| !o.dead && matches!(&o.appearance, Appearance::Monster { name, .. } if name == "Chicken"))
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
            if msgs.iter().any(
                |m| matches!(m, ServerMessage::ObjectMagic { magic, .. } if *magic == fire_ball),
            ) {
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
        assert!(msgs.iter().any(
            |m| matches!(m, ServerMessage::BuffAdd(b) if b.kind == mir_proto::buff_type::MIGHT)
        ));
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
                d.item_type == mir_proto::item_type::AMULET
                    && d.shape == 0
                    && d.required_amount <= 13
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
        // The chicken may have wandered off meanwhile; put it back in reach.
        world.teleport(victim, loc.step(dir, 1));
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
                d.item_type == mir_proto::item_type::AMULET
                    && d.shape == 0
                    && d.required_amount <= 17
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
                d.item_type == mir_proto::item_type::AMULET
                    && d.shape == 0
                    && d.required_amount <= 34
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
}
