use super::packet;
use crate::{
    enums::*,
    shared::{
        CellLinkInfo, ClientBlockInfo, ClientBuffInfo, ClientBundleItemInfo,
        ClientCompanionObject, ClientFortuneInfo, ClientFriendInfo, ClientGuildInfo,
        ClientGuildMemberInfo, ClientLookingForGroup, ClientLootBoxItemInfo,
        ClientMailInfo, ClientMarketPlaceInfo, ClientNpcValues, ClientPlayerInfo,
        ClientRefineInfo, ClientUserCompanion, ClientUserDiscipline, ClientUserItem,
        ClientUserMilestone, ClientUserQuest, RankInfo, SelectInfo, StartInformation,
    },
    wire::{ClassList, IntDict, NetDecimal, PrimList, Stats},
};

packet!(id = 8;   Activation      { result: ActivationResult });
packet!(id = 13;  BlockAdd        { info: ClientBlockInfo });
packet!(id = 15;  BlockRemove     { index: i32 });
packet!(id = 16;  BuffAdd         { buff: ClientBuffInfo });
packet!(id = 17;  BuffChanged     { index: i32, stats: Option<Stats> });
packet!(id = 18;  BuffPaused      { index: i32, paused: bool });
packet!(id = 19;  BuffRemove      { index: i32 });
packet!(id = 20;  BuffTime        { index: i32, time: i64 });
packet!(id = 21;  BundleClose);

// BundleOpen has List<ClientBundleItemInfo>
#[derive(Debug, Clone, Default)]
pub struct BundleOpen {
    pub slot: i32,
    pub items: ClassList<ClientBundleItemInfo>,
}
impl crate::codec::PacketCodec for BundleOpen {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(24) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self { slot: i32::wire_read(&mut buf)?, items: ClassList::wire_read(&mut buf)? })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.slot.wire_write(&mut buf);
        self.items.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 27;  ChangeAttackMode { mode: AttackMode });
