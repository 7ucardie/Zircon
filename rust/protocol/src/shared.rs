//! Shared data types that appear in packet fields — mirrors Globals.cs classes.
//!
//! All types implement WireRead and WireWrite matching .NET reflection
//! serialization (property order, null flags, list encoding).

use bytes::{Buf, BytesMut};

use crate::{
    enums::*,
    error::ProtocolError,
    wire::{
        read_class_list, read_prim_list, write_class_list, write_prim_list,
        NetDecimal, Stats, WireRead, WireWrite,
    },
};

// ── CellLinkInfo ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct CellLinkInfo {
    pub grid_type: GridType,
    pub slot: i32,
    pub count: i64,
}

impl WireRead for CellLinkInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            grid_type: GridType::wire_read(buf)?,
            slot: i32::wire_read(buf)?,
            count: i64::wire_read(buf)?,
        })
    }
}
impl WireWrite for CellLinkInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.grid_type.wire_write(buf);
        self.slot.wire_write(buf);
        self.count.wire_write(buf);
    }
}

// ── ClientBlockInfo ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientBlockInfo {
    pub index: i32,
    pub name: String,
}

impl WireRead for ClientBlockInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self { index: i32::wire_read(buf)?, name: String::wire_read(buf)? })
    }
}
impl WireWrite for ClientBlockInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.index.wire_write(buf);
        self.name.wire_write(buf);
    }
}

// ── ClientFriendInfo ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientFriendInfo {
    pub index: i32,
    pub name: String,
    pub state: OnlineState,
}

impl WireRead for ClientFriendInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            index: i32::wire_read(buf)?,
            name: String::wire_read(buf)?,
            state: OnlineState::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientFriendInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.index.wire_write(buf);
        self.name.wire_write(buf);
        self.state.wire_write(buf);
    }
}

// ── ClientUserItem ────────────────────────────────────────────────────────
//
// Serialized properties (in reflection order, [IgnorePropertyPacket] excluded):
// Index, InfoIndex, CurrentDurability, MaxDurability, Count, Slot, Level,
// Experience, Colour, SpecialRepairCoolDown, ResetCoolDown, AddedStats,
// Flags, ExpireTime
//
// AddedStats is Option<Stats> (null flag + Stats inner).
// SpecialRepairCoolDown, ResetCoolDown, ExpireTime are TimeSpan (i64 ticks).
// Colour is i32 ARGB.

#[derive(Debug, Clone, Default)]
pub struct ClientUserItem {
    pub index: i32,
    pub info_index: i32,
    pub current_durability: i32,
    pub max_durability: i32,
    pub count: i64,
    pub slot: i32,
    pub level: i32,
    pub experience: NetDecimal,
    pub colour: i32,
    pub special_repair_cool_down: i64,
    pub reset_cool_down: i64,
    pub added_stats: Option<Stats>,
    pub flags: UserItemFlags,
    pub expire_time: i64,
}

impl WireRead for ClientUserItem {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            index: i32::wire_read(buf)?,
            info_index: i32::wire_read(buf)?,
            current_durability: i32::wire_read(buf)?,
            max_durability: i32::wire_read(buf)?,
            count: i64::wire_read(buf)?,
            slot: i32::wire_read(buf)?,
            level: i32::wire_read(buf)?,
            experience: NetDecimal::wire_read(buf)?,
            colour: i32::wire_read(buf)?,
            special_repair_cool_down: i64::wire_read(buf)?,
            reset_cool_down: i64::wire_read(buf)?,
            added_stats: Option::<Stats>::wire_read(buf)?,
            flags: UserItemFlags::wire_read(buf)?,
            expire_time: i64::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientUserItem {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.index.wire_write(buf);
        self.info_index.wire_write(buf);
        self.current_durability.wire_write(buf);
        self.max_durability.wire_write(buf);
        self.count.wire_write(buf);
        self.slot.wire_write(buf);
        self.level.wire_write(buf);
        self.experience.wire_write(buf);
        self.colour.wire_write(buf);
        self.special_repair_cool_down.wire_write(buf);
        self.reset_cool_down.wire_write(buf);
        self.added_stats.wire_write(buf);
        self.flags.wire_write(buf);
        self.expire_time.wire_write(buf);
    }
}

