//! Test-only, loopback payment grant channel. The game loop owns account writes.
use std::io::Read;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use hmac::{Hmac, Mac};
use mir_server::accounts::Accounts;
use mir_server::world::World;
use serde::Deserialize;
use sha2::Sha256;
use tiny_http::{Method, Response, Server, StatusCode};
use tokio::sync::mpsc;

use crate::{Sessions, Stage};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Grant {
    pub session: String,
    pub account: u32,
    pub character: u32,
    pub gold: i64,
}

pub(super) struct PaymentRequest {
    pub grant: Grant,
    pub response: std_mpsc::Sender<std::result::Result<(), String>>,
}

fn valid_signature(secret: &str, body: &[u8], signature: &str) -> bool {
    let Ok(bytes) = hex::decode(signature) else {
        return false;
    };
    let Ok(mut verifier) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    verifier.update(body);
    verifier.verify_slice(&bytes).is_ok()
}

pub(super) fn listen(secret: String, tx: mpsc::UnboundedSender<PaymentRequest>) -> Result<()> {
    anyhow::ensure!(
        secret.len() >= 32,
        "ZIRCON_STRIPE_GRANT_SECRET must contain at least 32 characters"
    );
    let port = std::env::var("ZIRCON_STRIPE_GRANT_PORT")
        .unwrap_or_else(|_| "8788".into())
        .parse::<u16>()?;
    let server = Server::http(("127.0.0.1", port)).map_err(|error| anyhow!("{error}"))?;
    tracing::info!(port, "Stripe test grant listener on loopback");
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let status = if request.method() != &Method::Post || request.url() != "/stripe/grant" {
                404
            } else {
                let mut body = Vec::new();
                match request.as_reader().take(4097).read_to_end(&mut body) {
                    Ok(_) if body.len() <= 4096 => {
                        let signature = request
                            .headers()
                            .iter()
                            .find(|header| header.field.equiv("X-Zircon-Signature"))
                            .map(|header| header.value.as_str())
                            .unwrap_or("");
                        if !valid_signature(&secret, &body, signature) {
                            403
                        } else if let Ok(grant) = serde_json::from_slice::<Grant>(&body) {
                            let (reply, result) = std_mpsc::channel();
                            if tx
                                .send(PaymentRequest {
                                    grant,
                                    response: reply,
                                })
                                .is_err()
                            {
                                503
                            } else {
                                match result.recv_timeout(Duration::from_secs(5)) {
                                    Ok(Ok(())) => 200,
                                    Ok(Err(error)) => {
                                        tracing::warn!(reason = %error, "Stripe grant rejected");
                                        503
                                    }
                                    Err(_) => 503,
                                }
                            }
                        } else {
                            400
                        }
                    }
                    _ => 413,
                }
            };
            let _ = request.respond(Response::empty(StatusCode(status)));
        }
    });
    Ok(())
}

pub(super) fn apply(
    world: &mut World,
    accounts: &mut Accounts,
    sessions: &Sessions,
    grant: Grant,
) -> std::result::Result<(), String> {
    let currency = world
        .data
        .currencies
        .iter()
        .find(|currency| currency.currency_type == 1)
        .map(|currency| currency.index)
        .ok_or("Game Gold currency is not configured")?;
    // Snapshot first so a later periodic save cannot overwrite the paid balance.
    sessions.snapshot_all(world, accounts);
    let credited = accounts.credit_stripe_session(
        grant.account,
        grant.character,
        currency,
        &grant.session,
        grant.gold,
    )?;
    if let Some(balance) = credited {
        let online = sessions.state.values().find_map(|stage| match stage {
            Stage::InGame {
                account,
                character,
                object,
            } if *account == grant.account && *character == grant.character => Some(*object),
            _ => None,
        });
        if let Some(object) = online {
            if let Some(player) = world
                .objects
                .get_mut(&object)
                .and_then(|object| object.player_mut())
            {
                player.currencies.insert(currency, balance);
                world.send_currencies(object);
            }
        }
        tracing::info!(session = %grant.session, account = grant.account, character = grant.character, gold = grant.gold, "Stripe test grant credited");
    }
    accounts
        .save_if_dirty()
        .map_err(|error| format!("Saving paid grant failed: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmac::Mac;
    use mir_proto::{Class, Gender};
    use mir_server::data::GameData;
    use std::collections::HashMap;

    #[test]
    fn local_grants_require_valid_raw_body_mac() {
        let body = br#"{"session":"cs_test_1","account":1,"character":2,"gold":100}"#;
        let mut mac =
            Hmac::<Sha256>::new_from_slice(b"a long local test secret with enough bytes").unwrap();
        mac.update(body);
        let signature = hex::encode(mac.finalize().into_bytes());
        assert!(valid_signature(
            "a long local test secret with enough bytes",
            body,
            &signature
        ));
        assert!(!valid_signature(
            "a long local test secret with enough bytes",
            b"{}",
            &signature
        ));
    }

    #[test]
    fn paid_grant_persists_once_and_survives_restart() {
        let Some(assets) = std::env::var_os("ZIRCON_ASSETS") else {
            return;
        };
        let assets = std::path::PathBuf::from(assets);
        let data = GameData::load(assets.join("../Database/System.db")).unwrap();
        let currency = data
            .currencies
            .iter()
            .find(|currency| currency.currency_type == 1)
            .unwrap()
            .index;
        let mut world = World::new(data, assets.join("Map"), None);
        let dir = std::env::temp_dir().join(format!("zircon-stripe-grant-{}", std::process::id()));
        let path = dir.join("accounts.json");
        let mut accounts = Accounts::load(&path).unwrap();
        let account = accounts.create("buyer@example.test", "secret123").unwrap();
        let character = accounts
            .new_character(account, "Buyer", Class::Warrior, Gender::Male, 1)
            .unwrap()
            .id;
        let object = world
            .add_player(1, account, accounts.character(account, character).unwrap())
            .unwrap();
        let mut sessions = Sessions {
            conns: HashMap::new(),
            state: HashMap::new(),
            since: HashMap::new(),
            counts: HashMap::new(),
            dropped: Vec::new(),
        };
        sessions.state.insert(
            1,
            Stage::InGame {
                account,
                character,
                object,
            },
        );
        let grant = || Grant {
            session: "cs_test_paid".into(),
            account,
            character,
            gold: 100,
        };
        apply(&mut world, &mut accounts, &sessions, grant()).unwrap();
        assert_eq!(
            world.objects[&object].player().unwrap().currencies[&currency],
            100
        );
        // A webhook replay must not refund Game Gold already spent in game.
        world
            .objects
            .get_mut(&object)
            .unwrap()
            .player_mut()
            .unwrap()
            .currencies
            .insert(currency, 25);
        apply(&mut world, &mut accounts, &sessions, grant()).unwrap();
        let loaded = Accounts::load(&path).unwrap();
        let record = loaded.character(account, character).unwrap();
        assert_eq!(record.currencies, vec![(currency, 25)]);
        assert_eq!(record.stripe_grants, vec!["cs_test_paid"]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
