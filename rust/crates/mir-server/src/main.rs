//! Rust Zircon game server.
//!
//! ```text
//! mir-server [--assets DIR] [--db FILE] [--port N] [--map FILE] [--inspect]
//!   ZIRCON_ASSETS  client root containing Map/ (default ~/zircon-assets/Client)
//!   ZIRCON_DB      System.db path (default <assets>/../Database/System.db)
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use mir_proto::{
    ClientMessage, LoginResult, NewAccountResult, NewCharacterResult, ServerMessage,
    PROTOCOL_VERSION,
};

use mir_server::accounts::{now_secs, test_character, Accounts};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use mir_server::data::GameData;
use mir_server::net::{self, Inbound};
use mir_server::world::{ConnId, Outgoing, World};

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
    world.set_store_dir(&args.data);

    if args.inspect {
        return inspect(&mut world);
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let (in_tx, mut in_rx) = mpsc::unbounded_channel::<Inbound>();
    #[cfg(unix)]
    {
        let tx = in_tx.clone();
        runtime.spawn(async move {
            let Ok(mut hup) =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
            else {
                return;
            };
            while hup.recv().await.is_some() {
                let _ = tx.send(Inbound::Reload);
            }
        });
    }
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
    let mut next_db_check = Instant::now() + DB_CHECK_INTERVAL;
    let mut db_stamp = db_modified(&args.db);
    for p in world.data.validate(&args.assets.join("Map")) {
        tracing::warn!("data: {p}");
    }
    loop {
        // Drain input.
        while let Ok(ev) = in_rx.try_recv() {
            if matches!(ev, Inbound::Reload) {
                reload(&mut world, &args);
                continue;
            }
            handle_inbound(&mut world, &mut accounts, &mut sessions, ev);
        }
        // The editor saved a new System.db: reload it.
        if Instant::now() >= next_db_check {
            next_db_check = Instant::now() + DB_CHECK_INTERVAL;
            let stamp = db_modified(&args.db);
            if stamp != db_stamp {
                db_stamp = stamp;
                reload(&mut world, &args);
            }
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
const DB_CHECK_INTERVAL: Duration = Duration::from_secs(2);

fn db_modified(path: &PathBuf) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Re-read `System.db`; on failure the old data stays in use.
fn reload(world: &mut World, args: &Args) {
    match GameData::load(&args.db) {
        Ok(data) => {
            let problems = data.validate(&args.assets.join("Map"));
            for p in &problems {
                tracing::warn!("data: {p}");
            }
            world.reload_data(data);
            tracing::info!(problems = problems.len(), "game data reloaded");
        }
        Err(e) => tracing::error!("reload failed, keeping the old data: {e:#}"),
    }
}

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
        // Handled by the main loop before dispatch.
        Inbound::Reload => {}
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
            tracing::debug!(conn, ?stage, kind = %message_kind(&msg), "message");
            handle_message(world, accounts, sessions, conn, stage, msg);
        }
    }
}

/// Variant name of a client message, for debug logging.
fn message_kind(msg: &ClientMessage) -> String {
    let text = format!("{msg:?}");
    text.split([' ', '{', '('])
        .next()
        .unwrap_or("?")
        .to_string()
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
