//! Multiplayer smoke test against the real binary: a dozen bots log in,
//! walk and chat at once, a flooder and an oversized frame are dropped
//! without disturbing the others, and SIGTERM saves everything.
//! Needs `ZIRCON_ASSETS`; skipped otherwise.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use mir_proto::{
    decode_frame, encode, Class, ClientMessage, Direction, Gender, NewCharacterResult,
    ServerMessage, PROTOCOL_VERSION,
};

const BOTS: usize = 12;

struct Bot {
    stream: TcpStream,
    buf: Vec<u8>,
    welcomed: bool,
    moves: usize,
    pongs: usize,
    closed: bool,
}

impl Bot {
    fn connect(port: u16) -> Bot {
        let stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_millis(30)))
            .unwrap();
        stream.set_nodelay(true).unwrap();
        Bot {
            stream,
            buf: Vec::new(),
            welcomed: false,
            moves: 0,
            pongs: 0,
            closed: false,
        }
    }

    fn send(&mut self, m: &ClientMessage) {
        if self.stream.write_all(&encode(m).unwrap()).is_err() {
            self.closed = true;
        }
    }

    /// Read what has arrived; returns the decoded messages.
    fn pump(&mut self) -> Vec<ServerMessage> {
        let mut out = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match self.stream.read(&mut chunk) {
                Ok(0) => {
                    self.closed = true;
                    break;
                }
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => break,
                Err(_) => {
                    self.closed = true;
                    break;
                }
            }
        }
        while let Ok(Some(m)) = decode_frame::<ServerMessage>(&mut self.buf) {
            match &m {
                ServerMessage::Welcome { .. } => self.welcomed = true,
                ServerMessage::ObjectMove { .. } | ServerMessage::MoveDenied { .. } => {
                    self.moves += 1
                }
                ServerMessage::Pong { .. } => self.pongs += 1,
                _ => {}
            }
            out.push(m);
        }
        out
    }

    /// Pump until `pred` matches a message or the deadline passes.
    fn wait_for(
        &mut self,
        secs: u64,
        pred: impl Fn(&ServerMessage) -> bool,
    ) -> Option<ServerMessage> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < deadline {
            for m in self.pump() {
                if pred(&m) {
                    return Some(m);
                }
            }
            if self.closed {
                return None;
            }
        }
        None
    }

    fn enter_world(&mut self, i: usize) {
        self.send(&ClientMessage::Hello {
            version: PROTOCOL_VERSION,
        });
        self.wait_for(10, |m| matches!(m, ServerMessage::Connected))
            .expect("connected");
        let email = format!("bot{i}@load.test");
        self.send(&ClientMessage::NewAccount {
            email: email.clone(),
            password: "secret123".into(),
        });
        self.wait_for(10, |m| matches!(m, ServerMessage::NewAccountResult(_)))
            .expect("account result");
        self.send(&ClientMessage::Login {
            email,
            password: "secret123".into(),
        });
        self.wait_for(10, |m| matches!(m, ServerMessage::LoginResult(_)))
            .expect("login result");
        self.send(&ClientMessage::NewCharacter {
            name: format!("Bot{i}"),
            class: Class::Warrior,
            gender: Gender::Male,
            hair: 1,
        });
        let created = self
            .wait_for(10, |m| matches!(m, ServerMessage::NewCharacterResult(_)))
            .expect("character result");
        let id = match created {
            ServerMessage::NewCharacterResult(NewCharacterResult::Success { character }) => {
                character.id
            }
            other => panic!("character creation failed: {other:?}"),
        };
        self.send(&ClientMessage::StartGame { id });
        self.wait_for(20, |m| matches!(m, ServerMessage::Welcome { .. }))
            .expect("welcome");
    }
}

