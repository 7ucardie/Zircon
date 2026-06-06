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
        #region Instance / Dungeon Finder

        public void JoinInstance(C.JoinInstance p)
        {
            var instance = SEnvir.InstanceInfoList.Binding.FirstOrDefault(x => x.Index == p.Index);

            if (instance == null)
            {
                return;
            }

            if (CurrentMap.Instance != null)
            {
                //Cannot move from one instance to another
                return;
            }

            S.JoinInstance joinResult = new S.JoinInstance { Success = false };

            //Load up instance
            var (index, result) = GetInstance(instance, dungeonFinder: true);

            joinResult.Result = result;

            if (result != InstanceResult.Success)
            {
                SendInstanceMessage(instance, joinResult.Result);
                Enqueue(joinResult);
                return;
            }

            joinResult.Success = true;

            if (instance.Type == InstanceType.Group)
            {
                var map = SEnvir.GetMap(instance.ConnectRegion.Map, instance, index.Value);

                if (map.Players.Count == 0)
                {
                    foreach (PlayerObject member in GroupMembers)
                    {
                        if (!member.Teleport(instance.ConnectRegion, instance, index.Value))
                            member.SendInstanceMessage(instance, InstanceResult.NoMap);
                    }

                    Enqueue(joinResult);
                    return;
                }
            }

            if (!Teleport(instance.ConnectRegion, instance, index.Value))
            {
                joinResult.Success = false;
                joinResult.Result = InstanceResult.NoMap;
                SendInstanceMessage(instance, joinResult.Result);
            }
            else
            {
                LogMilestone(MilestoneType.InstanceJoin, 1, instance: instance);
            }

            Enqueue(joinResult);
        }

        public (byte? index, InstanceResult result) GetInstance(InstanceInfo instance, bool checkOnly = false, bool dungeonFinder = false, bool walkOn = false)
        {
            if (instance.ConnectRegion == null && !walkOn)
                return (null, InstanceResult.ConnectRegionNotSet);

            if (instance.MinPlayerLevel > 0 && Level < instance.MinPlayerLevel || instance.MaxPlayerLevel > 0 && Level > instance.MaxPlayerLevel)
                return (null, InstanceResult.InsufficientLevel);

            if (dungeonFinder && instance.SafeZoneOnly && !InSafeZone)
                return (null, InstanceResult.SafeZoneOnly);

            if (instance.UserRecord.ContainsKey(Name) && !instance.AllowRejoin)
                return (null, InstanceResult.NoRejoin);

            var mapInstance = SEnvir.GetInstance(instance);

            if (mapInstance == null)
                return (null, InstanceResult.Invalid);

            switch (instance.Type)
            {
                case InstanceType.Player:
                    {
                        if (instance.UserCooldown.TryGetValue(Name, out DateTime cooldown))
                        {
                            if (cooldown > SEnvir.Now)
                                return (null, InstanceResult.UserCooldown);
                        }

                        if (instance.UserRecord.ContainsKey(Name))
                        {
                            if (CheckInstanceFreeSpace(instance, instance.UserRecord[Name]))
                            {
                                if (!checkOnly)
                                    instance.UserCooldown.Remove(Name);

                                return (instance.UserRecord[Name], InstanceResult.Success);
                            }
                        }

                        for (byte i = 0; i < mapInstance.Length; i++)
                        {
                            if (CheckInstanceFreeSpace(instance, i))
                            {
                                if (!checkOnly)
                                {
                                    if (instance.UserRecord.ContainsKey(Name))
                                    {
                                        instance.UserRecord[Name] = i;
                                    }
                                    else
                                    {
                                        instance.UserRecord.Add(Name, i);
                                    }
                                }

                                if (!checkOnly)
                                    instance.UserCooldown.Remove(Name);

                                return (i, InstanceResult.Success);
                            }
                        }
                    }
                    break;
                case InstanceType.Group:
                    {
                        if (instance.UserCooldown.TryGetValue(Name, out DateTime cooldown))
                        {
                            if (cooldown > SEnvir.Now)
                                return (null, InstanceResult.UserCooldown);
                        }

                        if (GroupMembers == null)
                            return (null, InstanceResult.NotInGroup);

                        if (instance.MinPlayerCount > 1 && (GroupMembers.Count < instance.MinPlayerCount))
                            return (null, InstanceResult.TooFewInGroup);

                        if (instance.MaxPlayerCount > 1 && (GroupMembers.Count > instance.MaxPlayerCount))
                            return (null, InstanceResult.TooManyInGroup);

                        foreach (var member in GroupMembers)
                        {
                            if (member.CurrentMap.Instance == instance)
                            {
                                var sequence = member.CurrentMap.InstanceSequence;

                                if (CheckInstanceFreeSpace(instance, sequence))
                                {
                                    if (!checkOnly)
                                        instance.UserCooldown.Remove(Name);

                                    return (sequence, InstanceResult.Success);
                                }

                                return (sequence, InstanceResult.Invalid);
                            }
                        }

                        if (instance.UserRecord.ContainsKey(Name))
                        {
                            if (CheckInstanceFreeSpace(instance, instance.UserRecord[Name]))
                            {
                                if (!checkOnly)
                                    instance.UserCooldown.Remove(Name);

                                return (instance.UserRecord[Name], InstanceResult.Success);
                            }
                        }

                        if (dungeonFinder && GroupMembers[0] != this)
                            return (null, InstanceResult.NotGroupLeader);
                    }
                    break;
                case InstanceType.Guild:
                    {
                        if (Character.Account.GuildMember == null)
                            return (null, InstanceResult.NotInGuild);

                        if (instance.GuildCooldown.TryGetValue(Character.Account.GuildMember.Guild.GuildName, out DateTime cooldown))
                        {
                            if (cooldown > SEnvir.Now)
                            {
                                return (null, InstanceResult.GuildCooldown);
                            }
                        }

                        foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                        {
                            if (member.Account.Connection?.Player?.CurrentMap.Instance == instance)
                            {
                                var sequence = member.Account.Connection.Player.CurrentMap.InstanceSequence;

                                if (CheckInstanceFreeSpace(instance, sequence))
                                {
                                    if (!checkOnly)
                                        instance.GuildCooldown.Remove(Character.Account.GuildMember.Guild.GuildName);

                                    return (sequence, InstanceResult.Success);
                                }

                                return (sequence, InstanceResult.Invalid);
                            }
                        }

                        if (instance.UserRecord.ContainsKey(Name))
                        {
                            if (CheckInstanceFreeSpace(instance, instance.UserRecord[Name]))
                            {
                                if (!checkOnly)
                                    instance.GuildCooldown.Remove(Character.Account.GuildMember.Guild.GuildName);

                                return (instance.UserRecord[Name], InstanceResult.Success);
                            }
                        }
                    }
                    break;
                case InstanceType.Castle:
                    {
                        if (Character.Account.GuildMember == null)
                            return (null, InstanceResult.NotInGuild);

                        if (Character.Account.GuildMember.Guild.Castle == null)
                            return (null, InstanceResult.NotInGuild);

                        if (instance.GuildCooldown.TryGetValue(Character.Account.GuildMember.Guild.GuildName, out DateTime cooldown))
                        {
                            if (cooldown > SEnvir.Now)
                                return (null, InstanceResult.GuildCooldown);
                        }

                        foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                        {
                            if (member.Account.Connection?.Player?.CurrentMap.Instance == instance)
                            {
                                var sequence = member.Account.Connection.Player.CurrentMap.InstanceSequence;

                                if (CheckInstanceFreeSpace(instance, sequence))
                                {
                                    if (!checkOnly)
                                        instance.GuildCooldown.Remove(Character.Account.GuildMember.Guild.GuildName);

                                    return (sequence, InstanceResult.Success);
                                }

                                return (sequence, InstanceResult.Invalid);
                            }
                        }

                        if (instance.UserRecord.ContainsKey(Name))
                        {
                            if (CheckInstanceFreeSpace(instance, instance.UserRecord[Name]))
                            {
                                if (!checkOnly)
                                    instance.GuildCooldown.Remove(Character.Account.GuildMember.Guild.GuildName);

                                return (instance.UserRecord[Name], InstanceResult.Success);
                            }
                        }
                    }
                    break;
            }

            byte? instanceSequence = null;
            for (byte i = 0; i < mapInstance.Length; i++)
            {
                if (mapInstance[i] == null)
                {
                    instanceSequence = i;
                    break;
                }
            }

            if (instanceSequence == null)
                return (null, InstanceResult.NoSlots);

            if (instance.RequiredItem != null)
            {
                if (GetItemCount(instance.RequiredItem) == 0)
                    return (null, InstanceResult.MissingItem);

                if (instance.RequiredItemSingleUse && !checkOnly)
                    TakeItem(instance.RequiredItem, 1);
            }

            if (!checkOnly)
            {
                SEnvir.LoadInstance(instance, instanceSequence.Value);

                if (instance.UserRecord.ContainsKey(Name))
                {
                    instance.UserRecord[Name] = instanceSequence.Value;
                }
                else
                {
                    instance.UserRecord.Add(Name, instanceSequence.Value);
                }
            }

            return (instanceSequence.Value, InstanceResult.Success);
        }

        public bool CheckInstanceFreeSpace(InstanceInfo instance, int instanceSequence)
        {
            var mapInstance = SEnvir.GetInstance(instance);

            if (mapInstance == null)
                return false;

            if (instanceSequence < 0 || instanceSequence >= mapInstance.Length)
                return false;

            var maps = mapInstance[instanceSequence];

            if (maps == null)
                return false;

            if (instance.MaxPlayerCount > 0)
            {
                if (instance.SavePlace)
                {
                    var instanceUserRecord = new List<string>();

                    foreach (var userRecord in instance.UserRecord)
                    {
                        if (userRecord.Value != instanceSequence) continue;
                        instanceUserRecord.Add(userRecord.Key);
                    }

                    if (!instanceUserRecord.Contains(Name))
                    {
                        if (instanceUserRecord.Count >= instance.MaxPlayerCount)
                            return false;
                    }
                }
                else
                {
                    var playersOnInstance = maps.Values.SelectMany(x => x.Players);

                    if (playersOnInstance.Count() >= instance.MaxPlayerCount)
                        return false;
                }
            }

            return true;
        }

        public void SetTimer(string key, DateTime expiry)
        {
            var seconds = Math.Max(0, (int)(expiry - SEnvir.Now).TotalSeconds);

            Enqueue(new S.SetTimer { Key = key, Type = 0, Seconds = seconds });
        }

        public void SendInstanceMessage(InstanceInfo instance, InstanceResult result)
        {
            switch (result)
            {
                case InstanceResult.Invalid:
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.InstanceInvalid, MessageType.System);
                    }
                    break;
                case InstanceResult.InsufficientLevel:
                    {
                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.InstanceInsufficientLevel, instance.MinPlayerLevel, instance.MaxPlayerLevel), MessageType.System);
                    }
                    break;
                case InstanceResult.SafeZoneOnly:
                    {
                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.InstanceSafeZoneOnly, instance.MinPlayerLevel, instance.MaxPlayerLevel), MessageType.System);
                    }
                    break;
                case InstanceResult.NotInGroup:
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.InstanceNotInGroup, MessageType.System);
                    }
                    break;
                case InstanceResult.NotInGuild:
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.InstanceNotInGuild, MessageType.System);
                    }
                    break;
                case InstanceResult.NotInCastle:
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.InstanceNotInCastle, MessageType.System);
                    }
                    break;
                case InstanceResult.TooFewInGroup:
                    {
                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.InstanceTooFewInGroup, instance.MinPlayerCount), MessageType.System);
                    }
                    break;
                case InstanceResult.TooManyInGroup:
                    {
                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.InstanceTooManyInGroup, instance.MaxPlayerCount), MessageType.System);
                    }
                    break;
                case InstanceResult.ConnectRegionNotSet:
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.InstanceConnectRegionNotSet, MessageType.System);
                    }
                    break;
                case InstanceResult.NoSlots:
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.InstanceNoSlots, MessageType.System);
                    }
                    break;
                case InstanceResult.NoRejoin:
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.InstanceNoRejoin, MessageType.System);
                    }
                    break;
                case InstanceResult.MissingItem:
                    {
                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.InstanceMissingItem, instance.RequiredItem.ItemName), MessageType.System);
                    }
                    break;
                case InstanceResult.UserCooldown:
                    {
                        var cooldown = instance.UserCooldown[Name];
                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.InstanceUserCooldown, cooldown), MessageType.System);
                    }
                    break;
                case InstanceResult.GuildCooldown:
                    {
                        var cooldown = instance.GuildCooldown[Character.Account.GuildMember.Guild.GuildName];
                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.InstanceGuildCooldown, cooldown), MessageType.System);
                    }
                    break;
                case InstanceResult.NotGroupLeader:
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.InstanceNotGroupLeader, MessageType.System);
                    }
                    break;
                case InstanceResult.NoMap:
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.InstanceNoMap, MessageType.System);
                    }
                    break;
            }
        }

        #endregion

        #region Currency

        public UserCurrency GetCurrency(ItemInfo item)
        {
            var info = SEnvir.CurrencyInfoList.Binding.FirstOrDefault(x => x.DropItem == item);

            if (info == null)
            {
                return null;
            }

            return Character.Account.Currencies.First(x => x.Info == info);
        }

        public UserCurrency GetCurrency(CurrencyInfo info)
        {
            return Character.Account.Currencies.First(x => x.Info == info);
        }

        #endregion

        #region Friends

        public void UpdateOnlineState(bool sendMessage = false)
        {
            foreach (var info in Character.FriendedBy)
            {
                if (info.Character.Player == null) continue;

                var clientInfo = info.ToClientInfo();

                info.Character.Player.Enqueue(new S.FriendUpdate { Info = clientInfo });

                if (sendMessage && clientInfo.State == OnlineState.Online)
                {
                    info.Character.Player.Connection.ReceiveChatWithObservers(con => string.Format(con.Language.FriendStateChanged, clientInfo.Name, clientInfo.State), MessageType.System);
                }
            }
        }

        #endregion

        #region Discipline

        public void GainDisciplineExperience(int amount)
        {
            if (Character.Discipline == null)
                return;

            Character.Discipline.Experience += amount;

            Enqueue(new S.DisciplineExperienceChanged { Experience = Character.Discipline.Experience });
        }

        public void IncreaseDiscipline()
        {
            int currentLevel = 0;

            if (Character.Discipline != null)
                currentLevel = Character.Discipline.Level;

            var nextLevel = SEnvir.DisciplineInfoList.Binding.FirstOrDefault(x => x.Level == (currentLevel + 1));

            if (nextLevel == null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.DisciplineMaxLevel, MessageType.System);
                return;
            }

            if (Level < nextLevel.RequiredLevel)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.DisciplineRequiredLevel, nextLevel.RequiredLevel), MessageType.System);
                return;
            }

            if (Gold.Amount < nextLevel.RequiredGold)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.DisciplineRequiredGold, nextLevel.RequiredGold), MessageType.System);
                return;
            }

            var currentExp = Character?.Discipline?.Experience ?? 0;

            if (currentExp < nextLevel.RequiredExperience)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.DisciplineRequiredExp, nextLevel.RequiredExperience), MessageType.System);
                return;
            }

            UserDiscipline uFocus = Character.Discipline;

            if (uFocus == null)
            {
                uFocus = SEnvir.UserDisciplineList.CreateNewObject();
                Character.Discipline = uFocus;
            }

            if (nextLevel.RequiredGold > 0)
            {
                Gold.Amount -= nextLevel.RequiredGold;
                GoldChanged();
            }

            uFocus.Info = nextLevel;
            uFocus.Level = nextLevel.Level;

            var mInfos = SEnvir.MagicInfoList.Binding
                .Where(x => x.School == MagicSchool.Discipline && x.Class == Class)
                .OrderBy(x => x.NeedLevel1)
                .Take(4);

            var mInfo = mInfos.FirstOrDefault(x => x.NeedLevel1 <= nextLevel.RequiredLevel && !GetMagic(x.Magic, out MagicObject _));

            if (mInfo != null)
            {
                UserMagic uMagic = SEnvir.UserMagicList.CreateNewObject();
                uMagic.Character = Character;
                uMagic.Info = mInfo;

                SetupMagic(uMagic);

                uFocus.Magics.Add(uMagic);

                LogMilestone(MilestoneType.SkillLearn, 1, magic: mInfo);

                Enqueue(new S.NewMagic { Magic = uMagic.ToClientInfo() });
            }

            RefreshStats();

            Enqueue(new S.DisciplineUpdate { Discipline = uFocus.ToClientInfo() });
        }

        #endregion

        #region Loot Boxes

        public void LootBoxOpen(LootBoxOpen p)
        {
            if (p.Slot < 0 || p.Slot >= Inventory.Length) return;

            UserItem item = Inventory[p.Slot];

            if (item == null || item.Info.ItemType != ItemType.LootBox) return;

            LootBoxUpdate(item, p.Slot);
        }

        public void LootBoxReroll(LootBoxReroll p)
        {
            if (p.Slot < 0 || p.Slot >= Inventory.Length) return;

            UserItem item = Inventory[p.Slot];

            if (item == null || item.Info.ItemType != ItemType.LootBox) return;

            var lootBoxInfo = SEnvir.LootBoxInfoList.Binding.FirstOrDefault(x => x.Index == item.Info.Shape);
            if (lootBoxInfo == null) return;

            var remainingShuffles = item.Stats[Stat.Counter1];
            if (remainingShuffles <= 0) return;

            var state = item.Stats[Stat.Counter2];
            if (state > 1) return; // Already confirmed 

            var currency = GetCurrency(lootBoxInfo.Currency) ?? GameGold;

            if (currency.Amount < Globals.LootBoxRerollCost) return;
            currency.Amount -= Globals.LootBoxRerollCost;

            CurrencyChanged(currency);

            item.AddStat(Stat.Random1, SEnvir.Random.Next(byte.MaxValue), StatSource.Added);
            item.AddStat(Stat.Counter1, -1, StatSource.Added);
            item.StatsChanged();

            Enqueue(new S.ItemStatsRefreshed
            {
                GridType = GridType.Inventory,
                Slot = p.Slot,
                NewStats = new Stats(item.Stats, true)
            });

            LootBoxUpdate(item, p.Slot);
        }

        public void LootBoxConfirmSelection(LootBoxConfirmSelection p)
        {
            if (p.Slot < 0 || p.Slot >= Inventory.Length) return;

            UserItem item = Inventory[p.Slot];

            if (item == null || item.Info.ItemType != ItemType.LootBox) return;

            var lootBoxInfo = SEnvir.LootBoxInfoList.Binding.FirstOrDefault(x => x.Index == item.Info.Shape);
            if (lootBoxInfo == null) return;

            var state = item.Stats[Stat.Counter2];
            if (state > 1) return; // Already confirmed 

            item.AddStat(Stat.Counter2, 1, StatSource.Added);
            item.StatsChanged();

            Enqueue(new S.ItemStatsRefreshed
            {
                GridType = GridType.Inventory,
                Slot = p.Slot,
                NewStats = new Stats(item.Stats, true)
            });

            LootBoxUpdate(item, p.Slot);
        }

        public void LootBoxReveal(LootBoxReveal p)
        {
            if (p.Slot < 0 || p.Slot >= Inventory.Length) return;

            UserItem item = Inventory[p.Slot];

            if (item == null || item.Info.ItemType != ItemType.LootBox) return;

            var lootBoxInfo = SEnvir.LootBoxInfoList.Binding.FirstOrDefault(x => x.Index == item.Info.Shape);
            if (lootBoxInfo == null) return;

            if (p.Choice < 0 || p.Choice >= LootBoxInfo.SlotSize) return;

            var openCount = 0;

            for (int i = 0; i < LootBoxInfo.SlotSize; i++)
            {
                if ((item.CurrentDurability & (1 << i)) != 0)
                    openCount++;
            }

            var currency = GetCurrency(lootBoxInfo.Currency) ?? GameGold;

            var totalCost = openCount * Globals.LootBoxRevealCost;

            if (currency.Amount < totalCost) return;
            currency.Amount -= totalCost;

            CurrencyChanged(currency);

            // Update durability to mark the slot as revealed
            item.CurrentDurability |= (1 << p.Choice);

            Enqueue(new S.ItemDurability
            {
                GridType = GridType.Inventory,
                Slot = p.Slot,
                CurrentDurability = item.CurrentDurability,
            });

            LootBoxUpdate(item, p.Slot);
        }

        private void LootBoxUpdate(UserItem item, int slot)
        {
            var lootBoxInfo = SEnvir.LootBoxInfoList.Binding.FirstOrDefault(x => x.Index == item.Info.Shape);
            if (lootBoxInfo == null) return;

            var lootBoxContents = lootBoxInfo.Contents.ToList();

            // Shuffle the full list based on the random 1 seed
            Functions.Shuffle(lootBoxContents, item.Stats[Stat.Random1]);

            // Take the top selection based on slot amount
            var taken = lootBoxContents.Take(LootBoxInfo.SlotSize).ToList();

            // Calculate how many more items are needed to reach SlotSize
            int itemsToAdd = LootBoxInfo.SlotSize - taken.Count;

            // If more items are needed, pad the list with default values
            if (itemsToAdd > 0)
            {
                taken.AddRange(Enumerable.Repeat(default(LootBoxItemInfo), itemsToAdd));
            }

            var items = new List<ClientLootBoxItemInfo>();

            var lootBoxState = item.Stats[Stat.Counter2];

            if (lootBoxState > 1) // Confirmed Choice
            {
                // Shuffle the taken list based on random 2 seed
                Functions.Shuffle(taken, item.Stats[Stat.Random2]);

                var lockState = item.CurrentDurability;

                for (int i = 0; i < LootBoxInfo.SlotSize; i++)
                {
                    bool unlocked = (lockState & (1 << i)) != 0;

                    if (unlocked)
                    {
                        var content = taken[i];

                        if (content == default(LootBoxItemInfo))
                        {
                            items.Add(new ClientLootBoxItemInfo { ItemIndex = -1, Amount = 1, Slot = i });
                        }
                        else
                        {
                            items.Add(new ClientLootBoxItemInfo { ItemIndex = taken[i].Item.Index, Amount = taken[i].Amount, Slot = i });
                        }
                    }
                }

                Enqueue(new S.LootBoxOpen { Slot = slot, Items = items });
            }
            else
            {
                for (int i = 0; i < taken.Count; i++)
                {
                    items.Add(new ClientLootBoxItemInfo { ItemIndex = taken[i].Item.Index, Amount = taken[i].Amount, Slot = i });
                }

                Enqueue(new S.LootBoxOpen { Slot = slot, Items = items });
            }
        }

        public void LootBoxConfirm(LootBoxTakeItems p)
        {
            if (p.Slot < 0 || p.Slot >= Inventory.Length) return;

            UserItem item = Inventory[p.Slot];

            if (item == null || item.Info.ItemType != ItemType.LootBox) return;

            var lootBoxInfo = SEnvir.LootBoxInfoList.Binding.FirstOrDefault(x => x.Index == item.Info.Shape);
            if (lootBoxInfo == null) return;

            var lootBoxState = item.Stats[Stat.Counter2];
            if (lootBoxState < 2) return; // Hasn't been confirmed yet

            var lootBoxContents = lootBoxInfo.Contents.ToList();

            // Shuffle the full list based on the random 1 seed
            Functions.Shuffle(lootBoxContents, item.Stats[Stat.Random1]);

            // Take the top selection based on slot amount
            var taken = lootBoxContents.Take(LootBoxInfo.SlotSize).ToList();

            // Calculate how many more items are needed to reach SlotSize
            int itemsToAdd = LootBoxInfo.SlotSize - taken.Count;

            // If more items are needed, pad the list with default values
            if (itemsToAdd > 0)
            {
                taken.AddRange(Enumerable.Repeat(default(LootBoxItemInfo), itemsToAdd));
            }

            // Shuffle the taken list based on random 2 seed
            Functions.Shuffle(taken, item.Stats[Stat.Random2]);

            var itemChecks = new List<ItemCheck>();

            var lockState = item.CurrentDurability;

            for (int i = 0; i < taken.Count; i++)
            {
                bool unlocked = (lockState & (1 << i)) != 0;

                if (unlocked)
                {
                    var selection = taken[i];

                    if (selection == default(LootBoxItemInfo))
                    {
                        continue;
                    }

                    var amount = selection.Amount;

                    if (amount > selection.Item.StackSize)
                    {
                        while (amount > selection.Item.StackSize)
                        {
                            itemChecks.Add(new ItemCheck(selection.Item, selection.Item.StackSize, UserItemFlags.None, TimeSpan.Zero));

                            amount -= selection.Item.StackSize;
                        }
                    }

                    if (amount > 0)
                    {
                        itemChecks.Add(new ItemCheck(selection.Item, amount, UserItemFlags.None, TimeSpan.Zero));
                    }
                }
            }

            if (!CanGainItems(true, [.. itemChecks]))
            {
                Connection.ReceiveChat(Connection.Language.NotEnoughBagSpaceAvailable, MessageType.System);

                Enqueue(new S.LootBoxClose());
                return;
            }

            foreach (ItemCheck check in itemChecks)
            {
                while (check.Count > 0)
                    GainItem(SEnvir.CreateFreshItem(check));
            }

            S.ItemChanged result = new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = GridType.Inventory, Slot = p.Slot },
                Success = true
            };

            if (item.Count > 1)
            {
                item.Count--;
                result.Link.Count = item.Count;
            }
            else
            {
                RemoveItem(item);
                Inventory[p.Slot] = null;
                item.Delete();

                result.Link.Count = 0;
            }

            Enqueue(result);

            Companion?.RefreshWeight();
            RefreshWeight();

            Enqueue(new S.LootBoxClose());
        }

        #endregion

        #region Bundles

        public void BundleOpen(BundleOpen p)
        {
            if (p.Slot < 0 || p.Slot >= Inventory.Length) return;

            UserItem item = Inventory[p.Slot];

            if (item == null || item.Info.ItemType != ItemType.Bundle) return;

            BundleUpdate(item, p.Slot);
        }

        private void BundleUpdate(UserItem item, int slot)
        {
            var bundleInfo = SEnvir.BundleInfoList.Binding.FirstOrDefault(x => x.Index == item.Info.Shape);
            if (bundleInfo == null) return;

            var bundleContents = bundleInfo.Contents.ToList();

            // Shuffle the full list based on the random 1 seed
            Functions.Shuffle(bundleContents, item.Stats[Stat.Random1]);

            var bundleItems = new List<ClientBundleItemInfo>();

            for (int i = 0; i < bundleInfo.SlotSize; i++)
            {
                if (i >= bundleContents.Count) break;

                bundleItems.Add(new ClientBundleItemInfo { ItemIndex = bundleContents[i].Item.Index, Amount = bundleContents[i].Amount, Slot = i });
            }

            Enqueue(new S.BundleOpen { Slot = slot, Items = bundleItems });
        }

        public void BundleConfirm(BundleConfirm p)
        {
            if (p.Slot < 0 || p.Slot >= Inventory.Length) return;

            UserItem item = Inventory[p.Slot];

            if (item == null || item.Info.ItemType != ItemType.Bundle) return;

            var bundleInfo = SEnvir.BundleInfoList.Binding.FirstOrDefault(x => x.Index == item.Info.Shape);
            if (bundleInfo == null) return;

            var bundleContents = bundleInfo.Contents.ToList();

            // Shuffle the full list based on the random 1 seed
            Functions.Shuffle(bundleContents, item.Stats[Stat.Random1]);

            switch (bundleInfo.Type)
            {
                case BundleType.OneOf:
                case BundleType.AnyOf:
                    {
                        var choice = p.Choice;

                        int smallest = Math.Min(bundleInfo.SlotSize, bundleContents.Count);

                        if (bundleInfo.Type == BundleType.AnyOf)
                        {
                            choice = SEnvir.Random.Next(smallest);
                        }

                        if (choice < 0 || choice >= smallest) return;

                        var selection = bundleContents[choice];

                        var itemChecks = new List<ItemCheck>();

                        var amount = selection.Amount;

                        if (amount > selection.Item.StackSize)
                        {
                            while (amount > selection.Item.StackSize)
                            {
                                itemChecks.Add(new ItemCheck(selection.Item, selection.Item.StackSize, UserItemFlags.None, TimeSpan.Zero));

                                amount -= selection.Item.StackSize;
                            }
                        }

                        if (amount > 0)
                        {
                            itemChecks.Add(new ItemCheck(selection.Item, amount, UserItemFlags.None, TimeSpan.Zero));
                        }

                        if (!CanGainItems(true, itemChecks.ToArray()))
                        {
                            Connection.ReceiveChat(Connection.Language.NotEnoughBagSpaceAvailable, MessageType.System);

                            Enqueue(new S.BundleClose());
                            return;
                        }

                        for (int i = 0; i < itemChecks.Count; i++)
                        {
                            var itemCheck = itemChecks[i];

                            var gainItem = SEnvir.CreateFreshItem(itemCheck.Info);
                            gainItem.Count = itemCheck.Count;

                            if (gainItem != null)
                                GainItem(gainItem);
                        }
                    }
                    break;
                case BundleType.AllOf:
                    {
                        var itemChecks = new List<ItemCheck>();

                        for (int i = 0; i < bundleContents.Count; i++)
                        {
                            if (i >= bundleInfo.SlotSize) break;

                            var selection = bundleContents[i];

                            var amount = selection.Amount;

                            if (amount > selection.Item.StackSize)
                            {
                                while (amount > selection.Item.StackSize)
                                {
                                    itemChecks.Add(new ItemCheck(selection.Item, selection.Item.StackSize, UserItemFlags.None, TimeSpan.Zero));

                                    amount -= selection.Item.StackSize;
                                }
                            }

                            if (amount > 0)
                            {
                                itemChecks.Add(new ItemCheck(selection.Item, amount, UserItemFlags.None, TimeSpan.Zero));
                            }
                        }

                        if (!CanGainItems(true, [.. itemChecks]))
                        {
                            Connection.ReceiveChat(Connection.Language.NotEnoughBagSpaceAvailable, MessageType.System);

                            Enqueue(new S.BundleClose());
                            return;
                        }

                        for (int i = 0; i < itemChecks.Count; i++)
                        {
                            var itemCheck = itemChecks[i];

                            var gainItem = SEnvir.CreateFreshItem(itemCheck.Info);
                            gainItem.Count = itemCheck.Count;

                            if (gainItem != null)
                                GainItem(gainItem);
                        }
                    }
                    break;
            }

            S.ItemChanged result = new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = GridType.Inventory, Slot = p.Slot },
                Success = true
            };

            if (item.Count > 1)
            {
                item.Count--;
                result.Link.Count = item.Count;
            }
            else
            {
                RemoveItem(item);
                Inventory[p.Slot] = null;
                item.Delete();

                result.Link.Count = 0;
            }

            Enqueue(result);

            Companion?.RefreshWeight();
            RefreshWeight();

            Enqueue(new S.BundleClose());
        }

        #endregion
    }
}
