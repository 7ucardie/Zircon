//! Game environment — the single-threaded game loop (SEnvir equivalent).
//!
//! # Threading model
//!
//! All game state lives here.  The game loop runs in one Tokio task and
//! never shares mutable state with other tasks.  Connections communicate
//! with the loop exclusively through mpsc channels:
//!
//! ```text
//! listener task  ──(new_conn_rx)──►  Envir::run()  ──(conn.outbound_tx)──► writer task
//! reader task    ──(inbound_rx)  ──►              ◄────────────────────
//! ```

pub mod world;

use std::time::{Duration, Instant};

use tokio::{
    sync::mpsc,
    time::{interval, MissedTickBehavior},
};
use tracing::{debug, info, warn};
use zircon_db::GameData;

use crate::{
    config::Config,
    network::{
        connection::{ConnectionHandle, InboundFrame},
        NewConnection,
    },
};
use world::World;

/// Reason a connection is being removed.
#[derive(Debug)]
enum DropReason {
    PeerClosed,
    SendQueueFull,
    KickedByServer,
}

/// All mutable game state owned by the game loop.
pub struct Envir {
    config: Config,

    // ── Channels ─────────────────────────────────────────────────────────
    new_conn_rx: mpsc::Receiver<NewConnection>,
    inbound_rx: mpsc::Receiver<InboundFrame>,

    // ── World state ──────────────────────────────────────────────────────
    world: World,
    game_data: GameData,

    // ── Connection registry ───────────────────────────────────────────────
    connections: Vec<ConnectionHandle>,

    // ── Tick counters ─────────────────────────────────────────────────────
    tick: u64,
    started_at: Instant,
    last_save_tick: u64,
}

/// Channel capacities.
const NEW_CONN_CAP: usize = 64;
const INBOUND_CAP: usize = 4096;

impl Envir {
    /// Build an `Envir` and return both channel senders so the network layer
    /// can push new connections and inbound frames into the game loop.
    pub fn new(
        config: Config,
        game_data: GameData,
    ) -> (Self, mpsc::Sender<NewConnection>, mpsc::Sender<InboundFrame>) {
        let (new_conn_tx, new_conn_rx) = mpsc::channel(NEW_CONN_CAP);
        let (inbound_tx, inbound_rx) = mpsc::channel(INBOUND_CAP);

        let mut world = World::new();
        world.load_maps(game_data.maps.clone());

        let envir = Envir {
            config,
            new_conn_rx,
            inbound_rx,
            world,
            game_data,
            connections: Vec::new(),
            tick: 0,
            started_at: Instant::now(),
            last_save_tick: 0,
        };

        (envir, new_conn_tx, inbound_tx)
    }

    /// Run the game loop.  This future never completes normally.
    pub async fn run(mut self) {
        let tick_dur = Duration::from_millis(self.config.tick_ms);
        let mut ticker = interval(tick_dur);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        info!(
            tick_ms = self.config.tick_ms,
            maps = self.world.maps.len(),
            "game loop started"
        );

        loop {
            ticker.tick().await;
            self.tick += 1;
            self.process_tick();
        }
    }

    // ── Tick work ─────────────────────────────────────────────────────────

    fn process_tick(&mut self) {
        self.drain_new_connections();
        self.drain_inbound_frames();
        self.process_objects();
        self.periodic_work();
        self.reap_closed_connections();
    }

    /// Accept any connections queued by the listener task.
    fn drain_new_connections(&mut self) {
        let limit = 15; // mirrors C# (15 per tick)
        for _ in 0..limit {
            match self.new_conn_rx.try_recv() {
                Ok(nc) => {
                    info!(conn_id = nc.handle.conn_id, "registered connection");
                    self.connections.push(nc.handle);
                }
                Err(_) => break,
            }
        }
    }

    /// Drain the shared inbound-frame queue, dispatch each frame.
    fn drain_inbound_frames(&mut self) {
        let limit = self.config.max_packet_queue * self.connections.len().max(1);
        let mut processed = 0;

        while processed < limit {
            match self.inbound_rx.try_recv() {
                Ok(frame) => {
                    self.dispatch_frame(frame);
                    processed += 1;
                }
                Err(_) => break,
            }
        }
    }

    /// Route one decoded frame to the right handler.
    fn dispatch_frame(&mut self, frame: InboundFrame) {
        debug!(
            conn_id = frame.conn_id,
            packet_id = frame.frame.packet_id,
            "dispatch"
        );
        // Task 2.5 will implement full packet dispatch here.
        // For now: echo the raw frame back as a no-op.
        let _ = frame;
    }

    /// Process active game objects for this tick.
    fn process_objects(&mut self) {
        // Task 2.5 implements monster/player AI here.
        // The active_queue already contains IDs that need updating.
        let deadline = Instant::now() + Duration::from_millis(1);

        let ids: Vec<_> = self.world.active_queue.clone();
        for id in ids {
            if Instant::now() >= deadline {
                break; // yield — process remaining next tick
            }
            if let Some(_obj) = self.world.objects.get_mut(&id) {
                // obj.process(self.tick);   ← Task 2.5
            }
        }
    }

    /// Work that runs every N ticks (map spawns, DB saves, etc.).
    fn periodic_work(&mut self) {
        let tps = 1000 / self.config.tick_ms.max(1);
        let secs_elapsed = self.tick / tps.max(1);

        // Every ~1 second: map process (spawn checks, etc.)
        if self.tick % tps.max(1) == 0 {
            self.process_maps(secs_elapsed);
        }

        // DB save interval
        if self.tick - self.last_save_tick >= self.config.save_interval_ticks {
            self.save();
        }
    }

    fn process_maps(&mut self, _secs: u64) {
        // Task 2.5: trigger spawn checks, region events, etc.
    }

    fn save(&mut self) {
        info!(tick = self.tick, "database save");
        // Task 2.5: flush user data to Users.db / SQLite.
        self.last_save_tick = self.tick;
    }

    /// Remove connections whose outbound channel has been closed (peer gone).
    fn reap_closed_connections(&mut self) {
        let before = self.connections.len();
        self.connections.retain(|c| !c.is_closed());
        let removed = before - self.connections.len();
        if removed > 0 {
            warn!(removed, "reaped closed connections");
        }
    }

    // ── Public accessors (used by packet handlers in Task 2.5) ───────────

    pub fn tick(&self) -> u64 { self.tick }
    pub fn uptime(&self) -> Duration { self.started_at.elapsed() }
    pub fn connection_count(&self) -> usize { self.connections.len() }
    pub fn world(&self) -> &World { &self.world }
    pub fn world_mut(&mut self) -> &mut World { &mut self.world }
    pub fn game_data(&self) -> &GameData { &self.game_data }

    /// Send encoded bytes to a specific connection.
    pub fn send_to(&self, conn_id: u32, data: bytes::Bytes) {
        if let Some(c) = self.connections.iter().find(|c| c.conn_id == conn_id) {
            c.send(data);
        }
    }

    /// Broadcast encoded bytes to every connection.
    pub fn broadcast(&self, data: bytes::Bytes) {
        for c in &self.connections {
            c.send(data.clone());
        }
    }
}