/// Kills the server when the test ends, even on a panic.
struct Server(std::process::Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn a_dozen_bots_play_while_abusers_are_dropped_and_sigterm_saves() {
    let Some(assets) = std::env::var_os("ZIRCON_ASSETS") else {
        eprintln!("ZIRCON_ASSETS not set; skipping");
        return;
    };
    let data = std::env::temp_dir().join(format!("zircon-load-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    std::fs::create_dir_all(&data).unwrap();
    let port = free_port();
    let mut server = Server(
        Command::new(env!("CARGO_BIN_EXE_mir-server"))
            .args(["--port", &port.to_string(), "--data"])
            .arg(&data)
            .env("ZIRCON_ASSETS", &assets)
            .env(
                "RUST_LOG",
                std::env::var("ZIRCON_LOAD_LOG").unwrap_or_else(|_| "warn".into()),
            )
            .stdout(if std::env::var_os("ZIRCON_LOAD_LOG").is_some() {
                Stdio::inherit()
            } else {
                Stdio::null()
            })
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn server"),
    );
    // Wait for the listener.
    let start = Instant::now();
    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "server did not start"
        );
        std::thread::sleep(Duration::from_millis(200));
    }

    let mut bots: Vec<Bot> = (0..BOTS).map(|_| Bot::connect(port)).collect();
    for (i, b) in bots.iter_mut().enumerate() {
        b.enter_world(i);
    }
    assert!(bots.iter().all(|b| b.welcomed));

    // Three seconds of everyone walking, chatting and pinging at once.
    let dirs = Direction::ALL;
    let end = Instant::now() + Duration::from_secs(3);
    let mut step = 0u32;
    while Instant::now() < end {
        for (i, b) in bots.iter_mut().enumerate() {
            b.send(&ClientMessage::Move {
                direction: dirs[(step as usize + i) % dirs.len()],
                run: false,
            });
            if step.is_multiple_of(6) {
                b.send(&ClientMessage::Chat {
                    text: format!("hello from bot {i}"),
                });
                b.send(&ClientMessage::Ping { nonce: step });
            }
            b.pump();
        }
        step += 1;
        std::thread::sleep(Duration::from_millis(150));
    }
    for b in bots.iter_mut() {
        b.pump();
    }
    assert!(bots.iter().all(|b| !b.closed), "a bot was disconnected");
    assert!(bots.iter().all(|b| b.moves > 0), "a bot never moved");
    assert!(bots.iter().all(|b| b.pongs > 0), "a bot got no pong");

    // A flooder (hundreds of chat lines in one burst) is dropped.
    let mut flooder = Bot::connect(port);
    flooder.enter_world(BOTS);
    for n in 0..600 {
        flooder.send(&ClientMessage::Chat {
            text: format!("spam {n}"),
        });
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while !flooder.closed && Instant::now() < deadline {
        flooder.pump();
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(flooder.closed, "the flooder was not disconnected");

    // A frame larger than the client cap is dropped before it is read.
    let mut oversize = Bot::connect(port);
    let mut frame = (200u32 * 1024).to_le_bytes().to_vec();
    frame.extend(std::iter::repeat_n(0u8, 4096));
    let _ = oversize.stream.write_all(&frame);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !oversize.closed && Instant::now() < deadline {
        oversize.pump();
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(oversize.closed, "the oversized frame was accepted");

    // The honest bots are still served.
    for b in bots.iter_mut() {
        let before = b.pongs;
        b.send(&ClientMessage::Ping { nonce: 99 });
        assert!(
            b.wait_for(5, |m| matches!(m, ServerMessage::Pong { nonce: 99 }))
                .is_some(),
            "bot lost service (pongs {before})"
        );
    }

    // SIGTERM saves every character before exit.
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &server.0.id().to_string()])
            .status();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Ok(Some(_)) = server.0.try_wait() {
                break;
            }
            if Instant::now() > deadline {
                panic!("server did not exit on SIGTERM");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let saved = std::fs::read_to_string(data.join("accounts.json")).expect("accounts.json");
        for i in 0..BOTS {
            assert!(saved.contains(&format!("\"Bot{i}\"")), "Bot{i} not saved");
        }
    }
    drop(server);
    let _ = std::fs::remove_dir_all(&data);
}
