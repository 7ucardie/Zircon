//! Packet ID registry.
//!
//! The C# server sorts packet types to assign stable integer IDs.
//! We reproduce the same ordering here as a `const` array so IDs match
//! without reflection.
//!
//! Ordering rules (from Packet.cs):
//!   1. `Library.Network.GeneralPackets` namespace sorts to position 0 (all
//!      names within it sort alphabetically).
//!   2. All remaining packets sort alphabetically by name only (ignoring
//!      namespace).

/// Stable numeric identifier for a packet type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PacketId(pub u16);

/// All known packet names in their canonical sorted order.
///
/// The index into this array is the wire `PacketId`.
///
/// GeneralPackets (sorted by name) come first, then ClientPackets and
/// ServerPackets interleaved alphabetically by name.
pub static PACKET_NAMES: &[&str] = &[
    // ── GeneralPackets (sorted first by namespace) ────────────────────────
    "CheckVersion",               // 0
    "Connected",                  // 1
    "Disconnect",                 // 2
    "GoodVersion",                // 3
    "Ping",                       // 4
    "PingResponse",               // 5
    "Version",                    // 6
    // ── ClientPackets + ServerPackets merged alphabetically ───────────────
    // Within the same name, ClientPackets < ServerPackets.
    "Activation",                 // 7  C
    "Activation",                 // 8  S
    "Attack",                     // 9  C
    "AutoPotionLinkChanged",      // 10 C
    "BeltLinkChanged",            // 11 C
    "BlockAdd",                   // 12 C
    "BlockAdd",                   // 13 S
    "BlockRemove",                // 14 C
    "BlockRemove",                // 15 S
    "BuffAdd",                    // 16 S
    "BuffChanged",                // 17 S
    "BuffPaused",                 // 18 S
    "BuffRemove",                 // 19 S
    "BuffTime",                   // 20 S
    "BundleClose",                // 21 S
    "BundleConfirm",              // 22 C
    "BundleOpen",                 // 23 C
    "BundleOpen",                 // 24 S
    "CaptionChange",              // 25 C
    "ChangeAttackMode",           // 26 C
    "ChangeAttackMode",           // 27 S
    "ChangeOnlineState",          // 28 C
    "ChangePetMode",              // 29 C
    "ChangePetMode",              // 30 S
    "ChangePassword",             // 31 C
    "ChangePassword",             // 32 S
    "Chat",                       // 33 C
    "Chat",                       // 34 S
    "CombatTime",                 // 35 S
    "CompanionAdopt",             // 36 C
    "CompanionAdopt",             // 37 S
    "CompanionItemsGained",       // 38 S
    "CompanionRelease",           // 39 C
    "CompanionRelease",           // 40 S
    "CompanionRetrieve",          // 41 C
    "CompanionRetrieve",          // 42 S
    "CompanionShapeUpdate",       // 43 S
    "CompanionSkillUpdate",       // 44 S
    "CompanionStore",             // 45 C
    "CompanionStore",             // 46 S
    "CompanionUnlock",            // 47 C
    "CompanionUnlock",            // 48 S
    "CompanionUpdate",            // 49 S
    "CompanionWeightUpdate",      // 50 S
    "CurrencyChanged",            // 51 S
    "CurrencyDrop",               // 52 C
    "DataObjectHealthMana",       // 53 S
    "DataObjectItem",             // 54 S
    "DataObjectLocation",         // 55 S
    "DataObjectMaxHealthMana",    // 56 S
    "DataObjectMonster",          // 57 S
    "DataObjectPlayer",           // 58 S
    "DataObjectRemove",           // 59 S
    "DayChanged",                 // 60 S
    "DeleteCharacter",            // 61 C
    "DeleteCharacter",            // 62 S
    "DisciplineExperienceChanged",// 63 S
    "DisciplineUpdate",           // 64 S
    "FishingCast",                // 65 C
    "FishingStats",               // 66 S
    "FocusChanged",               // 67 S
    "FortuneCheck",               // 68 C
    "FortuneUpdate",              // 69 S
    "FriendAdd",                  // 70 C
    "FriendAdd",                  // 71 S
    "FriendRemove",               // 72 C
    "FriendRemove",               // 73 S
    "FriendUpdate",               // 74 S
    "GainedExperience",           // 75 S
    "GameGoldRecharge",           // 76 C
    "GameLogout",                 // 77 S
    "GenderChange",               // 78 C
    "GroupInvite",                // 79 C
    "GroupInvite",                // 80 S
    "GroupLFG",                   // 81 S
    "GroupLFGUpdate",             // 82 C
    "GroupMember",                // 83 S
    "GroupNotify",                // 84 C
    "GroupRemove",                // 85 C
    "GroupRemove",                // 86 S
    "GroupRequest",               // 87 C
    "GroupRequest",               // 88 S
    "GroupResponse",              // 89 C
    "GroupSwitch",                // 90 C
    "GroupSwitch",                // 91 S
    "GroupUpdate",                // 92 S
    "GuildCastleInfo",            // 93 S
    "GuildChanged",               // 94 S
    "GuildColour",                // 95 C
    "GuildConquestDate",          // 96 S
    "GuildConquestFinished",      // 97 S
    "GuildConquestStarted",       // 98 S
    "GuildCreate",                // 99  C
    "GuildCreate",                // 100 S
    "GuildDayReset",              // 101 S
    "GuildEditMember",            // 102 C
    "GuildEditNotice",            // 103 C
    "GuildFlag",                  // 104 C
    "GuildFundsChanged",          // 105 S
    "GuildGetItem",               // 106 S
    "GuildIncreaseMember",        // 107 C
    "GuildIncreaseMember",        // 108 S
    "GuildIncreaseStorage",       // 109 C
    "GuildIncreaseStorage",       // 110 S
    "GuildInfo",                  // 111 S
    "GuildInvite",                // 112 S
    "GuildInviteMember",          // 113 C
    "GuildInviteMember",          // 114 S
    "GuildKick",                  // 115 S
    "GuildKickMember",            // 116 C
    "GuildMemberContribution",    // 117 S
    "GuildMemberOffline",         // 118 S
    "GuildMemberOnline",          // 119 S
    "GuildNewItem",               // 120 S
    "GuildNoticeChanged",         // 121 S
    "GuildRepairCastleGates",     // 122 C
    "GuildRepairCastleGuards",    // 123 C
    "GuildRequestConquest",       // 124 C
    "GuildResponse",              // 125 C
    "GuildStats",                 // 126 S
    "GuildTax",                   // 127 C
    "GuildTax",                   // 128 S
    "GuildToggleCastleGates",     // 129 C
    "GuildUpdate",                // 130 S
    "GuildWar",                   // 131 C
    "GuildWar",                   // 132 S
    "GuildWarFinished",           // 133 S
    "GuildWarStarted",            // 134 S
    "HairChange",                 // 135 C
    "Harvest",                    // 136 C
    "HealthChanged",              // 137 S
    "HelmetToggle",               // 138 C
    "HelmetToggle",               // 139 S
    "Hermit",                     // 140 C
    "IncreaseDiscipline",         // 141 C
    "InformMaxExperience",        // 142 S
    "Inspect",                    // 143 C
    "Inspect",                    // 144 S
    "ItemAcessoryRefined",        // 145 S
    "ItemChanged",                // 146 S
    "ItemDelete",                 // 147 C
    "ItemDelete",                 // 148 S
    "ItemDrop",                   // 149 C
    "ItemDurability",             // 150 S
    "ItemExperience",             // 151 S
    "ItemLock",                   // 152 C
    "ItemLock",                   // 153 S
    "ItemMove",                   // 154 C
    "ItemMove",                   // 155 S
    "ItemSort",                   // 156 C
    "ItemSort",                   // 157 S
    "ItemSplit",                  // 158 C
    "ItemSplit",                  // 159 S
    "ItemStatsChanged",           // 160 S
    "ItemStatsRefreshed",         // 161 S
    "ItemUse",                    // 162 C
    "ItemUseDelay",               // 163 S
    "ItemsChanged",               // 164 S
    "ItemsGained",                // 165 S
    "JoinInstance",               // 166 C
    "JoinInstance",               // 167 S
    "JoinStarterGuild",           // 168 C
    "LevelChanged",               // 169 S
    "Login",                      // 170 C
    "Login",                      // 171 S
    "Logout",                     // 172 C
    "LootBoxClose",               // 173 S
    "LootBoxConfirmSelection",    // 174 C
    "LootBoxOpen",                // 175 C
    "LootBoxOpen",                // 176 S
    "LootBoxReroll",              // 177 C
    "LootBoxReveal",              // 178 C
    "LootBoxTakeItems",           // 179 C
    "Magic",                      // 180 C
    "MagicCooldown",              // 181 S
    "MagicKey",                   // 182 C
    "MagicLeveled",               // 183 S
    "MagicToggle",                // 184 C
    "MagicToggle",                // 185 S
    "MailDelete",                 // 186 C
    "MailDelete",                 // 187 S
    "MailGetItem",                // 188 C
    "MailItemDelete",             // 189 S
    "MailList",                   // 190 S
    "MailNew",                    // 191 S
    "MailOpened",                 // 192 C
    "MailSend",                   // 193 C
    "MailSend",                   // 194 S
    "ManaChanged",                // 195 S
    "MapChanged",                 // 196 S
    "MapEffect",                  // 197 S
    "MarketPlaceBuy",             // 198 C
    "MarketPlaceBuy",             // 199 S
    "MarketPlaceCancelConsign",   // 200 C
    "MarketPlaceConsign",         // 201 C
    "MarketPlaceConsign",         // 202 S
    "MarketPlaceConsignChanged",  // 203 S
    "MarketPlaceHistory",         // 204 C
    "MarketPlaceHistory",         // 205 S
    "MarketPlaceSearch",          // 206 C
    "MarketPlaceSearch",          // 207 S
    "MarketPlaceSearchCount",     // 208 S
    "MarketPlaceSearchIndex",     // 209 C
    "MarketPlaceSearchIndex",     // 210 S
    "MarketPlaceStoreBuy",        // 211 C
    "MarketPlaceStoreBuy",        // 212 S
    "MarriageInfo",               // 213 S
    "MarriageInvite",             // 214 S
    "MarriageMakeRing",           // 215 C
    "MarriageMakeRing",           // 216 S
    "MarriageOnlineChanged",      // 217 S
    "MarriageRemoveRing",         // 218 S
    "MarriageResponse",           // 219 C
    "MarriageTeleport",           // 220 C
    "MilestoneActive",            // 221 C
    "MilestoneClaim",             // 222 C
    "MilestoneEarned",            // 223 S
    "MilestoneNotify",            // 224 C
    "Mining",                     // 225 C
    "Mount",                      // 226 C
    "MountFailed",                // 227 S
    "Move",                       // 228 C
    "NameChange",                 // 229 C
    "NewAccount",                 // 230 C
    "NewAccount",                 // 231 S
    "NewCharacter",               // 232 C
    "NewCharacter",               // 233 S
    "NewMagic",                   // 234 S
    "NPCAccessoryLevelUp",        // 235 C
    "NPCAccessoryLevelUp",        // 236 S
    "NPCAccessoryRefine",         // 237 C
    "NPCAccessoryRefine",         // 238 S
    "NPCAccessoryReset",          // 239 C
    "NPCAccessoryUpgrade",        // 240 C
    "NPCButton",                  // 241 C
    "NPCBuy",                     // 242 C
    "NPCCall",                    // 243 C
    "NPCClose",                   // 244 C
    "NPCClose",                   // 245 S
    "NPCFragment",                // 246 C
    "NPCMasterRefine",            // 247 C
    "NPCMasterRefine",            // 248 S
    "NPCMasterRefineEvaluate",    // 249 C
    "NPCRefine",                  // 250 C
    "NPCRefine",                  // 251 S
    "NPCRefinementStone",         // 252 C
    "NPCRefinementStone",         // 253 S
    "NPCRefineRetrieve",          // 254 C
    "NPCRefineRetrieve",          // 255 S
    "NPCRepair",                  // 256 C
    "NPCRepair",                  // 257 S
    "NPCResponse",                // 258 S
    "NPCRoll",                    // 259 C
    "NPCRoll",                    // 260 S
    "NPCRollResult",              // 261 C
    "NPCSell",                    // 262 C
    "NPCWeaponCraft",             // 263 C
    "NPCWeaponCraft",             // 264 S
    "ObjectAttack",               // 265 S
    "ObjectBuffAdd",              // 266 S
    "ObjectBuffRemove",           // 267 S
    "ObjectDash",                 // 268 S
    "ObjectDied",                 // 269 S
    "ObjectEffect",               // 270 S
    "ObjectFishing",              // 271 S
    "ObjectHarvest",              // 272 S
    "ObjectHarvested",            // 273 S
    "ObjectHide",                 // 274 S
    "ObjectIdle",                 // 275 S
    "ObjectItem",                 // 276 S
    "ObjectLeveled",              // 277 S
    "ObjectMagic",                // 278 S
    "ObjectMining",               // 279 S
    "ObjectMonster",              // 280 S
    "ObjectMount",                // 281 S
    "ObjectNameColour",           // 282 S
    "ObjectNPC",                  // 283 S
    "ObjectPetOwnerChanged",      // 284 S
    "ObjectPlayer",               // 285 S
    "ObjectPoison",               // 286 S
    "ObjectProjectile",           // 287 S
    "ObjectPushed",               // 288 S
    "ObjectRangeAttack",          // 289 S
    "ObjectRemove",               // 290 S
    "ObjectRevive",               // 291 S
    "ObjectShow",                 // 292 S
    "ObjectSpell",                // 293 S
    "ObjectSpellChanged",         // 294 S
    "ObjectStats",                // 295 S
    "ObjectStruck",               // 296 S
    "ObjectTurn",                 // 297 S
    "ObservableSwitch",           // 298 C
    "ObservableSwitch",           // 299 S
    "ObserverRequest",            // 300 C
    "PickUp",                     // 301 C
    "PlayerChangeUpdate",         // 302 S
    "PlayerUpdate",               // 303 S
    "QuestAbandon",               // 304 C
    "QuestAccept",                // 305 C
    "QuestCancelled",             // 306 S
    "QuestChanged",               // 307 S
    "QuestComplete",              // 308 C
    "QuestTrack",                 // 309 C
    "RangeAttack",                // 310 C
    "Rankings",                   // 311 S
    "RankRequest",                // 312 C
    "RankSearch",                 // 313 C
    "RankSearch",                 // 314 S
    "RefineList",                 // 315 S
    "RequestActivationKey",       // 316 C
    "RequestActivationKey",       // 317 S
    "RequestPasswordReset",       // 318 C
    "RequestPasswordReset",       // 319 S
    "ResetPassword",              // 320 C
    "ResetPassword",              // 321 S
    "ReviveTimers",               // 322 S
    "SafeZoneChanged",            // 323 S
    "SelectLanguage",             // 324 C
    "SelectLogout",               // 325 S
    "SendCompanionFilters",       // 326 C
    "SendCompanionFilters",       // 327 S
    "SetTimer",                   // 328 S
    "StartGame",                  // 329 C
    "StartGame",                  // 330 S
    "StartObserver",              // 331 S
    "StatsUpdate",                // 332 S
    "StorageSize",                // 333 S
    "TeleportRing",               // 334 C
    "TimeOfDayChanged",           // 335 S
    "TownRevive",                 // 336 C
    "TradeAddGold",               // 337 C
    "TradeAddGold",               // 338 S
    "TradeAddItem",               // 339 C
    "TradeAddItem",               // 340 S
    "TradeClose",                 // 341 C
    "TradeClose",                 // 342 S
    "TradeConfirm",               // 343 C
    "TradeGoldAdded",             // 344 S
    "TradeItemAdded",             // 345 S
    "TradeOpen",                  // 346 S
    "TradeRequest",               // 347 C
    "TradeRequest",               // 348 S
    "TradeRequestResponse",       // 349 C
    "TradeUnlock",                // 350 S
    "Turn",                       // 351 C
    "UserLocation",               // 352 S
    "UserMilestones",             // 353 S
    "WeightUpdate",               // 354 S
];

/// Look up the `PacketId` for a given packet name.
///
/// Returns `None` if the name is not registered.
pub fn id_by_name(name: &str) -> Option<PacketId> {
    PACKET_NAMES
        .iter()
        .position(|&n| n == name)
        .map(|i| PacketId(i as u16))
}

/// A trait implemented by every packet type.
pub trait PacketCodec: Sized {
    /// The stable wire ID for this packet type.
    fn packet_id() -> PacketId;

    /// Decode the packet payload (after the 6-byte frame header has been
    /// stripped by `RawFrame::parse`).
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::ProtocolError>;

    /// Encode the packet fields into a complete framed byte sequence
    /// (including the 4-byte length prefix and 2-byte packet ID).
    fn encode(&self) -> bytes::Bytes;
}