// ── SelectInfo ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct SelectInfo {
    pub character_index: i32,
    pub character_name: String,
    pub caption: String,
    pub level: i32,
    pub gender: MirGender,
    pub class: MirClass,
    pub location: i32,
    pub last_login: i64, // DateTime binary
}

impl WireRead for SelectInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            character_index: i32::wire_read(buf)?,
            character_name: String::wire_read(buf)?,
            caption: String::wire_read(buf)?,
            level: i32::wire_read(buf)?,
            gender: MirGender::wire_read(buf)?,
            class: MirClass::wire_read(buf)?,
            location: i32::wire_read(buf)?,
            last_login: i64::wire_read(buf)?,
        })
    }
}
impl WireWrite for SelectInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.character_index.wire_write(buf);
        self.character_name.wire_write(buf);
        self.caption.wire_write(buf);
        self.level.wire_write(buf);
        self.gender.wire_write(buf);
        self.class.wire_write(buf);
        self.location.wire_write(buf);
        self.last_login.wire_write(buf);
    }
}

// ── ClientBeltLink ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientBeltLink {
    pub slot: i32,
    pub link_index: i32,
    pub link_item_index: i32,
}
impl WireRead for ClientBeltLink {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            slot: i32::wire_read(buf)?,
            link_index: i32::wire_read(buf)?,
            link_item_index: i32::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientBeltLink {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.slot.wire_write(buf);
        self.link_index.wire_write(buf);
        self.link_item_index.wire_write(buf);
    }
}

// ── ClientAutoPotionLink ──────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientAutoPotionLink {
    pub slot: i32,
    pub link_index: i32,
    pub health: i32,
    pub mana: i32,
    pub enabled: bool,
}
impl WireRead for ClientAutoPotionLink {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            slot: i32::wire_read(buf)?,
            link_index: i32::wire_read(buf)?,
            health: i32::wire_read(buf)?,
            mana: i32::wire_read(buf)?,
            enabled: bool::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientAutoPotionLink {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.slot.wire_write(buf);
        self.link_index.wire_write(buf);
        self.health.wire_write(buf);
        self.mana.wire_write(buf);
        self.enabled.wire_write(buf);
    }
}

// ── ClientUserMagic ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientUserMagic {
    pub info_index: i32,
    pub key1: SpellKey,
    pub key2: SpellKey,
    pub key3: SpellKey,
    pub key4: SpellKey,
    pub level: i32,
    pub experience: i64,
    pub cast_time: i64,
}
impl WireRead for ClientUserMagic {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            info_index: i32::wire_read(buf)?,
            key1: SpellKey::wire_read(buf)?,
            key2: SpellKey::wire_read(buf)?,
            key3: SpellKey::wire_read(buf)?,
            key4: SpellKey::wire_read(buf)?,
            level: i32::wire_read(buf)?,
            experience: i64::wire_read(buf)?,
            cast_time: i64::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientUserMagic {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.info_index.wire_write(buf);
        self.key1.wire_write(buf);
        self.key2.wire_write(buf);
        self.key3.wire_write(buf);
        self.key4.wire_write(buf);
        self.level.wire_write(buf);
        self.experience.wire_write(buf);
        self.cast_time.wire_write(buf);
    }
}

// ── ClientBuffInfo ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientBuffInfo {
    pub index: i32,
    pub buff_type: BuffType,
    pub remaining_time: i64,
    pub tick_frequency: i64,
    pub stats: Option<Stats>,
    pub pause: bool,
    pub item_index: i32,
    pub extra: i32,
}
impl WireRead for ClientBuffInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            index: i32::wire_read(buf)?,
            buff_type: BuffType::wire_read(buf)?,
            remaining_time: i64::wire_read(buf)?,
            tick_frequency: i64::wire_read(buf)?,
            stats: Option::<Stats>::wire_read(buf)?,
            pause: bool::wire_read(buf)?,
            item_index: i32::wire_read(buf)?,
            extra: i32::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientBuffInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.index.wire_write(buf);
        self.buff_type.wire_write(buf);
        self.remaining_time.wire_write(buf);
        self.tick_frequency.wire_write(buf);
        self.stats.wire_write(buf);
        self.pause.wire_write(buf);
        self.item_index.wire_write(buf);
        self.extra.wire_write(buf);
    }
}

