use super::packet;
use crate::{
    enums::*,
    shared::CellLinkInfo,
    wire::{ClassList, PrimList},
};

// ── IDs assigned from the full sorted merge (ClientPackets namespace) ─────

packet!(id = 7;   Activation     { activation_key: String, check_sum: String });
packet!(id = 9;   Attack         { direction: MirDirection, action: MirAction, attack_magic: MagicType });
packet!(id = 10;  AutoPotionLinkChanged {
    slot: i32, link_index: i32, health: i32, mana: i32, enabled: bool
});
packet!(id = 11;  BeltLinkChanged { slot: i32, link_index: i32, link_item_index: i32 });
packet!(id = 12;  BlockAdd        { name: String });
packet!(id = 14;  BlockRemove     { index: i32 });
packet!(id = 22;  BundleConfirm   { slot: i32, choice: i32 });
packet!(id = 23;  BundleOpen      { slot: i32 });
packet!(id = 25;  CaptionChange   { caption: String });
packet!(id = 26;  ChangeAttackMode { mode: AttackMode });
packet!(id = 28;  ChangeOnlineState { state: OnlineState });
packet!(id = 29;  ChangePetMode   { mode: PetMode });
packet!(id = 31;  ChangePassword  {
    e_mail_address: String, current_password: String,
    new_password: String, check_sum: String
});
packet!(id = 33;  Chat            { text: String });
packet!(id = 36;  CompanionAdopt  { index: i32, name: String });
packet!(id = 39;  CompanionRelease { index: i32 });
packet!(id = 41;  CompanionRetrieve { index: i32 });
packet!(id = 45;  CompanionStore  { index: i32 });
packet!(id = 47;  CompanionUnlock { index: i32 });
packet!(id = 52;  CurrencyDrop    { currency_index: i32, amount: i64 });
packet!(id = 61;  DeleteCharacter { character_index: i32, check_sum: String });
packet!(id = 65;  FishingCast     {
    state: FishingState, direction: MirDirection,
    float_location: (i32, i32), caught_fish: bool
});
packet!(id = 68;  FortuneCheck    { item_index: i32 });
packet!(id = 70;  FriendAdd       { name: String });
packet!(id = 72;  FriendRemove    { index: i32 });
packet!(id = 76;  GameGoldRecharge);
packet!(id = 78;  GenderChange    { gender: MirGender, hair_type: i32, hair_colour: i32 });
packet!(id = 79;  GroupInvite     { name: String });
packet!(id = 82;  GroupLFGUpdate  { enabled: bool, name: String, r#type: String, max_count: i32 });
packet!(id = 84;  GroupNotify     { receive: bool });
packet!(id = 85;  GroupRemove     { name: String });
packet!(id = 87;  GroupRequest    { name: String });
packet!(id = 89;  GroupResponse   { name: String, accept: bool });
packet!(id = 90;  GroupSwitch     { allow: bool });
packet!(id = 95;  GuildColour     { colour: i32 });
packet!(id = 99;  GuildCreate     { name: String, use_gold: bool, members: i32, storage: i32 });
packet!(id = 102; GuildEditMember { index: i32, rank: String, permission: GuildPermission });
packet!(id = 103; GuildEditNotice { notice: String });
packet!(id = 104; GuildFlag       { flag: i32 });
packet!(id = 107; GuildIncreaseMember);
packet!(id = 109; GuildIncreaseStorage);
packet!(id = 113; GuildInviteMember { name: String });
packet!(id = 116; GuildKickMember { index: i32 });
packet!(id = 122; GuildRepairCastleGates);
packet!(id = 123; GuildRepairCastleGuards);
packet!(id = 124; GuildRequestConquest { index: i32 });
packet!(id = 125; GuildResponse   { accept: bool });
packet!(id = 127; GuildTax        { tax: i64 });
packet!(id = 129; GuildToggleCastleGates);
packet!(id = 131; GuildWar        { guild_name: String });
packet!(id = 135; HairChange      { hair_type: i32, hair_colour: i32 });
packet!(id = 136; Harvest         { direction: MirDirection });
packet!(id = 138; HelmetToggle    { hide_helmet: bool });
packet!(id = 140; Hermit          { stat: i32 });
packet!(id = 141; IncreaseDiscipline);
packet!(id = 143; Inspect         { index: i32, ranking: bool });
packet!(id = 147; ItemDelete      { grid: GridType, slot: i32 });
packet!(id = 149; ItemDrop        { link: CellLinkInfo, slot: i32 });
packet!(id = 152; ItemLock        { grid_type: GridType, slot_index: i32, locked: bool });
packet!(id = 154; ItemMove        { from_grid: GridType, to_grid: GridType, from_slot: i32, to_slot: i32, merge_item: bool });
packet!(id = 156; ItemSort        { grid: GridType });
packet!(id = 158; ItemSplit       { grid: GridType, slot: i32, count: i64 });
packet!(id = 162; ItemUse         { link: CellLinkInfo });
packet!(id = 166; JoinInstance    { index: i32 });
packet!(id = 168; JoinStarterGuild);
packet!(id = 170; Login           { e_mail_address: String, password: String, check_sum: String });
packet!(id = 172; Logout);
packet!(id = 174; LootBoxConfirmSelection { slot: i32 });
packet!(id = 175; LootBoxOpen     { slot: i32 });
packet!(id = 177; LootBoxReroll   { slot: i32 });
packet!(id = 178; LootBoxReveal   { slot: i32, choice: i32 });
packet!(id = 179; LootBoxTakeItems { slot: i32, choice: i32 });
packet!(id = 180; Magic           {
    direction: MirDirection, action: MirAction,
    r#type: MagicType, target: u32, location: (i32, i32)
});
packet!(id = 182; MagicKey        {
    magic: MagicType,
    set1_key: SpellKey, set2_key: SpellKey, set3_key: SpellKey, set4_key: SpellKey
});
packet!(id = 184; MagicToggle     { magic: MagicType, can_use: bool });
packet!(id = 186; MailDelete      { index: i32 });
packet!(id = 188; MailGetItem     { index: i32, slot: i32 });
packet!(id = 192; MailOpened      { index: i32 });

// MailSend has a List<CellLinkInfo> (class) field
#[derive(Debug, Clone, Default)]
pub struct MailSend {
    pub links: ClassList<CellLinkInfo>,
    pub recipient: String,
    pub subject: String,
    pub message: String,
    pub gold: i64,
}
impl crate::codec::PacketCodec for MailSend {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(193) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            links: ClassList::wire_read(&mut buf)?,
            recipient: String::wire_read(&mut buf)?,
            subject: String::wire_read(&mut buf)?,
            message: String::wire_read(&mut buf)?,
            gold: i64::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.links.wire_write(&mut buf);
        self.recipient.wire_write(&mut buf);
        self.subject.wire_write(&mut buf);
        self.message.wire_write(&mut buf);
        self.gold.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 198; MarketPlaceBuy  { index: i64, count: i64, guild_funds: bool });
packet!(id = 200; MarketPlaceCancelConsign { index: i32, count: i64 });
packet!(id = 201; MarketPlaceConsign {
    link: CellLinkInfo, price: i32, message: String, guild_funds: bool
});
packet!(id = 204; MarketPlaceHistory { index: i32, display: i32, part_index: i32 });
packet!(id = 206; MarketPlaceSearch {
    name: String, item_type_filter: bool, item_type: ItemType, sort: MarketPlaceSort
});
packet!(id = 209; MarketPlaceSearchIndex { index: i32 });
packet!(id = 211; MarketPlaceStoreBuy { index: i32, count: i64, use_hunt_gold: bool });
packet!(id = 215; MarriageMakeRing { slot: i32 });
packet!(id = 219; MarriageResponse { accept: bool });
packet!(id = 220; MarriageTeleport);
packet!(id = 221; MilestoneActive { index: i32, active: bool });
packet!(id = 222; MilestoneClaim  { index: i32 });
packet!(id = 224; MilestoneNotify { receive: bool });
packet!(id = 225; Mining          { direction: MirDirection });
packet!(id = 226; Mount);
packet!(id = 228; Move            { direction: MirDirection, distance: i32 });
packet!(id = 229; NameChange      { name: String });
packet!(id = 230; NewAccount      {
    e_mail_address: String, password: String, birth_date: i64,
    real_name: String, referral: String, check_sum: String
});
packet!(id = 232; NewCharacter    {
    character_name: String, class: MirClass, gender: MirGender,
    hair_type: i32, hair_colour: i32, armour_colour: i32, check_sum: String
});

// NPCAccessoryLevelUp has a List<CellLinkInfo>
#[derive(Debug, Clone, Default)]
pub struct NPCAccessoryLevelUp {
    pub target: CellLinkInfo,
    pub links: ClassList<CellLinkInfo>,
}
impl crate::codec::PacketCodec for NPCAccessoryLevelUp {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(235) }
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

// NPCAccessoryRefine has a List<CellLinkInfo>
#[derive(Debug, Clone, Default)]
pub struct NPCAccessoryRefine {
    pub target: CellLinkInfo,
    pub ore_target: CellLinkInfo,
    pub links: ClassList<CellLinkInfo>,
    pub refine_type: RefineType,
}
impl crate::codec::PacketCodec for NPCAccessoryRefine {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(237) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            target: CellLinkInfo::wire_read(&mut buf)?,
            ore_target: CellLinkInfo::wire_read(&mut buf)?,
            links: ClassList::wire_read(&mut buf)?,
            refine_type: RefineType::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.target.wire_write(&mut buf);
        self.ore_target.wire_write(&mut buf);
        self.links.wire_write(&mut buf);
        self.refine_type.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 239; NPCAccessoryReset { cell: CellLinkInfo });
packet!(id = 240; NPCAccessoryUpgrade { target: CellLinkInfo, refine_type: RefineType });
packet!(id = 241; NPCButton         { button_id: i32 });
packet!(id = 242; NPCBuy            { index: i32, amount: i64, guild_funds: bool });
packet!(id = 243; NPCCall           { object_id: u32 });
packet!(id = 244; NPCClose);

// NPCFragment has List<CellLinkInfo>
#[derive(Debug, Clone, Default)]
pub struct NPCFragment { pub links: ClassList<CellLinkInfo> }
impl crate::codec::PacketCodec for NPCFragment {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(246) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self { links: ClassList::wire_read(&mut buf)? })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.links.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

// NPCMasterRefine has multiple List<CellLinkInfo> fields
#[derive(Debug, Clone, Default)]
pub struct NPCMasterRefine {
    pub refine_type: RefineType,
    pub fragment1s: ClassList<CellLinkInfo>,
    pub fragment2s: ClassList<CellLinkInfo>,
    pub fragment3s: ClassList<CellLinkInfo>,
    pub stones: ClassList<CellLinkInfo>,
    pub specials: ClassList<CellLinkInfo>,
}
impl crate::codec::PacketCodec for NPCMasterRefine {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(247) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            refine_type: RefineType::wire_read(&mut buf)?,
            fragment1s: ClassList::wire_read(&mut buf)?,
            fragment2s: ClassList::wire_read(&mut buf)?,
            fragment3s: ClassList::wire_read(&mut buf)?,
            stones: ClassList::wire_read(&mut buf)?,
            specials: ClassList::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.refine_type.wire_write(&mut buf);
        self.fragment1s.wire_write(&mut buf);
        self.fragment2s.wire_write(&mut buf);
        self.fragment3s.wire_write(&mut buf);
        self.stones.wire_write(&mut buf);
        self.specials.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

// NPCMasterRefineEvaluate has multiple List<CellLinkInfo> fields
#[derive(Debug, Clone, Default)]
pub struct NPCMasterRefineEvaluate {
    pub refine_type: RefineType,
    pub fragment1s: ClassList<CellLinkInfo>,
    pub fragment2s: ClassList<CellLinkInfo>,
    pub fragment3s: ClassList<CellLinkInfo>,
    pub stones: ClassList<CellLinkInfo>,
    pub specials: ClassList<CellLinkInfo>,
}
impl crate::codec::PacketCodec for NPCMasterRefineEvaluate {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(249) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            refine_type: RefineType::wire_read(&mut buf)?,
            fragment1s: ClassList::wire_read(&mut buf)?,
            fragment2s: ClassList::wire_read(&mut buf)?,
            fragment3s: ClassList::wire_read(&mut buf)?,
            stones: ClassList::wire_read(&mut buf)?,
            specials: ClassList::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.refine_type.wire_write(&mut buf);
        self.fragment1s.wire_write(&mut buf);
        self.fragment2s.wire_write(&mut buf);
        self.fragment3s.wire_write(&mut buf);
        self.stones.wire_write(&mut buf);
        self.specials.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

// NPCRefine has multiple List<CellLinkInfo> fields
#[derive(Debug, Clone, Default)]
pub struct NPCRefine {
    pub refine_type: RefineType,
    pub refine_quality: RefineQuality,
    pub ores: ClassList<CellLinkInfo>,
    pub items: ClassList<CellLinkInfo>,
    pub specials: ClassList<CellLinkInfo>,
}
impl crate::codec::PacketCodec for NPCRefine {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(250) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            refine_type: RefineType::wire_read(&mut buf)?,
            refine_quality: RefineQuality::wire_read(&mut buf)?,
            ores: ClassList::wire_read(&mut buf)?,
            items: ClassList::wire_read(&mut buf)?,
            specials: ClassList::wire_read(&mut buf)?,
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
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

// NPCRefinementStone has multiple List<CellLinkInfo> fields + long Gold
#[derive(Debug, Clone, Default)]
pub struct NPCRefinementStone {
    pub iron_ores: ClassList<CellLinkInfo>,
    pub silver_ores: ClassList<CellLinkInfo>,
    pub diamond_ores: ClassList<CellLinkInfo>,
    pub gold_ores: ClassList<CellLinkInfo>,
    pub crystal: ClassList<CellLinkInfo>,
    pub gold: i64,
}
impl crate::codec::PacketCodec for NPCRefinementStone {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(252) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            iron_ores: ClassList::wire_read(&mut buf)?,
            silver_ores: ClassList::wire_read(&mut buf)?,
            diamond_ores: ClassList::wire_read(&mut buf)?,
            gold_ores: ClassList::wire_read(&mut buf)?,
            crystal: ClassList::wire_read(&mut buf)?,
            gold: i64::wire_read(&mut buf)?,
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
        self.gold.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 254; NPCRefineRetrieve { index: i32 });

// NPCRepair has List<CellLinkInfo> + bool + bool
#[derive(Debug, Clone, Default)]
pub struct NPCRepair {
    pub links: ClassList<CellLinkInfo>,
    pub special: bool,
    pub guild_funds: bool,
}
impl crate::codec::PacketCodec for NPCRepair {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(256) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self {
            links: ClassList::wire_read(&mut buf)?,
            special: bool::wire_read(&mut buf)?,
            guild_funds: bool::wire_read(&mut buf)?,
        })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.links.wire_write(&mut buf);
        self.special.wire_write(&mut buf);
        self.guild_funds.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

packet!(id = 259; NPCRoll          { r#type: i32 });
packet!(id = 261; NPCRollResult);

// NPCSell has List<CellLinkInfo>
#[derive(Debug, Clone, Default)]
pub struct NPCSell { pub links: ClassList<CellLinkInfo> }
impl crate::codec::PacketCodec for NPCSell {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(262) }
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
        use crate::wire::WireRead;
        let mut buf = payload;
        Ok(Self { links: ClassList::wire_read(&mut buf)? })
    }
    fn encode(&self) -> bytes::Bytes {
        use crate::wire::WireWrite;
        let mut buf = bytes::BytesMut::new();
        self.links.wire_write(&mut buf);
        crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
    }
}

// NPCWeaponCraft has many CellLinkInfo fields
packet!(id = 263; NPCWeaponCraft {
    class: RequiredClass,
    template: CellLinkInfo, yellow: CellLinkInfo, blue: CellLinkInfo,
    red: CellLinkInfo, purple: CellLinkInfo, green: CellLinkInfo, grey: CellLinkInfo
});

packet!(id = 298; ObservableSwitch { allow: bool });
packet!(id = 300; ObserverRequest  { name: String });
packet!(id = 301; PickUp);
packet!(id = 304; QuestAbandon     { index: i32 });
packet!(id = 305; QuestAccept      { index: i32 });
packet!(id = 308; QuestComplete    { index: i32, choice_index: i32 });
packet!(id = 309; QuestTrack       { index: i32, track: bool });
packet!(id = 310; RangeAttack      { direction: MirDirection, target: u32 });
packet!(id = 312; RankRequest      { class: RequiredClass, online_only: bool, start_index: i32 });
packet!(id = 313; RankSearch       { name: String });
packet!(id = 316; RequestActivationKey { e_mail_address: String, check_sum: String });
packet!(id = 318; RequestPasswordReset { e_mail_address: String, check_sum: String });
packet!(id = 320; ResetPassword    { reset_key: String, new_password: String, check_sum: String });
packet!(id = 324; SelectLanguage   { language: String });

// SendCompanionFilters has List<MirClass>, List<Rarity>, List<ItemType> (all prim)
#[derive(Debug, Clone, Default)]
pub struct SendCompanionFilters {
    pub filter_class: PrimList<MirClass>,
    pub filter_rarity: PrimList<Rarity>,
    pub filter_item_type: PrimList<ItemType>,
}
impl crate::codec::PacketCodec for SendCompanionFilters {
    fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId(326) }
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

packet!(id = 329; StartGame        { character_index: i32 });
packet!(id = 334; TeleportRing     { location: (i32, i32), index: i32 });
packet!(id = 336; TownRevive);
packet!(id = 337; TradeAddGold     { gold: i64 });
packet!(id = 339; TradeAddItem     { cell: CellLinkInfo });
packet!(id = 341; TradeClose);
packet!(id = 343; TradeConfirm);
packet!(id = 347; TradeRequest);
packet!(id = 349; TradeRequestResponse { accept: bool });
packet!(id = 351; Turn             { direction: MirDirection });

