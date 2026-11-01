# OVHcloud VPS deployment

The Rust server is one authoritative process. It saves accounts, characters and
other state under `ZIRCON_DATA` as local files. Run **one active server**. Three
concurrent copies would diverge and overwrite player progress. The current game
protocol is plain TCP and sends login passwords without transport encryption;
limit access to trusted testers or a VPN, and do not reuse real passwords until
encrypted transport is implemented.

## One host

Use a Linux VPS with Docker Engine, the Compose plugin, and enough disk for the
maps, database and saved data. Allow TCP 7000 to players and SSH only to admins.
Put this repository's `rust/` directory at `/srv/zircon/rust`, and copy the
required original assets to:

```
/srv/zircon/assets/Client/Map/*.map
/srv/zircon/assets/Database/System.db
```

The server does not need the client sprite and sound libraries. As the SSH user
who owns `/srv/zircon`, run. On a new VPS, first create that directory and
give the SSH user ownership; copy `rust/`, `Client/Map/` and `System.db` into
the paths above (for example, with `rsync -a`, excluding `rust/target` and
`rust/data`). Then run:

```sh
/srv/zircon/rust/deploy/ovh/node.sh up
/srv/zircon/rust/deploy/ovh/node.sh ps
/srv/zircon/rust/deploy/ovh/node.sh logs -f
```

The script builds the server container and binds TCP 7000. It stores saves in
`/srv/zircon/data`, which must be backed up off the VPS. Set
`ZIRCON_ASSETS_DIR` and `ZIRCON_DATA_DIR` to override those paths. Point the
client at the VPS address with `ZIRCON_SERVER=address:7000`.

## Three hosts: active server and two standby copies

Put the same `rust/` directory, Docker installation and static assets on all
three VPSs. Use SSH keys for the admin machine to reach each host; `rsync` must
be installed on the admin machine and VPSs. Keep the standby containers down.
Start only the primary with `node.sh up`. Place all three VPSs in an eligible
OVHcloud Additional IP move area, configure that IP on each host according to
OVHcloud's OS guide, and give players the Additional IP as their server address.

From an admin machine, create a consistent checkpoint:

```sh
rust/deploy/ovh/checkpoint.sh user@primary user@standby1 user@standby2
```

This briefly stops the primary, copies `/srv/zircon/data` through the admin
machine, restarts the primary, then installs a dated snapshot on each standby.
Run it regularly from a trusted admin machine or scheduler. Its exit code is
nonzero if either replica failed; keep the previous snapshot until a complete
new one exists. Saves since the last successful checkpoint can be lost if the
primary becomes inaccessible. Also retain off-site backups; two copies at one
provider are not independent disaster recovery.

To recover from a failed primary:

1. **Power off or otherwise fence the old primary in OVHcloud first.** A failed
   network check alone does not prove it stopped writing saves.
2. On the chosen standby, run
   `FENCED_OLD_PRIMARY=yes /srv/zircon/rust/deploy/ovh/node.sh restore-latest`.
   It archives any existing local data, restores the last complete checkpoint,
   and starts the server.
3. Move the Additional IP to that VPS in the OVHcloud Control Panel and verify
   that a client can connect. Rebuild the other standby from the new primary
   before resuming checkpoints. Do not restart the old primary with stale data.

This is **manual failover with a nonzero recovery point**, not automatic HA.
Automatic, near-zero-loss failover needs a shared/replicated transactional
state store, leader fencing, and a tested failover controller. The current JSON
save architecture has none of these. OVHcloud documents [moving an Additional
IP](https://docs.ovhcloud.com/en/guides/bare-metal-cloud/dedicated-servers/move-failover-ip)
and [VPS IP alias setup](https://docs.ovhcloud.com/en/guides/bare-metal-cloud/virtual-private-servers/configuring-ip-aliasing);
check current regional eligibility before ordering.