// ── ClientNPCValues ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientNpcValues {
    pub id: i32,
    pub value: String,
}
impl WireRead for ClientNpcValues {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self { id: i32::wire_read(buf)?, value: String::wire_read(buf)? })
    }
}
impl WireWrite for ClientNpcValues {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.id.wire_write(buf);
        self.value.wire_write(buf);
    }
}

// ── ClientRefineInfo ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientRefineInfo {
    pub index: i32,
    pub weapon: Option<ClientUserItem>,
    pub refine_type: RefineType,
    pub quality: RefineQuality,
    pub chance: i32,
    pub max_chance: i32,
    pub ready_duration: i64,
}
impl WireRead for ClientRefineInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            index: i32::wire_read(buf)?,
            weapon: Option::<ClientUserItem>::wire_read(buf)?,
            refine_type: RefineType::wire_read(buf)?,
            quality: RefineQuality::wire_read(buf)?,
            chance: i32::wire_read(buf)?,
            max_chance: i32::wire_read(buf)?,
            ready_duration: i64::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientRefineInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.index.wire_write(buf);
        self.weapon.wire_write(buf);
        self.refine_type.wire_write(buf);
        self.quality.wire_write(buf);
        self.chance.wire_write(buf);
        self.max_chance.wire_write(buf);
        self.ready_duration.wire_write(buf);
    }
}

// ── RankInfo ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct RankInfo {
    pub rank: i32,
    pub index: i32,
    pub name: String,
    pub class: MirClass,
    pub level: i32,
    pub experience: NetDecimal,
    pub max_experience: NetDecimal,
    pub online: bool,
    pub observable: bool,
    pub rebirth: i32,
    pub rank_change: i32,
}
impl WireRead for RankInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            rank: i32::wire_read(buf)?,
            index: i32::wire_read(buf)?,
            name: String::wire_read(buf)?,
            class: MirClass::wire_read(buf)?,
            level: i32::wire_read(buf)?,
            experience: NetDecimal::wire_read(buf)?,
            max_experience: NetDecimal::wire_read(buf)?,
            online: bool::wire_read(buf)?,
            observable: bool::wire_read(buf)?,
            rebirth: i32::wire_read(buf)?,
            rank_change: i32::wire_read(buf)?,
        })
    }
}
impl WireWrite for RankInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.rank.wire_write(buf);
        self.index.wire_write(buf);
        self.name.wire_write(buf);
        self.class.wire_write(buf);
        self.level.wire_write(buf);
        self.experience.wire_write(buf);
        self.max_experience.wire_write(buf);
        self.online.wire_write(buf);
        self.observable.wire_write(buf);
        self.rebirth.wire_write(buf);
        self.rank_change.wire_write(buf);
    }
}

// ── ClientMarketPlaceInfo ─────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientMarketPlaceInfo {
    pub index: i32,
    pub item: Option<ClientUserItem>,
    pub price: i32,
    pub seller: String,
    pub message: String,
    pub is_owner: bool,
}
impl WireRead for ClientMarketPlaceInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            index: i32::wire_read(buf)?,
            item: Option::<ClientUserItem>::wire_read(buf)?,
            price: i32::wire_read(buf)?,
            seller: String::wire_read(buf)?,
            message: String::wire_read(buf)?,
            is_owner: bool::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientMarketPlaceInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.index.wire_write(buf);
        self.item.wire_write(buf);
        self.price.wire_write(buf);
        self.seller.wire_write(buf);
        self.message.wire_write(buf);
        self.is_owner.wire_write(buf);
    }
}

// ── ClientMailInfo ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientMailInfo {
    pub index: i32,
    pub opened: bool,
    pub has_item: bool,
    pub date: i64,
    pub sender: String,
    pub subject: String,
    pub message: String,
    pub gold: i32,
    pub items: Vec<ClientUserItem>,
}
impl WireRead for ClientMailInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            index: i32::wire_read(buf)?,
            opened: bool::wire_read(buf)?,
            has_item: bool::wire_read(buf)?,
            date: i64::wire_read(buf)?,
            sender: String::wire_read(buf)?,
            subject: String::wire_read(buf)?,
            message: String::wire_read(buf)?,
            gold: i32::wire_read(buf)?,
            items: read_class_list(buf)?,
        })
    }
}
impl WireWrite for ClientMailInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.index.wire_write(buf);
        self.opened.wire_write(buf);
        self.has_item.wire_write(buf);
        self.date.wire_write(buf);
        self.sender.wire_write(buf);
        self.subject.wire_write(buf);
        self.message.wire_write(buf);
        self.gold.wire_write(buf);
        write_class_list(&self.items, buf);
    }
}

