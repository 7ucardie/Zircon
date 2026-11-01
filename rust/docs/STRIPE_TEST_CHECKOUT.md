# Stripe Game Gold checkout (test mode)

This is a local test integration. It accepts only `sk_test_` keys and
`cs_test_` Checkout Sessions. It never charges real money. Stripe hosts card
entry; ZirconRust never receives card details.

Test packages are fixed in `mir-stripe-checkout.rs`: 100 Game Gold for €1.00
and 500 Game Gold for €4.50. These are test prices, not approved live prices.
Checkout accepts a package ID, not a client-supplied amount or gold quantity.

## Run locally

1. From `rust/`, build both binaries: `cargo build -p mir-server --bins`.
2. Generate a shared local grant secret with `openssl rand -hex 32`. Set the
   same `ZIRCON_STRIPE_GRANT_SECRET` in both server and checkout processes.
   Keep it out of the repository and terminal command history.
3. Set `ZIRCON_ASSETS`, `ZIRCON_DATA`, and
   `ZIRCON_STRIPE_GRANT_SECRET`; start `cargo run -p mir-server --bin
   mir-server`. It binds its grant endpoint to `127.0.0.1:8788` only.
4. Set `ZIRCON_STRIPE_SECRET_KEY=sk_test_…`,
   `ZIRCON_STRIPE_WEBHOOK_SECRET=whsec_…`, and the same data directory and
   grant secret; start `cargo run -p mir-server --bin mir-stripe-checkout`.
   It binds to `127.0.0.1:8787` only.
5. Start [Stripe CLI forwarding](https://docs.stripe.com/stripe-cli/use-cli):
   `stripe listen --forward-to 127.0.0.1:8787/stripe/webhook`. Use the
   `whsec_` secret printed by this CLI session for the checkout process.
6. Open `http://127.0.0.1:8787/`, enter an existing game account and
   character, and use a [Stripe test card](https://docs.stripe.com/testing#cards).
   Check Game Gold in game after the paid webhook arrives.

The web page takes the game password only on loopback. Do not publish this
HTTP service. A public checkout needs HTTPS and a separate web authentication
flow before live use.

## Grant rules

The sidecar reads `accounts.json` to authenticate but never writes it. Stripe
receives account and character IDs as Checkout metadata. The sidecar verifies
the raw webhook body using Stripe's timestamped HMAC signature, then requires
`livemode=false`, `payment_status=paid`, `cs_test_`, fixed EUR price, package,
and matching account/character metadata. Other events do not grant currency.
It sends an HMAC-signed request to the local game server.
Any local process that obtains the shared grant secret can request test Game
Gold credits. Protect that secret like a GM credential and rotate it if exposed.

The game server snapshots online characters, checks the recipient and package
amount, and records the Checkout Session ID beside the new Game Gold balance.
It syncs and atomically renames `accounts.json` before acknowledging the
grant. Repeated events for that Session ID do not credit twice. If the game
server is unavailable, the webhook returns HTTP 503 so Stripe can retry.
Never edit `accounts.json` from the sidecar.

Tests: `cargo test -p mir-server stripe_` and
`cargo test -p mir-server --bin mir-stripe-checkout`. They cover forged/stale
webhooks, paid price/recipient checks, local grant authentication, and replay
after an account reload. Tests use fake events and require no Stripe account.

Live checkout still needs merchant approval, final prices, tax/refund and
chargeback handling, HTTPS, a public authentication design, and operational
monitoring. Stripe's [restricted businesses policy](https://stripe.com/legal/restricted-businesses)
has a specific exception for in-game currency sold by the virtual world's
operator. Confirm eligibility with Stripe before enabling real charges.

Stripe references:
[Checkout Sessions](https://docs.stripe.com/api/checkout/sessions/create),
[webhook signatures](https://docs.stripe.com/webhooks/signature),
[fulfillment](https://docs.stripe.com/checkout/fulfillment.md?payment-ui=stripe-hosted).
