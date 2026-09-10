//! Rust Zircon game server.
//!
//! ```text
//! mir-server [--assets DIR] [--db FILE] [--port N] [--map FILE] [--inspect]
//!   ZIRCON_ASSETS  client root containing Map/ (default ~/zircon-assets/Client)
//!   ZIRCON_DB      System.db path (default <assets>/../Database/System.db)
//! ```

mod accounts;
mod data;
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
            match world.add_player(conn, &rec) {
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
        (Stage::InGame { object, .. }, ClientMessage::Attack { direction }) => {
            world.player_attack(object, direction)
        }
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
    }
}

/// Print what a new warrior would get, then exit. Useful without a client.
fn inspect(world: &mut World) -> anyhow::Result<()> {
    let id = world.add_player(0, &test_character("Inspector"))?;
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
        let me = world.add_player(1, &test_character("Tester")).unwrap();
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
            world.player_attack(me, dir);
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
    fn passive_animals_do_not_aggro_but_hunters_do() {
        let Some(mut world) = world() else {
            return;
        };
        let _me = world.add_player(1, &test_character("Tester")).unwrap();
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
        let me = world.add_player(1, &test_character("Tester")).unwrap();
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
