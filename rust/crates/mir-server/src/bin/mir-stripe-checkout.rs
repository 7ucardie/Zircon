//! Local, test-mode Stripe Checkout bridge. Never bind this HTTP listener publicly.
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use hmac::{Hmac, Mac};
use mir_server::accounts::Accounts;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;
use tiny_http::{Header, Method, Response, Server, StatusCode};

type HmacSha256 = Hmac<Sha256>;
const MAX_BODY: u64 = 64 * 1024;
const SIGNATURE_AGE: i64 = 300;
const SHOP_PAGE: &str = r#"<!doctype html>
<html lang="en"><meta charset="utf-8"><title>ZirconRust test checkout</title>
<style>body{font:18px system-ui;background:#19120d;color:#f4dfaf;max-width:30rem;margin:4rem auto}
label{display:block;margin:1rem 0}input,select,button{font:inherit;padding:.5rem;width:100%;box-sizing:border-box}
button{background:#956d24;color:white;border:1px solid #e3b54c;cursor:pointer}</style>
<h1>Game Gold test checkout</h1>
<p>Stripe test mode. No real money is charged.</p>
<form id="shop">
<label>Email<input name="email" type="email" autocomplete="username" required></label>
<label>Password<input name="password" type="password" autocomplete="current-password" required></label>
<label>Character name<input name="character" required></label>
<label>Package<select name="package"><option value="100">100 Game Gold · €1.00 test</option>
<option value="500">500 Game Gold · €4.50 test</option></select></label>
<button>Continue to Stripe</button>
</form><p id="message" role="status"></p>
<script>
document.querySelector('#shop').addEventListener('submit', async e => {
  e.preventDefault();
  const form = e.currentTarget;
  const button = form.querySelector('button');
  button.disabled = true;
  const message = document.querySelector('#message');
  message.textContent = 'Creating test checkout…';
  try {
    const response = await fetch('/checkout', {method:'POST', headers:{'Content-Type':'application/json'},
      body:JSON.stringify(Object.fromEntries(new FormData(form)))});
    const result = await response.json();
    if (!response.ok) throw new Error('Check account, character, and checkout service.');
    location.assign(result.url);
  } catch (error) {
    message.textContent = error.message;
    button.disabled = false;
  }
});
</script></html>"#;

// These are test prices only. Set final packages after merchant review.
#[derive(Clone, Copy)]
struct Package {
    id: &'static str,
    gold: i64,
    cents: i64,
}

const PACKAGES: [Package; 2] = [
    Package {
        id: "100",
        gold: 100,
        cents: 100,
    },
    Package {
        id: "500",
        gold: 500,
        cents: 450,
    },
];

fn package(id: &str) -> Option<Package> {
    PACKAGES.iter().copied().find(|p| p.id == id)
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn mac(secret: &str, body: &[u8]) -> Result<String> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
    mac.update(body);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn verify_stripe(secret: &str, signature: &str, body: &[u8], clock: i64) -> Result<()> {
    let mut timestamp = None;
    let mut signatures = Vec::new();
    for part in signature.split(',').map(str::trim) {
        if let Some(value) = part.strip_prefix("t=") {
            timestamp = Some(value.parse::<i64>()?);
        } else if let Some(value) = part.strip_prefix("v1=") {
            signatures.push(value);
        }
    }
    let timestamp = timestamp.context("Missing Stripe timestamp")?;
    anyhow::ensure!(
        clock.abs_diff(timestamp) <= SIGNATURE_AGE as u64,
        "Expired Stripe signature"
    );
    let mut signed = timestamp.to_string().into_bytes();
    signed.push(b'.');
    signed.extend_from_slice(body);
    let mut verifier = HmacSha256::new_from_slice(secret.as_bytes())?;
    verifier.update(&signed);
    anyhow::ensure!(
        signatures.iter().any(|signature| {
            hex::decode(signature)
                .ok()
                .is_some_and(|bytes| verifier.clone().verify_slice(&bytes).is_ok())
        }),
        "Invalid Stripe signature"
    );
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct Grant {
    session: String,
    account: u32,
    character: u32,
    gold: i64,
}

fn paid_grant(event: &Value) -> Result<Option<Grant>> {
    anyhow::ensure!(event["object"] == "event", "Invalid Stripe event");
    let event_type = event["type"].as_str().unwrap_or_default();
    if event_type != "checkout.session.completed"
        && event_type != "checkout.session.async_payment_succeeded"
    {
        return Ok(None);
    }
    let session = &event["data"]["object"];
    anyhow::ensure!(event["livemode"] == false && session["livemode"] == false);
    anyhow::ensure!(session["object"] == "checkout.session");
    anyhow::ensure!(session["mode"] == "payment" && session["status"] == "complete");
    if session["payment_status"] != "paid" {
        return Ok(None);
    }
    let id = session["id"].as_str().context("Missing Checkout Session")?;
    anyhow::ensure!(
        id.starts_with("cs_test_"),
        "Only test sessions are accepted"
    );
    let metadata = &session["metadata"];
    let package = package(metadata["package"].as_str().context("Missing package")?)
        .context("Unknown package")?;
    anyhow::ensure!(
        session["currency"] == "eur" && session["amount_total"] == package.cents,
        "Payment does not match package price"
    );
    let account = metadata["account_id"]
        .as_str()
        .context("Missing account")?
        .parse::<u32>()?;
    let character = metadata["character_id"]
        .as_str()
        .context("Missing character")?
        .parse::<u32>()?;
    anyhow::ensure!(
        session["client_reference_id"] == format!("{account}:{character}"),
        "Checkout recipient mismatch"
    );
    Ok(Some(Grant {
        session: id.to_owned(),
        account,
        character,
        gold: package.gold,
    }))
}

#[derive(Deserialize)]
struct Checkout {
    email: String,
    password: String,
    character: String,
    package: String,
}

struct Config {
    data: PathBuf,
    stripe_key: String,
    webhook_secret: String,
    grant_secret: String,
    port: u16,
    grant_port: u16,
}

impl Config {
    fn load() -> Result<Self> {
        let stripe_key = std::env::var("ZIRCON_STRIPE_SECRET_KEY")?;
        anyhow::ensure!(
            stripe_key.starts_with("sk_test_"),
            "Only Stripe test keys are accepted"
        );
        let webhook_secret = std::env::var("ZIRCON_STRIPE_WEBHOOK_SECRET")?;
        anyhow::ensure!(
            webhook_secret.starts_with("whsec_"),
            "Invalid Stripe webhook secret"
        );
        let grant_secret = std::env::var("ZIRCON_STRIPE_GRANT_SECRET")?;
        anyhow::ensure!(
            grant_secret.len() >= 32,
            "ZIRCON_STRIPE_GRANT_SECRET must contain at least 32 characters"
        );
        Ok(Self {
            data: std::env::var_os("ZIRCON_DATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("data")),
            stripe_key,
            webhook_secret,
            grant_secret,
            port: std::env::var("ZIRCON_STRIPE_PORT")
                .unwrap_or_else(|_| "8787".into())
                .parse()?,
            grant_port: std::env::var("ZIRCON_STRIPE_GRANT_PORT")
                .unwrap_or_else(|_| "8788".into())
                .parse()?,
        })
    }
}

fn authenticate(config: &Config, checkout: &Checkout) -> Result<(u32, u32)> {
    let mut accounts = Accounts::load(config.data.join("accounts.json"))?;
    let account = accounts
        .login(&checkout.email, &checkout.password)
        .map_err(|_| anyhow!("Invalid account credentials"))?;
    let character = accounts
        .account(account)
        .and_then(|account| {
            account.characters.iter().find(|character| {
                !character.deleted && character.name.eq_ignore_ascii_case(&checkout.character)
            })
        })
        .map(|character| character.id)
        .context("Character not found on account")?;
    Ok((account, character))
}

fn create_session(config: &Config, checkout: &Checkout) -> Result<Value> {
    let package = package(&checkout.package).context("Unknown package")?;
    let (account, character) = authenticate(config, checkout)?;
    let base = format!("http://127.0.0.1:{}", config.port);
    let success = format!("{base}/success");
    let reference = format!("{account}:{character}");
    let cents = package.cents.to_string();
    let account_id = account.to_string();
    let character_id = character.to_string();
    let form = [
        ("mode", "payment"),
        ("payment_method_types[0]", "card"),
        ("line_items[0][price_data][currency]", "eur"),
        ("line_items[0][price_data][unit_amount]", cents.as_str()),
        (
            "line_items[0][price_data][product_data][name]",
            "ZirconRust Game Gold",
        ),
        ("line_items[0][quantity]", "1"),
        ("metadata[package]", package.id),
        ("metadata[account_id]", account_id.as_str()),
        ("metadata[character_id]", character_id.as_str()),
        ("client_reference_id", reference.as_str()),
        ("success_url", success.as_str()),
        ("cancel_url", base.as_str()),
    ];
    let auth = format!("Bearer {}", config.stripe_key);
    let response = ureq::post("https://api.stripe.com/v1/checkout/sessions")
        .set("Authorization", &auth)
        .timeout(Duration::from_secs(15))
        .send_form(&form)
        .context("Stripe Checkout request failed")?;
    let session: Value = response.into_json()?;
    let id = session["id"]
        .as_str()
        .context("Stripe returned no session ID")?;
    let url = session["url"]
        .as_str()
        .context("Stripe returned no Checkout URL")?;
    anyhow::ensure!(id.starts_with("cs_test_") && url.starts_with("https://checkout.stripe.com/"));
    Ok(json!({"url":url}))
}

fn send_grant(config: &Config, grant: &Grant) -> Result<()> {
    let body = serde_json::to_vec(grant)?;
    let signature = mac(&config.grant_secret, &body)?;
    let url = format!("http://127.0.0.1:{}/stripe/grant", config.grant_port);
    ureq::post(&url)
        .set("Content-Type", "application/json")
        .set("X-Zircon-Signature", &signature)
        .timeout(Duration::from_secs(5))
        .send_bytes(&body)
        .context("Game server could not confirm grant")?;
    Ok(())
}

fn process_webhook(
    secret: &str,
    signature: &str,
    body: &[u8],
    clock: i64,
    deliver: impl FnOnce(&Grant) -> Result<()>,
) -> Result<()> {
    verify_stripe(secret, signature, body, clock)?;
    let event: Value = serde_json::from_slice(body)?;
    if let Some(grant) = paid_grant(&event)? {
        deliver(&grant)?;
    }
    Ok(())
}

fn handle(
    config: &Config,
    method: &Method,
    path: &str,
    signature: Option<&str>,
    body: &[u8],
) -> (u16, String) {
    let result: Result<Value> = (|| match (method, path) {
        (Method::Post, "/checkout") => {
            let checkout: Checkout = serde_json::from_slice(body)?;
            create_session(config, &checkout)
        }
        (Method::Post, "/stripe/webhook") => {
            process_webhook(
                &config.webhook_secret,
                signature.context("Missing Stripe signature")?,
                body,
                now(),
                |grant| send_grant(config, grant),
            )?;
            Ok(json!({"received":true}))
        }
        _ => bail!("Unknown route"),
    })();
    match result {
        Ok(value) => (200, value.to_string()),
        Err(error) => {
            let server_error = error
                .to_string()
                .contains("Game server could not confirm grant")
                || error.to_string().contains("Stripe Checkout request failed");
            tracing::warn!(reason = %error, "Stripe test request failed");
            (
                if server_error { 503 } else { 400 },
                json!({"error":"Request failed"}).to_string(),
            )
        }
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let config = Config::load()?;
    let server = Server::http(("127.0.0.1", config.port)).map_err(|e| anyhow!("{e}"))?;
    tracing::info!(
        port = config.port,
        "Stripe test checkout listening on loopback"
    );
    for mut request in server.incoming_requests() {
        if request.method() == &Method::Get && matches!(request.url(), "/" | "/success") {
            let (body, content_type) = if request.url() == "/" {
                (SHOP_PAGE, "text/html; charset=utf-8")
            } else {
                (
                    "Payment submitted. Check your Game Gold balance in ZirconRust.",
                    "text/plain; charset=utf-8",
                )
            };
            request.respond(Response::from_string(body).with_header(
                Header::from_bytes("Content-Type", content_type).expect("static header"),
            ))?;
            continue;
        }
        let mut body = Vec::new();
        let read = request
            .as_reader()
            .take(MAX_BODY + 1)
            .read_to_end(&mut body);
        let body = match read {
            Ok(_) if body.len() as u64 <= MAX_BODY => body,
            _ => {
                let _ = request.respond(Response::empty(StatusCode(413)));
                continue;
            }
        };
        let signature = request
            .headers()
            .iter()
            .find(|header| header.field.equiv("Stripe-Signature"))
            .map(|header| header.value.as_str());
        let (status, text) = handle(&config, request.method(), request.url(), signature, &body);
        let response = Response::from_string(text)
            .with_status_code(StatusCode(status))
            .with_header(
                Header::from_bytes("Content-Type", "application/json").expect("static header"),
            );
        request.respond(response)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(status: &str, amount: i64) -> Value {
        json!({"object":"event","livemode":false,"type":"checkout.session.completed","data":{"object":{
            "object":"checkout.session","id":"cs_test_123","livemode":false,"mode":"payment",
            "status":"complete","payment_status":status,"currency":"eur","amount_total":amount,
            "metadata":{"package":"100","account_id":"1","character_id":"2"},
            "client_reference_id":"1:2"
        }}})
    }

    #[test]
    fn signed_raw_body_rejects_tampering_and_old_events() {
        let body = br#"{"type":"checkout.session.completed"}"#;
        let signed = [b"1000.".as_slice(), body].concat();
        let header = format!("t=1000,v1={}", mac("whsec_test", &signed).unwrap());
        verify_stripe("whsec_test", &header, body, 1000).unwrap();
        assert!(verify_stripe("whsec_test", &header, b"{}", 1000).is_err());
        assert!(verify_stripe("whsec_test", &header, body, 1301).is_err());
    }

    #[test]
    fn paid_event_requires_test_mode_fixed_price_and_recipient() {
        assert!(paid_grant(&event("unpaid", 100)).unwrap().is_none());
        assert!(paid_grant(&event("paid", 99)).is_err());
        let mut wrong = event("paid", 100);
        wrong["data"]["object"]["livemode"] = json!(true);
        assert!(paid_grant(&wrong).is_err());
        let grant = paid_grant(&event("paid", 100)).unwrap().unwrap();
        assert_eq!((grant.account, grant.character, grant.gold), (1, 2, 100));
    }

    #[test]
    fn fake_signed_webhook_delivers_paid_grant_only() {
        let body = serde_json::to_vec(&event("paid", 100)).unwrap();
        let signed = [b"1000.".as_slice(), body.as_slice()].concat();
        let header = format!("t=1000,v1={}", mac("whsec_test", &signed).unwrap());
        let mut delivered = 0;
        process_webhook("whsec_test", &header, &body, 1000, |grant| {
            assert_eq!(grant.session, "cs_test_123");
            delivered += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(delivered, 1);
        assert!(process_webhook("whsec_test", &header, b"{}", 1000, |_| {
            delivered += 1;
            Ok(())
        })
        .is_err());
        assert_eq!(delivered, 1);
    }
}
