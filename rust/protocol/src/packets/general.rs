use super::packet;
use crate::enums::DisconnectReason;

packet!(id = 0; CheckVersion);
packet!(id = 1; Connected);
packet!(id = 2; Disconnect { reason: DisconnectReason });
packet!(id = 3; GoodVersion { database_key: Vec<u8> });
packet!(id = 4; Ping);
packet!(id = 5; PingResponse { ping: i32 });
packet!(id = 6; Version { client_hash: Vec<u8> });
