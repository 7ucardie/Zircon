//! Blocking TCP client on a background thread.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use mir_proto::{decode_frame, encode, ClientMessage, ServerMessage};

pub enum NetEvent {
    Message(ServerMessage),
    Disconnected(String),
}

pub struct Connection {
    rx: Receiver<NetEvent>,
    tx: Sender<ClientMessage>,
}

impl Connection {
    pub fn connect(addr: &str) -> std::io::Result<Connection> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        let mut reader = stream.try_clone()?;
        let mut writer = stream;
        let (ev_tx, ev_rx) = mpsc::channel::<NetEvent>();
        let (msg_tx, msg_rx) = mpsc::channel::<ClientMessage>();

        let ev_tx_read = ev_tx.clone();
        thread::Builder::new()
            .name("net-read".into())
            .spawn(move || {
                let mut buf = Vec::with_capacity(8192);
                let mut chunk = [0u8; 8192];
                loop {
                    match reader.read(&mut chunk) {
                        Ok(0) => {
                            let _ = ev_tx_read.send(NetEvent::Disconnected("server closed".into()));
                            return;
                        }
                        Ok(n) => {
                            buf.extend_from_slice(&chunk[..n]);
                            loop {
                                match decode_frame::<ServerMessage>(&mut buf) {
                                    Ok(Some(msg)) => {
                                        if ev_tx_read.send(NetEvent::Message(msg)).is_err() {
                                            return;
                                        }
                                    }
                                    Ok(None) => break,
                                    Err(e) => {
                                        let _ =
                                            ev_tx_read.send(NetEvent::Disconnected(e.to_string()));
                                        return;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let _ = ev_tx_read.send(NetEvent::Disconnected(e.to_string()));
                            return;
                        }
                    }
                }
            })?;

        thread::Builder::new()
            .name("net-write".into())
            .spawn(move || {
                while let Ok(msg) = msg_rx.recv() {
                    let frame = match encode(&msg) {
                        Ok(f) => f,
                        Err(_) => continue,
                    };
                    if writer.write_all(&frame).is_err() {
                        let _ = ev_tx.send(NetEvent::Disconnected("write failed".into()));
                        return;
                    }
                }
            })?;

        Ok(Connection {
            rx: ev_rx,
            tx: msg_tx,
        })
    }

    pub fn send(&self, msg: ClientMessage) {
        let _ = self.tx.send(msg);
    }

    pub fn poll(&self) -> Option<NetEvent> {
        match self.rx.try_recv() {
            Ok(ev) => Some(ev),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(NetEvent::Disconnected("channel closed".into()))
            }
        }
    }
}
