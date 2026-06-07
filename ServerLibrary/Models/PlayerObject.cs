using Library;
using Library.Network;
using Library.Network.ClientPackets;
using Library.SystemModels;
using Server.DBModels;
using Server.Envir;
using Server.Envir.Events.Triggers;
using Server.Models.Magics;
using Server.Models.Monsters;
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text.RegularExpressions;
using C = Library.Network.ClientPackets;
using S = Library.Network.ServerPackets;

namespace Server.Models
{
    public partial class PlayerObject : MapObject
    {
        public override ObjectType Race => ObjectType.Player;

        public CharacterInfo Character;
        public SConnection Connection;

        public override string Name
        {
            get { return Character.CharacterName; }
            set { Character.CharacterName = value; }
        }

        public override string Caption
        {
            get { return Character.Caption; }
            set { Character.Caption = value; }
        }
        public override int Level
        {
            get { return Character.Level; }
            set { Character.Level = value; }
        }
        public override Point CurrentLocation
        {
            get { return Character.CurrentLocation; }
            set { Character.CurrentLocation = value; }
        }

        public MirGender Gender => Character.Gender;
        public MirClass Class => Character.Class;

        public UserMilestone ActiveMilestone => Character.Milestones.FirstOrDefault(x => x.Active);

        public override int CurrentHP
        {
            get { return Character.CurrentHP; }
            set { Character.CurrentHP = value; }
        }
        public override int CurrentMP
        {
            get { return Character.CurrentMP; }
            set { Character.CurrentMP = value; }
        }

        public AttackMode AttackMode
        {
            get { return Character.AttackMode; }
            set { Character.AttackMode = value; }
        }

        public PetMode PetMode
        {
            get { return Character.PetMode; }
            set { Character.PetMode = value; }
        }

        public OnlineState OnlineState
        {
            get { return Character.OnlineState; }
            set { Character.OnlineState = value; }
        }

        public UserCurrency Gold => Character.Account.Gold;
        public UserCurrency GameGold => Character.Account.GameGold;
        public UserCurrency HuntGold => Character.Account.HuntGold;

        public decimal Experience
        {
            get { return Character.Experience; }
            set { Character.Experience = value; }
        }

        public int BagWeight, WearWeight, HandWeight;

        public int HairType
        {
            get { return Character.HairType; }
            set { Character.HairType = value; }
        }
        public Color HairColour
        {
            get { return Character.HairColour; }
            set { Character.HairColour = value; }
        }

        public override MirDirection Direction
        {
            get { return Character.Direction; }
            set { Character.Direction = value; }
        }

        public DateTime ShoutExpiry, UseItemTime, TorchTime, CombatTime, PvPTime, SentCombatTime, AutoPotionTime, AutoPotionCheckTime, ItemTime, RevivalTime, TeleportTime, DailyQuestTime, FishingCastTime, MailTime, ExperienceTime;
        public bool PacketWaiting;

        public bool GameMaster, Observer, Superman;

        public override bool Blocking => base.Blocking && !Observer;

        public NPCObject NPC;
        public NPCPage NPCPage;
        public Dictionary<string, object> NPCVals = new Dictionary<string, object>();

        public HorseType Horse;

        public bool BlockWhisper;
        public bool CompanionLevelLock3, CompanionLevelLock5, CompanionLevelLock7, CompanionLevelLock10, CompanionLevelLock11, CompanionLevelLock13, CompanionLevelLock15;
        public bool ExtractorLock;

        public override bool CanMove => base.CanMove && !Fishing;
        public override bool CanAttack => base.CanAttack && Horse == HorseType.None;
        public override bool CanCast => base.CanCast && Horse == HorseType.None && !Fishing;

        private bool HideHead
        {
            get
            {
                return Equipment[(int)EquipmentSlot.Armour]?.Info.ItemEffect == ItemEffect.FishingRobe || Equipment[(int)EquipmentSlot.Costume]?.Info != null;
            }
        }

        public List<MonsterObject> Pets = new List<MonsterObject>();

        public HashSet<MapObject> VisibleObjects = new HashSet<MapObject>();
        public HashSet<MapObject> VisibleDataObjects = new HashSet<MapObject>();
        public HashSet<MonsterObject> TaggedMonsters = new HashSet<MonsterObject>();
        public HashSet<MapObject> NearByObjects = new HashSet<MapObject>();

        public UserItem[]
            Inventory = new UserItem[Globals.InventorySize],
            Equipment = new UserItem[Globals.EquipmentSize],
            Storage = new UserItem[1000],
            PartsStorage = new UserItem[1000];

        public Companion Companion;

        public MapObject LastHitter;

        public PlayerObject GroupInvitation, GuildInvitation, MarriageInvitation;
        public HashSet<PlayerObject> GroupInvitationRequest = new();

        public PlayerObject TradePartner, TradePartnerRequest;
        public Dictionary<UserItem, CellLinkInfo> TradeItems = new Dictionary<UserItem, CellLinkInfo>();
        public bool TradeConfirmed;
        public long TradeGold;

        public MagicList MagicObjects = new MagicList();

        public List<AutoPotionLink> AutoPotions = new List<AutoPotionLink>();
        public CellLinkInfo DelayItemUse;
        public decimal MaxExperience, ExperienceAccumulated;

        public string FiltersClass;
        public string FiltersRarity;
        public string FiltersItemType;

        public bool Fishing = false, FishFound = false;
        public int FishThrowQuality = 0, FishPointsCurrent = 0, FishAttempts = 0, FishFails = 0;

        public Point FishingLocation;
        public MirDirection FishingDirection;

        public PlayerObject(CharacterInfo info, SConnection con)
        {
            Character = info;
            Connection = con;

            DisplayMP = CurrentMP;
            DisplayHP = CurrentHP;

            Character.LastStats = Stats = new Stats();

            foreach (UserItem item in Character.Account.Items)
            {
                if (item.Slot >= Globals.PartsStorageOffset)
                {
                    PartsStorage[item.Slot - Globals.PartsStorageOffset] = item;
                    continue;
                }

                Storage[item.Slot] = item;
            }

            foreach (UserItem item in Character.Items)
            {
                if (item.Slot >= Globals.EquipmentOffSet)
                {
                    Equipment[item.Slot - Globals.EquipmentOffSet] = item;
                    continue;
                }

                Inventory[item.Slot] = item;
            }

            ItemReviveTime = info.ItemReviveTime;
            ItemTime = SEnvir.Now;

            Buffs.AddRange(Character.Account.Buffs);
            Buffs.AddRange(Character.Buffs);

            AutoPotions.AddRange(Character.AutoPotionLinks);

            AutoPotions.Sort((x1, x2) => x1.Slot.CompareTo(x2.Slot));

            if (Character.Account.Admin || Character.Account.TempAdmin)
            {
                GameMaster = Config.AdminStartInGamemasterMode;
                Observer = Config.AdminStartInObserverMode;
                Superman = Config.AdminStartInSupermanMode;
            }

            FiltersClass = Character.FiltersClass ?? "";
            FiltersItemType = Character.FiltersItemType ?? "";
            FiltersRarity = Character.FiltersRarity ?? "";

            AddDefaultCurrencies();

            SetupMagic();
        }