// ── ClientGuildMemberInfo ─────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientGuildMemberInfo {
    pub index: i32,
    pub name: String,
    pub rank: String,
    pub total_contribution: i64,
    pub daily_contribution: i64,
    pub online: i64, // TimeSpan ticks
    pub permission: GuildPermission,
    pub object_id: u32,
}
impl WireRead for ClientGuildMemberInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            index: i32::wire_read(buf)?,
            name: String::wire_read(buf)?,
            rank: String::wire_read(buf)?,
            total_contribution: i64::wire_read(buf)?,
            daily_contribution: i64::wire_read(buf)?,
            online: i64::wire_read(buf)?,
            permission: GuildPermission::wire_read(buf)?,
            object_id: u32::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientGuildMemberInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.index.wire_write(buf);
        self.name.wire_write(buf);
        self.rank.wire_write(buf);
        self.total_contribution.wire_write(buf);
        self.daily_contribution.wire_write(buf);
        self.online.wire_write(buf);
        self.permission.wire_write(buf);
        self.object_id.wire_write(buf);
    }
}

// ── ClientGuildInfo ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientGuildInfo {
    pub guild_name: String,
    pub notice: String,
    pub member_limit: i32,
    pub guild_funds: i64,
    pub daily_growth: i64,
    pub total_contribution: i64,
    pub daily_contribution: i64,
    pub user_index: i32,
    pub storage_limit: i32,
    pub tax: i32,
    pub default_rank: String,
    pub default_permission: GuildPermission,
    pub colour: i32,
    pub flag: i32,
    pub members: Vec<ClientGuildMemberInfo>,
    pub storage: Vec<ClientUserItem>,
}
impl WireRead for ClientGuildInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            guild_name: String::wire_read(buf)?,
            notice: String::wire_read(buf)?,
            member_limit: i32::wire_read(buf)?,
            guild_funds: i64::wire_read(buf)?,
            daily_growth: i64::wire_read(buf)?,
            total_contribution: i64::wire_read(buf)?,
            daily_contribution: i64::wire_read(buf)?,
            user_index: i32::wire_read(buf)?,
            storage_limit: i32::wire_read(buf)?,
            tax: i32::wire_read(buf)?,
            default_rank: String::wire_read(buf)?,
            default_permission: GuildPermission::wire_read(buf)?,
            colour: i32::wire_read(buf)?,
            flag: i32::wire_read(buf)?,
            members: read_class_list(buf)?,
            storage: read_class_list(buf)?,
        })
    }
}
impl WireWrite for ClientGuildInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.guild_name.wire_write(buf);
        self.notice.wire_write(buf);
        self.member_limit.wire_write(buf);
        self.guild_funds.wire_write(buf);
        self.daily_growth.wire_write(buf);
        self.total_contribution.wire_write(buf);
        self.daily_contribution.wire_write(buf);
        self.user_index.wire_write(buf);
        self.storage_limit.wire_write(buf);
        self.tax.wire_write(buf);
        self.default_rank.wire_write(buf);
        self.default_permission.wire_write(buf);
        self.colour.wire_write(buf);
        self.flag.wire_write(buf);
        write_class_list(&self.members, buf);
        write_class_list(&self.storage, buf);
    }
}

// ── ClientUserQuestTask ───────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientUserQuestTask {
    pub index: i32,
    pub task_index: i32,
    pub amount: i64,
}
impl WireRead for ClientUserQuestTask {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            index: i32::wire_read(buf)?,
            task_index: i32::wire_read(buf)?,
            amount: i64::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientUserQuestTask {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.index.wire_write(buf);
        self.task_index.wire_write(buf);
        self.amount.wire_write(buf);
    }
}

