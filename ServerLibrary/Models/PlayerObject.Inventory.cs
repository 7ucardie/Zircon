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
    public partial class PlayerObject
    {
        #region Items

        public bool ParseLinks(List<CellLinkInfo> links, int minCount, int maxCount)
        {
            if (links == null || links.Count < minCount || links.Count > maxCount) return false;

            List<CellLinkInfo> tempLinks = new List<CellLinkInfo>();

            foreach (CellLinkInfo link in links)
            {
                if (link == null || link.Count <= 0) return false;

                CellLinkInfo tempLink = tempLinks.FirstOrDefault(x => x.GridType == link.GridType && x.Slot == link.Slot);

                if (tempLink == null)
                {
                    tempLinks.Add(link);
                    continue;
                }

                tempLink.Count += link.Count;
            }

            links.Clear();
            links.AddRange(tempLinks);

            return true;
        }

        public bool ParseLinks(CellLinkInfo link)
        {
            return link != null && link.Count > 0;
        }

        public void RemoveItem(UserItem item)
        {
            /*  foreach (BeltLink link in Character.BeltLinks)
              {
                  if (link.LinkItemIndex != item) continue;
  
                  link.LinkSlot = -1;
              }*/

            item.Slot = -1;
            item.Character = null;
            item.Account = null;
            item.Mail = null;
            item.Auction = null;
            item.Companion = null;
            item.Guild = null;

            /*item.GuildInfo = null;
            item.SaleInfo = null;*/


            //   item.Flags &= ~UserItemFlags.Locked;
        }

        public bool CanGainItems(bool checkWeight, params ItemCheck[] checks)
        {
            int index = 0;
            foreach (ItemCheck check in checks)
            {
                if ((check.Flags & UserItemFlags.QuestItem) == UserItemFlags.QuestItem) continue;

                if (check.Info.ItemEffect == ItemEffect.Experience) continue;

                if (SEnvir.IsCurrencyItem(check.Info)) continue;

                long count = check.Count;

                if (checkWeight)
                {
                    switch (check.Info.ItemType)
                    {
                        case ItemType.Amulet:
                        case ItemType.Poison:
                            if (BagWeight + check.Info.Weight > Stats[Stat.BagWeight]) return false;
                            break;
                        default:
                            if (BagWeight + check.Info.Weight * count > Stats[Stat.BagWeight]) return false;
                            break;
                    }
                }

                if (check.Info.StackSize > 1 && (check.Flags & UserItemFlags.Expirable) != UserItemFlags.Expirable)
                {
                    foreach (UserItem oldItem in Inventory)
                    {
                        if (oldItem == null) continue;

                        if (oldItem.Info != check.Info || oldItem.Count >= check.Info.StackSize) continue;

                        if ((oldItem.Flags & UserItemFlags.Expirable) == UserItemFlags.Expirable) continue;
                        if ((oldItem.Flags & UserItemFlags.Bound) != (check.Flags & UserItemFlags.Bound)) continue;
                        if ((oldItem.Flags & UserItemFlags.Worthless) != (check.Flags & UserItemFlags.Worthless)) continue;
                        if ((oldItem.Flags & UserItemFlags.NonRefinable) != (check.Flags & UserItemFlags.NonRefinable)) continue;
                        if (!oldItem.Stats.Compare(check.Stats)) continue;

                        count -= check.Info.StackSize - oldItem.Count;

                        if (count <= 0) break;
                    }

                    if (count <= 0) break;
                }

                //Start Index
                for (int i = index; i < Inventory.Length; i++)
                {
                    index++;
                    UserItem item = Inventory[i];
                    if (item == null)
                    {
                        count -= check.Info.StackSize;

                        if (count <= 0) break;
                    }
                }

                if (count > 0) return false;
            }

            return true;
        }
        public void GainItem(params UserItem[] items)
        {
            Enqueue(new S.ItemsGained { Items = items.Where(x => x.Info.ItemEffect != ItemEffect.Experience).Select(x => x.ToClientInfo()).ToList() });

            HashSet<UserQuest> changedQuests = new HashSet<UserQuest>();

            foreach (UserItem item in items)
            {
                if (item.UserTask != null)
                {
                    if (item.UserTask.Completed) continue;

                    item.UserTask.Amount = Math.Min(item.UserTask.Task.Amount, item.UserTask.Amount + item.Count);

                    changedQuests.Add(item.UserTask.Quest);

                    if (item.UserTask.Completed)
                    {
                        for (int i = item.UserTask.Objects.Count - 1; i >= 0; i--)
                            item.UserTask.Objects[i].Despawn();
                    }

                    item.UserTask = null;
                    item.Flags &= ~UserItemFlags.QuestItem;

                    item.IsTemporary = true;
                    item.Delete();

                    LogMilestone(MilestoneType.ItemGain, item.Count, item: item.Info);
                    continue;
                }

                var currency = GetCurrency(item.Info);

                if (currency != null)
                {
                    currency.Amount += item.Count;
                    item.IsTemporary = true;
                    item.Delete();

                    LogMilestone(MilestoneType.CurrencyGain, item.Count, currency: currency.Info);
                    continue;
                }

                if (item.Info.ItemEffect == ItemEffect.Experience)
                {
                    GainExperience(item.Count, false);
                    item.IsTemporary = true;
                    item.Delete();
                    continue;
                }

                bool handled = false;
                if (item.Info.StackSize > 1 && (item.Flags & UserItemFlags.Expirable) != UserItemFlags.Expirable)
                {
                    foreach (UserItem oldItem in Inventory)
                    {
                        if (oldItem == null || oldItem.Info != item.Info || oldItem.Count >= oldItem.Info.StackSize) continue;

                        if ((oldItem.Flags & UserItemFlags.Expirable) == UserItemFlags.Expirable) continue;
                        if ((oldItem.Flags & UserItemFlags.Bound) != (item.Flags & UserItemFlags.Bound)) continue;
                        if ((oldItem.Flags & UserItemFlags.Worthless) != (item.Flags & UserItemFlags.Worthless)) continue;
                        if ((oldItem.Flags & UserItemFlags.NonRefinable) != (item.Flags & UserItemFlags.NonRefinable)) continue;
                        if (!oldItem.Stats.Compare(item.Stats)) continue;

                        if (oldItem.Count + item.Count <= item.Info.StackSize)
                        {
                            oldItem.Count += item.Count;
                            item.IsTemporary = true;
                            item.Delete();
                            handled = true;
                            LogMilestone(MilestoneType.ItemGain, item.Count, item: item.Info);
                            break;
                        }

                        item.Count -= item.Info.StackSize - oldItem.Count;
                        oldItem.Count = item.Info.StackSize;
                    }
                    if (handled) continue;
                }

                for (int i = 0; i < Inventory.Length; i++)
                {
                    if (Inventory[i] != null) continue;

                    Inventory[i] = item;
                    item.Slot = i;
                    item.Character = Character;
                    item.IsTemporary = false;
                    LogMilestone(MilestoneType.ItemGain, item.Count, item: item.Info);
                    break;
                }
            }

            foreach (UserQuest quest in changedQuests)
                Enqueue(new S.QuestChanged { Quest = quest.ToClientInfo() });

            RefreshWeight();
        }

        public void ItemUse(CellLinkInfo link)
        {
            if (!ParseLinks(link)) return;

            UserItem[] fromArray;
            switch (link.GridType)
            {
                case GridType.Inventory:
                    fromArray = Inventory;
                    break;
                case GridType.PartsStorage:
                    fromArray = PartsStorage;
                    break;
                case GridType.CompanionInventory:
                    if (Companion == null) return;

                    fromArray = Companion.Inventory;
                    break;
                case GridType.CompanionEquipment:
                    if (Companion == null) return;

                    fromArray = Companion.Equipment;
                    break;
                default:
                    return;
            }

            if (link.Slot < 0 || link.Slot >= fromArray.Length) return;

            UserItem item = fromArray[link.Slot];

            if (item == null) return;

            if (SEnvir.Now < AutoPotionTime && item.Info.ItemEffect != ItemEffect.ElixirOfPurification)
            {
                if (DelayItemUse != null)
                    Enqueue(new S.ItemChanged
                    {
                        Link = DelayItemUse
                    });

                DelayItemUse = link;
                return;
            }

            S.ItemChanged result = new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = link.GridType, Slot = link.Slot }
            };
            Enqueue(result);

            if (Fishing) return;

            if (Buffs.Any(x => x.Type == BuffType.DragonRepulse)) return;

            if (!CanUseItem(item)) return;

            if (Dead && item.Info.ItemEffect != ItemEffect.PillOfReincarnation) return;

            int useCount = 1;

            BuffInfo buff;
            UserItem gainItem = null;
            switch (item.Info.ItemType)
            {
                case ItemType.Consumable:
                    if ((SEnvir.Now < UseItemTime && item.Info.ItemEffect != ItemEffect.ElixirOfPurification)) return;

                    bool work;
                    bool hasSpace;
                    UserItem weapon;
                    ItemInfo extractorInfo;
                    switch (item.Info.Shape)
                    {
                        case 0: //Potion

                            int health = item.Info.Stats[Stat.Health];
                            int mana = item.Info.Stats[Stat.Mana];
                            int focus = item.Info.Stats[Stat.Focus];

                            if (GetMagic(MagicType.PotionMastery, out PotionMastery potionMastery))
                            {
                                health += health * potionMastery.Magic.GetPower() / 100;
                                mana += mana * potionMastery.Magic.GetPower() / 100;
                                focus += focus * potionMastery.Magic.GetPower() / 100;

                                if (CurrentHP < Stats[Stat.Health] || CurrentMP < Stats[Stat.Mana] || CurrentFP < Stats[Stat.Focus])
                                    LevelMagic(potionMastery.Magic);
                            }

                            if (GetMagic(MagicType.AdvancedPotionMastery, out AdvancedPotionMastery advancedPotionMastery))
                            {
                                health += health * advancedPotionMastery.Magic.GetPower() / 100;
                                mana += mana * advancedPotionMastery.Magic.GetPower() / 100;
                                focus += focus * advancedPotionMastery.Magic.GetPower() / 100;

                                if (CurrentHP < Stats[Stat.Health] || CurrentMP < Stats[Stat.Mana] || CurrentFP < Stats[Stat.Focus])
                                    LevelMagic(advancedPotionMastery.Magic);
                            }

                            if (GetMagic(MagicType.Vitality, out Vitality vitality))
                            {
                                if (vitality.LowHP)
                                {
                                    health += health * vitality.Magic.GetPower() / 100;

                                    LevelMagic(vitality.Magic);
                                }
                            }

                            ChangeHP(health);
                            ChangeMP(mana);
                            ChangeFP(focus);

                            if (item.Info.Stats[Stat.Experience] > 0) GainExperience(item.Info.Stats[Stat.Experience], false);
                            break;
                        case 1: //Buff
                            if (!ItemBuffAdd(item.Info)) return;
                            break;
                        case 2: //Town Teleport

                            if (CurrentMap.Instance != null && !CurrentMap.Instance.AllowTeleport)
                            {
                                Connection.ReceiveChatWithObservers(con => con.Language.CannotTownTeleport, MessageType.System);
                                return;
                            }

                            if (!CurrentMap.Info.AllowTT)
                            {
                                Connection.ReceiveChatWithObservers(con => con.Language.CannotTownTeleport, MessageType.System);
                                return;
                            }

                            var bindMap = SEnvir.GetMap(Character.BindPoint.BindRegion.Map);
                            if (bindMap == null) return;

                            if (!Teleport(bindMap, Character.BindPoint.ValidBindPoints[SEnvir.Random.Next(Character.BindPoint.ValidBindPoints.Count)]))
                                return;
                            break;
                        case 3: //Random Teleport

                            if (!CurrentMap.Info.AllowRT)
                            {
                                Connection.ReceiveChatWithObservers(con => con.Language.CannotRandomTeleport, MessageType.System);
                                return;
                            }

                            if (!Teleport(CurrentMap, CurrentMap.GetRandomLocation()))
                                return;
                            break;
                        case 4: //Benediction
                            if (!UseOilOfBenediction()) return;
                            RefreshStats();
                            break;
                        case 5: //Conservation
                            if (!UseOilOfConservation()) return;
                            RefreshStats();
                            break;
                        case 6: //WarGod
                            work = SpecialRepair(EquipmentSlot.Weapon);

                            work = SpecialRepair(EquipmentSlot.Shield) || work;

                            if (!work) return;
                            RefreshStats();
                            break;
                        case 7: //Potion of Forgetfulness
                            if (Character.SpentPoints == 0) return;

                            Character.SpentPoints = 0;
                            Character.HermitStats.Clear();

                            decimal loss = Math.Min(Experience, 1000000);

                            if (loss != 0)
                            {
                                Experience -= loss;
                                Enqueue(new S.GainedExperience { Amount = -loss });
                            }

                            RefreshStats();
                            break;
                        case 8: //Potion of Repentence

                            buff = Buffs.FirstOrDefault(x => x.Type == BuffType.PKPoint);
                            if (buff == null) return;

                            buff.Stats[Stat.PKPoint] = Math.Max(0, buff.Stats[Stat.PKPoint] + item.Info.Stats[Stat.PKPoint]);

                            if (buff.Stats[Stat.PKPoint] == 0)
                                BuffRemove(buff);
                            else
                            {
                                Enqueue(new S.BuffChanged { Index = buff.Index, Stats = buff.Stats });
                                RefreshStats();
                            }

                            break;
                        case 9: //Redemption Key Stone

                            TimeSpan duration = TimeSpan.FromSeconds(item.Info.Stats[Stat.Duration]);

                            buff = Buffs.FirstOrDefault(x => x.Type == BuffType.Redemption);
                            if (buff != null)
                                duration += buff.RemainingTime;

                            Stats stats = new Stats(item.Info.Stats) { [Stat.Duration] = 0 };

                            BuffAdd(BuffType.Redemption, duration, stats, false, false, TimeSpan.Zero);

                            buff = Buffs.FirstOrDefault(x => x.Type == BuffType.PvPCurse);

                            if (buff != null)
                            {
                                buff.RemainingTime = TimeSpan.FromTicks(buff.RemainingTime.Ticks / 2);
                                Enqueue(new S.BuffTime { Index = buff.Index, Time = buff.RemainingTime });
                            }

                            break;
                        case 10: //Potion of Oblivion
                            if (Character.SpentPoints == 0) return;

                            Character.SpentPoints = 0;
                            Character.HermitStats.Clear();

                            RefreshStats();
                            break;
                        case 11: //Superior Repair Oil

                            work = SpecialRepair(EquipmentSlot.Weapon);
                            work = SpecialRepair(EquipmentSlot.Shield);

                            work = SpecialRepair(EquipmentSlot.Helmet) || work;
                            work = SpecialRepair(EquipmentSlot.Armour) || work;
                            work = SpecialRepair(EquipmentSlot.Necklace) || work;
                            work = SpecialRepair(EquipmentSlot.BraceletL) || work;
                            work = SpecialRepair(EquipmentSlot.BraceletR) || work;
                            work = SpecialRepair(EquipmentSlot.RingL) || work;
                            work = SpecialRepair(EquipmentSlot.RingR) || work;
                            work = SpecialRepair(EquipmentSlot.Shoes) || work;

                            if (!work) return;
                            RefreshStats();
                            break;
                        case 12: //Accessory Repair Oil
                            work = SpecialRepair(EquipmentSlot.Necklace);
                            work = SpecialRepair(EquipmentSlot.BraceletL) || work;
                            work = SpecialRepair(EquipmentSlot.BraceletR) || work;
                            work = SpecialRepair(EquipmentSlot.RingL) || work;
                            work = SpecialRepair(EquipmentSlot.RingR) || work;

                            if (!work) return;
                            RefreshStats();
                            break;
                        case 13: //Armour Repair Oil

                            work = SpecialRepair(EquipmentSlot.Helmet);
                            work = SpecialRepair(EquipmentSlot.Armour) || work;
                            work = SpecialRepair(EquipmentSlot.Shoes) || work;

                            if (!work) return;
                            RefreshStats();
                            break;
                        case 14: //ElixirOfPurification
                            work = false;

                            for (int i = PoisonList.Count - 1; i >= 0; i--)
                            {
                                Poison pois = PoisonList[i];

                                switch (pois.Type)
                                {
                                    case PoisonType.Green:
                                    case PoisonType.Red:
                                    case PoisonType.Slow:
                                    case PoisonType.Paralysis:
                                    case PoisonType.HellFire:
                                    case PoisonType.Silenced:
                                    case PoisonType.Abyss:
                                    case PoisonType.Burn:
                                    case PoisonType.Containment:
                                    case PoisonType.Binding:
                                        work = true;
                                        PoisonList.Remove(pois);
                                        break;
                                    default:
                                        continue;
                                }
                            }

                            if (!work)
                            {
                                if (SEnvir.Now.AddSeconds(3) > UseItemTime)
                                    UseItemTime = UseItemTime.AddMilliseconds(item.Info.Durability);

                                AutoPotionCheckTime = UseItemTime.AddMilliseconds(500);
                                return;
                            }

                            break;
                        case 15: //ElixirOfPurification

                            if (!Dead || SEnvir.Now < Character.ReincarnationPillTime) return;

                            Dead = false;
                            SetHP(Stats[Stat.Health]);
                            SetMP(Stats[Stat.Mana]);

                            Character.ReincarnationPillTime = SEnvir.Now.AddSeconds(item.Info.Stats[Stat.ItemReviveTime]);

                            UpdateReviveTimers(Connection);
                            Broadcast(new S.ObjectRevive { ObjectID = ObjectID, Location = CurrentLocation, Effect = true });
                            break;
                        case 16: //Elixir of Regret
                            if (Companion == null) return;

                            switch (item.Info.RequiredAmount)
                            {
                                case 3:
                                    if (Companion.UserCompanion.Level3 == null) return;

                                    if (!CompanionLevelLock3)
                                    {
                                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.ConnotResetCompanionSkill, item.Info.ItemName, 3), MessageType.System);
                                        return;
                                    }

                                    Stats current = new Stats(Companion.UserCompanion.Level3);

                                    while (current.Compare(Companion.UserCompanion.Level3))
                                    {
                                        Companion.UserCompanion.Level3 = null;

                                        Companion.CheckSkills();
                                    }

                                    break;
                                case 5:
                                    if (Companion.UserCompanion.Level5 == null) return;

                                    if (!CompanionLevelLock5)
                                    {
                                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.ConnotResetCompanionSkill, item.Info.ItemName, 5), MessageType.System);
                                        return;
                                    }

                                    current = new Stats(Companion.UserCompanion.Level5);

                                    while (current.Compare(Companion.UserCompanion.Level5))
                                    {
                                        Companion.UserCompanion.Level5 = null;

                                        Companion.CheckSkills();
                                    }
                                    break;
                                case 7:
                                    if (Companion.UserCompanion.Level7 == null) return;

                                    if (!CompanionLevelLock7)
                                    {
                                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.ConnotResetCompanionSkill, item.Info.ItemName, 7), MessageType.System);

                                        return;
                                    }

                                    current = new Stats(Companion.UserCompanion.Level7);

                                    while (current.Compare(Companion.UserCompanion.Level7))
                                    {
                                        Companion.UserCompanion.Level7 = null;

                                        Companion.CheckSkills();
                                    }
                                    break;
                                case 10:
                                    if (Companion.UserCompanion.Level10 == null) return;

                                    if (!CompanionLevelLock10)
                                    {
                                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.ConnotResetCompanionSkill, item.Info.ItemName, 10), MessageType.System);
                                        return;
                                    }

                                    current = new Stats(Companion.UserCompanion.Level10);

                                    while (current.Compare(Companion.UserCompanion.Level10))
                                    {
                                        Companion.UserCompanion.Level10 = null;

                                        Companion.CheckSkills();
                                    }
                                    break;
                                case 11:
                                    if (Companion.UserCompanion.Level11 == null) return;

                                    if (!CompanionLevelLock11)
                                    {
                                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.ConnotResetCompanionSkill, item.Info.ItemName, 11), MessageType.System);
                                        return;
                                    }

                                    current = new Stats(Companion.UserCompanion.Level11);

                                    while (current.Compare(Companion.UserCompanion.Level11))
                                    {
                                        Companion.UserCompanion.Level11 = null;

                                        Companion.CheckSkills();
                                    }
                                    break;
                                case 13:
                                    if (Companion.UserCompanion.Level13 == null) return;

                                    if (!CompanionLevelLock13)
                                    {
                                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.ConnotResetCompanionSkill, item.Info.ItemName, 13), MessageType.System);
                                        return;
                                    }

                                    current = new Stats(Companion.UserCompanion.Level13);

                                    while (current.Compare(Companion.UserCompanion.Level13))
                                    {
                                        Companion.UserCompanion.Level13 = null;

                                        Companion.CheckSkills();
                                    }
                                    break;
                                case 15:
                                    if (Companion.UserCompanion.Level15 == null) return;

                                    if (!CompanionLevelLock15)
                                    {
                                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.ConnotResetCompanionSkill, item.Info.ItemName, 15), MessageType.System);
                                        return;
                                    }

                                    current = new Stats(Companion.UserCompanion.Level15);

                                    while (current.Compare(Companion.UserCompanion.Level15))
                                    {
                                        Companion.UserCompanion.Level15 = null;

                                        Companion.CheckSkills();
                                    }
                                    break;
                                default:
                                    return;
                            }
                            break;
                        case 17: //Storage Increase

                            int size = Character.Account.StorageSize + 10;

                            if (size >= Storage.Length)
                            {
                                Connection.ReceiveChatWithObservers(con => con.Language.StorageLimit, MessageType.System);
                                return;
                            }

                            Character.Account.StorageSize = size;
                            Enqueue(new S.StorageSize { Size = Character.Account.StorageSize });
                            break;
                        case 18: //Football Whistle
                            if (item.Info.Stats[Stat.MapSummoning] > 0 && CurrentMap.HasSafeZone)
                            {
                                Connection.ReceiveChat(string.Format(Connection.Language.CannotUseItemWithSafeZone, item.Info.ItemName), MessageType.System);
                                return;
                            }

                            if (item.Info.Stats[Stat.Experience] > 0) GainExperience(item.Info.Stats[Stat.Experience], false);

                            IncreasePKPoints(item.Info.Stats[Stat.PKPoint]);

                            if (item.Info.Stats[Stat.FootballArmourAction] > 0 && SEnvir.Random.Next(item.Info.Stats[Stat.FootballArmourAction]) == 0)
                            {
                                hasSpace = false;

                                foreach (UserItem slot in Inventory)
                                {
                                    if (slot != null) continue;

                                    hasSpace = true;
                                    break;
                                }

                                if (!hasSpace)
                                {
                                    Connection.ReceiveChat(Connection.Language.NoEmptyInventorySlot, MessageType.System);
                                    return;
                                }

                                //Give armour
                                ItemInfo armourInfo = SEnvir.ItemInfoList.Binding.FirstOrDefault(x => x.ItemEffect == ItemEffect.FootballArmour && CanStartWith(x));

                                if (armourInfo != null)
                                {
                                    gainItem = SEnvir.CreateDropItem(armourInfo, 2);
                                    gainItem.CurrentDurability = gainItem.MaxDurability;
                                }
                            }

                            if (item.Info.Stats[Stat.MapSummoning] > 0)
                            {
                                MonsterInfo boss;
                                while (true)
                                {
                                    boss = SEnvir.BossList[SEnvir.Random.Next(SEnvir.BossList.Count)];

                                    if (boss.Level >= 300) continue;

                                    break;
                                }

                                MonsterObject mob = MonsterObject.GetMonster(boss);

                                if (mob.Spawn(CurrentMap, CurrentMap.GetRandomLocation(CurrentLocation, 2)))
                                {
                                    if (SEnvir.Random.Next(item.Info.Stats[Stat.MapSummoning]) == 0)
                                    {
                                        for (int i = CurrentMap.Objects.Count - 1; i >= 0; i--)
                                        {
                                            mob = CurrentMap.Objects[i] as MonsterObject;

                                            if (mob == null) continue;

                                            if (mob.PetOwner != null) continue;

                                            if (mob is Guard) continue;

                                            if (mob.Dead || mob.MoveDelay == 0 || !mob.CanMove) continue;

                                            if (mob.Target != null) continue;

                                            if (mob.Level >= 300) continue;

                                            mob.Teleport(CurrentMap, CurrentMap.GetRandomLocation(CurrentLocation, 30));
                                        }


                                        string text = $"A [{item.Info.ItemName}] has been used in {CurrentMap.Info.Description}";

                                        foreach (SConnection con in SEnvir.Connections)
                                        {
                                            switch (con.Stage)
                                            {
                                                case GameStage.Game:
                                                case GameStage.Observer:
                                                    con.ReceiveChat(text, MessageType.System);
                                                    break;
                                                default: continue;
                                            }
                                        }
                                    }
                                }
                            }
                            break;
                        case 19: //Stat Extractor [From Weapon] (All Added Stats)
                            if (Horse != HorseType.None) return;
                            weapon = Equipment[(int)EquipmentSlot.Weapon];

                            if (weapon == null)
                            {
                                Connection.ReceiveChat("You are not holding a weapon.", MessageType.System);
                                return;
                            }

                            if (!ExtractorLock)
                            {
                                Connection.ReceiveChat("Extraction functions are locked, please type @ExtractorLock and try again", MessageType.System);
                                return;
                            }

                            if (weapon.Info.ItemEffect == ItemEffect.SpiritBlade)
                            {
                                Connection.ReceiveChat($"You cannot extract a {weapon.Info.ItemName}.", MessageType.System);
                                return;
                            }

                            if (weapon.Level != Globals.WeaponExperienceList.Count)
                            {
                                Connection.ReceiveChat("Your weapon is not the max level.", MessageType.System);
                                return;
                            }

                            if (weapon.AddedStats.Count == 0)
                            {
                                Connection.ReceiveChat("Your weapon does not have any added stats.", MessageType.System);
                                return;
                            }

                            hasSpace = false;

                            foreach (UserItem slot in Inventory)
                            {
                                if (slot != null) continue;

                                hasSpace = true;
                                break;
                            }

                            if (!hasSpace)
                            {
                                Connection.ReceiveChat("You do not have any empty inventory slot", MessageType.System);
                                return;
                            }

                            //Give extractor
                            extractorInfo = SEnvir.ItemInfoList.Binding.FirstOrDefault(x => x.ItemEffect == ItemEffect.StatExtractor);

                            if (extractorInfo == null) return;

                            gainItem = SEnvir.CreateFreshItem(extractorInfo);

                            for (int i = weapon.AddedStats.Count - 1; i >= 0; i--)
                                weapon.AddedStats[i].Item = gainItem;

                            gainItem.StatsChanged();
                            weapon.StatsChanged();

                            Enqueue(new S.ItemChanged { Link = new CellLinkInfo { GridType = GridType.Equipment, Slot = (int)EquipmentSlot.Weapon, Count = 0 }, Success = true });
                            RemoveItem(weapon);
                            Equipment[(int)EquipmentSlot.Weapon] = null;
                            weapon.Delete();
                            RefreshStats();

                            break;
                        case 20: //Stat Extractor [To Weapon] (All Added Stats)
                            if (Horse != HorseType.None) return;
                            weapon = Equipment[(int)EquipmentSlot.Weapon];

                            if (weapon == null)
                            {
                                Connection.ReceiveChat("You are not holding a weapon.", MessageType.System);
                                return;
                            }
                            if (!ExtractorLock)
                            {
                                Connection.ReceiveChat("Extraction functions are locked, please type @ExtractorLock and try again", MessageType.System);
                                return;
                            }

                            if (weapon.Info.ItemEffect == ItemEffect.SpiritBlade)
                            {
                                Connection.ReceiveChat($"You cannot apply to a {weapon.Info.ItemName}.", MessageType.System);
                                return;
                            }

                            if (weapon.Level != Globals.WeaponExperienceList.Count)
                            {
                                Connection.ReceiveChat("Your weapon is not the max level.", MessageType.System);
                                return;
                            }

                            weapon.Flags &= ~UserItemFlags.Refinable;

                            for (int i = weapon.AddedStats.Count - 1; i >= 0; i--)
                            {
                                UserItemStat stat = weapon.AddedStats[i];
                                stat.Delete();
                            }

                            weapon.StatsChanged();

                            //Give stats to weapon
                            for (int i = item.AddedStats.Count - 1; i >= 0; i--)
                                weapon.AddStat(item.AddedStats[i].Stat, item.AddedStats[i].Amount, item.AddedStats[i].StatSource);

                            item.StatsChanged();
                            weapon.StatsChanged();

                            Enqueue(new S.ItemStatsRefreshed { Slot = (int)EquipmentSlot.Weapon, GridType = GridType.Equipment, NewStats = new Stats(weapon.Stats) });
                            RefreshStats();
                            break;
                        case 21: //Stat Extractor [From Weapon] (Refine Only)
                            if (Horse != HorseType.None) return;
                            weapon = Equipment[(int)EquipmentSlot.Weapon];

                            if (weapon == null)
                            {
                                Connection.ReceiveChat("You are not holding a weapon.", MessageType.System);
                                return;
                            }
                            if (!ExtractorLock)
                            {
                                Connection.ReceiveChat("Extraction functions are locked, please type @ExtractorLock and try again", MessageType.System);
                                return;
                            }

                            if (weapon.Level != Globals.WeaponExperienceList.Count)
                            {
                                Connection.ReceiveChat("Your weapon is not the max level.", MessageType.System);
                                return;
                            }

                            bool hasRefine = false;

                            foreach (UserItemStat stat in weapon.AddedStats)
                            {
                                if (stat.StatSource != StatSource.Refine) continue;

                                hasRefine = true;
                                break;
                            }

                            if (!hasRefine)
                            {
                                Connection.ReceiveChat("Your weapon does not have any refine stats.", MessageType.System);
                                return;
                            }

                            hasSpace = false;

                            foreach (UserItem slot in Inventory)
                            {
                                if (slot != null) continue;

                                hasSpace = true;
                                break;
                            }

                            if (!hasSpace)
                            {
                                Connection.ReceiveChat("You do not have any empty inventory slot", MessageType.System);
                                return;
                            }

                            //Give extractor
                            extractorInfo = SEnvir.ItemInfoList.Binding.FirstOrDefault(x => x.ItemEffect == ItemEffect.RefineExtractor);

                            if (extractorInfo == null) return;

                            gainItem = SEnvir.CreateFreshItem(extractorInfo);

                            for (int i = weapon.AddedStats.Count - 1; i >= 0; i--)
                            {
                                if (weapon.AddedStats[i].StatSource != StatSource.Refine) continue;

                                weapon.AddedStats[i].Item = gainItem;
                            }

                            gainItem.StatsChanged();
                            weapon.StatsChanged();

                            Enqueue(new S.ItemStatsRefreshed { GridType = GridType.Equipment, Slot = (int)EquipmentSlot.Weapon, NewStats = new Stats(weapon.Stats) });

                            RefreshStats();

                            break;
                        case 22: //Stat Extractor [To Weapon] (Refine Only)
                            if (Horse != HorseType.None) return;
                            weapon = Equipment[(int)EquipmentSlot.Weapon];

                            if (weapon == null)
                            {
                                Connection.ReceiveChat("You are not holding a weapon.", MessageType.System);
                                return;
                            }
                            if (!ExtractorLock)
                            {
                                Connection.ReceiveChat("Extraction functions are locked, please type @ExtractorLock and try again", MessageType.System);
                                return;
                            }

                            if (weapon.Level != Globals.WeaponExperienceList.Count)
                            {
                                Connection.ReceiveChat("Your weapon is not the max level.", MessageType.System);
                                return;
                            }

                            weapon.Flags &= ~UserItemFlags.Refinable;

                            for (int i = weapon.AddedStats.Count - 1; i >= 0; i--)
                            {
                                UserItemStat stat = weapon.AddedStats[i];
                                if (stat.StatSource != StatSource.Refine) continue;
                                stat.Delete();
                            }

                            weapon.StatsChanged();

                            //Give stats to weapon
                            for (int i = item.AddedStats.Count - 1; i >= 0; i--)
                                weapon.AddStat(item.AddedStats[i].Stat, item.AddedStats[i].Amount, item.AddedStats[i].StatSource);

                            item.StatsChanged();
                            weapon.StatsChanged();
                            weapon.ResetCoolDown = SEnvir.Now.AddDays(14);

                            Enqueue(new S.ItemStatsRefreshed { Slot = (int)EquipmentSlot.Weapon, GridType = GridType.Equipment, NewStats = new Stats(weapon.Stats) });
                            RefreshStats();
                            break;
                    }

                    if (item.Info.ItemEffect != ItemEffect.ElixirOfPurification || UseItemTime < SEnvir.Now)
                        UseItemTime = SEnvir.Now.AddMilliseconds(item.Info.Durability);
                    else
                        UseItemTime = UseItemTime.AddMilliseconds(item.Info.Durability);

                    AutoPotionCheckTime = UseItemTime.AddMilliseconds(500);
                    break;
                case ItemType.CompanionFood:
                    if (Companion == null) return;
                    if (SEnvir.Now < UseItemTime) return;

                    if (Companion.UserCompanion.Hunger >= Companion.LevelInfo.MaxHunger) return;

                    Companion.UserCompanion.Hunger = Math.Min(Companion.LevelInfo.MaxHunger, Companion.UserCompanion.Hunger + item.Info.Stats[Stat.CompanionHunger]);

                    if (Buffs.All(x => x.Type != BuffType.Companion))
                        CompanionApplyBuff();

                    Companion.RefreshStats();

                    Enqueue(new S.CompanionUpdate
                    {
                        Level = Companion.UserCompanion.Level,
                        Experience = Companion.UserCompanion.Experience,
                        Hunger = Companion.UserCompanion.Hunger,
                    });
                    break;
                case ItemType.Book:
                    if (SEnvir.Now < UseItemTime || Horse != HorseType.None) return;

                    MagicInfo info = SEnvir.GetMagicInfo(item.Info.Shape);

                    if (info.School == MagicSchool.None) return;

                    if (GetMagic(info.Magic, out MagicObject magicObject))
                    {
                        var magic = magicObject.Magic;

                        if (magic.ItemRequired)
                        {
                            magic.ItemRequired = false;
                            Enqueue(new S.NewMagic { Magic = magic.ToClientInfo() });
                        }

                        if (magic.Level < 3) return;

                        if (magic.Level >= Globals.MagicMaxLevel)
                        {
                            Connection.ReceiveChatWithObservers(con => string.Format(con.Language.MagicMaxLevelReached, magic.Info.Name), MessageType.System);

                            return;
                        }
                    }

                    if (SEnvir.Random.Next(100) >= item.CurrentDurability)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.LearnBookFailed, MessageType.System);

                        break;
                    }

                    if (magicObject != null)
                    {
                        var magic = magicObject.Magic;

                        int rate = (magic.Level - 2) * 500;

                        magic.Experience += item.CurrentDurability;

                        if (magic.Experience >= rate || (magic.Level == 3 && SEnvir.Random.Next(rate) == 0))
                        {
                            magic.Level++;
                            magic.Experience = 0;

                            Enqueue(new S.MagicLeveled { InfoIndex = magic.Info.Index, Level = magic.Level, Experience = magic.Experience });

                            Connection.ReceiveChatWithObservers(con => string.Format(con.Language.LearnBook4Success, magic.Info.Name, magic.Level), MessageType.System);

                            RefreshStats();
                        }
                        else
                        {
                            long remaining = rate - magic.Experience;

                            Connection.ReceiveChatWithObservers(con => string.Format(con.Language.LearnBook4Failed, remaining, magic.Level + 1), MessageType.System);

                            Enqueue(new S.MagicLeveled { InfoIndex = magic.Info.Index, Level = magic.Level, Experience = magic.Experience });
                        }
                    }
                    else
                    {
                        var magic = SEnvir.UserMagicList.CreateNewObject();
                        magic.Character = Character;
                        magic.Info = info;

                        SetupMagic(magic);

                        Enqueue(new S.NewMagic { Magic = magic.ToClientInfo() });

                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.LearnBookSuccess, magic.Info.Name), MessageType.System);

                        RefreshStats();
                    }

                    break;
                case ItemType.ItemPart:
                    ItemInfo partInfo = SEnvir.ItemInfoList.Binding.First(x => x.Index == item.Stats[Stat.ItemIndex]);

                    if (partInfo.PartCount < 1 || partInfo.PartCount > item.Count) return;

                    if (!CanGainItems(false, new ItemCheck(partInfo, 1, UserItemFlags.None, TimeSpan.Zero))) return;

                    useCount = partInfo.PartCount;

                    gainItem = SEnvir.CreateDropItem(partInfo, 2);

                    break;
                case ItemType.Bundle:
                    break;
                default:
                    return;
            }

            result.Success = true;

            BuffRemove(BuffType.Transparency);

            if (!GetMagic(MagicType.Stealth, out Stealth stealth) || !stealth.CheckCloak())
            {
                BuffRemove(BuffType.Cloak);
            }

            if (item.Count > useCount)
            {
                item.Count -= useCount;
                result.Link.Count = item.Count;
            }
            else
            {
                RemoveItem(item);
                fromArray[link.Slot] = null;
                item.Delete();

                result.Link.Count = 0;
            }

            LogMilestone(MilestoneType.ItemUse, useCount, item: item.Info);

            if (gainItem != null)
                GainItem(gainItem);

            Companion?.RefreshWeight();
            RefreshWeight();
        }
        public bool CanStartWith(ItemInfo info)
        {
            switch (Gender)
            {
                case MirGender.Male:
                    if ((info.RequiredGender & RequiredGender.Male) != RequiredGender.Male)
                        return false;
                    break;
                case MirGender.Female:
                    if ((info.RequiredGender & RequiredGender.Female) != RequiredGender.Female)
                        return false;
                    break;
            }

            switch (Class)
            {
                case MirClass.Warrior:
                    if ((info.RequiredClass & RequiredClass.Warrior) != RequiredClass.Warrior)
                        return false;
                    break;
                case MirClass.Wizard:
                    if ((info.RequiredClass & RequiredClass.Wizard) != RequiredClass.Wizard)
                        return false;
                    break;
                case MirClass.Taoist:
                    if ((info.RequiredClass & RequiredClass.Taoist) != RequiredClass.Taoist)
                        return false;
                    break;
                case MirClass.Assassin:
                    if ((info.RequiredClass & RequiredClass.Assassin) != RequiredClass.Assassin)
                        return false;
                    break;
            }

            return true;
        }
        public bool CanUseItem(UserItem item)
        {
            switch (Gender)
            {
                case MirGender.Male:
                    if ((item.Info.RequiredGender & RequiredGender.Male) != RequiredGender.Male)
                        return false;
                    break;
                case MirGender.Female:
                    if ((item.Info.RequiredGender & RequiredGender.Female) != RequiredGender.Female)
                        return false;
                    break;
            }

            switch (Class)
            {
                case MirClass.Warrior:
                    if ((item.Info.RequiredClass & RequiredClass.Warrior) != RequiredClass.Warrior)
                        return false;
                    break;
                case MirClass.Wizard:
                    if ((item.Info.RequiredClass & RequiredClass.Wizard) != RequiredClass.Wizard)
                        return false;
                    break;
                case MirClass.Taoist:
                    if ((item.Info.RequiredClass & RequiredClass.Taoist) != RequiredClass.Taoist)
                        return false;
                    break;
                case MirClass.Assassin:
                    if ((item.Info.RequiredClass & RequiredClass.Assassin) != RequiredClass.Assassin)
                        return false;
                    break;
            }


            switch (item.Info.RequiredType)
            {
                case RequiredType.Level:
                    if (Level < item.Info.RequiredAmount && Stats[Stat.Rebirth] == 0) return false;
                    break;
                case RequiredType.MaxLevel:
                    if (Level > item.Info.RequiredAmount || Stats[Stat.Rebirth] > 0) return false;
                    break;
                case RequiredType.CompanionLevel:
                    if (Companion == null) return false;

                    if (Companion.UserCompanion.Level < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.MaxCompanionLevel:
                    if (Companion == null) return false;

                    if (Companion.UserCompanion.Level > item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.AC:
                    if (Stats[Stat.MaxAC] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.MR:
                    if (Stats[Stat.MaxMR] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.DC:
                    if (Stats[Stat.MaxDC] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.MC:
                    if (Stats[Stat.MaxMC] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.SC:
                    if (Stats[Stat.MaxSC] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.Health:
                    if (Stats[Stat.Health] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.Mana:
                    if (Stats[Stat.Mana] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.Accuracy:
                    if (Stats[Stat.Accuracy] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.Agility:
                    if (Stats[Stat.Agility] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.RebirthLevel:
                    if (Stats[Stat.Rebirth] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.MaxRebirthLevel:
                    if (Stats[Stat.Rebirth] > item.Info.RequiredAmount) return false;
                    break;
            }


            switch (item.Info.ItemType)
            {
                case ItemType.Book:
                    MagicInfo magic = SEnvir.GetMagicInfo(item.Info.Shape);
                    if (magic == null) return false;
                    if (GetMagic(magic.Magic, out MagicObject magicObject) && (magicObject.Magic.Level < 3 || (item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable)) return false;
                    return true;
                case ItemType.Consumable:
                    switch (item.Info.Shape)
                    {
                        case 1: //Item Buffs
                            BuffInfo buff = Buffs.FirstOrDefault(x => x.Type == BuffType.ItemBuff && x.ItemIndex == item.Info.Index);

                            if (buff != null && buff.RemainingTime == TimeSpan.MaxValue) return false;
                            break;
                    }
                    break;
            }

            return true;
        }
        public void ItemMove(C.ItemMove p)
        {
            S.ItemMove result = new S.ItemMove
            {
                FromGrid = p.FromGrid,
                FromSlot = p.FromSlot,
                ToGrid = p.ToGrid,
                ToSlot = p.ToSlot,
                MergeItem = p.MergeItem,

                ObserverPacket = p.ToGrid != GridType.GuildStorage && p.FromGrid != GridType.GuildStorage,
            };

            Enqueue(result);

            if (Dead || (p.FromGrid == p.ToGrid && p.FromSlot == p.ToSlot)) return;

            UserItem[] fromArray, toArray;

            switch (p.FromGrid)
            {
                case GridType.Inventory:
                    fromArray = Inventory;
                    break;
                case GridType.Equipment:
                    fromArray = Equipment;
                    break;
                case GridType.PartsStorage:
                    if (!InSafeZone && !Character.Account.TempAdmin)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.StorageSafeZone, MessageType.System);
                        return;
                    }

                    fromArray = PartsStorage;
                    break;
                case GridType.Storage:
                    if (!InSafeZone && !Character.Account.TempAdmin)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.StorageSafeZone, MessageType.System);
                        return;
                    }

                    fromArray = Storage;

                    if (p.FromSlot >= Character.Account.StorageSize) return;
                    break;
                case GridType.GuildStorage:
                    if (Character.Account.GuildMember == null) return;

                    if ((Character.Account.GuildMember.Permission & GuildPermission.Storage) != GuildPermission.Storage)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.GuildStoragePermission, MessageType.System);
                        return;
                    }

                    if (!InSafeZone && !(p.ToGrid == GridType.Storage || p.ToGrid == GridType.PartsStorage))
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.GuildStorageSafeZone, MessageType.System);
                        return;
                    }

                    fromArray = Character.Account.GuildMember.Guild.Storage;

                    if (p.FromSlot >= Character.Account.GuildMember.Guild.StorageSize) return;
                    break;
                case GridType.CompanionInventory:
                    if (Companion == null) return;

                    fromArray = Companion.Inventory;
                    if (p.FromSlot >= Companion.Stats[Stat.CompanionInventory]) return;
                    break;
                case GridType.CompanionEquipment:
                    if (Companion == null) return;

                    fromArray = Companion.Equipment;
                    break;
                default:
                    return;
            }

            if (p.FromSlot < 0 || p.FromSlot >= fromArray.Length) return;

            UserItem fromItem = fromArray[p.FromSlot];

            if (fromItem == null) return;
            if ((fromItem.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return;

            switch (p.ToGrid)
            {
                case GridType.Inventory:
                    toArray = Inventory;
                    break;
                case GridType.Equipment:
                    toArray = Equipment;
                    break;
                case GridType.PartsStorage:
                    if (!InSafeZone && !Character.Account.TempAdmin)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.StorageSafeZone, MessageType.System);
                        return;
                    }

                    if (fromItem.Info.ItemEffect != ItemEffect.ItemPart) return;

                    toArray = PartsStorage;

                    break;
                case GridType.Storage:
                    if (!InSafeZone && !Character.Account.TempAdmin)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.StorageSafeZone, MessageType.System);
                        return;
                    }

                    if (fromItem.Info.ItemEffect == ItemEffect.ItemPart) return;

                    toArray = Storage;

                    if (p.ToSlot >= Character.Account.StorageSize) return;
                    break;
                case GridType.GuildStorage:
                    if (Character.Account.GuildMember == null) return;

                    if ((Character.Account.GuildMember.Permission & GuildPermission.Storage) != GuildPermission.Storage)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.GuildStoragePermission, MessageType.System);
                        return;
                    }

                    if (!InSafeZone && !(p.ToGrid == GridType.Storage || p.ToGrid == GridType.PartsStorage))
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.GuildStorageSafeZone, MessageType.System);
                        return;
                    }

                    toArray = Character.Account.GuildMember.Guild.Storage;

                    if (p.ToSlot >= Character.Account.GuildMember.Guild.StorageSize) return;
                    break;
                case GridType.CompanionInventory:
                    if (Companion == null) return;

                    toArray = Companion.Inventory;

                    if (p.ToSlot >= Companion.Stats[Stat.CompanionInventory]) return;
                    break;
                case GridType.CompanionEquipment:
                    if (Companion == null) return;

                    toArray = Companion.Equipment;
                    break;
                default:
                    return;
            }

            if (p.ToSlot < 0 || p.ToSlot >= toArray.Length) return;

            UserItem toItem = toArray[p.ToSlot];

            if (toItem != null && (toItem.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return;

            if (p.FromGrid == GridType.Equipment)
            {
                //CanRemove Item
                if (p.ToGrid == GridType.Equipment) return;

                if (!p.MergeItem && toItem != null) return;
            }

            if (p.FromGrid == GridType.CompanionEquipment)
            {
                //CanRemove Item
                if (p.ToGrid == GridType.CompanionEquipment) return;

                if (!p.MergeItem && toItem != null) return;

                if (p.ToGrid == GridType.CompanionInventory)
                {
                    int space = fromItem.Stats[Stat.CompanionInventory] + fromItem.Info.Stats[Stat.CompanionInventory];

                    if (p.ToSlot >= Companion.Stats[Stat.CompanionInventory] - space) return;
                }
            }

            if (p.ToGrid == GridType.Equipment)
            {
                if (!CanWearItem(fromItem, (EquipmentSlot)p.ToSlot)) return;
            }

            if (p.ToGrid == GridType.CompanionEquipment)
            {
                if (!Companion.CanWearItem(fromItem, (CompanionSlot)p.ToSlot)) return;

                if (p.FromGrid == GridType.CompanionInventory && toItem != null)
                {
                    int space = fromItem.Stats[Stat.CompanionInventory] + fromItem.Info.Stats[Stat.CompanionInventory]
                                - toItem.Stats[Stat.CompanionInventory] - toItem.Info.Stats[Stat.CompanionInventory];

                    if (p.ToSlot >= Companion.Stats[Stat.CompanionInventory] + space) return;
                }
            }

            if (p.ToGrid == GridType.CompanionInventory && p.FromGrid != GridType.CompanionInventory)
            {
                int weight = 0;

                switch (fromItem.Info.ItemType)
                {
                    case ItemType.Poison:
                    case ItemType.Amulet:
                        if (p.MergeItem) break;
                        weight = fromItem.Weight;

                        if (toItem != null)
                            weight -= toItem.Weight;

                        break;
                    default:
                        if (p.MergeItem)
                        {
                            if (toItem != null && toItem.Count < toItem.Info.StackSize)
                                weight = (int)(Math.Min(fromItem.Count, toItem.Info.StackSize - toItem.Count) * fromItem.Info.Weight);
                        }
                        else
                        {
                            weight = fromItem.Weight;

                            if (toItem != null)
                                weight -= toItem.Weight;
                        }
                        break;
                }

                if (Companion.BagWeight + weight > Companion.Stats[Stat.CompanionBagWeight])
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.CompanionNoRoom, MessageType.System);
                    return;
                }
            }

            if (p.FromGrid == GridType.CompanionInventory && p.ToGrid != GridType.CompanionInventory && toItem != null && !p.MergeItem)
            {
                int weight = toItem.Weight;

                weight -= fromItem.Weight;

                if (Companion.BagWeight + weight > Companion.Stats[Stat.CompanionBagWeight])
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.CompanionNoRoom, MessageType.System);
                    return;
                }
            }

            Packet guildpacket;
            if (p.MergeItem)
            {
                if (toItem == null || toItem.Info != fromItem.Info || toItem.Count >= toItem.Info.StackSize || toItem.ExpireTime != fromItem.ExpireTime) return;

                if ((toItem.Flags & UserItemFlags.Bound) != (fromItem.Flags & UserItemFlags.Bound)) return;
                if ((toItem.Flags & UserItemFlags.Worthless) != (fromItem.Flags & UserItemFlags.Worthless)) return;
                if ((toItem.Flags & UserItemFlags.Expirable) != (fromItem.Flags & UserItemFlags.Expirable)) return;
                if ((toItem.Flags & UserItemFlags.NonRefinable) != (fromItem.Flags & UserItemFlags.NonRefinable)) return;
                if (!toItem.Stats.Compare(fromItem.Stats)) return;

                long fromCount, toCount;
                if (toItem.Count + fromItem.Count <= toItem.Info.StackSize)
                {
                    toItem.Count += fromItem.Count;

                    fromArray[p.FromSlot] = null;
                    fromItem.Delete();

                    toCount = toItem.Count;
                    fromCount = 0;
                }
                else
                {
                    fromItem.Count -= fromItem.Info.StackSize - toItem.Count;
                    toItem.Count = toItem.Info.StackSize;

                    toCount = toItem.Count;
                    fromCount = fromItem.Count;
                }

                result.Success = true;
                RefreshWeight();
                Companion?.RefreshWeight();

                if (p.ToGrid == GridType.GuildStorage || p.FromGrid == GridType.GuildStorage)
                {
                    if (p.ToGrid == GridType.GuildStorage)
                    {
                        guildpacket = new S.ItemChanged
                        {
                            Link = new CellLinkInfo { GridType = p.ToGrid, Slot = p.ToSlot, Count = toCount, },
                            Success = true,

                            ObserverPacket = false
                        };

                        foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                        {
                            PlayerObject player = member.Account.Connection?.Player;

                            if (player == null || player == this) continue;

                            player.Enqueue(guildpacket);

                        }
                    }
                    else
                    {
                        foreach (SConnection con in Connection.Observers)
                        {
                            con.Enqueue(new S.ItemChanged
                            {
                                Link = new CellLinkInfo { GridType = p.ToGrid, Slot = p.ToSlot, Count = toCount },
                                Success = true,
                            });
                        }
                    }

                    if (p.FromGrid == GridType.GuildStorage)
                    {
                        guildpacket = new S.ItemChanged
                        {
                            Link = new CellLinkInfo { GridType = p.FromGrid, Slot = p.FromSlot, Count = fromCount, },
                            Success = true,

                            ObserverPacket = false
                        };

                        foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                        {
                            PlayerObject player = member.Account.Connection?.Player;

                            if (player == null || player == this) continue;

                            player.Enqueue(guildpacket);

                        }
                    }
                    else
                    {
                        foreach (SConnection con in Connection.Observers)
                        {
                            con.Enqueue(new S.ItemChanged
                            {
                                Link = new CellLinkInfo { GridType = p.FromGrid, Slot = p.FromSlot, Count = fromCount },
                                Success = true,
                            });
                        }
                    }
                }
                return;
            }

            if (p.ToGrid == GridType.GuildStorage)
            {
                if (toItem != null && p.FromGrid != GridType.GuildStorage) //This should force us to merging stacks OR empty item?
                    return;

                if (!fromItem.Info.CanTrade || (fromItem.Flags & UserItemFlags.Bound) == UserItemFlags.Bound) return;
            }

            if (p.FromGrid == GridType.GuildStorage)
            {
                if (toItem != null && p.ToGrid != GridType.GuildStorage) //This should force us to merging stacks OR empty item?
                    return;
            }

            fromArray[p.FromSlot] = toItem;
            toArray[p.ToSlot] = fromItem;
            bool sendShape = false, sendCompanionShape = false;

            switch (p.FromGrid)
            {
                case GridType.Inventory:
                    if (toItem == null) break;

                    toItem.Slot = p.FromSlot;
                    toItem.Character = Character;
                    break;
                case GridType.Equipment:
                    sendShape = true;
                    if (toItem == null) break;
                    throw new Exception("Shitty Move Item Logic");
                case GridType.CompanionInventory:
                    if (toItem == null) break;

                    toItem.Slot = p.FromSlot;
                    toItem.Companion = Companion.UserCompanion;
                    break;
                case GridType.CompanionEquipment:
                    sendCompanionShape = true;
                    if (toItem == null) break;
                    throw new Exception("Shitty Move Item Logic");
                case GridType.PartsStorage:
                    if (toItem == null) break;

                    toItem.Slot = p.FromSlot;
                    toItem.Account = Character.Account;
                    break;
                case GridType.Storage:
                    if (toItem == null) break;

                    toItem.Slot = p.FromSlot;
                    toItem.Account = Character.Account;
                    break;
                case GridType.GuildStorage:
                    if (p.ToGrid == GridType.GuildStorage)
                    {
                        //GuildStore -> GuildStore send Update to other players
                        guildpacket = new S.ItemMove
                        {
                            FromGrid = p.FromGrid,
                            FromSlot = p.FromSlot,
                            ToGrid = p.ToGrid,
                            ToSlot = p.ToSlot,
                            MergeItem = p.MergeItem,
                            Success = true,
                            ObserverPacket = false,
                        };
                    }
                    else
                    {
                        //Sendto MY observers I got item from guild store and what slot?

                        foreach (SConnection con in Connection.Observers)
                        {
                            con.Enqueue(new S.GuildGetItem
                            {
                                Grid = p.ToGrid,
                                Slot = p.ToSlot,
                                Item = fromItem.ToClientInfo(), //To Destination IS to be empty, so Not merging
                            });
                        }

                        guildpacket = new S.ItemChanged
                        {
                            Link = new CellLinkInfo { GridType = p.FromGrid, Slot = p.FromSlot, },
                            Success = true,

                            ObserverPacket = false
                        };
                    }

                    foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                    {
                        PlayerObject player = member.Account.Connection?.Player;

                        if (player == null || player == this) continue;

                        //Send Removal Command
                        player.Enqueue(guildpacket);
                    }

                    if (toItem == null) break; //CAN ONLY BE GS -> GS

                    toItem.Slot = p.FromSlot;
                    break;
            }

            switch (p.ToGrid)
            {
                case GridType.Inventory:
                    fromItem.Slot = p.ToSlot;
                    fromItem.Character = Character;
                    break;
                case GridType.Equipment:
                    sendShape = true;
                    fromItem.Slot = p.ToSlot + Globals.EquipmentOffSet;
                    fromItem.Character = Character;
                    break;
                case GridType.CompanionInventory:
                    fromItem.Slot = p.ToSlot;
                    fromItem.Companion = Companion.UserCompanion;
                    break;
                case GridType.CompanionEquipment:
                    sendCompanionShape = true;
                    fromItem.Slot = p.ToSlot + Globals.EquipmentOffSet;
                    fromItem.Companion = Companion.UserCompanion;
                    break;
                case GridType.PartsStorage:
                    fromItem.Slot = p.ToSlot + Globals.PartsStorageOffset;
                    fromItem.Account = Character.Account;
                    break;
                case GridType.Storage:
                    fromItem.Slot = p.ToSlot;
                    fromItem.Account = Character.Account;
                    break;
                case GridType.GuildStorage:
                    fromItem.Slot = p.ToSlot;
                    fromItem.Guild = Character.Account.GuildMember.Guild;

                    if (p.FromGrid == GridType.GuildStorage) break; //Already Handled

                    //Must be removing from player to GuildStorage, Update Observer's bag
                    foreach (SConnection con in Connection.Observers)
                    {
                        con.Enqueue(new S.ItemChanged
                        {
                            Link = new CellLinkInfo { GridType = p.FromGrid, Slot = p.FromSlot }, //Should Always be Count = 0;
                            Success = true,
                        });
                    }

                    guildpacket = new S.GuildNewItem
                    {
                        Slot = p.ToSlot,
                        Item = fromItem.ToClientInfo(),
                        ObserverPacket = false
                    };

                    foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                    {
                        PlayerObject player = member.Account.Connection?.Player;

                        if (player == null || player == this) continue;

                        player.Enqueue(guildpacket);
                    }

                    break;
            }

            result.Success = true;

            RefreshStats();

            if (sendShape) SendShapeUpdate();
            if (sendCompanionShape) Companion?.SendShapeUpdate();

            if (p.ToGrid == GridType.CompanionEquipment || p.ToGrid == GridType.CompanionInventory || p.FromGrid == GridType.CompanionEquipment || p.FromGrid == GridType.CompanionInventory)
            {
                Companion.SearchTime = DateTime.MinValue;
                Companion.RefreshStats();
            }
        }
        public void ItemSort(C.ItemSort p)
        {
            if (Dead) return;

            UserItem[] array;

            switch (p.Grid)
            {
                case GridType.Inventory:
                    array = Inventory;
                    break;
                case GridType.Storage:
                    array = Storage;
                    break;
                case GridType.PartsStorage:
                    array = PartsStorage;
                    break;
                default:
                    return;
            }

            int length = array.Length;

            if (p.Grid == GridType.Storage)
                length = Math.Min(array.Length, Character.Account.StorageSize);

            List<UserItem> items = new();

            for (int i = 0; i < length; i++)
            {
                var item = array[i];

                if (item == null) continue;

                items.Add(item);

                if (item.Info.StackSize <= 1) continue;
                if (item.Count == item.Info.StackSize) continue;

                var count = item.Count;

                for (int j = i + 1; j < length; j++)
                {
                    var otherItem = array[j];

                    if (otherItem == null) continue;

                    if (item.Info != otherItem.Info) continue;
                    if (item.ExpireTime != otherItem.ExpireTime) continue;

                    if ((item.Flags & UserItemFlags.Expirable) != (otherItem.Flags & UserItemFlags.Expirable)) continue;
                    if ((item.Flags & UserItemFlags.Bound) != (otherItem.Flags & UserItemFlags.Bound)) continue;
                    if ((item.Flags & UserItemFlags.Worthless) != (otherItem.Flags & UserItemFlags.Worthless)) continue;
                    if ((item.Flags & UserItemFlags.NonRefinable) != (otherItem.Flags & UserItemFlags.NonRefinable)) continue;
                    if (!item.Stats.Compare(otherItem.Stats)) continue;

                    count += otherItem.Count;

                    array[j] = null;
                    otherItem.Delete();
                }

                item.Count = count;
            }

            var sorted = items
                .OrderBy(item => item?.Info.ItemType)
                .ThenBy(item => item?.Info.ItemName)
                .ThenBy(item => item?.Count)
                .ToArray();

            for (int i = 0; i < length; i++)
            {
                array[i] = null;
            }

            int index = 0;
            int slot = 0;

            if (p.Grid == GridType.PartsStorage)
            {
                slot = Globals.PartsStorageOffset;
            }

            for (int i = 0; i < sorted.Length; i++)
            {
                var item = sorted[i];

                while (item.Count > item.Info.StackSize)
                {
                    UserItem newItem = SEnvir.CreateFreshItem(item);
                    newItem.Count = item.Info.StackSize;

                    item.Count -= item.Info.StackSize;

                    array[index] = newItem;
                    newItem.Slot = slot;

                    switch (p.Grid)
                    {
                        case GridType.Inventory:
                            newItem.Character = Character;
                            break;
                        case GridType.PartsStorage:
                            newItem.Account = Character.Account;
                            break;
                        case GridType.Storage:
                            newItem.Account = Character.Account;
                            break;
                    }

                    index++;
                    slot++;
                }

                array[index] = item;
                item.Slot = slot;

                index++;
                slot++;
            }

            S.ItemSort result = new()
            {
                Grid = p.Grid,
                Items = array.Where(x => x != null).Select(x => x.ToClientInfo()).ToList(),
                Success = true
            };

            Enqueue(result);
        }
        public void ItemDelete(C.ItemDelete p)
        {
            S.ItemDelete result = new S.ItemDelete
            {
                Grid = p.Grid,
                Slot = p.Slot
            };

            Enqueue(result);

            if (Dead) return;

            UserItem[] array;

            switch (p.Grid)
            {
                case GridType.Inventory:
                    array = Inventory;
                    break;
                default:
                    return;
            }

            if (p.Slot < 0 || p.Slot >= array.Length) return;

            UserItem item = array[p.Slot];

            if (item == null) return;
            if ((item.Flags & UserItemFlags.Locked) == UserItemFlags.Locked) return;
            if ((item.Flags & UserItemFlags.Bound) == UserItemFlags.Bound) return;
            if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return;

            RemoveItem(item);
            array[p.Slot] = null;
            result.Success = true;
        }
        public long GetItemCount(ItemInfo info)
        {
            long count = 0;
            foreach (UserItem item in Inventory)
            {
                if (item == null || item.Info != info) continue;

                count += item.Count;
            }

            if (Companion != null)
            {
                foreach (UserItem item in Companion.Inventory)
                {
                    if (item == null || item.Info != info) continue;

                    count += item.Count;
                }
            }

            return count;
        }
        public void TakeItem(ItemInfo info, long count)
        {
            for (int i = 0; i < Inventory.Length; i++)
            {
                UserItem item = Inventory[i];

                if (item == null || item.Info != info) continue;

                if (item.Count > count)
                {
                    item.Count -= count;

                    Enqueue(new S.ItemChanged { Link = new CellLinkInfo { GridType = GridType.Inventory, Slot = i, Count = item.Count }, Success = true });
                    return;
                }

                count -= item.Count;

                RemoveItem(item);
                Inventory[i] = null;
                item.Delete();

                Enqueue(new S.ItemChanged { Link = new CellLinkInfo { GridType = GridType.Inventory, Slot = i }, Success = true });

                if (count == 0) return;
            }

            for (int i = 0; i < Companion.Inventory.Length; i++)
            {
                UserItem item = Companion.Inventory[i];

                if (item == null || item.Info != info) continue;

                if (item.Count > count)
                {
                    item.Count -= count;

                    Enqueue(new S.ItemChanged { Link = new CellLinkInfo { GridType = GridType.CompanionInventory, Slot = i, Count = item.Count }, Success = true });
                    return;
                }

                count -= item.Count;

                RemoveItem(item);
                Companion.Inventory[i] = null;
                item.Delete();

                Enqueue(new S.ItemChanged { Link = new CellLinkInfo { GridType = GridType.CompanionInventory, Slot = i }, Success = true });

                if (count == 0) return;
            }

            throw new Exception(string.Format("Unable to Take {0}x{1} from {2}", info.ItemName, count, Name));
        }
        public void ItemLock(C.ItemLock p)
        {
            UserItem[] itemArray;

            switch (p.GridType)
            {
                case GridType.Inventory:
                    itemArray = Inventory;
                    break;
                case GridType.Equipment:
                    itemArray = Equipment;
                    break;
                case GridType.PartsStorage:
                    itemArray = PartsStorage;
                    break;
                case GridType.Storage:
                    itemArray = Storage;
                    break;
                case GridType.CompanionInventory:
                    if (Companion == null) return;

                    itemArray = Companion.Inventory;
                    break;
                case GridType.CompanionEquipment:
                    if (Companion == null) return;

                    itemArray = Companion.Equipment;
                    break;
                default:
                    return;
            }

            if (p.SlotIndex < 0 || p.SlotIndex >= itemArray.Length) return;


            UserItem fromItem = itemArray[p.SlotIndex];

            if (fromItem == null) return;

            if (p.Locked)
                fromItem.Flags |= UserItemFlags.Locked;
            else
                fromItem.Flags &= ~UserItemFlags.Locked;

            S.ItemLock result = new S.ItemLock
            {
                Grid = p.GridType,
                Slot = p.SlotIndex,
                Locked = p.Locked,
            };

            Enqueue(result);

        }
        public void ItemSplit(C.ItemSplit p)
        {
            S.ItemSplit result = new S.ItemSplit
            {
                Grid = p.Grid,
                Slot = p.Slot,
                Count = p.Count,
                ObserverPacket = p.Grid != GridType.GuildStorage,
            };

            Enqueue(result);

            if (Dead || p.Count <= 0) return;

            UserItem[] array;

            switch (p.Grid)
            {
                case GridType.Inventory:
                    array = Inventory;
                    break;
                case GridType.PartsStorage:
                    array = PartsStorage;
                    break;
                case GridType.Storage:
                    array = Storage;
                    break;
                case GridType.GuildStorage:
                    if (Character.Account.GuildMember == null) return;

                    if ((Character.Account.GuildMember.Permission & GuildPermission.Storage) != GuildPermission.Storage) return;

                    array = Character.Account.GuildMember.Guild.Storage;
                    break;
                case GridType.CompanionInventory:
                    if (Companion == null) return;

                    array = Companion.Inventory;
                    break;
                default:
                    return;
            }

            if (p.Slot < 0 || p.Slot >= array.Length) return;

            UserItem item = array[p.Slot];

            if (item == null || item.Count <= p.Count || item.Info.StackSize < p.Count) return;

            int length = array.Length;
            if (p.Grid == GridType.CompanionInventory)
                length = Math.Min(array.Length, Companion.Stats[Stat.CompanionInventory]);

            if (p.Grid == GridType.Storage)
                length = Math.Min(array.Length, Character.Account.StorageSize);

            if (p.Grid == GridType.GuildStorage)
                length = Math.Min(array.Length, Character.Account.GuildMember.Guild.StorageSize);

            for (int i = 0; i < length; i++)
            {
                if (array[i] != null) continue;

                if (p.Grid == GridType.GuildStorage && i >= Character.Account.GuildMember.Guild.StorageSize) break;


                result.Success = true;
                result.NewSlot = i;

                item.Count -= p.Count;

                UserItem newItem = SEnvir.CreateFreshItem(item);
                newItem.Count = p.Count;

                array[i] = newItem;
                newItem.Slot = i;

                switch (p.Grid)
                {
                    case GridType.Inventory:
                        newItem.Character = Character;
                        break;
                    case GridType.PartsStorage:
                        newItem.Account = Character.Account;
                        break;
                    case GridType.Storage:
                        newItem.Account = Character.Account;
                        break;
                    case GridType.CompanionInventory:
                        newItem.Companion = Companion.UserCompanion;
                        break;
                    case GridType.GuildStorage:
                        newItem.Guild = Character.Account.GuildMember.Guild;

                        foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                        {
                            PlayerObject player = member.Account.Connection?.Player;

                            if (player == null || player == this) continue;

                            player.Enqueue(new S.ItemChanged
                            {
                                Link = new CellLinkInfo { GridType = p.Grid, Slot = p.Slot, Count = item.Count },
                                Success = true,

                                ObserverPacket = false
                            });

                            player.Enqueue(new S.GuildNewItem
                            {
                                Slot = newItem.Slot,
                                Item = newItem.ToClientInfo(),

                                ObserverPacket = false
                            });
                        }
                        break;
                }

                return;
            }
        }

        public void CurrencyChanged(UserCurrency currency)
        {
            Enqueue(new S.CurrencyChanged { CurrencyIndex = currency.Info.Index, Amount = currency.Amount });
        }
        public void GoldChanged()
        {
            Enqueue(new S.CurrencyChanged { CurrencyIndex = Gold.Info.Index, Amount = Gold.Amount });
        }
        public void HuntGoldChanged()
        {
            Enqueue(new S.CurrencyChanged { CurrencyIndex = HuntGold.Info.Index, Amount = HuntGold.Amount });
        }
        public void GameGoldChanged()
        {
            Enqueue(new S.CurrencyChanged { CurrencyIndex = GameGold.Info.Index, Amount = GameGold.Amount, ObserverPacket = false });
        }

        public void ItemDrop(C.ItemDrop p)
        {
            S.ItemChanged result = new S.ItemChanged
            {
                Link = p.Link
            };
            Enqueue(result);

            if (Dead || !ParseLinks(p.Link))
                return;


            UserItem[] fromArray;

            switch (p.Link.GridType)
            {
                case GridType.Inventory:
                    fromArray = Inventory;
                    break;
                case GridType.CompanionInventory:
                    if (Companion == null) return;

                    fromArray = Companion.Inventory;
                    break;
                default:
                    return;
            }

            if (p.Link.Slot < 0 || p.Link.Slot >= fromArray.Length) return;

            UserItem fromItem = fromArray[p.Link.Slot];

            if (fromItem == null || p.Link.Count > fromItem.Count || !fromItem.Info.CanDrop || (fromItem.Flags & UserItemFlags.Locked) == UserItemFlags.Locked) return;

            if ((fromItem.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return;
            Cell cell = GetDropLocation(1, null);

            if (cell == null) return;

            result.Success = true;

            UserItem dropItem;

            if (p.Link.Count == fromItem.Count)
            {
                dropItem = fromItem;
                RemoveItem(fromItem);
                fromArray[p.Link.Slot] = null;

                result.Link.Count = 0;
            }
            else
            {
                dropItem = SEnvir.CreateFreshItem(fromItem);
                dropItem.Count = p.Link.Count;
                fromItem.Count -= p.Link.Count;

                result.Link.Count = fromItem.Count;
            }

            RefreshWeight();
            Companion?.RefreshWeight();
            dropItem.IsTemporary = true;

            ItemObject ob = new ItemObject
            {
                Item = dropItem,
            };

            if ((fromItem.Flags & UserItemFlags.Bound) == UserItemFlags.Bound)
                ob.Account = Character.Account;

            ob.Spawn(CurrentMap, cell.Location);
        }
        public void CurrencyDrop(C.CurrencyDrop p)
        {
            if (Dead) return;

            var currency = SEnvir.CurrencyInfoList.Binding.FirstOrDefault(x => x.Index == p.CurrencyIndex);

            if (currency == null) return;

            var userCurrency = GetCurrency(currency);

            var amount = userCurrency.Amount;

            if (currency.DropItem == null || !currency.DropItem.CanDrop || p.Amount <= 0 || p.Amount > amount) return;

            Cell cell = GetDropLocation(Config.DropDistance, null);

            if (cell == null) return;

            userCurrency.Amount -= p.Amount;
            CurrencyChanged(userCurrency);

            UserItem dropItem = SEnvir.CreateFreshItem(currency.DropItem);
            dropItem.Count = p.Amount;
            dropItem.IsTemporary = true;

            ItemObject ob = new ItemObject
            {
                Item = dropItem,
            };

            ob.Spawn(CurrentMap, cell.Location);
        }
        public void BeltLinkChanged(C.BeltLinkChanged p)
        {
            if (p.Slot < 0 || p.Slot >= Globals.MaxBeltCount) return;
            if (p.LinkIndex > 0 && p.LinkItemIndex > 0) return;
            if (p.Slot >= Inventory.Length) return;

            ItemInfo info = null;
            UserItem item = null;

            if (p.LinkIndex > 0)
                info = SEnvir.ItemInfoList.Binding.FirstOrDefault(x => x.Index == p.LinkIndex);
            else if (p.LinkItemIndex > 0)
                item = Inventory.FirstOrDefault(x => x?.Index == p.LinkItemIndex);

            foreach (CharacterBeltLink link in Character.BeltLinks)
            {
                if (link.Slot != p.Slot/* && (link.LinkInfoIndex != -1 || link.LinkItemIndex != -1)*/) continue;

                link.Slot = p.Slot;
                link.LinkInfoIndex = info?.Index ?? -1;
                link.LinkItemIndex = item?.Index ?? -1;
                return;
            }

            if (info == null && item == null) return;

            CharacterBeltLink bLink = SEnvir.BeltLinkList.CreateNewObject();

            bLink.Character = Character;
            bLink.Slot = p.Slot;
            bLink.LinkInfoIndex = p.LinkIndex;
            bLink.LinkItemIndex = p.LinkItemIndex;
        }

        public void AutoPotionLinkChanged(C.AutoPotionLinkChanged p)
        {
            if (p.Slot < 0 || p.Slot >= Globals.MaxAutoPotionCount) return;
            if (p.Slot >= Inventory.Length) return;

            ItemInfo info = SEnvir.ItemInfoList.Binding.FirstOrDefault(x => x.Index == p.LinkIndex);

            foreach (AutoPotionLink link in Character.AutoPotionLinks)
            {
                if (link.Slot != p.Slot) continue;

                link.Slot = p.Slot;
                link.LinkInfoIndex = info?.Index ?? -1;
                link.Health = p.Health;
                link.Mana = p.Mana;
                link.Enabled = p.Enabled;
                return;
            }

            AutoPotionLink aLink = SEnvir.AutoPotionLinkList.CreateNewObject();

            aLink.Character = Character;
            aLink.Slot = p.Slot;
            aLink.LinkInfoIndex = info?.Index ?? -1;
            aLink.Health = p.Health;
            aLink.Mana = p.Mana;
            aLink.Enabled = p.Enabled;

            AutoPotions.Add(aLink);
            AutoPotions.Sort((x1, x2) => x1.Slot.CompareTo(x2.Slot));
        }
        public void PickUp()
        {
            if (Dead) return;

            int range = Stats[Stat.PickUpRadius];

            for (int d = 0; d <= range; d++)
            {
                for (int y = CurrentLocation.Y - d; y <= CurrentLocation.Y + d; y++)
                {
                    if (y < 0) continue;
                    if (y >= CurrentMap.Height) break;

                    for (int x = CurrentLocation.X - d; x <= CurrentLocation.X + d; x += Math.Abs(y - CurrentLocation.Y) == d ? 1 : d * 2)
                    {
                        if (x < 0) continue;
                        if (x >= CurrentMap.Width) break;

                        Cell cell = CurrentMap.Cells[x, y]; //Direct Access we've checked the boudaries.

                        if (cell?.Objects == null) continue;

                        foreach (MapObject cellObject in cell.Objects)
                        {
                            if (cellObject.Race != ObjectType.Item) continue;

                            ItemObject item = (ItemObject)cellObject;

                            if (item.PickUpItem(this)) return;
                        }

                    }
                }
            }
        }

        public bool CanWearItem(UserItem item, EquipmentSlot slot)
        {
            if (!Functions.CorrectSlot(item.Info.ItemType, slot) || !CanUseItem(item))
                return false;

            switch (item.Info.ItemType)
            {
                case ItemType.Weapon:
                case ItemType.Torch:
                case ItemType.Shield:
                    if (HandWeight - (Equipment[(int)slot]?.Info.Weight ?? 0) + item.Weight > Stats[Stat.HandWeight]) return false;
                    break;
                case ItemType.Hook:
                case ItemType.Float:
                case ItemType.Bait:
                case ItemType.Finder:
                case ItemType.Reel:
                    if (Equipment[(int)EquipmentSlot.Weapon]?.Info.ItemEffect != ItemEffect.FishingRod) return false;
                    break;
                default:
                    if (WearWeight - (Equipment[(int)slot]?.Info.Weight ?? 0) + item.Weight > Stats[Stat.WearWeight]) return false;
                    break;

            }
            return true;
        }

        public bool DamageItem(GridType grid, int slot, int rate = 1, bool delayStats = false)
        {
            UserItem item;
            switch (grid)
            {
                case GridType.Inventory:
                    item = Inventory[slot];
                    break;
                case GridType.Equipment:
                    item = Equipment[slot];
                    break;
                default:
                    return false;
            }

            if (item == null || item.Info.Durability == 0 || item.CurrentDurability == 0) return false;

            if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return false;

            switch (item.Info.ItemType)
            {
                case ItemType.Nothing:
                case ItemType.Consumable:
                case ItemType.Poison:
                case ItemType.Amulet:
                case ItemType.Scroll:
                    return false;
                case ItemType.Weapon:
                    if (SEnvir.Random.Next(Stats[Stat.Strength]) > 0) return false;
                    break;
                default:
                    if (SEnvir.Random.Next(3) == 0 && SEnvir.Random.Next(Stats[Stat.Strength]) > 0) return false;
                    break;
            }

            item.CurrentDurability = Math.Max(0, item.CurrentDurability - rate);

            Enqueue(new S.ItemDurability
            {
                GridType = grid,
                Slot = slot,
                CurrentDurability = item.CurrentDurability,
            });

            if (item.CurrentDurability == 0)
            {
                SendShapeUpdate();
                RefreshStats();
                return true;
            }
            return false;
        }
        public void DamageDarkStone(int rate = 1)
        {
            DamageItem(GridType.Equipment, (int)EquipmentSlot.Amulet, rate);

            UserItem stone = Equipment[(int)EquipmentSlot.Amulet];

            if (stone == null || stone.CurrentDurability != 0 || stone.Info.Durability <= 0) return;

            RemoveItem(stone);
            Equipment[(int)EquipmentSlot.Amulet] = null;
            stone.Delete();

            Enqueue(new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = GridType.Equipment, Slot = (int)EquipmentSlot.Amulet },
                Success = true,
            });
        }

        public bool UsePoison(int count, out int shape, int requiredShape = -1)
        {
            shape = 0;

            UserItem poison = Equipment[(int)EquipmentSlot.Poison];

            if (poison == null || poison.Info.ItemType != ItemType.Poison || poison.Count < count || (requiredShape > -1 && poison.Info.Shape != requiredShape)) return false;

            shape = poison.Info.Shape;

            poison.Count -= count;

            Enqueue(new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = GridType.Equipment, Slot = (int)EquipmentSlot.Poison, Count = poison.Count },
                Success = true
            });

            if (poison.Count != 0) return true;

            RemoveItem(poison);
            Equipment[(int)EquipmentSlot.Poison] = null;
            poison.Delete();

            RefreshStats();
            RefreshWeight();

            return true;
        }

        public bool UseAmulet(int count, int shape)
        {
            UserItem amulet = Equipment[(int)EquipmentSlot.Amulet];

            if (amulet == null || amulet.Info.ItemType != ItemType.Amulet || amulet.Count < count || amulet.Info.Shape != shape) return false;

            amulet.Count -= count;

            Enqueue(new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = GridType.Equipment, Slot = (int)EquipmentSlot.Amulet, Count = amulet.Count },
                Success = true
            });


            if (amulet.Count != 0) return true;

            RemoveItem(amulet);
            Equipment[(int)EquipmentSlot.Amulet] = null;
            amulet.Delete();

            RefreshStats();
            RefreshWeight();

            return true;
        }

        public bool UseAmulet(int count, int shape, out Stats stats)
        {
            stats = null;
            UserItem amulet = Equipment[(int)EquipmentSlot.Amulet];

            if (amulet == null || amulet.Info.ItemType != ItemType.Amulet || amulet.Count < count || amulet.Info.Shape != shape) return false;

            amulet.Count -= count;

            Enqueue(new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = GridType.Equipment, Slot = (int)EquipmentSlot.Amulet, Count = amulet.Count },
                Success = true
            });

            stats = new Stats(amulet.Info.Stats);

            if (amulet.Count != 0) return true;

            RemoveItem(amulet);
            Equipment[(int)EquipmentSlot.Amulet] = null;
            amulet.Delete();

            RefreshStats();
            RefreshWeight();

            return true;
        }

        public bool UseOilOfBenediction()
        {
            UserItem weapon = Equipment[(int)EquipmentSlot.Weapon];

            if (weapon == null) return false;

            int luck = 0;

            foreach (UserItemStat stat in weapon.AddedStats)
            {
                if (stat.Stat != Stat.Luck) continue;
                if (stat.StatSource != StatSource.Enhancement) continue;

                luck += stat.Amount;
            }

            if (luck >= Config.MaxLuck) return false;

            S.ItemStatsChanged result = new S.ItemStatsChanged { GridType = GridType.Equipment, Slot = (int)EquipmentSlot.Weapon, NewStats = new Stats() };
            Enqueue(result);

            if (luck > Config.MaxCurse && SEnvir.Random.Next(Config.CurseRate) == 0)
            {
                weapon.AddStat(Stat.Luck, -1, StatSource.Enhancement);
                weapon.StatsChanged();
                result.NewStats[Stat.Luck]--;

                Stats[Stat.Luck]--;
            }
            else if (luck <= 0 || SEnvir.Random.Next(luck * Config.LuckRate) == 0)
            {
                weapon.AddStat(Stat.Luck, 1, StatSource.Enhancement);
                weapon.StatsChanged();
                result.NewStats[Stat.Luck]++;

                Stats[Stat.Luck]++;
            }

            return true;
        }
        public bool UseOilOfConservation()
        {
            UserItem weapon = Equipment[(int)EquipmentSlot.Weapon];

            int strength = 0;

            if (weapon == null) return false;

            foreach (UserItemStat stat in weapon.AddedStats)
            {
                if (stat.Stat != Stat.Strength) continue;
                if (stat.StatSource != StatSource.Enhancement) continue;

                strength += stat.Amount;
            }

            if (strength >= Config.MaxLuck) return false;



            S.ItemStatsChanged result = new S.ItemStatsChanged { GridType = GridType.Equipment, Slot = (int)EquipmentSlot.Weapon, NewStats = new Stats() };
            Enqueue(result);

            if (strength > 0 && SEnvir.Random.Next(Config.StrengthLossRate) == 0)
            {
                weapon.AddStat(Stat.Strength, -1, StatSource.Enhancement);
                weapon.StatsChanged();
                result.NewStats[Stat.Strength]--;
            }
            else if (strength <= 0 || SEnvir.Random.Next(strength * Config.StrengthAddRate) == 0)
            {
                weapon.AddStat(Stat.Strength, 1, StatSource.Enhancement);
                weapon.StatsChanged();
                result.NewStats[Stat.Strength]++;
            }

            return true;
        }

        public bool SpecialRepair(EquipmentSlot slot)
        {
            UserItem item = Equipment[(int)slot];

            if (item == null) return false;

            if (item.CurrentDurability >= item.MaxDurability || !item.Info.CanRepair) return false;

            if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return false;

            item.CurrentDurability = item.MaxDurability;

            Enqueue(new S.NPCRepair { Links = new List<CellLinkInfo> { new CellLinkInfo { GridType = GridType.Equipment, Slot = (int)slot, Count = 1 } }, Special = true, Success = true, SpecialRepairDelay = TimeSpan.Zero });

            return true;
        }

        public void HelmetToggle(bool value)
        {
            if (Character.HideHelmet == value) return;

            Character.HideHelmet = value;
            SendShapeUpdate();
            Enqueue(new S.HelmetToggle { HideHelmet = Character.HideHelmet });
        }
        #endregion

        #region Change

        public void GenderChange(C.GenderChange p)
        {
            switch (p.Gender)
            {
                case MirGender.Male:
                    if (Gender == MirGender.Male) return;
                    break;
                case MirGender.Female:
                    if (Gender == MirGender.Female) return;
                    break;
            }

            if (p.HairType < 0) return;

            if ((p.HairType == 0 && p.HairColour.ToArgb() != 0) || (p.HairType != 0 && p.HairColour.A != 255)) return;

            if (Equipment[(int)EquipmentSlot.Armour] != null) return;

            switch (Class)
            {
                case MirClass.Warrior:
                    if (p.HairType > (p.Gender == MirGender.Male ? 10 : 11)) return;
                    break;
                case MirClass.Wizard:
                    if (p.HairType > (p.Gender == MirGender.Male ? 10 : 11)) return;
                    break;
                case MirClass.Taoist:
                    if (p.HairType > (p.Gender == MirGender.Male ? 10 : 11)) return;
                    break;
                case MirClass.Assassin:
                    if (p.HairType > 5) return;
                    break;
            }

            int index = 0;
            UserItem item = null;

            for (int i = 0; i < Inventory.Length; i++)
            {
                if (Inventory[i] == null || Inventory[i].Info.ItemEffect != ItemEffect.GenderChange) continue;

                if (!CanUseItem(Inventory[i])) continue;

                index = i;
                item = Inventory[i];
                break;
            }

            if (item == null) return;

            S.ItemChanged result = new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = GridType.Inventory, Slot = index },
                Success = true
            };
            Enqueue(result);

            if (item.Count > 1)
            {
                item.Count--;
                result.Link.Count = item.Count;
            }
            else
            {
                RemoveItem(item);
                Inventory[index] = null;
                item.Delete();

                result.Link.Count = 0;
            }


            Character.Gender = p.Gender;
            Character.HairType = p.HairType;
            Character.HairColour = p.HairColour;

            SendChangeUpdate();
        }
        public void HairChange(C.HairChange p)
        {
            if (p.HairType < 0) return;

            if ((p.HairType == 0 && p.HairColour.ToArgb() != 0) || (p.HairType != 0 && p.HairColour.A != 255)) return;

            switch (Class)
            {
                case MirClass.Warrior:
                    if (p.HairType > (Gender == MirGender.Male ? 10 : 11)) return;
                    break;
                case MirClass.Wizard:
                    if (p.HairType > (Gender == MirGender.Male ? 10 : 11)) return;
                    break;
                case MirClass.Taoist:
                    if (p.HairType > (Gender == MirGender.Male ? 10 : 11)) return;
                    break;
                case MirClass.Assassin:
                    if (p.HairType > 5) return;
                    break;
            }

            int index = 0;
            UserItem item = null;

            for (int i = 0; i < Inventory.Length; i++)
            {
                if (Inventory[i] == null || Inventory[i].Info.ItemEffect != ItemEffect.HairChange) continue;

                if (!CanUseItem(Inventory[i])) continue;

                index = i;
                item = Inventory[i];
                break;
            }

            if (item == null) return;

            S.ItemChanged result = new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = GridType.Inventory, Slot = index },
                Success = true
            };
            Enqueue(result);

            if (item.Count > 1)
            {
                item.Count--;
                result.Link.Count = item.Count;
            }
            else
            {
                RemoveItem(item);
                Inventory[index] = null;
                item.Delete();

                result.Link.Count = 0;
            }


            Character.HairType = p.HairType;
            Character.HairColour = p.HairColour;

            SendChangeUpdate();
        }
        public void ArmourDye(Color colour)
        {
            if (Equipment[(int)EquipmentSlot.Armour] == null) return;

            switch (Class)
            {
                case MirClass.Warrior:
                case MirClass.Wizard:
                case MirClass.Taoist:
                    if (colour.A != 255) return;
                    break;
                case MirClass.Assassin:
                    if (colour.ToArgb() != 0) return;
                    return;
            }

            int index = 0;
            UserItem item = null;

            for (int i = 0; i < Inventory.Length; i++)
            {
                if (Inventory[i] == null || Inventory[i].Info.ItemEffect != ItemEffect.ArmourDye) continue;

                if (!CanUseItem(Inventory[i])) continue;

                index = i;
                item = Inventory[i];
                break;
            }

            if (item == null) return;

            S.ItemChanged result = new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = GridType.Inventory, Slot = index },
                Success = true
            };
            Enqueue(result);

            if (item.Count > 1)
            {
                item.Count--;
                result.Link.Count = item.Count;
            }
            else
            {
                RemoveItem(item);
                Inventory[index] = null;
                item.Delete();

                result.Link.Count = 0;
            }

            Equipment[(int)EquipmentSlot.Armour].Colour = colour;


            SendChangeUpdate();
        }
        public void NameChange(string newName)
        {
            if (!Globals.CharacterReg.IsMatch(newName))
            {
                Connection.ReceiveChat("Unacceptable character name.", MessageType.System);
                return;
            }

            if (newName == Name)
            {
                Connection.ReceiveChat($"Your name is already {newName}.", MessageType.System);
                return;
            }

            for (int i = 0; i < SEnvir.CharacterInfoList.Count; i++)
                if (string.Compare(SEnvir.CharacterInfoList[i].CharacterName, newName, StringComparison.OrdinalIgnoreCase) == 0)
                {
                    if (SEnvir.CharacterInfoList[i].Account == Character.Account) continue;

                    Connection.ReceiveChat("This name is already in use.", MessageType.System);
                    return;
                }


            int index = 0;
            UserItem item = null;

            for (int i = 0; i < Inventory.Length; i++)
            {
                if (Inventory[i] == null || Inventory[i].Info.ItemEffect != ItemEffect.NameChange) continue;

                if (!CanUseItem(Inventory[i])) continue;

                index = i;
                item = Inventory[i];
                break;
            }
            if (item == null) return;

            S.ItemChanged result = new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = GridType.Inventory, Slot = index },
                Success = true
            };
            Enqueue(result);

            if (item.Count > 1)
            {
                item.Count--;
                result.Link.Count = item.Count;
            }
            else
            {
                RemoveItem(item);
                Inventory[index] = null;
                item.Delete();

                result.Link.Count = 0;
            }

            SEnvir.Log($"[NAME CHANGED] Old: {Name}, New: {newName}.", true);
            Name = newName;

            SendChangeUpdate();
        }

        public void CaptionChange(string newCaption)
        {
            int index = 0;
            UserItem item = null;

            for (int i = 0; i < Inventory.Length; i++)
            {
                if (Inventory[i] is null || Inventory[i].Info.ItemEffect is not ItemEffect.Caption) continue;

                if (!CanUseItem(Inventory[i])) continue;

                index = i;
                item = Inventory[i];
                break;
            }

            if (item == null) return;

            S.ItemChanged result = new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = GridType.Inventory, Slot = index },
                Success = true
            };
            Enqueue(result);

            if (item.Count > 1)
            {
                item.Count--;
                result.Link.Count = item.Count;
            }
            else
            {
                RemoveItem(item);
                Inventory[index] = null;
                item.Delete();

                result.Link.Count = 0;
            }
            Character.Caption = newCaption;
            Caption = newCaption;
            SEnvir.Log($"[CAPTION CHANGED] {Character.CharacterName} caption changed to: {Caption}", true);
            Connection.ReceiveChat($"Your caption changed to: {Caption}.", MessageType.System);


            SendChangeUpdate();
        }
        public void FortuneCheck(int index)
        {
            if (!Config.EnableFortune || SEnvir.FortuneCheckerInfo == null) return;

            long count = GetItemCount(SEnvir.FortuneCheckerInfo);

            if (count == 0)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.NeedItem, SEnvir.FortuneCheckerInfo.ItemName), MessageType.System);
                return;
            }

            ItemInfo info = SEnvir.ItemInfoList.Binding.FirstOrDefault(x => x.Index == index);

            if (info == null || info.Drops.Count == 0) return;

            if (Config.TestServer && !SEnvir.IsCurrencyItem(info)) return;

            UserFortuneInfo savedFortune = null;

            foreach (UserFortuneInfo fortune in Character.Account.Fortunes)
            {
                if (fortune.Item != info) continue;

                savedFortune = fortune;
                break;
            }

            TakeItem(SEnvir.FortuneCheckerInfo, 1);

            if (savedFortune == null)
            {
                savedFortune = SEnvir.UserFortuneInfoList.CreateNewObject();
                savedFortune.Account = Character.Account;
                savedFortune.Item = info;
            }

            UserDrop drop = Character.Account.UserDrops.FirstOrDefault(x => x.Item == info);

            savedFortune.CheckTime = SEnvir.Now;


            if (drop != null)
            {
                savedFortune.DropCount = drop.DropCount;
                savedFortune.DropProgress = drop.Progress;
            }

            Enqueue(new S.FortuneUpdate { Fortunes = new List<ClientFortuneInfo> { savedFortune.ToClientInfo() } });
        }


        #endregion
    }
}