        public MagicObject SetupMagic(UserMagic userMagic)
        {
            var type = userMagic.Info.Magic;

            var found = SEnvir.MagicTypes.FirstOrDefault(x => x.GetCustomAttribute<MagicTypeAttribute>().Type == type);

            if (found != null)
            {
                var magicObject = (MagicObject)Activator.CreateInstance(found, this, userMagic);

                MagicObjects.Add(type, magicObject);

                return magicObject;
            }

            return null;
        }

        public void SetupMagic()
        {
            foreach (UserMagic magic in Character.Magics)
            {
                if (magic.Info.School == MagicSchool.None) continue;

                var type = magic.Info.Magic;

                var found = SEnvir.MagicTypes.FirstOrDefault(x => x.GetCustomAttribute<MagicTypeAttribute>().Type == type);

                if (found != null)
                {
                    MagicObjects.Add(magic.Info.Magic, (MagicObject)Activator.CreateInstance(found, this, magic));
                }
            }
        }

        private void AddDefaultCurrencies()
        {
            foreach (var currency in SEnvir.CurrencyInfoList.Binding)
            {
                var userCurrency = Character.Account.Currencies.FirstOrDefault(x => x.Info == currency);

                if (userCurrency == null)
                {
                    userCurrency = SEnvir.UserCurrencyList.CreateNewObject();
                    userCurrency.Account = Character.Account;
                    userCurrency.Info = currency;
                }
            }
        }

        #region Process

        public override void Process()
        {
            base.Process();

            // if (LastHitter != null && LastHitter.Node == null) LastHitter = null;
            if (GroupInvitation != null && GroupInvitation.Node == null) GroupInvitation = null;
            if (GuildInvitation != null && GuildInvitation.Node == null) GuildInvitation = null;

            if (MarriageInvitation != null && MarriageInvitation.Node == null) MarriageInvitation = null;

            if (CombatTime != SentCombatTime)
            {
                SentCombatTime = CombatTime;
                Enqueue(new S.CombatTime());
            }

            if (Fishing && FishingCastTime < SEnvir.Now)
            {
                ResetFishing();
            }

            ProcessRegen();

            ProcessExperience();

            HashSet<MonsterObject> clearList = new HashSet<MonsterObject>();

            foreach (MonsterObject ob in TaggedMonsters)
            {
                if (SEnvir.Now < ob.EXPOwnerTime) continue;
                clearList.Add(ob);
            }

            foreach (MonsterObject ob in clearList)
                ob.EXPOwner = null;

            foreach (MagicType type in MagicObjects.Keys)
                MagicObjects[type].Process();

            if (Dead && SEnvir.Now >= RevivalTime)
                TownRevive();

            ProcessTorch();

            ProcessAutoPotion();

            ProcessItemExpire();

            ProcessQuests();

            ProcessGroup();
        }
        public override void ProcessAction(DelayedAction action)
        {
            MapObject ob;
            MagicType type;

            switch (action.Type)
            {
                case ActionType.Turn:
                    PacketWaiting = false;
                    Turn((MirDirection)action.Data[0]);
                    return;
                case ActionType.Harvest:
                    PacketWaiting = false;
                    Harvest((MirDirection)action.Data[0]);
                    return;
                case ActionType.Move:
                    PacketWaiting = false;
                    Move((MirDirection)action.Data[0], (int)action.Data[1]);
                    return;
                case ActionType.Magic:
                    PacketWaiting = false;
                    Magic((C.Magic)action.Data[0]);
                    return;
                case ActionType.Mining:
                    PacketWaiting = false;
                    Mining((MirDirection)action.Data[0]);
                    return;
                case ActionType.Fishing:
                    PacketWaiting = false;
                    FishingCast((FishingState)action.Data[0], (MirDirection)action.Data[1], (Point)action.Data[2], (bool)action.Data[3]);
                    break;
                case ActionType.Attack:
                    PacketWaiting = false;
                    Attack((MirDirection)action.Data[0], (MagicType)action.Data[1]);
                    return;
                case ActionType.RangeAttack:
                    PacketWaiting = false;
                    RangeAttack((MirDirection)action.Data[0], (uint)action.Data[1]);
                    return;
                case ActionType.DelayAttack:
                    Attack((MapObject)action.Data[0], (List<MagicType>)action.Data[1], (bool)action.Data[2], (int)action.Data[3]);
                    return;
                case ActionType.DelayMagic:
                    {
                        type = (MagicType)action.Data[0];

                        if (GetMagic(type, out MagicObject magicObject))
                        {
                            magicObject.MagicComplete(action.Data);
                        }
                    }
                    return;
                case ActionType.DelayedAttackDamage:
                    {
                        ob = (MapObject)action.Data[0];

                        if (!CanAttackTarget(ob)) return;

                        ob.Attacked(this, (int)action.Data[1], (Element)action.Data[2], (bool)action.Data[3], (bool)action.Data[4], (bool)action.Data[5], (bool)action.Data[6]);
                    }
                    return;
                case ActionType.DelayedMagicDamage:
                    {
                        ob = (MapObject)action.Data[1];

                        if (!CanAttackTarget(ob)) return;

                        MagicAttack((List<MagicType>)action.Data[0], ob, (bool)action.Data[2], (Stats)action.Data[3], (int)action.Data[4]);
                    }
                    return;
                case ActionType.Mount:
                    PacketWaiting = false;
                    Mount();
                    break;
            }

            base.ProcessAction(action);
        }