// ── ClientUserQuest ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientUserQuest {
    pub index: i32,
    pub quest_index: i32,
    pub track: bool,
    pub completed: bool,
    pub selected_reward: i32,
    pub date_taken: i64,
    pub date_completed: i64,
    pub tasks: Vec<ClientUserQuestTask>,
}
impl WireRead for ClientUserQuest {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            index: i32::wire_read(buf)?,
            quest_index: i32::wire_read(buf)?,
            track: bool::wire_read(buf)?,
            completed: bool::wire_read(buf)?,
            selected_reward: i32::wire_read(buf)?,
            date_taken: i64::wire_read(buf)?,
            date_completed: i64::wire_read(buf)?,
            tasks: read_class_list(buf)?,
        })
    }
}
impl WireWrite for ClientUserQuest {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.index.wire_write(buf);
        self.quest_index.wire_write(buf);
        self.track.wire_write(buf);
        self.completed.wire_write(buf);
        self.selected_reward.wire_write(buf);
        self.date_taken.wire_write(buf);
        self.date_completed.wire_write(buf);
        write_class_list(&self.tasks, buf);
    }
}

// ── ClientUserDiscipline ──────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientUserDiscipline {
    pub info_index: i32,
    pub level: i32,
    pub experience: i64,
    pub magics: Vec<ClientUserMagic>,
}
impl WireRead for ClientUserDiscipline {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            info_index: i32::wire_read(buf)?,
            level: i32::wire_read(buf)?,
            experience: i64::wire_read(buf)?,
            magics: read_class_list(buf)?,
        })
    }
}
impl WireWrite for ClientUserDiscipline {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.info_index.wire_write(buf);
        self.level.wire_write(buf);
        self.experience.wire_write(buf);
        write_class_list(&self.magics, buf);
    }
}

// ── ClientUserCurrency ────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientUserCurrency {
    pub currency_index: i32,
    pub amount: i64,
}
impl WireRead for ClientUserCurrency {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            currency_index: i32::wire_read(buf)?,
            amount: i64::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientUserCurrency {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.currency_index.wire_write(buf);
        self.amount.wire_write(buf);
    }
}

// ── ClientUserMilestoneTask ───────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientUserMilestoneTask {
    pub info_task_index: i32,
    pub count: i64,
}
impl WireRead for ClientUserMilestoneTask {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            info_task_index: i32::wire_read(buf)?,
            count: i64::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientUserMilestoneTask {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.info_task_index.wire_write(buf);
        self.count.wire_write(buf);
    }
}

// ── ClientUserMilestone ───────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientUserMilestone {
    pub index: i32,
    pub info_index: i32,
    pub active: bool,
    pub claimed: bool,
    pub date_earned: i64,
    pub tasks: Vec<ClientUserMilestoneTask>,
}
impl WireRead for ClientUserMilestone {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            index: i32::wire_read(buf)?,
            info_index: i32::wire_read(buf)?,
            active: bool::wire_read(buf)?,
            claimed: bool::wire_read(buf)?,
            date_earned: i64::wire_read(buf)?,
            tasks: read_class_list(buf)?,
        })
    }
}
impl WireWrite for ClientUserMilestone {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.index.wire_write(buf);
        self.info_index.wire_write(buf);
        self.active.wire_write(buf);
        self.claimed.wire_write(buf);
        self.date_earned.wire_write(buf);
        write_class_list(&self.tasks, buf);
    }
}

// ── ClientUserCompanion ───────────────────────────────────────────────────
// (abbreviated — serialized properties only)

#[derive(Debug, Clone, Default)]
pub struct ClientUserCompanion {
    pub index: i32,
    pub info_index: i32,
    pub name: String,
    pub level: i32,
    pub experience: i32,
    pub hunger: i32,
    pub head_shape: i32,
    pub back_shape: i32,
    pub equipment: Vec<ClientUserItem>,
    pub inventory: Vec<ClientUserItem>,
}
impl WireRead for ClientUserCompanion {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            index: i32::wire_read(buf)?,
            info_index: i32::wire_read(buf)?,
            name: String::wire_read(buf)?,
            level: i32::wire_read(buf)?,
            experience: i32::wire_read(buf)?,
            hunger: i32::wire_read(buf)?,
            head_shape: i32::wire_read(buf)?,
            back_shape: i32::wire_read(buf)?,
            equipment: read_class_list(buf)?,
            inventory: read_class_list(buf)?,
        })
    }
}
impl WireWrite for ClientUserCompanion {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.index.wire_write(buf);
        self.info_index.wire_write(buf);
        self.name.wire_write(buf);
        self.level.wire_write(buf);
        self.experience.wire_write(buf);
        self.hunger.wire_write(buf);
        self.head_shape.wire_write(buf);
        self.back_shape.wire_write(buf);
        write_class_list(&self.equipment, buf);
        write_class_list(&self.inventory, buf);
    }
}