packet!(id = 30;  ChangePetMode    { mode: PetMode });
packet!(id = 32;  ChangePassword   { result: ChangePasswordResult, message: String, duration: i64 });
packet!(id = 34;  Chat             { object_id: u32, text: String, r#type: MessageType, linked_items: ClassList<ClientUserItem>, overhead_only: bool });
packet!(id = 35;  CombatTime);
packet!(id = 37;  CompanionAdopt   { user_companion: Option<ClientUserCompanion> });
packet!(id = 38;  CompanionItemsGained { items: ClassList<ClientUserItem> });
packet!(id = 40;  CompanionRelease { index: i32 });
packet!(id = 42;  CompanionRetrieve { index: i32 });
packet!(id = 43;  CompanionShapeUpdate { object_id: u32, head_shape: i32, back_shape: i32 });
packet!(id = 44;  CompanionSkillUpdate {
    level3: Option<Stats>, level5: Option<Stats>, level7: Option<Stats>,
    level10: Option<Stats>, level11: Option<Stats>, level13: Option<Stats>, level15: Option<Stats>
});
packet!(id = 46;  CompanionStore);
packet!(id = 48;  CompanionUnlock  { index: i32 });
packet!(id = 49;  CompanionUpdate  { level: i32, experience: i32, hunger: i32 });
packet!(id = 50;  CompanionWeightUpdate { bag_weight: i32, max_bag_weight: i32, inventory_size: i32 });
packet!(id = 51;  CurrencyChanged  { currency_index: i32, amount: i64 });
packet!(id = 53;  DataObjectHealthMana { object_id: u32, health: i32, mana: i32, dead: bool });
packet!(id = 54;  DataObjectItem   { object_id: u32, map_index: i32, current_location: (i32, i32), item_index: i32 });
packet!(id = 55;  DataObjectLocation { object_id: u32, map_index: i32, current_location: (i32, i32) });
packet!(id = 56;  DataObjectMaxHealthMana { object_id: u32, max_health: i32, max_mana: i32, stats: Option<Stats> });
packet!(id = 57;  DataObjectMonster {
    object_id: u32, map_index: i32, current_location: (i32, i32),
    monster_index: i32, pet_owner: String, health: i32, stats: Option<Stats>, dead: bool
});
packet!(id = 58;  DataObjectPlayer {
    object_id: u32, map_index: i32, current_location: (i32, i32),
    name: String, health: i32, mana: i32, dead: bool, max_health: i32, max_mana: i32
});
packet!(id = 59;  DataObjectRemove { object_id: u32 });
packet!(id = 60;  DayChanged       { day_time: f32 });
packet!(id = 62;  DeleteCharacter  { result: DeleteCharacterResult, deleted_index: i32 });
packet!(id = 63;  DisciplineExperienceChanged { experience: i64 });
packet!(id = 64;  DisciplineUpdate { discipline: Option<ClientUserDiscipline> });
packet!(id = 66;  FishingStats     {
    can_auto_cast: bool, current_points: i32,
    throw_quality: i32, required_points: i32, movement_speed: i32, required_accuracy: i32
});
packet!(id = 67;  FocusChanged     { object_id: u32, change: i32 });
packet!(id = 69;  FortuneUpdate    { fortunes: ClassList<ClientFortuneInfo> });
packet!(id = 71;  FriendAdd        { info: ClientFriendInfo });
packet!(id = 73;  FriendRemove     { index: i32 });
packet!(id = 74;  FriendUpdate     { info: ClientFriendInfo });
packet!(id = 75;  GainedExperience { amount: NetDecimal });
packet!(id = 77;  GameLogout       { characters: ClassList<SelectInfo> });
packet!(id = 80;  GroupInvite      { name: String });
packet!(id = 81;  GroupLFG         { list: ClassList<ClientLookingForGroup> });
packet!(id = 83;  GroupMember      { object_id: u32, name: String });
packet!(id = 86;  GroupRemove      { object_id: u32 });
packet!(id = 88;  GroupRequest     { name: String, level: i32, class: MirClass });
packet!(id = 91;  GroupSwitch      { allow: bool });
packet!(id = 92;  GroupUpdate      { group: ClientLookingForGroup });
packet!(id = 93;  GuildCastleInfo  { index: i32, owner: String });
packet!(id = 94;  GuildChanged     { object_id: u32, guild_name: String, guild_rank: String });
packet!(id = 96;  GuildConquestDate { index: i32, war_time: i64 });
packet!(id = 97;  GuildConquestFinished { index: i32 });
packet!(id = 98;  GuildConquestStarted { index: i32 });
packet!(id = 100; GuildCreate);
packet!(id = 101; GuildDayReset);
packet!(id = 105; GuildFundsChanged { change: i64 });
packet!(id = 106; GuildGetItem     { grid: GridType, slot: i32, item: Option<ClientUserItem> });
packet!(id = 108; GuildIncreaseMember);
packet!(id = 110; GuildIncreaseStorage);
packet!(id = 111; GuildInfo        { guild: Option<ClientGuildInfo> });
packet!(id = 112; GuildInvite      { name: String, guild_name: String });
packet!(id = 114; GuildInviteMember);
packet!(id = 115; GuildKick        { index: i32 });
packet!(id = 117; GuildMemberContribution { index: i32, contribution: i64 });
packet!(id = 118; GuildMemberOffline { index: i32 });
packet!(id = 119; GuildMemberOnline { index: i32, name: String, object_id: u32 });
packet!(id = 120; GuildNewItem     { slot: i32, item: Option<ClientUserItem> });
packet!(id = 121; GuildNoticeChanged { notice: String });
packet!(id = 126; GuildStats       { index: i32, stats: Option<Stats> });
packet!(id = 128; GuildTax);

// GuildUpdate has List<ClientGuildMemberInfo>
#[derive(Debug, Clone, Default)]
pub struct GuildUpdate {
    pub member_limit: i32,
    pub storage_limit: i32,
    pub guild_funds: i64,
    pub daily_growth: i64,
    pub guild_level: i32,
    pub tax: i32,
    pub total_contribution: i64,
    pub daily_contribution: i64,
    pub default_rank: String,
    pub default_permission: GuildPermission,
    pub colour: i32,
    pub flag: i32,
    pub members: ClassList<ClientGuildMemberInfo>,
}
impl crate::codec::PacketCodec for GuildUpdate {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(130) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            member_limit: i32::wire_read(&mut buf)?,
            storage_limit: i32::wire_read(&mut buf)?,
            guild_funds: i64::wire_read(&mut buf)?,
            daily_growth: i64::wire_read(&mut buf)?,
            guild_level: i32::wire_read(&mut buf)?,
            tax: i32::wire_read(&mut buf)?,
            total_contribution: i64::wire_read(&mut buf)?,
            daily_contribution: i64::wire_read(&mut buf)?,
            default_rank: String::wire_read(&mut buf)?,
            default_permission: GuildPermission::wire_read(&mut buf)?,
            colour: i32::wire_read(&mut buf)?,
            flag: i32::wire_read(&mut buf)?,
            members: ClassList::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.member_limit.wire_write(&mut buf);
        self.storage_limit.wire_write(&mut buf);
        self.guild_funds.wire_write(&mut buf);
        self.daily_growth.wire_write(&mut buf);
        self.guild_level.wire_write(&mut buf);
        self.tax.wire_write(&mut buf);
        self.total_contribution.wire_write(&mut buf);
        self.daily_contribution.wire_write(&mut buf);
        self.default_rank.wire_write(&mut buf);
        self.default_permission.wire_write(&mut buf);
        self.colour.wire_write(&mut buf);
        self.flag.wire_write(&mut buf);
        self.members.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 132; GuildWar         { success: bool });
packet!(id = 133; GuildWarFinished { guild_name: String });
packet!(id = 134; GuildWarStarted  { guild_name: String, duration: i64 });
packet!(id = 137; HealthChanged    { object_id: u32, change: i32, miss: bool, block: bool, critical: bool });
packet!(id = 139; HelmetToggle     { hide_helmet: bool });
packet!(id = 142; InformMaxExperience { max_experience: NetDecimal });

// Inspect has List<ClientUserItem>
#[derive(Debug, Clone, Default)]
pub struct Inspect {
    pub name: String,
    pub guild_name: String,
    pub guild_rank: String,
    pub guild_flag: i32,
    pub guild_colour: i32,
    pub partner: String,
    pub class: MirClass,
    pub level: i32,
    pub gender: MirGender,
    pub items: ClassList<ClientUserItem>,
    pub hair: i32,
    pub hair_colour: i32,
    pub fame: i32,
    pub ranking: bool,
}
impl crate::codec::PacketCodec for Inspect {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(144) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            name: String::wire_read(&mut buf)?,
            guild_name: String::wire_read(&mut buf)?,
            guild_rank: String::wire_read(&mut buf)?,
            guild_flag: i32::wire_read(&mut buf)?,
            guild_colour: i32::wire_read(&mut buf)?,
            partner: String::wire_read(&mut buf)?,
            class: MirClass::wire_read(&mut buf)?,
            level: i32::wire_read(&mut buf)?,
            gender: MirGender::wire_read(&mut buf)?,
            items: ClassList::wire_read(&mut buf)?,
            hair: i32::wire_read(&mut buf)?,
            hair_colour: i32::wire_read(&mut buf)?,
            fame: i32::wire_read(&mut buf)?,
            ranking: bool::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.name.wire_write(&mut buf);
        self.guild_name.wire_write(&mut buf);
        self.guild_rank.wire_write(&mut buf);
        self.guild_flag.wire_write(&mut buf);
        self.guild_colour.wire_write(&mut buf);
        self.partner.wire_write(&mut buf);
        self.class.wire_write(&mut buf);
        self.level.wire_write(&mut buf);
        self.gender.wire_write(&mut buf);
        self.items.wire_write(&mut buf);
        self.hair.wire_write(&mut buf);
        self.hair_colour.wire_write(&mut buf);
        self.fame.wire_write(&mut buf);
        self.ranking.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 145; ItemAcessoryRefined { grid_type: GridType, slot: i32, new_stats: Option<Stats> });
packet!(id = 146; ItemChanged        { link: CellLinkInfo, success: bool });
packet!(id = 148; ItemDelete         { grid: GridType, slot: i32, success: bool });
packet!(id = 150; ItemDurability     { grid_type: GridType, slot: i32, current_durability: i32 });
packet!(id = 151; ItemExperience     { target: CellLinkInfo, experience: NetDecimal, level: i32, flags: UserItemFlags });
packet!(id = 153; ItemLock           { grid: GridType, slot: i32, locked: bool });
packet!(id = 155; ItemMove           { from_grid: GridType, to_grid: GridType, from_slot: i32, to_slot: i32, merge_item: bool, success: bool });

// ItemSort has List<ClientUserItem>
#[derive(Debug, Clone, Default)]
pub struct ItemSort {
    pub grid: GridType,
    pub items: ClassList<ClientUserItem>,
    pub success: bool,
}
impl crate::codec::PacketCodec for ItemSort {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(157) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            grid: GridType::wire_read(&mut buf)?,
            items: ClassList::wire_read(&mut buf)?,
            success: bool::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.grid.wire_write(&mut buf);
        self.items.wire_write(&mut buf);
        self.success.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 159; ItemSplit         { grid: GridType, slot: i32, count: i64, new_slot: i32, success: bool });
packet!(id = 160; ItemStatsChanged  { grid_type: GridType, slot: i32, new_stats: Option<Stats> });
packet!(id = 161; ItemStatsRefreshed { grid_type: GridType, slot: i32, new_stats: Option<Stats> });
packet!(id = 163; ItemUseDelay      { delay: i64 });
packet!(id = 164; ItemsChanged      { links: ClassList<CellLinkInfo>, success: bool });
packet!(id = 165; ItemsGained       { items: ClassList<ClientUserItem> });
packet!(id = 167; JoinInstance      { result: InstanceResult, success: bool });
packet!(id = 169; LevelChanged      { level: i32, experience: NetDecimal, max_experience: NetDecimal });
packet!(id = 171; Login             {
    result: LoginResult, message: String, duration: i64,
    characters: ClassList<SelectInfo>,
    items: ClassList<ClientUserItem>,
    block_list: ClassList<ClientBlockInfo>,
    address: String, test_server: bool
});
packet!(id = 173; LootBoxClose);

// LootBoxOpen has List<ClientLootBoxItemInfo>
#[derive(Debug, Clone, Default)]
pub struct LootBoxOpen {
    pub slot: i32,
    pub items: ClassList<ClientLootBoxItemInfo>,
}
impl crate::codec::PacketCodec for LootBoxOpen {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(176) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self { slot: i32::wire_read(&mut buf)?, items: ClassList::wire_read(&mut buf)? })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.slot.wire_write(&mut buf);
        self.items.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 181; MagicCooldown    { info_index: i32, delay: i32 });
packet!(id = 183; MagicLeveled     { info_index: i32, level: i32, experience: i64 });
packet!(id = 185; MagicToggle      { magic: MagicType, can_use: bool });
packet!(id = 187; MailDelete       { index: i32 });
packet!(id = 189; MailItemDelete   { index: i32, slot: i32 });
packet!(id = 190; MailList         { mail: ClassList<ClientMailInfo> });
packet!(id = 191; MailNew          { mail: ClientMailInfo });
packet!(id = 194; MailSend);
packet!(id = 195; ManaChanged      { object_id: u32, change: i32 });
packet!(id = 196; MapChanged       { map_index: i32, instance_index: i32 });
packet!(id = 197; MapEffect        { location: (i32, i32), effect: Effect, direction: MirDirection });
packet!(id = 199; MarketPlaceBuy   { index: i32, count: i64, success: bool });
packet!(id = 202; MarketPlaceConsign { consignments: ClassList<ClientMarketPlaceInfo> });
packet!(id = 203; MarketPlaceConsignChanged { index: i32, count: i64 });
packet!(id = 205; MarketPlaceHistory { index: i32, sale_count: i64, last_price: i64, average_price: i64, display: i32 });
packet!(id = 207; MarketPlaceSearch { count: i32, results: ClassList<ClientMarketPlaceInfo> });
packet!(id = 208; MarketPlaceSearchCount { count: i32 });
packet!(id = 210; MarketPlaceSearchIndex { index: i32, result: Option<ClientMarketPlaceInfo> });
packet!(id = 212; MarketPlaceStoreBuy);
packet!(id = 213; MarriageInfo     { partner: Option<ClientPlayerInfo> });
packet!(id = 214; MarriageInvite   { name: String });
packet!(id = 216; MarriageMakeRing);
packet!(id = 217; MarriageOnlineChanged { object_id: u32 });
packet!(id = 218; MarriageRemoveRing);
packet!(id = 223; MilestoneEarned  { index: i32 });
packet!(id = 227; MountFailed      { horse: HorseType });
packet!(id = 231; NewAccount       { result: NewAccountResult });
packet!(id = 233; NewCharacter     { result: NewCharacterResult, character: Option<SelectInfo> });
packet!(id = 234; NewMagic         { magic: crate::shared::ClientUserMagic });

// NPCAccessoryLevelUp has List<CellLinkInfo>
#[derive(Debug, Clone, Default)]
pub struct NPCAccessoryLevelUp {
    pub target: CellLinkInfo,
    pub links: ClassList<CellLinkInfo>,
}
impl crate::codec::PacketCodec for NPCAccessoryLevelUp {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(236) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self { target: CellLinkInfo::wire_read(&mut buf)?, links: ClassList::wire_read(&mut buf)? })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.target.wire_write(&mut buf);
        self.links.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

// NPCAccessoryRefine has List<CellLinkInfo>
#[derive(Debug, Clone, Default)]
pub struct NPCAccessoryRefine {
    pub target: CellLinkInfo,
    pub ore_target: CellLinkInfo,
    pub links: ClassList<CellLinkInfo>,
    pub refine_type: RefineType,
    pub success: bool,
}
impl crate::codec::PacketCodec for NPCAccessoryRefine {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(238) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            target: CellLinkInfo::wire_read(&mut buf)?,
            ore_target: CellLinkInfo::wire_read(&mut buf)?,
            links: ClassList::wire_read(&mut buf)?,
            refine_type: RefineType::wire_read(&mut buf)?,
            success: bool::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.target.wire_write(&mut buf);
        self.ore_target.wire_write(&mut buf);
        self.links.wire_write(&mut buf);
        self.refine_type.wire_write(&mut buf);
        self.success.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 245; NPCClose);

// NPCRefine has multiple List<CellLinkInfo> fields
#[derive(Debug, Clone, Default)]
pub struct NPCRefine {
    pub refine_type: RefineType,
    pub refine_quality: RefineQuality,
    pub ores: ClassList<CellLinkInfo>,
    pub items: ClassList<CellLinkInfo>,
    pub specials: ClassList<CellLinkInfo>,
    pub success: bool,
}
impl crate::codec::PacketCodec for NPCRefine {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(251) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            refine_type: RefineType::wire_read(&mut buf)?,
            refine_quality: RefineQuality::wire_read(&mut buf)?,
            ores: ClassList::wire_read(&mut buf)?,
            items: ClassList::wire_read(&mut buf)?,
            specials: ClassList::wire_read(&mut buf)?,
            success: bool::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.refine_type.wire_write(&mut buf);
        self.refine_quality.wire_write(&mut buf);
        self.ores.wire_write(&mut buf);
        self.items.wire_write(&mut buf);
        self.specials.wire_write(&mut buf);
        self.success.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

// NPCRefinementStone has multiple List<CellLinkInfo> fields
#[derive(Debug, Clone, Default)]
pub struct NPCRefinementStone {
    pub iron_ores: ClassList<CellLinkInfo>,
    pub silver_ores: ClassList<CellLinkInfo>,
    pub diamond_ores: ClassList<CellLinkInfo>,
    pub gold_ores: ClassList<CellLinkInfo>,
    pub crystal: ClassList<CellLinkInfo>,
}
impl crate::codec::PacketCodec for NPCRefinementStone {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(253) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            iron_ores: ClassList::wire_read(&mut buf)?,
            silver_ores: ClassList::wire_read(&mut buf)?,
            diamond_ores: ClassList::wire_read(&mut buf)?,
            gold_ores: ClassList::wire_read(&mut buf)?,
            crystal: ClassList::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.iron_ores.wire_write(&mut buf);
        self.silver_ores.wire_write(&mut buf);
        self.diamond_ores.wire_write(&mut buf);
        self.gold_ores.wire_write(&mut buf);
        self.crystal.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 255; NPCRefineRetrieve { index: i32 });

// NPCRepair has List<CellLinkInfo>
#[derive(Debug, Clone, Default)]
pub struct NPCRepair {
    pub links: ClassList<CellLinkInfo>,
    pub special: bool,
    pub success: bool,
    pub special_repair_delay: i64,
}
impl crate::codec::PacketCodec for NPCRepair {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(257) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            links: ClassList::wire_read(&mut buf)?,
            special: bool::wire_read(&mut buf)?,
            success: bool::wire_read(&mut buf)?,
            special_repair_delay: i64::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.links.wire_write(&mut buf);
        self.special.wire_write(&mut buf);
        self.success.wire_write(&mut buf);
        self.special_repair_delay.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

// NPCResponse has List<ClientNpcValues>
#[derive(Debug, Clone, Default)]
pub struct NPCResponse {
    pub object_id: u32,
    pub index: i32,
    pub values: ClassList<ClientNpcValues>,
}
impl crate::codec::PacketCodec for NPCResponse {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(258) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            object_id: u32::wire_read(&mut buf)?,
            index: i32::wire_read(&mut buf)?,
            values: ClassList::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.object_id.wire_write(&mut buf);
        self.index.wire_write(&mut buf);
        self.values.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 260; NPCRoll          { r#type: i32, result: i32 });
packet!(id = 264; NPCWeaponCraft   {
    template: CellLinkInfo, yellow: CellLinkInfo, blue: CellLinkInfo,
    red: CellLinkInfo, purple: CellLinkInfo, green: CellLinkInfo, grey: CellLinkInfo,
    success: bool
});

// NPCMasterRefine has multiple List<CellLinkInfo> fields
#[derive(Debug, Clone, Default)]
pub struct NPCMasterRefine {
    pub fragment1s: ClassList<CellLinkInfo>,
    pub fragment2s: ClassList<CellLinkInfo>,
    pub fragment3s: ClassList<CellLinkInfo>,
    pub stones: ClassList<CellLinkInfo>,
    pub specials: ClassList<CellLinkInfo>,
    pub success: bool,
}
impl crate::codec::PacketCodec for NPCMasterRefine {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(248) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            fragment1s: ClassList::wire_read(&mut buf)?,
            fragment2s: ClassList::wire_read(&mut buf)?,
            fragment3s: ClassList::wire_read(&mut buf)?,
            stones: ClassList::wire_read(&mut buf)?,
            specials: ClassList::wire_read(&mut buf)?,
            success: bool::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.fragment1s.wire_write(&mut buf);
        self.fragment2s.wire_write(&mut buf);
        self.fragment3s.wire_write(&mut buf);
        self.stones.wire_write(&mut buf);
        self.specials.wire_write(&mut buf);
        self.success.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 265; ObjectAttack     {
    object_id: u32, direction: MirDirection, location: (i32, i32),
    attack_magic: MagicType, attack_element: Element, target_id: u32, slow: i64
});
packet!(id = 266; ObjectBuffAdd    { object_id: u32, r#type: BuffType, extra: i32 });
packet!(id = 267; ObjectBuffRemove { object_id: u32, r#type: BuffType });
packet!(id = 268; ObjectDash       { object_id: u32, direction: MirDirection, location: (i32, i32), distance: i32, magic: MagicType });
packet!(id = 269; ObjectDied       { object_id: u32, direction: MirDirection, location: (i32, i32) });
packet!(id = 270; ObjectEffect     { object_id: u32, effect: Effect });
packet!(id = 271; ObjectFishing    {
    object_id: u32, state: FishingState, direction: MirDirection,
    float_location: (i32, i32), fish_found: bool
});
packet!(id = 272; ObjectHarvest    { object_id: u32, direction: MirDirection, location: (i32, i32), slow: i64 });
packet!(id = 273; ObjectHarvested  { object_id: u32, direction: MirDirection, location: (i32, i32) });
packet!(id = 274; ObjectHide       { object_id: u32, direction: MirDirection, location: (i32, i32) });
packet!(id = 275; ObjectIdle       { object_id: u32, direction: MirDirection, location: (i32, i32), r#type: i32 });
packet!(id = 276; ObjectItem       { object_id: u32, item: Option<ClientUserItem>, location: (i32, i32) });
packet!(id = 277; ObjectLeveled    { object_id: u32 });

// ObjectMagic has List<uint> and List<Point>
#[derive(Debug, Clone, Default)]
pub struct ObjectMagic {
    pub object_id: u32,
    pub direction: MirDirection,
    pub current_location: (i32, i32),
    pub r#type: MagicType,
    pub targets: PrimList<u32>,
    pub locations: PrimList<(i32, i32)>,
    pub cast: bool,
    pub attack_element: Element,
    pub slow: i64,
}
impl crate::codec::PacketCodec for ObjectMagic {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(278) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            object_id: u32::wire_read(&mut buf)?,
            direction: MirDirection::wire_read(&mut buf)?,
            current_location: <(i32, i32)>::wire_read(&mut buf)?,
            r#type: MagicType::wire_read(&mut buf)?,
            targets: PrimList::wire_read(&mut buf)?,
            locations: PrimList::wire_read(&mut buf)?,
            cast: bool::wire_read(&mut buf)?,
            attack_element: Element::wire_read(&mut buf)?,
            slow: i64::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.object_id.wire_write(&mut buf);
        self.direction.wire_write(&mut buf);
        self.current_location.wire_write(&mut buf);
        self.r#type.wire_write(&mut buf);
        self.targets.wire_write(&mut buf);
        self.locations.wire_write(&mut buf);
        self.cast.wire_write(&mut buf);
        self.attack_element.wire_write(&mut buf);
        self.slow.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 279; ObjectMining     { object_id: u32, direction: MirDirection, location: (i32, i32), slow: i64, effect: bool });
packet!(id = 280; ObjectMonster    {
    object_id: u32, monster_index: i32, custom_name: String, name_colour: i32, pet_owner: String,
    direction: MirDirection, location: (i32, i32),
    dead: bool, skeleton: bool, poison: PoisonType,
    easter_event: bool, halloween_event: bool, christmas_event: bool,
    buffs: Option<IntDict>, extra: bool, extra1: i32, colour: i32,
    companion_object: Option<ClientCompanionObject>
});
packet!(id = 281; ObjectMount      { object_id: u32, horse: HorseType });
packet!(id = 282; ObjectNameColour { object_id: u32, colour: i32 });
packet!(id = 283; ObjectNPC        { object_id: u32, npc_index: i32, current_location: (i32, i32), direction: MirDirection });
packet!(id = 284; ObjectPetOwnerChanged { object_id: u32, pet_owner: String });

// ObjectPlayer has many fields including Option<IntDict> Buffs
#[derive(Debug, Clone, Default)]
pub struct ObjectPlayer {
    pub index: i32,
    pub object_id: u32,
    pub name: String,
    pub caption: String,
    pub name_colour: i32,
    pub guild_name: String,
    pub direction: MirDirection,
    pub location: (i32, i32),
    pub class: MirClass,
    pub gender: MirGender,
    pub hair_type: i32,
    pub hair_colour: i32,
    pub weapon: i32,
    pub shield: i32,
    pub armour: i32,
    pub costume: i32,
    pub armour_colour: i32,
    pub armour_effect: ExteriorEffect,
    pub emblem_effect: ExteriorEffect,
    pub weapon_effect: ExteriorEffect,
    pub shield_effect: ExteriorEffect,
    pub light: i32,
    pub dead: bool,
    pub poison: PoisonType,
    pub buffs: Option<IntDict>,
    pub horse: HorseType,
    pub helmet: i32,
    pub horse_shape: i32,
}
impl crate::codec::PacketCodec for ObjectPlayer {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(285) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            index: i32::wire_read(&mut buf)?,
            object_id: u32::wire_read(&mut buf)?,
            name: String::wire_read(&mut buf)?,
            caption: String::wire_read(&mut buf)?,
            name_colour: i32::wire_read(&mut buf)?,
            guild_name: String::wire_read(&mut buf)?,
            direction: MirDirection::wire_read(&mut buf)?,
            location: <(i32, i32)>::wire_read(&mut buf)?,
            class: MirClass::wire_read(&mut buf)?,
            gender: MirGender::wire_read(&mut buf)?,
            hair_type: i32::wire_read(&mut buf)?,
            hair_colour: i32::wire_read(&mut buf)?,
            weapon: i32::wire_read(&mut buf)?,
            shield: i32::wire_read(&mut buf)?,
            armour: i32::wire_read(&mut buf)?,
            costume: i32::wire_read(&mut buf)?,
            armour_colour: i32::wire_read(&mut buf)?,
            armour_effect: ExteriorEffect::wire_read(&mut buf)?,
            emblem_effect: ExteriorEffect::wire_read(&mut buf)?,
            weapon_effect: ExteriorEffect::wire_read(&mut buf)?,
            shield_effect: ExteriorEffect::wire_read(&mut buf)?,
            light: i32::wire_read(&mut buf)?,
            dead: bool::wire_read(&mut buf)?,
            poison: PoisonType::wire_read(&mut buf)?,
            buffs: Option::<IntDict>::wire_read(&mut buf)?,
            horse: HorseType::wire_read(&mut buf)?,
            helmet: i32::wire_read(&mut buf)?,
            horse_shape: i32::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.index.wire_write(&mut buf);
        self.object_id.wire_write(&mut buf);
        self.name.wire_write(&mut buf);
        self.caption.wire_write(&mut buf);
        self.name_colour.wire_write(&mut buf);
        self.guild_name.wire_write(&mut buf);
        self.direction.wire_write(&mut buf);
        self.location.wire_write(&mut buf);
        self.class.wire_write(&mut buf);
        self.gender.wire_write(&mut buf);
        self.hair_type.wire_write(&mut buf);
        self.hair_colour.wire_write(&mut buf);
        self.weapon.wire_write(&mut buf);
        self.shield.wire_write(&mut buf);
        self.armour.wire_write(&mut buf);
        self.costume.wire_write(&mut buf);
        self.armour_colour.wire_write(&mut buf);
        self.armour_effect.wire_write(&mut buf);
        self.emblem_effect.wire_write(&mut buf);
        self.weapon_effect.wire_write(&mut buf);
        self.shield_effect.wire_write(&mut buf);
        self.light.wire_write(&mut buf);
        self.dead.wire_write(&mut buf);
        self.poison.wire_write(&mut buf);
        self.buffs.wire_write(&mut buf);
        self.horse.wire_write(&mut buf);
        self.helmet.wire_write(&mut buf);
        self.horse_shape.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 286; ObjectPoison     { object_id: u32, poison: PoisonType });

// ObjectProjectile has List<uint> and List<Point>
#[derive(Debug, Clone, Default)]
pub struct ObjectProjectile {
    pub object_id: u32,
    pub direction: MirDirection,
    pub current_location: (i32, i32),
    pub r#type: MagicType,
    pub targets: PrimList<u32>,
    pub locations: PrimList<(i32, i32)>,
}
impl crate::codec::PacketCodec for ObjectProjectile {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(287) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            object_id: u32::wire_read(&mut buf)?,
            direction: MirDirection::wire_read(&mut buf)?,
            current_location: <(i32, i32)>::wire_read(&mut buf)?,
            r#type: MagicType::wire_read(&mut buf)?,
            targets: PrimList::wire_read(&mut buf)?,
            locations: PrimList::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.object_id.wire_write(&mut buf);
        self.direction.wire_write(&mut buf);
        self.current_location.wire_write(&mut buf);
        self.r#type.wire_write(&mut buf);
        self.targets.wire_write(&mut buf);
        self.locations.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 288; ObjectPushed     { object_id: u32, direction: MirDirection, location: (i32, i32) });

// ObjectRangeAttack has List<uint>
#[derive(Debug, Clone, Default)]
pub struct ObjectRangeAttack {
    pub object_id: u32,
    pub direction: MirDirection,
    pub location: (i32, i32),
    pub attack_magic: MagicType,
    pub attack_element: Element,
    pub targets: PrimList<u32>,
}
impl crate::codec::PacketCodec for ObjectRangeAttack {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(289) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            object_id: u32::wire_read(&mut buf)?,
            direction: MirDirection::wire_read(&mut buf)?,
            location: <(i32, i32)>::wire_read(&mut buf)?,
            attack_magic: MagicType::wire_read(&mut buf)?,
            attack_element: Element::wire_read(&mut buf)?,
            targets: PrimList::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.object_id.wire_write(&mut buf);
        self.direction.wire_write(&mut buf);
        self.location.wire_write(&mut buf);
        self.attack_magic.wire_write(&mut buf);
        self.attack_element.wire_write(&mut buf);
        self.targets.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 290; ObjectRemove     { object_id: u32 });
packet!(id = 291; ObjectRevive     { object_id: u32, location: (i32, i32), effect: bool });
packet!(id = 292; ObjectShow       { object_id: u32, direction: MirDirection, location: (i32, i32) });
packet!(id = 293; ObjectSpell      { object_id: u32, direction: MirDirection, location: (i32, i32), effect: SpellEffect, power: i32 });
packet!(id = 294; ObjectSpellChanged { object_id: u32, power: i32 });
packet!(id = 295; ObjectStats      { object_id: u32, stats: Option<Stats> });
packet!(id = 296; ObjectStruck     { object_id: u32, direction: MirDirection, location: (i32, i32), attacker_id: u32, element: Element });
packet!(id = 297; ObjectTurn       { object_id: u32, direction: MirDirection, location: (i32, i32), slow: i64 });
packet!(id = 299; ObservableSwitch { allow: bool });
packet!(id = 302; PlayerChangeUpdate {
    object_id: u32, name: String, caption: String, caption_outline_colour: i32,
    gender: MirGender, hair_type: i32, hair_colour: i32, armour_colour: i32
});
packet!(id = 303; PlayerUpdate     {
    object_id: u32, weapon: i32, shield: i32, armour: i32, costume: i32,
    armour_colour: i32, armour_effect: ExteriorEffect, emblem_effect: ExteriorEffect,
    weapon_effect: ExteriorEffect, shield_effect: ExteriorEffect,
    horse_armour: i32, helmet: i32, light: i32, hide_head: bool
});
packet!(id = 306; QuestCancelled   { index: i32 });
packet!(id = 307; QuestChanged     { quest: Option<ClientUserQuest> });

// Rankings has List<RankInfo>
#[derive(Debug, Clone, Default)]
pub struct Rankings {
    pub online_only: bool,
    pub class: RequiredClass,
    pub start_index: i32,
    pub total: i32,
    pub allow_observation: bool,
    pub ranks: ClassList<RankInfo>,
}
impl crate::codec::PacketCodec for Rankings {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(311) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            online_only: bool::wire_read(&mut buf)?,
            class: RequiredClass::wire_read(&mut buf)?,
            start_index: i32::wire_read(&mut buf)?,
            total: i32::wire_read(&mut buf)?,
            allow_observation: bool::wire_read(&mut buf)?,
            ranks: ClassList::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.online_only.wire_write(&mut buf);
        self.class.wire_write(&mut buf);
        self.start_index.wire_write(&mut buf);
        self.total.wire_write(&mut buf);
        self.allow_observation.wire_write(&mut buf);
        self.ranks.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 314; RankSearch       { rank: Option<RankInfo>, start_index: i32 });
packet!(id = 315; RefineList       { list: ClassList<ClientRefineInfo> });
packet!(id = 317; RequestActivationKey { result: RequestActivationKeyResult, duration: i64 });
packet!(id = 319; RequestPasswordReset { result: RequestPasswordResetResult, message: String, duration: i64 });
packet!(id = 321; ResetPassword    { result: ResetPasswordResult });
packet!(id = 322; ReviveTimers     { item_revive_time: i64, reincarnation_pill_time: i64 });
packet!(id = 323; SafeZoneChanged  { in_safe_zone: bool });
packet!(id = 325; SelectLogout);

// SendCompanionFilters has prim lists
#[derive(Debug, Clone, Default)]
pub struct SendCompanionFilters {
    pub filter_class: PrimList<MirClass>,
    pub filter_rarity: PrimList<Rarity>,
    pub filter_item_type: PrimList<ItemType>,
}
impl crate::codec::PacketCodec for SendCompanionFilters {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(327) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            filter_class: PrimList::wire_read(&mut buf)?,
            filter_rarity: PrimList::wire_read(&mut buf)?,
            filter_item_type: PrimList::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.filter_class.wire_write(&mut buf);
        self.filter_rarity.wire_write(&mut buf);
        self.filter_item_type.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 328; SetTimer         { key: String, r#type: u8, seconds: i32 });
packet!(id = 330; StartGame        { result: StartGameResult, message: String, duration: i64, start_information: Option<StartInformation> });

// StartObserver has List<ClientUserItem>
#[derive(Debug, Clone, Default)]
pub struct StartObserver {
    pub start_information: Option<StartInformation>,
    pub items: ClassList<ClientUserItem>,
}
impl crate::codec::PacketCodec for StartObserver {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(331) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            start_information: Option::<StartInformation>::wire_read(&mut buf)?,
            items: ClassList::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.start_information.wire_write(&mut buf);
        self.items.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 332; StatsUpdate      { stats: Option<Stats>, hermit_stats: Option<Stats>, hermit_points: i32 });
packet!(id = 333; StorageSize      { size: i32 });
packet!(id = 335; TimeOfDayChanged { time_of_day: TimeOfDay, time_of_day_label: String });
packet!(id = 338; TradeAddGold     { gold: i64 });
packet!(id = 340; TradeAddItem     { cell: CellLinkInfo, success: bool });
packet!(id = 342; TradeClose);
packet!(id = 344; TradeGoldAdded   { gold: i64 });
packet!(id = 345; TradeItemAdded   { item: Option<ClientUserItem> });
packet!(id = 346; TradeOpen        { name: String });
packet!(id = 348; TradeRequest     { name: String });
packet!(id = 350; TradeUnlock);
packet!(id = 352; UserLocation     { direction: MirDirection, location: (i32, i32) });
packet!(id = 353; UserMilestones   { milestones: ClassList<ClientUserMilestone> });
packet!(id = 354; WeightUpdate     { bag_weight: i32, wear_weight: i32, hand_weight: i32 });