        public void ProcessTorch()
        {
            if (SEnvir.Now <= TorchTime || InSafeZone) return;

            TorchTime = SEnvir.Now.AddSeconds(10);

            DamageItem(GridType.Equipment, (int)EquipmentSlot.Torch, Config.TorchRate);

            UserItem torch = Equipment[(int)EquipmentSlot.Torch];
            if (torch == null || torch.CurrentDurability != 0 || torch.Info.Durability <= 0) return;

            RemoveItem(torch);
            Equipment[(int)EquipmentSlot.Torch] = null;
            torch.Delete();

            RefreshWeight();

            Enqueue(new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = GridType.Equipment, Slot = (int)EquipmentSlot.Torch },
                Success = true,
            });
        }

        public void ProcessRegen()
        {
            if (Dead || SEnvir.Now < RegenTime) return;

            RegenTime = SEnvir.Now + RegenDelay;

            if ((Poison & PoisonType.Hemorrhage) == PoisonType.Hemorrhage) return;

            float rate = 2; //2%

            if (Class == MirClass.Wizard) rate += 1;

            if (GetMagic(MagicType.Rejuvenation, out Rejuvenation rejuvenation))
            {
                rate += 0.5F + rejuvenation.Magic.Level * 0.5F;

                if (CurrentHP < Stats[Stat.Health] || CurrentMP < Stats[Stat.Mana])
                    LevelMagic(rejuvenation.Magic);
            }

            if (GetMagic(MagicType.Vitality, out Vitality vitality))
            {
                if (vitality.LowHP)
                {
                    rate += 0.5F + vitality.Magic.Level * 0.5F;

                    LevelMagic(vitality.Magic);
                }
            }

            rate /= 100F;

            if (CurrentHP < Stats[Stat.Health])
            {
                int regen = (int)Math.Max(1, Stats[Stat.Health] * rate);

                ChangeHP(regen);
            }

            if (CurrentMP < Stats[Stat.Mana])
            {
                int regen = (int)Math.Max(1, Stats[Stat.Mana] * rate);

                ChangeMP(regen);
            }

            if (CurrentFP < Stats[Stat.Focus])
            {
                int regen = (int)Math.Max(1, Stats[Stat.Focus] * rate);

                ChangeFP(regen);
            }
        }

        public void ProcessAutoPotion()
        {
            if (SEnvir.Now < UseItemTime || Buffs.Any(x => x.Type == BuffType.Cloak || x.Type == BuffType.Transparency || x.Type == BuffType.DragonRepulse)) return; //Can't auto Pot

            if (DelayItemUse != null)
            {
                ItemUse(DelayItemUse);
                DelayItemUse = null;
                return;
            }

            if (Dead) return;

            foreach (AutoPotionLink link in AutoPotions)
            {
                if (!link.Enabled) continue;

                if (CurrentHP > link.Health && link.Health > 0) continue;
                if (CurrentMP > link.Mana && link.Mana > 0) continue;

                if (link.Health == 0 && link.Mana == 0) continue;

                for (int i = 0; i < Inventory.Length; i++)
                {
                    if (Inventory[i] == null || Inventory[i].Info.Index != link.LinkInfoIndex) continue;

                    if ((Inventory[i].Info.Stats[Stat.Health] == 0 || CurrentHP == Stats[Stat.Health]) &&
                        (Inventory[i].Info.Stats[Stat.Mana] == 0 || CurrentMP == Stats[Stat.Mana])) continue;

                    if (SEnvir.Now < AutoPotionCheckTime) return;

                    ItemUse(new CellLinkInfo { GridType = GridType.Inventory, Count = 1, Slot = i });
                    AutoPotionTime = UseItemTime;
                    AutoPotionCheckTime = UseItemTime;
                    return;
                }

                if (Companion == null) continue;

                for (int i = 0; i < Companion.Inventory.Length; i++)
                {
                    if (i >= Companion.Stats[Stat.CompanionInventory]) break;

                    if (Companion.Inventory[i] == null || Companion.Inventory[i].Info.Index != link.LinkInfoIndex) continue;

                    if ((Companion.Inventory[i].Info.Stats[Stat.Health] == 0 || CurrentHP == Stats[Stat.Health]) &&
                        (Companion.Inventory[i].Info.Stats[Stat.Mana] == 0 || CurrentMP == Stats[Stat.Mana])) continue;

                    if (SEnvir.Now < AutoPotionCheckTime) return;

                    ItemUse(new CellLinkInfo { GridType = GridType.CompanionInventory, Count = 1, Slot = i });
                    AutoPotionTime = UseItemTime;
                    AutoPotionCheckTime = UseItemTime;
                    return;
                }

            }

            AutoPotionCheckTime = SEnvir.Now.AddMilliseconds(200);
        }

        public void ProcessItemExpire()
        {
            if (ItemTime.AddSeconds(1) > SEnvir.Now) return;

            TimeSpan ticks = SEnvir.Now - ItemTime;
            ItemTime = SEnvir.Now;

            if (InSafeZone) return;
            bool refresh = false;

            for (int i = 0; i < Equipment.Length; i++)
            {
                UserItem item = Equipment[i];
                if (item == null) continue;
                if ((item.Flags & UserItemFlags.Expirable) != UserItemFlags.Expirable) continue;

                item.ExpireTime -= ticks;

                if (item.ExpireTime > TimeSpan.Zero) continue;

                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.Expired, item.Info.ItemName), MessageType.System);

                RemoveItem(item);
                Equipment[i] = null;
                item.Delete();

                refresh = true;

                Enqueue(new S.ItemChanged
                {
                    Link = new CellLinkInfo { GridType = GridType.Equipment, Slot = i },
                    Success = true,
                });
            }


            for (int i = 0; i < Inventory.Length; i++)
            {
                UserItem item = Inventory[i];
                if (item == null) continue;
                if ((item.Flags & UserItemFlags.Expirable) != UserItemFlags.Expirable) continue;


                item.ExpireTime -= ticks;

                if (item.ExpireTime > TimeSpan.Zero) continue;

                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.Expired, item.Info.ItemName), MessageType.System);

                RemoveItem(item);
                Inventory[i] = null;
                item.Delete();

                refresh = true;

                Enqueue(new S.ItemChanged
                {
                    Link = new CellLinkInfo { GridType = GridType.Inventory, Slot = i },
                    Success = true,
                });
            }

            if (Companion != null)
            {
                for (int i = 0; i < Companion.Inventory.Length; i++)
                {
                    UserItem item = Companion.Inventory[i];
                    if (item == null) continue;
                    if ((item.Flags & UserItemFlags.Expirable) != UserItemFlags.Expirable) continue;


                    item.ExpireTime -= ticks;

                    if (item.ExpireTime > TimeSpan.Zero) continue;

                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.Expired, item.Info.ItemName), MessageType.System);

                    RemoveItem(item);
                    Companion.Inventory[i] = null;
                    item.Delete();

                    refresh = true;

                    Enqueue(new S.ItemChanged
                    {
                        Link = new CellLinkInfo { GridType = GridType.CompanionInventory, Slot = i },
                        Success = true,
                    });
                }
                for (int i = 0; i < Companion.Equipment.Length; i++)
                {
                    UserItem item = Companion.Equipment[i];
                    if (item == null) continue;
                    if ((item.Flags & UserItemFlags.Expirable) != UserItemFlags.Expirable) continue;


                    item.ExpireTime -= ticks;

                    if (item.ExpireTime > TimeSpan.Zero) continue;

                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.Expired, item.Info.ItemName), MessageType.System);

                    RemoveItem(item);
                    Companion.Equipment[i] = null;
                    item.Delete();

                    refresh = true;

                    Enqueue(new S.ItemChanged
                    {
                        Link = new CellLinkInfo { GridType = GridType.CompanionEquipment, Slot = i },
                        Success = true,
                    });
                }
            }


            if (refresh)
                RefreshStats();
        }

        public void ProcessQuests()
        {
            if (SEnvir.Now <= DailyQuestTime) return;

            DailyQuestTime = SEnvir.Now.AddSeconds(20);

            for (int i = Character.Quests.Count - 1; i >= 0; i--)
            {
                bool cancel = false;

                var quest = Character.Quests[i];

                switch (quest.QuestInfo.QuestType)
                {
                    case QuestType.Daily:
                        {
                            if (quest.Completed && quest.DateCompleted.Date != DateTime.UtcNow.Date)
                            {
                                Character.Quests.RemoveAt(i);
                                cancel = true;
                            }
                        }
                        break;
                    case QuestType.Weekly:
                        {
                            CultureInfo cul = CultureInfo.CurrentCulture;

                            if (quest.Completed &&
                                cul.Calendar.GetWeekOfYear(quest.DateCompleted.Date, CalendarWeekRule.FirstDay, DayOfWeek.Monday) != cul.Calendar.GetWeekOfYear(DateTime.UtcNow.Date, CalendarWeekRule.FirstDay, DayOfWeek.Monday))
                            {
                                Character.Quests.RemoveAt(i);
                                cancel = true;
                            }
                        }
                        break;
                    case QuestType.Repeatable:
                        {
                            if (quest.Completed)
                            {
                                Character.Quests.RemoveAt(i);
                                cancel = true;
                            }
                        }
                        break;
                }

                if (cancel)
                {
                    Enqueue(new S.QuestCancelled { Index = quest.Index });
                }
            }
        }

        public void ProcessExperience(bool force = false)
        {
            if (ExperienceAccumulated > 0 && (ExperienceTime < SEnvir.Now || force))
            {
                Enqueue(new S.GainedExperience { Amount = ExperienceAccumulated });
                ExperienceTime = SEnvir.Now.AddSeconds(1);
                ExperienceAccumulated = 0;
            }
        }

        public override void ProcessNameColour()
        {
            NameColour = Color.White;

            if (Stats[Stat.Rebirth] > 0)
                NameColour = Color.DeepPink;


            if (Stats[Stat.PKPoint] >= Config.RedPoint)
                NameColour = Globals.RedNameColour;
            else if (Stats[Stat.Brown] > 0)
                NameColour = Globals.BrownNameColour;
            else if (Stats[Stat.PKPoint] >= 50)
                NameColour = Color.Yellow;
        }

        #endregion

        private StartInformation GetStartInformation(bool observer = false)
        {
            List<ClientBeltLink> blinks = new List<ClientBeltLink>();

            foreach (CharacterBeltLink link in Character.BeltLinks)
            {
                if (link.LinkItemIndex > 0 && Inventory.FirstOrDefault(x => x?.Index == link.LinkItemIndex) == null)
                    link.LinkItemIndex = -1;

                blinks.Add(link.ToClientInfo());
            }

            List<ClientAutoPotionLink> alinks = new List<ClientAutoPotionLink>();

            foreach (AutoPotionLink link in Character.AutoPotionLinks)
                alinks.Add(link.ToClientInfo());

            return new StartInformation
            {
                Index = Character.Index,
                ObjectID = ObjectID,
                Name = Name,
                Caption = ActiveMilestone?.Info.Title ?? Character.Caption,
                CaptionOutlineColour = ActiveMilestone?.Info.OutlineColour ?? Color.Black,
                GuildName = Character.Account.GuildMember?.Guild.GuildName,
                GuildRank = Character.Account.GuildMember?.Rank,
                NameColour = NameColour,

                Level = Level,
                Class = Class,
                Gender = Gender,
                Location = CurrentLocation,
                Direction = Direction,

                MapIndex = CurrentMap.Info.Index,
                InstanceIndex = CurrentMap.Instance?.Index ?? -1,

                HairType = HairType,
                HairColour = HairColour,

                Weapon = Equipment[(int)EquipmentSlot.Weapon]?.Info.Shape ?? -1,

                Shield = Equipment[(int)EquipmentSlot.Shield]?.Info.Shape ?? -1,

                Armour = Equipment[(int)EquipmentSlot.Armour]?.Info.Shape ?? 0,
                ArmourColour = Equipment[(int)EquipmentSlot.Armour]?.Colour ?? Color.Empty,

                Costume = Equipment[(int)EquipmentSlot.Costume]?.Info.Shape ?? -1,

                ArmourEffect = Equipment[(int)EquipmentSlot.Armour]?.Info.ExteriorEffect ?? 0,
                EmblemEffect = Equipment[(int)EquipmentSlot.Emblem]?.Info.ExteriorEffect ?? 0,
                WeaponEffect = Equipment[(int)EquipmentSlot.Weapon]?.Info.ExteriorEffect ?? 0,
                ShieldEffect = Equipment[(int)EquipmentSlot.Shield]?.Info.ExteriorEffect ?? 0,

                Experience = Experience,

                DayTime = SEnvir.DayTime,
                TimeOfDay = SEnvir.TimeOfDay,
                TimeOfDayLabel = SEnvir.GetDayCycleLabel(),

                AllowGroup = Character.Account.AllowGroup,
                AllowTrade = Character.Account.AllowTrade,

                CurrentHP = DisplayHP,
                CurrentMP = DisplayMP,
                CurrentFP = DisplayFP,

                AttackMode = AttackMode,
                PetMode = PetMode,

                OnlineState = OnlineState,
                Friends = Character.Friends.Select(x => x.ToClientInfo()).ToList(),

                Discipline = Character.Discipline?.ToClientInfo(),

                Items = Character.Items.Select(x => x.ToClientInfo()).ToList(),
                BeltLinks = blinks,
                AutoPotionLinks = alinks,
                Milestones = GetClientUserMilestones(),
                Magics = Character.Magics.Select(x => x.ToClientInfo()).ToList(),
                Buffs = Buffs.Select(x => x.ToClientInfo()).ToList(),
                Currencies = Character.Account.Currencies.Select(x => x.ToClientInfo(x.Info.Type == CurrencyType.GameGold && observer)).ToList(),

                Poison = Poison,

                InSafeZone = InSafeZone,

                Observable = Character.Observable,
                HermitPoints = Math.Max(0, Level - 39 - Character.SpentPoints),

                Dead = Dead,

                Horse = Horse,

                HelmetShape = Character.HideHelmet ? 0 : Equipment[(int)EquipmentSlot.Helmet]?.Info.Shape ?? 0,

                HideHead = HideHead,

                HorseShape = Equipment[(int)EquipmentSlot.HorseArmour]?.Info.Shape ?? 0,

                Quests = Quests.Select(x => x.ToClientInfo()).ToList(),

                CompanionUnlocks = Character.Account.CompanionUnlocks.Select(x => x.CompanionInfo.Index).ToList(),

                Companions = Character.Account.Companions.Select(x => x.ToClientInfo()).ToList(),

                Companion = Character.Companion?.Index ?? 0,

                StorageSize = Character.Account.StorageSize,

                FiltersClass = Character.FiltersClass,
                FiltersRarity = Character.FiltersRarity,
                FiltersItemType = Character.FiltersItemType,

                StruckEnabled = Config.EnableStruck,
                HermitEnabled = Config.EnableHermit
            };
        }

        public void StartGame()
        {
            if (!SetBindPoint())
            {
                SEnvir.Log($"[Failed to spawn Character] Index: {Character.Index}, Name: {Character.CharacterName}, Failed to reset bind point.");
                Enqueue(new S.StartGame { Result = StartGameResult.UnableToSpawn });
                Connection = null;
                Character = null;
                return;
            }

            if (Character.CurrentInstance != null)
            {
                if (Character.CurrentInstance.ReconnectRegion != null && Spawn(Character.CurrentInstance.ReconnectRegion, null, 0))
                {
                    return;
                }

                if (Character.CurrentMap.ReconnectMap != null)
                {
                    var reconnectMap = SEnvir.GetMap(Character.CurrentMap.ReconnectMap);
                    if (Spawn(reconnectMap, reconnectMap.GetRandomLocation()))
                    {
                        return;
                    }
                }

                if (Spawn(Character.BindPoint.BindRegion, null, 0))
                {
                    return;
                }
                else
                {
                    SEnvir.Log($"[Failed to spawn Character] Index: {Character.Index}, Name: {Character.CharacterName}");
                    Enqueue(new S.StartGame { Result = StartGameResult.UnableToSpawn });
                    Connection = null;
                    Character = null;
                    return;
                }
            }
            else if (!Spawn(Character.CurrentMap, null, 0, CurrentLocation) && !Spawn(Character.BindPoint.BindRegion, null, 0))
            {
                SEnvir.Log($"[Failed to spawn Character] Index: {Character.Index}, Name: {Character.CharacterName}");
                Enqueue(new S.StartGame { Result = StartGameResult.UnableToSpawn });
                Connection = null;
                Character = null;
                return;
            }

            UpdateOnlineState(true);
        }

        public void StopGame()
        {
            Character.LastLogin = SEnvir.Now;

            if (Character.Account.GuildMember != null)
            {
                foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                {
                    if (member == Character.Account.GuildMember || member.Account.Connection?.Player == null) continue;

                    member.Account.Connection.Enqueue(new S.GuildMemberOffline { Index = Character.Account.GuildMember.Index, ObserverPacket = false });
                }
            }

            TradeClose();

            BuffRemove(BuffType.DragonRepulse);
            BuffRemove(BuffType.Developer);
            BuffRemove(BuffType.Ranking);
            BuffRemove(BuffType.Castle);
            BuffRemove(BuffType.Veteran);
            BuffRemove(BuffType.ElementalHurricane);
            BuffRemove(BuffType.SuperiorMagicShield);
            BuffRemove(BuffType.ElementalSwords);

            SEnvir.EventLogs.RemoveAll(x => x.PlayerIndex == Character.Index);

            if (GroupMembers != null) GroupLeave();

            UpdateLFGStatus(false);

            HashSet<MonsterObject> clearList = new HashSet<MonsterObject>(TaggedMonsters);

            foreach (MonsterObject ob in clearList)
                ob.EXPOwner = null;

            TaggedMonsters.Clear();

            for (int i = SpellList.Count - 1; i >= 0; i--)
                SpellList[i].Despawn();
            SpellList.Clear();

            for (int i = Pets.Count - 1; i >= 0; i--)
                Pets[i].Despawn();
            Pets.Clear();

            for (int i = Connection.Observers.Count - 1; i >= 0; i--)
                Connection.Observers[i].EndObservation();
            Connection.Observers.Clear();

            CompanionDespawn();

            if (Character.Partner?.Player != null)
                Character.Partner.Player.Enqueue(new S.MarriageOnlineChanged());

            Despawn();

            Connection.Player = null;
            Character.Player = null;

            UpdateOnlineState(true);

            Connection = null;
            Character = null;
        }

        protected override void OnSpawned()
        {
            base.OnSpawned();

            Character.LastLogin = SEnvir.Now;

            SEnvir.Players.Add(this);

            Character.Account.LastCharacter = Character;

            Character.Player = this;
            Connection.Player = this;
            Connection.Stage = GameStage.Game;

            ShoutExpiry = SEnvir.Now.AddSeconds(10);

            //Broadcast Appearance(?)

            Enqueue(new S.StartGame { Result = StartGameResult.Success, StartInformation = GetStartInformation() });
            //Send Items

            Connection.ReceiveChatWithObservers(con => con.Language.Welcome, MessageType.Announcement);

            SendGuildInfo();

            if (Level > 0)
            {
                RefreshStats();

                if (CurrentHP <= 0)
                {
                    Dead = true;
                    TownRevive();
                }
                Enqueue(new S.InformMaxExperience { MaxExperience = MaxExperience });
            }

            if (Character.Account.GuildMember != null)
            {
                foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                {
                    if (member == Character.Account.GuildMember || member.Account.Connection?.Player == null) continue;

                    member.Account.Connection.Enqueue(new S.GuildMemberOnline
                    {
                        Index = Character.Account.GuildMember.Index,
                        Name = Name,
                        ObjectID = ObjectID,
                        ObserverPacket = false
                    });
                }
            }

            AddAllObjects();

            if (Level == 0)
                NewCharacter();

            foreach (var key in MagicObjects.Keys)
            {
                MagicObjects[key].RefreshToggle();
            }

            List<ClientRefineInfo> refines = new List<ClientRefineInfo>();

            foreach (RefineInfo info in Character.Refines)
                refines.Add(info.ToClientInfo());

            if (refines.Count > 0)
                Enqueue(new S.RefineList { List = refines });

            Enqueue(new S.MarketPlaceConsign { Consignments = Character.Account.Auctions.Select(x => x.ToClientInfo(Character.Account)).ToList(), ObserverPacket = false });

            Enqueue(new S.MailList { Mail = Character.Account.Mail.Select(x => x.ToClientInfo()).ToList() });


            if (Character.Account.Characters.Max(x => x.Level) > Level && Character.Rebirth == 0)
                BuffAdd(BuffType.Veteran, TimeSpan.MaxValue, new Stats { [Stat.ExperienceRate] = 50 }, false, false, TimeSpan.Zero);

            Map map = SEnvir.GetMap(CurrentMap.Info.ReconnectMap);

            if (map != null && !InSafeZone)
                Teleport(map, map.GetRandomLocation());

            UpdateReviveTimers(Connection);

            CompanionSpawn();

            Enqueue(GetMarriageInfo());

            if (Character.Partner?.Player != null)
                Character.Partner.Player.Enqueue(new S.MarriageOnlineChanged { ObjectID = ObjectID });

            ApplyMapBuff();
            ApplyServerBuff();
            ApplyCastleBuff();
            ApplyGuildBuff();
            ApplyObserverBuff();
            ApplyFameBuff();

            PauseBuffs();

            SendLFGList();

            if (SEnvir.TopRankings.Contains(Character))
                BuffAdd(BuffType.Ranking, TimeSpan.MaxValue, null, true, false, TimeSpan.Zero);

            if (Character.Account.Admin)
                BuffAdd(BuffType.Developer, TimeSpan.MaxValue, null, true, false, TimeSpan.Zero);

            Enqueue(new S.HelmetToggle { HideHelmet = Character.HideHelmet });

            //Send War Date to guild.
            foreach (CastleInfo castle in SEnvir.CastleInfoList.Binding)
            {
                GuildInfo ownerGuild = SEnvir.GuildInfoList.Binding.FirstOrDefault(x => x.Castle == castle);

                Enqueue(new S.GuildCastleInfo { Index = castle.Index, Owner = ownerGuild?.GuildName ?? String.Empty, ObserverPacket = false });
            }

            foreach (ConquestWar conquest in SEnvir.ConquestWars)
                Enqueue(new S.GuildConquestStarted { Index = conquest.Castle.Index });


            Enqueue(new S.FortuneUpdate { Fortunes = Character.Account.Fortunes.Select(x => x.ToClientInfo()).ToList() });
        }
        public void SetUpObserver(SConnection con)
        {
            con.Stage = GameStage.Observer;
            con.Observed = Connection;
            Connection.Observers.Add(con);

            con.Enqueue(new S.StartObserver
            {
                StartInformation = GetStartInformation(true),

                Items = Character.Account.Items.Select(x => x.ToClientInfo()).ToList(),
            });

            if (Level > 0)
                con.Enqueue(new S.InformMaxExperience { MaxExperience = MaxExperience });

            //Send Items

            foreach (MapObject ob in VisibleObjects)
            {
                if (ob == this) continue;

                con.Enqueue(ob.GetInfoPacket(this));
            }

            List<ClientRefineInfo> refines = new List<ClientRefineInfo>();

            foreach (RefineInfo info in Character.Refines)
                refines.Add(info.ToClientInfo());

            if (refines.Count > 0)
                con.Enqueue(new S.RefineList { List = refines });


            con.Enqueue(new S.StatsUpdate { Stats = Stats, HermitStats = Config.EnableHermit ? Character.HermitStats : new Stats(), HermitPoints = Math.Max(0, Level - 39 - Character.SpentPoints) });

            con.Enqueue(new S.WeightUpdate { BagWeight = BagWeight, WearWeight = WearWeight, HandWeight = HandWeight });

            HuntGoldChanged();

            if (TradePartner != null)
            {
                con.Enqueue(new S.TradeOpen { Name = TradePartner.Name });

                if (TradeGold > 0)
                    con.Enqueue(new S.TradeAddGold { Gold = TradeGold });

                foreach (KeyValuePair<UserItem, CellLinkInfo> pair in TradeItems)
                    con.Enqueue(new S.TradeAddItem { Cell = pair.Value, Success = true });

                if (TradePartner.TradeGold > 0)
                    con.Enqueue(new S.TradeGoldAdded { Gold = TradePartner.TradeGold });

                foreach (KeyValuePair<UserItem, CellLinkInfo> pair in TradePartner.TradeItems)
                {
                    S.TradeItemAdded packet = new S.TradeItemAdded
                    {
                        Item = pair.Key.ToClientInfo()
                    };
                    packet.Item.Count = pair.Value.Count;
                    con.Enqueue(packet);
                }
            }

            if (NPCPage != null)
                con.Enqueue(new S.NPCResponse { ObjectID = NPC.ObjectID, Index = NPCPage.Index });


            UpdateReviveTimers(con);

            if (Companion != null)
                con.Enqueue(new S.CompanionWeightUpdate { BagWeight = Companion.BagWeight, MaxBagWeight = Companion.Stats[Stat.CompanionBagWeight], InventorySize = Companion.Stats[Stat.CompanionInventory] });

            con.Enqueue(GetMarriageInfo());

            foreach (MapObject ob in VisibleDataObjects)
            {
                // if (ob.Race == ObjectType.Player) continue;

                con.Enqueue(ob.GetDataPacket(this));
            }

            if (GroupMembers != null)
                foreach (PlayerObject ob in GroupMembers)
                    con.Enqueue(new S.GroupMember { ObjectID = ob.ObjectID, Name = ob.Name });

            con.ReceiveChatWithObservers(c => string.Format(c.Language.WelcomeObserver, Name), MessageType.Announcement);

            if (Character.Account.GuildMember != null)
                foreach (GuildWarInfo warInfo in SEnvir.GuildWarInfoList.Binding)
                {
                    if (warInfo.Guild1 == Character.Account.GuildMember.Guild)
                        con.Enqueue(new S.GuildWarStarted { GuildName = warInfo.Guild2.GuildName, Duration = warInfo.Duration });

                    if (warInfo.Guild2 == Character.Account.GuildMember.Guild)
                        con.Enqueue(new S.GuildWarStarted { GuildName = warInfo.Guild1.GuildName, Duration = warInfo.Duration });
                }

            foreach (CastleInfo castle in SEnvir.CastleInfoList.Binding)
            {
                GuildInfo ownerGuild = SEnvir.GuildInfoList.Binding.FirstOrDefault(x => x.Castle == castle);

                con.Enqueue(new S.GuildCastleInfo { Index = castle.Index, Owner = ownerGuild?.GuildName ?? String.Empty });
            }

            foreach (ConquestWar conquest in SEnvir.ConquestWars)
                Enqueue(new S.GuildConquestStarted { Index = conquest.Castle.Index });

            con.Enqueue(new S.FortuneUpdate { Fortunes = Character.Account.Fortunes.Select(x => x.ToClientInfo()).ToList() });
        }

        public void ObservableSwitch(bool allow)
        {
            if (allow == Character.Observable) return;

            if (!InSafeZone)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.ObserverChangeFail, MessageType.System);
                return;
            }

            Character.Observable = allow;
            Enqueue(new S.ObservableSwitch { Allow = Character.Observable, ObserverPacket = false });

            for (int i = Connection.Observers.Count - 1; i >= 0; i--)
            {
                if (Connection.Observers[i].Account != null && (Connection.Observers[i].Account.Observer || Connection.Observers[i].Account.TempAdmin)) continue;

                Connection.Observers[i].EndObservation();
            }

            ApplyObserverBuff();
        }

        private void NewCharacter()
        {
            Level = 1;
            LevelUp();

            foreach (ItemInfo info in SEnvir.ItemInfoList.Binding)
            {
                if (!info.StartItem) continue;

                if (!CanStartWith(info)) continue;

                ItemCheck check = new ItemCheck(info, 1, UserItemFlags.Bound | UserItemFlags.Worthless, TimeSpan.Zero);

                if (CanGainItems(false, check))
                {
                    UserItem item = SEnvir.CreateFreshItem(check);

                    if (info.ItemType == ItemType.Armour)
                        item.Colour = Character.ArmourColour;

                    GainItem(item);
                }
            }

            RefreshStats();

            SetHP(Stats[Stat.Health]);
            SetMP(Stats[Stat.Mana]);

            Direction = MirDirection.Down;
        }
        private bool SetBindPoint()
        {
            if (Character.BindPoint != null && SEnvir.EnsureSafeZoneBindPoints(Character.BindPoint))
                return true;

            List<SafeZoneInfo> spawnPoints = new List<SafeZoneInfo>();

            foreach (SafeZoneInfo info in SEnvir.SafeZoneInfoList.Binding)
            {
                if (info.ValidBindPoints.Count == 0) continue;

                switch (Class)
                {
                    case MirClass.Warrior:
                        if ((info.StartClass & RequiredClass.Warrior) != RequiredClass.Warrior) continue;
                        break;
                    case MirClass.Wizard:
                        if ((info.StartClass & RequiredClass.Wizard) != RequiredClass.Wizard) continue;
                        break;
                    case MirClass.Taoist:
                        if ((info.StartClass & RequiredClass.Taoist) != RequiredClass.Taoist) continue;
                        break;
                    case MirClass.Assassin:
                        if ((info.StartClass & RequiredClass.Assassin) != RequiredClass.Assassin) continue;
                        break;
                }

                spawnPoints.Add(info);
            }

            if (spawnPoints.Count > 0)
                Character.BindPoint = spawnPoints[SEnvir.Random.Next(spawnPoints.Count)];

            return Character.BindPoint != null;
        }
        public void TownRevive()
        {
            if (!Dead) return;

            Map bindMap = SEnvir.GetMap(Character.BindPoint.BindRegion.Map);
            if (bindMap == null) return;

            Cell cell = bindMap.GetCell(Character.BindPoint.ValidBindPoints[SEnvir.Random.Next(Character.BindPoint.ValidBindPoints.Count)]);

            CurrentCell = cell.GetMovement(this);

            RemoveAllObjects();

            AddAllObjects();

            Dead = false;
            SetHP(Stats[Stat.Health]);
            SetMP(Stats[Stat.Mana]);
            SetFP(0);

            Broadcast(new S.ObjectRevive { ObjectID = ObjectID, Location = CurrentLocation, Effect = true });
        }

        protected override void OnMapChanged()
        {
            base.OnMapChanged();

            if (CurrentMap == null) return;

            Character.CurrentMap = CurrentMap.Info;
            Character.CurrentInstance = CurrentMap.Instance;

            if (!Spawned) return;

            for (int i = SpellList.Count - 1; i >= 0; i--)
                if (SpellList[i].CurrentMap != CurrentMap)
                    SpellList[i].Despawn();

            Enqueue(new S.MapChanged
            {
                MapIndex = CurrentMap.Info.Index,
                InstanceIndex = CurrentMap.Instance?.Index ?? -1
            });

            if (!CurrentMap.Info.CanHorse)
                RemoveMount();

            ApplyMapBuff();

            if (PlayerMoveMap.QuickCheck(this))
            {
                SEnvir.EventHandler.Process(this, "PLAYERMOVEMAP");
            }
        }

        protected override void OnLocationChanged()
        {
            base.OnLocationChanged();

            TradeClose();

            if (CurrentCell == null) return;

            if (Companion != null)
                Companion.SearchTime = DateTime.MinValue;

            for (int i = SpellList.Count - 1; i >= 0; i--)
                if (SpellList[i].CurrentMap != CurrentMap || !Functions.InRange(SpellList[i].DisplayLocation, CurrentLocation, Config.MaxViewRange))
                    SpellList[i].Despawn();

            UpdateBindPoint(CurrentCell.SafeZone);

            if (InSafeZone != (CurrentCell.SafeZone != null))
            {
                InSafeZone = CurrentCell.SafeZone != null;

                if (!Spawned) return;

                Enqueue(new S.SafeZoneChanged { InSafeZone = InSafeZone });
                PauseBuffs();
            }
            else if (Spawned && CurrentMap.Info.CanMine)
                PauseBuffs();

            if (PreviousCell != null && 
                PreviousCell.Map != CurrentCell.Map && 
                PreviousCell.Map.InstanceExpiry != CurrentCell.Map.InstanceExpiry)
            {
                // Show instance timer, or reset timer when instance is null
                if (CurrentMap.Instance == null || CurrentMap.Instance.ShowTimer)
                    SetTimer("Map", CurrentMap.InstanceExpiry);
            }

            if (PlayerMoveRegion.QuickCheck(this))
            {
                SEnvir.EventHandler.Process(this, "PLAYERMOVEREGION");
            }
        }

        public override void OnDespawned()
        {
            base.OnDespawned();

            SEnvir.Players.Remove(this);
        }

        public override void CleanUp()
        {
            base.CleanUp();

            NPC = null;
            NPCPage = null;

            MagicObjects?.Clear();

            Pets?.Clear();

            VisibleObjects?.Clear();

            VisibleDataObjects?.Clear();

            TaggedMonsters?.Clear();

            NearByObjects?.Clear();

            Inventory = null;
            Equipment = null;
            PartsStorage = null;
            Storage = null;

            Companion = null;

            LastHitter = null;

            GroupInvitation = null;

            GuildInvitation = null;

            MarriageInvitation = null;

            TradePartner = null;

            TradePartnerRequest = null;

            TradeItems?.Clear();

            AutoPotions?.Clear();
        }

        public void RemoveMount()
        {
            if (Horse == HorseType.None) return;

            Horse = HorseType.None;
            Broadcast(new S.ObjectMount { ObjectID = ObjectID, Horse = Horse });
        }


        #region Objects View
        public override void AddAllObjects()
        {
            base.AddAllObjects();

            int minX = Math.Max(0, CurrentLocation.X - Config.MaxViewRange);
            int maxX = Math.Min(CurrentMap.Width - 1, CurrentLocation.X + Config.MaxViewRange);

            for (int i = minX; i <= maxX; i++)
                foreach (MapObject ob in CurrentMap.OrderedObjects[i])
                {
                    if (ob.IsNearBy(this))
                        AddNearBy(ob);
                }

            foreach (MapObject ob in NearByObjects)
            {
                if (ob.CanBeSeenBy(this))
                    AddObject(ob);

                if (ob.CanDataBeSeenBy(this))
                    AddDataObject(ob);
            }

            if (Stats[Stat.BossTracker] > 0)
            {
                foreach (MonsterObject ob in CurrentMap.Bosses)
                {
                    if (ob.CanDataBeSeenBy(this))
                        AddDataObject(ob);
                }
            }


            foreach (PlayerObject ob in SEnvir.Players)
            {
                if (ob.CanDataBeSeenBy(this))
                    AddDataObject(ob);
            }
        }
        public override void RemoveAllObjects()
        {
            base.RemoveAllObjects();

            HashSet<MapObject> templist = new HashSet<MapObject>();

            foreach (MapObject ob in VisibleObjects)
            {
                if (ob.CanBeSeenBy(this)) continue;

                templist.Add(ob);
            }
            foreach (MapObject ob in templist)
                RemoveObject(ob);


            templist = new HashSet<MapObject>();
            foreach (MapObject ob in VisibleDataObjects)
            {
                if (ob.CanDataBeSeenBy(this)) continue;

                templist.Add(ob);
            }
            foreach (MapObject ob in templist)
                RemoveDataObject(ob);

            templist = new HashSet<MapObject>();
            foreach (MapObject ob in NearByObjects)
            {
                if (ob.IsNearBy(this)) continue;

                templist.Add(ob);
            }
            foreach (MapObject ob in templist)
                RemoveNearBy(ob);
        }

        public void AddObject(MapObject ob)
        {
            if (ob.SeenByPlayers.Contains(this)) return;

            ob.SeenByPlayers.Add(this);
            VisibleObjects.Add(ob);

            Enqueue(ob.GetInfoPacket(this));
        }
        public void AddNearBy(MapObject ob)
        {
            if (ob.NearByPlayers.Contains(this)) return;

            NearByObjects.Add(ob);
            ob.NearByPlayers.Add(this);

            ob.Activate();
        }
        public void AddDataObject(MapObject ob)
        {
            if (ob.DataSeenByPlayers.Contains(this)) return;

            ob.DataSeenByPlayers.Add(this);
            VisibleDataObjects.Add(ob);

            Enqueue(ob.GetDataPacket(this));
        }
        public void RemoveObject(MapObject ob)
        {
            if (!ob.SeenByPlayers.Contains(this)) return;

            ob.SeenByPlayers.Remove(this);
            VisibleObjects.Remove(ob);

            if (ob == NPC)
            {
                NPC = null;
                NPCPage = null;
            }

            if (ob.Race == ObjectType.Spell)
            {
                SpellObject spell = (SpellObject)ob;

                if (spell.Effect == SpellEffect.Rubble)
                    PauseBuffs();
            }

            if (ob.Race == ObjectType.Monster)
                foreach (MonsterObject mob in TaggedMonsters)
                {
                    if (mob != ob) continue;

                    mob.EXPOwner = null;
                    break;
                }

            Enqueue(new S.ObjectRemove { ObjectID = ob.ObjectID });
        }
        public void RemoveNearBy(MapObject ob)
        {
            if (!ob.NearByPlayers.Contains(this)) return;

            ob.NearByPlayers.Remove(this);
            NearByObjects.Remove(ob);
        }
        public void RemoveDataObject(MapObject ob)
        {
            if (!ob.DataSeenByPlayers.Contains(this)) return;

            ob.DataSeenByPlayers.Remove(this);
            VisibleDataObjects.Remove(ob);

            Enqueue(new S.DataObjectRemove { ObjectID = ob.ObjectID });
        }
        #endregion

        public override bool Teleport(Map map, Point location, bool leaveEffect = true, bool enterEffect = true)
        {
            bool res = base.Teleport(map, location, leaveEffect, enterEffect);

            if (Fishing) return false;

            if (res)
            {
                BuffRemove(BuffType.Cloak);
                BuffRemove(BuffType.Transparency);
                Companion?.Recall();
            }

            return res;
        }

        public void TeleportRing(Point location, int MapIndex)
        {
            MapInfo destInfo = SEnvir.MapInfoList.Binding.FirstOrDefault(x => x.Index == MapIndex);

            if (destInfo == null) return;

            if (!Character.Account.TempAdmin)
            {
                if (!Config.TestServer && Stats[Stat.TeleportRing] == 0) return;

                if (CurrentMap.Instance != null && !CurrentMap.Instance.AllowTeleport) return;

                if (!CurrentMap.Info.AllowRT || !CurrentMap.Info.AllowTT) return;

                if (!destInfo.AllowRT || !destInfo.AllowTT) return;

                if (SEnvir.Now < TeleportTime) return;

                TeleportTime = SEnvir.Now.AddSeconds(1);
            }

            Map destMap = SEnvir.GetMap(destInfo, CurrentMap.Instance, CurrentMap.InstanceSequence);

            if (destMap == null) return;

            if (location.X < 0 || location.Y < 0 || location.X > destMap.Width || location.Y > destMap.Height) return;

            if (!Teleport(destMap, destMap.GetRandomLocation(location, 10, 25))) return;

            TeleportTime = SEnvir.Now.AddMinutes(5);
        }

        public override void Dodged()
        {
            base.Dodged();

            if (GetMagic(MagicType.WillowDance, out WillowDance willowDance))
                LevelMagic(willowDance.Magic);
        }

        public void Enqueue(Packet p) => Connection.Enqueue(p);
        public override Packet GetInfoPacket(PlayerObject ob)
        {
            if (ob == this) return null;

            return new S.ObjectPlayer
            {
                Index = Character.Index,

                ObjectID = ObjectID,
                Name = Name,
                Caption = Character.Caption,
                GuildName = Character.Account.GuildMember?.Guild.GuildName,
                NameColour = NameColour,
                Location = CurrentLocation,
                Direction = Direction,

                Light = Stats[Stat.Light],
                Dead = Dead,

                Class = Class,
                Gender = Gender,
                HairType = HairType,
                HairColour = HairColour,

                Weapon = Equipment[(int)EquipmentSlot.Weapon]?.Info.Shape ?? -1,

                Shield = Equipment[(int)EquipmentSlot.Shield]?.Info.Shape ?? -1,

                Helmet = Character.HideHelmet ? 0 : Equipment[(int)EquipmentSlot.Helmet]?.Info.Shape ?? 0,

                HideHead = HideHead,

                Armour = Equipment[(int)EquipmentSlot.Armour]?.Info.Shape ?? 0,
                ArmourColour = Equipment[(int)EquipmentSlot.Armour]?.Colour ?? Color.Empty,

                Costume = Equipment[(int)EquipmentSlot.Costume]?.Info.Shape ?? -1,

                ArmourEffect = Equipment[(int)EquipmentSlot.Armour]?.Info.ExteriorEffect ?? 0,
                EmblemEffect = Equipment[(int)EquipmentSlot.Emblem]?.Info.ExteriorEffect ?? 0,
                WeaponEffect = Equipment[(int)EquipmentSlot.Weapon]?.Info.ExteriorEffect ?? 0,
                ShieldEffect = Equipment[(int)EquipmentSlot.Shield]?.Info.ExteriorEffect ?? 0,

                Poison = Poison,

                Buffs = Character.Buffs.Where(x => x.Visible).Select(x => new KeyValuePair<BuffType, int>(x.Type, x.Extra)).ToDictionary(),

                Horse = Horse,

                HorseShape = Equipment[(int)EquipmentSlot.HorseArmour]?.Info.Shape ?? 0,

                FiltersClass = Character.FiltersClass,
                FiltersItemType = Character.FiltersItemType,
                FiltersRarity = Character.FiltersRarity,
            };
        }
        public override Packet GetDataPacket(PlayerObject ob)
        {
            return new S.DataObjectPlayer
            {
                ObjectID = ObjectID,

                MapIndex = CurrentMap.Info.Index,
                CurrentLocation = CurrentLocation,

                Name = Name,

                Health = DisplayHP,
                MaxHealth = Stats[Stat.Health],
                Dead = Dead,

                Mana = DisplayMP,
                MaxMana = Stats[Stat.Mana]
            };
        }

        public void SendShapeUpdate()
        {
            S.PlayerUpdate p = new S.PlayerUpdate
            {
                ObjectID = ObjectID,

                Weapon = Equipment[(int)EquipmentSlot.Weapon]?.Info.Shape ?? -1,

                Shield = Equipment[(int)EquipmentSlot.Shield]?.Info.Shape ?? -1,

                Helmet = Character.HideHelmet ? 0 : Equipment[(int)EquipmentSlot.Helmet]?.Info.Shape ?? 0,

                HideHead = HideHead,

                Armour = Equipment[(int)EquipmentSlot.Armour]?.Info.Shape ?? 0,
                ArmourColour = Equipment[(int)EquipmentSlot.Armour]?.Colour ?? Color.Empty,

                Costume = Equipment[(int)EquipmentSlot.Costume]?.Info.Shape ?? -1,

                ArmourEffect = Equipment[(int)EquipmentSlot.Armour]?.Info.ExteriorEffect ?? 0,
                EmblemEffect = Equipment[(int)EquipmentSlot.Emblem]?.Info.ExteriorEffect ?? 0,
                WeaponEffect = Equipment[(int)EquipmentSlot.Weapon]?.Info.ExteriorEffect ?? 0,
                ShieldEffect = Equipment[(int)EquipmentSlot.Shield]?.Info.ExteriorEffect ?? 0,

                HorseArmour = Equipment[(int)EquipmentSlot.HorseArmour]?.Info.Shape ?? 0,

                Light = Stats[Stat.Light]
            };

            Broadcast(p);
        }

        public void SendChangeUpdate()
        {
            S.PlayerChangeUpdate p = new S.PlayerChangeUpdate
            {
                ObjectID = ObjectID,

                Name = Name,
                Caption = ActiveMilestone?.Info.Title ?? Caption,
                CaptionOutlineColour = ActiveMilestone?.Info.OutlineColour ?? Color.Black,
                Gender = Gender,
                HairType = HairType,
                HairColour = HairColour,
                ArmourColour = Equipment[(int)EquipmentSlot.Armour]?.Colour ?? Color.Empty,
            };

            Broadcast(p);
        }

        public override void SetHP(int amount)
        {
            if (Superman)
            {
                CurrentHP = Stats[Stat.Health];
                return;
            }
            base.SetHP(amount);
        }

        public override void ChangeHP(int amount)
        {
            if (Superman)
            {
                CurrentHP = Stats[Stat.Health];
                return;
            }
            base.ChangeHP(amount);
        }

        public override void SetMP(int amount)
        {
            if (Superman)
            {
                CurrentMP = Stats[Stat.Mana];
                return;
            }
            base.SetMP(amount);
        }

        public override void ChangeMP(int amount)
        {
            if (Superman)
            {
                CurrentMP = Stats[Stat.Mana];
                return;
            }
            base.ChangeMP(amount);
        }
    }
}