// ── ClientCompanionObject ─────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientCompanionObject {
    pub info_index: i32,
    pub name: String,
    pub head_shape: i32,
    pub back_shape: i32,
}
impl WireRead for ClientCompanionObject {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            info_index: i32::wire_read(buf)?,
            name: String::wire_read(buf)?,
            head_shape: i32::wire_read(buf)?,
            back_shape: i32::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientCompanionObject {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.info_index.wire_write(buf);
        self.name.wire_write(buf);
        self.head_shape.wire_write(buf);
        self.back_shape.wire_write(buf);
    }
}

// ── ClientPlayerInfo ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientPlayerInfo {
    pub index: i32,
    pub name: String,
    pub object_id: u32,
    pub online: bool,
}
impl WireRead for ClientPlayerInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            index: i32::wire_read(buf)?,
            name: String::wire_read(buf)?,
            object_id: u32::wire_read(buf)?,
            online: bool::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientPlayerInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.index.wire_write(buf);
        self.name.wire_write(buf);
        self.object_id.wire_write(buf);
        self.online.wire_write(buf);
    }
}

// ── ClientFortuneInfo ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientFortuneInfo {
    pub item_index: i32,
    pub check_time: i64,
    pub drop_count: i64,
    pub progress: NetDecimal,
}
impl WireRead for ClientFortuneInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            item_index: i32::wire_read(buf)?,
            check_time: i64::wire_read(buf)?,
            drop_count: i64::wire_read(buf)?,
            progress: NetDecimal::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientFortuneInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.item_index.wire_write(buf);
        self.check_time.wire_write(buf);
        self.drop_count.wire_write(buf);
        self.progress.wire_write(buf);
    }
}

// ── ClientLookingForGroup ─────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientLookingForGroup {
    pub leader_name: String,
    pub group_name: String,
    pub group_type: String,
    pub member_info: Vec<String>,
    pub max_count: i32,
    pub enabled: bool,
}
impl WireRead for ClientLookingForGroup {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            leader_name: String::wire_read(buf)?,
            group_name: String::wire_read(buf)?,
            group_type: String::wire_read(buf)?,
            member_info: read_prim_list(buf)?,
            max_count: i32::wire_read(buf)?,
            enabled: bool::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientLookingForGroup {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.leader_name.wire_write(buf);
        self.group_name.wire_write(buf);
        self.group_type.wire_write(buf);
        write_prim_list(&self.member_info, buf);
        self.max_count.wire_write(buf);
        self.enabled.wire_write(buf);
    }
}

// ── ClientBundleItemInfo / ClientLootBoxItemInfo ───────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClientBundleItemInfo {
    pub item_index: i32,
    pub amount: i32,
    pub slot: i32,
}
impl WireRead for ClientBundleItemInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            item_index: i32::wire_read(buf)?,
            amount: i32::wire_read(buf)?,
            slot: i32::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientBundleItemInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.item_index.wire_write(buf);
        self.amount.wire_write(buf);
        self.slot.wire_write(buf);
    }
}

#[derive(Debug, Clone, Default)]
pub struct ClientLootBoxItemInfo {
    pub item_index: i32,
    pub amount: i32,
    pub slot: i32,
}
impl WireRead for ClientLootBoxItemInfo {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            item_index: i32::wire_read(buf)?,
            amount: i32::wire_read(buf)?,
            slot: i32::wire_read(buf)?,
        })
    }
}
impl WireWrite for ClientLootBoxItemInfo {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.item_index.wire_write(buf);
        self.amount.wire_write(buf);
        self.slot.wire_write(buf);
    }
}

// ── StartInformation ──────────────────────────────────────────────────────
// (large; includes most game startup state)

#[derive(Debug, Clone, Default)]
pub struct StartInformation {
    pub index: i32,
    pub object_id: u32,
    pub name: String,
    pub caption: String,
    pub caption_outline_colour: i32,
    pub name_colour: i32,
    pub guild_name: String,
    pub guild_rank: String,
    pub class: MirClass,
    pub gender: MirGender,
    pub location: (i32, i32),
    pub direction: MirDirection,
    pub map_index: i32,
    pub instance_index: i32,
    pub level: i32,
    pub hair_type: i32,
    pub hair_colour: i32,
    pub weapon: i32,
    pub armour: i32,
    pub costume: i32,
    pub shield: i32,
    pub armour_colour: i32,
    pub armour_effect: ExteriorEffect,
    pub emblem_effect: ExteriorEffect,
    pub weapon_effect: ExteriorEffect,
    pub shield_effect: ExteriorEffect,
    pub experience: NetDecimal,
    pub current_hp: i32,
    pub current_mp: i32,
    pub current_fp: i32,
    pub attack_mode: AttackMode,
    pub pet_mode: PetMode,
    pub online_state: OnlineState,
    pub discipline: Option<ClientUserDiscipline>,
    pub hermit_points: i32,
    pub day_time: f32,
    pub time_of_day: TimeOfDay,
    pub time_of_day_label: String,
    pub allow_group: bool,
    pub allow_trade: bool,
    pub friends: Vec<ClientFriendInfo>,
    pub items: Vec<ClientUserItem>,
    pub belt_links: Vec<ClientBeltLink>,
    pub auto_potion_links: Vec<ClientAutoPotionLink>,
    pub milestones: Vec<ClientUserMilestone>,
    pub magics: Vec<ClientUserMagic>,
    pub buffs: Vec<ClientBuffInfo>,
    pub currencies: Vec<ClientUserCurrency>,
    pub poison: PoisonType,
    pub in_safe_zone: bool,
    pub observable: bool,
    pub dead: bool,
    pub horse: HorseType,
    pub helmet_shape: i32,
    pub horse_shape: i32,
    pub hide_head: bool,
    pub quests: Vec<ClientUserQuest>,
    pub companion_unlocks: Vec<i32>,
    pub companions: Vec<ClientUserCompanion>,
    pub companion: i32,
    pub storage_size: i32,
    pub filters_class: String,
    pub filters_rarity: String,
    pub filters_item_type: String,
    pub struck_enabled: bool,
    pub hermit_enabled: bool,
}

impl WireRead for StartInformation {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Self {
            index: i32::wire_read(buf)?,
            object_id: u32::wire_read(buf)?,
            name: String::wire_read(buf)?,
            caption: String::wire_read(buf)?,
            caption_outline_colour: i32::wire_read(buf)?,
            name_colour: i32::wire_read(buf)?,
            guild_name: String::wire_read(buf)?,
            guild_rank: String::wire_read(buf)?,
            class: MirClass::wire_read(buf)?,
            gender: MirGender::wire_read(buf)?,
            location: <(i32,i32)>::wire_read(buf)?,
            direction: MirDirection::wire_read(buf)?,
            map_index: i32::wire_read(buf)?,
            instance_index: i32::wire_read(buf)?,
            level: i32::wire_read(buf)?,
            hair_type: i32::wire_read(buf)?,
            hair_colour: i32::wire_read(buf)?,
            weapon: i32::wire_read(buf)?,
            armour: i32::wire_read(buf)?,
            costume: i32::wire_read(buf)?,
            shield: i32::wire_read(buf)?,
            armour_colour: i32::wire_read(buf)?,
            armour_effect: ExteriorEffect::wire_read(buf)?,
            emblem_effect: ExteriorEffect::wire_read(buf)?,
            weapon_effect: ExteriorEffect::wire_read(buf)?,
            shield_effect: ExteriorEffect::wire_read(buf)?,
            experience: NetDecimal::wire_read(buf)?,
            current_hp: i32::wire_read(buf)?,
            current_mp: i32::wire_read(buf)?,
            current_fp: i32::wire_read(buf)?,
            attack_mode: AttackMode::wire_read(buf)?,
            pet_mode: PetMode::wire_read(buf)?,
            online_state: OnlineState::wire_read(buf)?,
            discipline: Option::<ClientUserDiscipline>::wire_read(buf)?,
            hermit_points: i32::wire_read(buf)?,
            day_time: f32::wire_read(buf)?,
            time_of_day: TimeOfDay::wire_read(buf)?,
            time_of_day_label: String::wire_read(buf)?,
            allow_group: bool::wire_read(buf)?,
            allow_trade: bool::wire_read(buf)?,
            friends: read_class_list(buf)?,
            items: read_class_list(buf)?,
            belt_links: read_class_list(buf)?,
            auto_potion_links: read_class_list(buf)?,
            milestones: read_class_list(buf)?,
            magics: read_class_list(buf)?,
            buffs: read_class_list(buf)?,
            currencies: read_class_list(buf)?,
            poison: PoisonType::wire_read(buf)?,
            in_safe_zone: bool::wire_read(buf)?,
            observable: bool::wire_read(buf)?,
            dead: bool::wire_read(buf)?,
            horse: HorseType::wire_read(buf)?,
            helmet_shape: i32::wire_read(buf)?,
            horse_shape: i32::wire_read(buf)?,
            hide_head: bool::wire_read(buf)?,
            quests: read_class_list(buf)?,
            companion_unlocks: read_prim_list(buf)?,
            companions: read_class_list(buf)?,
            companion: i32::wire_read(buf)?,
            storage_size: i32::wire_read(buf)?,
            filters_class: String::wire_read(buf)?,
            filters_rarity: String::wire_read(buf)?,
            filters_item_type: String::wire_read(buf)?,
            struck_enabled: bool::wire_read(buf)?,
            hermit_enabled: bool::wire_read(buf)?,
        })
    }
}
impl WireWrite for StartInformation {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.index.wire_write(buf);
        self.object_id.wire_write(buf);
        self.name.wire_write(buf);
        self.caption.wire_write(buf);
        self.caption_outline_colour.wire_write(buf);
        self.name_colour.wire_write(buf);
        self.guild_name.wire_write(buf);
        self.guild_rank.wire_write(buf);
        self.class.wire_write(buf);
        self.gender.wire_write(buf);
        self.location.wire_write(buf);
        self.direction.wire_write(buf);
        self.map_index.wire_write(buf);
        self.instance_index.wire_write(buf);
        self.level.wire_write(buf);
        self.hair_type.wire_write(buf);
        self.hair_colour.wire_write(buf);
        self.weapon.wire_write(buf);
        self.armour.wire_write(buf);
        self.costume.wire_write(buf);
        self.shield.wire_write(buf);
        self.armour_colour.wire_write(buf);
        self.armour_effect.wire_write(buf);
        self.emblem_effect.wire_write(buf);
        self.weapon_effect.wire_write(buf);
        self.shield_effect.wire_write(buf);
        self.experience.wire_write(buf);
        self.current_hp.wire_write(buf);
        self.current_mp.wire_write(buf);
        self.current_fp.wire_write(buf);
        self.attack_mode.wire_write(buf);
        self.pet_mode.wire_write(buf);
        self.online_state.wire_write(buf);
        self.discipline.wire_write(buf);
        self.hermit_points.wire_write(buf);
        self.day_time.wire_write(buf);
        self.time_of_day.wire_write(buf);
        self.time_of_day_label.wire_write(buf);
        self.allow_group.wire_write(buf);
        self.allow_trade.wire_write(buf);
        write_class_list(&self.friends, buf);
        write_class_list(&self.items, buf);
        write_class_list(&self.belt_links, buf);
        write_class_list(&self.auto_potion_links, buf);
        write_class_list(&self.milestones, buf);
        write_class_list(&self.magics, buf);
        write_class_list(&self.buffs, buf);
        write_class_list(&self.currencies, buf);
        self.poison.wire_write(buf);
        self.in_safe_zone.wire_write(buf);
        self.observable.wire_write(buf);
        self.dead.wire_write(buf);
        self.horse.wire_write(buf);
        self.helmet_shape.wire_write(buf);
        self.horse_shape.wire_write(buf);
        self.hide_head.wire_write(buf);
        write_class_list(&self.quests, buf);
        write_prim_list(&self.companion_unlocks, buf);
        write_class_list(&self.companions, buf);
        self.companion.wire_write(buf);
        self.storage_size.wire_write(buf);
        self.filters_class.wire_write(buf);
        self.filters_rarity.wire_write(buf);
        self.filters_item_type.wire_write(buf);
        self.struck_enabled.wire_write(buf);
        self.hermit_enabled.wire_write(buf);
    }
}
