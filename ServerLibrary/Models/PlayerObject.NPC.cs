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
        #region NPCs
        public void NPCCall(uint objectID)
        {
            if (Dead) return;

            NPC = null;
            NPCPage = null;

            foreach (NPCObject ob in CurrentMap.NPCs)
            {
                if (ob.ObjectID != objectID) continue;
                if (!Functions.InRange(ob.CurrentLocation, CurrentLocation, Config.MaxViewRange)) return;

                ob.NPCCall(this, ob.NPCInfo.EntryPage);
                return;
            }
        }

        public void NPCButton(int buttonID)
        {
            if (Dead || NPC == null || NPCPage == null) return;


            foreach (Library.SystemModels.NPCButton button in NPCPage.Buttons)
            {
                if (button.ButtonID != buttonID || button.DestinationPage == null) continue;

                NPC.NPCCall(this, button.DestinationPage);
                return;
            }
        }

        public void NPCRoll(int type)
        {
            if (Dead || NPC == null || NPCPage == null || NPCPage.SuccessPage == null) return;

            var roll = SEnvir.Random.Next(1, 7);

            NPCVals["ROLLRESULT"] = roll;

            Enqueue(new S.NPCRoll { Type = type, Result = roll });
        }

        public void NPCRollResult()
        {
            if (Dead || NPC == null || NPCPage == null || NPCPage.SuccessPage == null) return;

            NPC.NPCCall(this, NPCPage.SuccessPage);
        }

        public void NPCBuy(C.NPCBuy p)
        {
            if (Dead || NPC == null || NPCPage == null || p.Amount <= 0) return;

            var currency = NPCPage.Currency ?? SEnvir.CurrencyInfoList.Binding.First(x => x.Type == CurrencyType.Gold);

            var userCurrency = GetCurrency(currency);

            var amount = userCurrency.Amount;

            foreach (NPCGood good in NPCPage.Goods)
            {
                if (good.Index != p.Index || good.Item == null) continue;

                if (p.Amount > good.Item.StackSize) return;

                var price = (int)Math.Max(1, good.Cost * currency.ExchangeRate);

                long cost = (long)(price * p.Amount);

                if (p.GuildFunds && currency.Type != CurrencyType.Gold)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.NPCFundsCurrency, MessageType.System);
                    return;
                }

                if (p.GuildFunds)
                {
                    if (Character.Account.GuildMember == null)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.NPCFundsGuild, MessageType.System);
                        return;
                    }
                    if ((Character.Account.GuildMember.Permission & GuildPermission.FundsMerchant) != GuildPermission.FundsMerchant)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.NPCFundsPermission, MessageType.System);
                        return;
                    }

                    if (cost > Character.Account.GuildMember.Guild.GuildFunds)
                    {
                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.NPCFundsCost, cost - Character.Account.GuildMember.Guild.GuildFunds), MessageType.System);
                        return;
                    }
                }
                else
                {
                    if (cost > amount)
                    {
                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.NPCCost, amount - cost), MessageType.System);

                        return;
                    }
                }

                UserItemFlags flags = UserItemFlags.Locked;

                switch (good.Item.ItemType)
                {
                    case ItemType.Weapon:
                    case ItemType.Armour:
                    case ItemType.Helmet:
                    case ItemType.Necklace:
                    case ItemType.Bracelet:
                    case ItemType.Ring:
                    case ItemType.Shoes:
                    case ItemType.Book:
                        flags |= UserItemFlags.NonRefinable;
                        break;
                }

                ItemCheck check = new ItemCheck(good.Item, p.Amount, flags, TimeSpan.Zero);

                if (!CanGainItems(true, check))
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.NPCNoRoom, MessageType.System);

                    return;
                }

                UserItem item = SEnvir.CreateFreshItem(check);

                if (p.GuildFunds)
                {
                    Character.Account.GuildMember.Guild.GuildFunds -= cost;
                    Character.Account.GuildMember.Guild.DailyGrowth -= cost;

                    foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                    {
                        member.Account.Connection?.Player?.Enqueue(new S.GuildFundsChanged { Change = -cost, ObserverPacket = false });
                        member.Account.Connection?.ReceiveChat(string.Format(member.Account.Connection.Language.NPCFundsBuy, Name, cost, item.Info.ItemName, item.Count), MessageType.System);
                    }
                }
                else
                {
                    userCurrency.Amount -= cost;

                    CurrencyChanged(userCurrency);
                }

                LogMilestone(MilestoneType.ShopPurchase, item.Count, item: item.Info);

                GainItem(item);
            }
        }

        public void NPCSell(List<CellLinkInfo> links)
        {
            S.ItemsChanged p = new S.ItemsChanged { Links = links };
            Enqueue(p);

            if (Dead || NPC == null || NPCPage == null || NPCPage.DialogType != NPCDialogType.BuySell || NPCPage.Types.Count == 0) return;

            var currency = NPCPage.Currency ?? SEnvir.CurrencyInfoList.Binding.First(x => x.Type == CurrencyType.Gold);

            var userCurrency = GetCurrency(currency);

            if (!ParseLinks(p.Links, 0, 100)) return;

            long amount = 0;
            long count = 0;

            foreach (CellLinkInfo link in links)
            {
                UserItem[] fromArray = null;

                switch (link.GridType)
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

                if (link.Slot < 0 || link.Slot >= fromArray.Length) return;
                UserItem item = fromArray[link.Slot];

                if (item == null || link.Count > item.Count || !item.Info.CanSell || (item.Flags & UserItemFlags.Locked) == UserItemFlags.Locked) return;
                if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return;
                if ((item.Flags & UserItemFlags.Worthless) == UserItemFlags.Worthless) return;

                if (!NPCPage.Types.Any(x => x.ItemType == item.Info.ItemType)) return;

                var price = (long)(item.Price(link.Count) * currency.ExchangeRate);

                count += link.Count;
                amount += price;
            }

            if (amount < 0)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.NPCSellWorthless, MessageType.System);

                return;
            }

            foreach (CellLinkInfo link in links)
            {
                UserItem[] fromArray = null;

                switch (link.GridType)
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

                UserItem item = fromArray[link.Slot];

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    fromArray[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;

                LogMilestone(MilestoneType.ShopSell, link.Count, item: item.Info);
            }

            if (p.Links.Count > 0)
            {
                Companion?.RefreshWeight();
                RefreshWeight();
            }

            Connection.ReceiveChatWithObservers(con => string.Format(con.Language.NPCSellResult, count, amount, currency.Name), MessageType.System);

            p.Success = true;
            userCurrency.Amount += amount;

            LogMilestone(MilestoneType.CurrencyGain, amount, currency: userCurrency.Info);

            CurrencyChanged(userCurrency);
        }

        public void NPCFragment(List<CellLinkInfo> links)
        {
            S.ItemsChanged p = new S.ItemsChanged { Links = links };
            Enqueue(p);

            if (Dead || NPC == null || NPCPage == null || NPCPage.DialogType != NPCDialogType.ItemFragment) return;

            if (!ParseLinks(p.Links, 0, 100)) return;

            if (SEnvir.FragmentInfo == null || SEnvir.Fragment2Info == null || SEnvir.Fragment3Info == null) return;

            long cost = 0;
            int fragmentCount = 0;
            int fragment2Count = 0;
            int itemCount = 0;


            foreach (CellLinkInfo link in links)
            {
                UserItem[] fromArray = null;

                switch (link.GridType)
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

                if (link.Slot < 0 || link.Slot >= fromArray.Length) return;
                UserItem item = fromArray[link.Slot];

                if (item == null || link.Count > item.Count || (item.Flags & UserItemFlags.Locked) == UserItemFlags.Locked) return;
                if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return; //No harm in checking
                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;
                if (!item.CanFragment()) return;

                cost += item.FragmentCost();
                itemCount++;

                if (item.Info.Rarity == Rarity.Common)
                    fragmentCount += item.FragmentCount();
                else
                    fragment2Count += item.FragmentCount();
            }


            if (cost > Gold.Amount)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.FragmentCost, Gold.Amount - cost), MessageType.System);
                return;
            }

            List<ItemCheck> checks = new List<ItemCheck>();

            if (fragmentCount > 0)
                checks.Add(new ItemCheck(SEnvir.FragmentInfo, fragmentCount, UserItemFlags.None, TimeSpan.Zero));

            if (fragment2Count > 0)
                checks.Add(new ItemCheck(SEnvir.Fragment2Info, fragment2Count, UserItemFlags.None, TimeSpan.Zero));


            if (!CanGainItems(false, checks.ToArray()))
            {
                Connection.ReceiveChatWithObservers(con => con.Language.FragmentSpace, MessageType.System);
                return;
            }

            foreach (CellLinkInfo link in links)
            {
                UserItem[] fromArray = null;

                switch (link.GridType)
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

                UserItem item = fromArray[link.Slot];


                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    fromArray[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;
            }

            foreach (ItemCheck check in checks)
                while (check.Count > 0)
                    GainItem(SEnvir.CreateFreshItem(check));

            if (p.Links.Count > 0)
            {
                Companion?.RefreshWeight();
                RefreshWeight();
            }

            Connection.ReceiveChatWithObservers(con => string.Format(con.Language.FragmentResult, itemCount, cost), MessageType.System);

            p.Success = true;
            Gold.Amount -= cost;

            GoldChanged();
        }
        public void NPCAccessoryLevelUp(C.NPCAccessoryLevelUp p)
        {
            Enqueue(new S.NPCAccessoryLevelUp { Target = p.Target, Links = p.Links });

            if (Dead || NPC == null || NPCPage == null || NPCPage.DialogType != NPCDialogType.AccessoryRefineLevel) return;

            if (!ParseLinks(p.Links, 0, 100) || !ParseLinks(p.Target)) return;


            UserItem[] targetArray = null;

            switch (p.Target.GridType)
            {
                case GridType.Inventory:
                    targetArray = Inventory;
                    break;
                case GridType.Equipment:
                    targetArray = Equipment;
                    break;
                case GridType.Storage:
                    targetArray = Storage;
                    break;
                case GridType.CompanionInventory:
                    if (Companion == null) return;

                    targetArray = Companion.Inventory;
                    break;
                default:
                    return;
            }

            if (p.Target.Slot < 0 || p.Target.Slot >= targetArray.Length) return;
            UserItem targetItem = targetArray[p.Target.Slot];

            if (targetItem == null || p.Target.Count > targetItem.Count) return; //Already Leveled.
            if ((targetItem.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return; //No harm in checking

            switch (targetItem.Info.ItemType)
            {
                case ItemType.Ring:
                case ItemType.Bracelet:
                case ItemType.Necklace:
                    break;
                default: return;
            }

            if (targetItem.Level >= Globals.AccessoryExperienceList.Count) return;

            bool changed = false;

            S.ItemsChanged result = new S.ItemsChanged { Links = new List<CellLinkInfo>(), Success = true };
            Enqueue(result);

            foreach (CellLinkInfo link in p.Links)
            {
                if ((targetItem.Flags & UserItemFlags.Refinable) == UserItemFlags.Refinable) break;


                UserItem[] fromArray = null;

                switch (link.GridType)
                {
                    case GridType.Inventory:
                        fromArray = Inventory;
                        break;
                    case GridType.Storage:
                        fromArray = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) continue;

                        fromArray = Companion.Inventory;
                        break;
                    default:
                        continue;
                }

                if (link.Slot < 0 || link.Slot >= fromArray.Length) continue;
                UserItem item = fromArray[link.Slot];

                if (item == null || link.Count > item.Count || (item.Flags & UserItemFlags.Locked) == UserItemFlags.Locked) continue;
                if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) continue;
                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) continue;
                if (item.Info != targetItem.Info) continue;
                if ((item.Flags & UserItemFlags.Bound) == UserItemFlags.Bound && (targetItem.Flags & UserItemFlags.Bound) != UserItemFlags.Bound) continue;

                long cost = Globals.AccessoryLevelCost * link.Count;


                if (Gold.Amount < cost)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.AccessoryLevelCost, MessageType.System);
                    continue;
                }

                result.Links.Add(link);


                if (targetItem.Info.Rarity != Rarity.Common || targetItem.Level == 1)
                    targetItem.Experience += link.Count * 5;
                else
                    targetItem.Experience += link.Count;

                if (item.Level > 1 && targetItem.Info.Rarity == Rarity.Common)
                    targetItem.Experience -= 4;


                while (item.Level > 1)
                {
                    targetItem.Experience += Globals.AccessoryExperienceList[item.Level - 1];
                    item.Level--;
                }

                targetItem.Experience += item.Experience;

                Gold.Amount -= cost;

                if (targetItem.Experience >= Globals.AccessoryExperienceList[targetItem.Level])
                {
                    targetItem.Experience -= Globals.AccessoryExperienceList[targetItem.Level];
                    targetItem.Level++;

                    targetItem.Flags |= UserItemFlags.Refinable;
                }

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    fromArray[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;

                changed = true;
            }


            if (changed)
            {
                if ((targetItem.Flags & UserItemFlags.Refinable) == UserItemFlags.Refinable)
                {
                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.AccessoryLeveled, targetItem.Info.ItemName), MessageType.System);
                }

                Companion?.RefreshWeight();
                RefreshWeight();
                GoldChanged();

                Enqueue(new S.ItemExperience { Target = p.Target, Experience = targetItem.Experience, Level = targetItem.Level, Flags = targetItem.Flags });
            }
        }
        public void NPCAccessoryUpgrade(C.NPCAccessoryUpgrade p)
        {
            Enqueue(new S.ItemChanged { Link = p.Target }); //Unlock Item

            if (Dead || NPC == null || NPCPage == null || NPCPage.DialogType != NPCDialogType.AccessoryRefineUpgrade) return;

            if (!ParseLinks(p.Target)) return;


            UserItem[] targetArray = null;

            switch (p.Target.GridType)
            {
                case GridType.Inventory:
                    targetArray = Inventory;
                    break;
                case GridType.Equipment:
                    targetArray = Equipment;
                    break;
                case GridType.Storage:
                    targetArray = Storage;
                    break;
                case GridType.CompanionInventory:
                    if (Companion == null) return;

                    targetArray = Companion.Inventory;
                    break;
                default:
                    return;
            }

            if (p.Target.Slot < 0 || p.Target.Slot >= targetArray.Length) return;
            UserItem targetItem = targetArray[p.Target.Slot];

            if (targetItem == null || p.Target.Count > targetItem.Count) return; //Already Leveled.
            if ((targetItem.Flags & UserItemFlags.Refinable) != UserItemFlags.Refinable) return;
            if ((targetItem.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;

            switch (targetItem.Info.ItemType)
            {
                case ItemType.Ring:
                case ItemType.Bracelet:
                case ItemType.Necklace:
                    break;
                default: return;
            }

            S.ItemStatsChanged result = new S.ItemStatsChanged { GridType = p.Target.GridType, Slot = p.Target.Slot, NewStats = new Stats() };
            Enqueue(result);

            switch (p.RefineType)
            {
                case RefineType.DC:
                    targetItem.AddStat(Stat.MaxDC, 1, StatSource.Refine);
                    result.NewStats[Stat.MaxDC] = 1;
                    break;
                case RefineType.SpellPower:
                    if (targetItem.Info.Stats[Stat.MinMC] == 0 && targetItem.Info.Stats[Stat.MaxMC] == 0 && targetItem.Info.Stats[Stat.MinSC] == 0 && targetItem.Info.Stats[Stat.MaxSC] == 0)
                    {
                        targetItem.AddStat(Stat.MaxMC, 1, StatSource.Refine);
                        result.NewStats[Stat.MaxMC] = 1;

                        targetItem.AddStat(Stat.MaxSC, 1, StatSource.Refine);
                        result.NewStats[Stat.MaxSC] = 1;
                    }

                    if (targetItem.Info.Stats[Stat.MinMC] > 0 || targetItem.Info.Stats[Stat.MaxMC] > 0)
                    {
                        targetItem.AddStat(Stat.MaxMC, 1, StatSource.Refine);
                        result.NewStats[Stat.MaxMC] = 1;
                    }

                    if (targetItem.Info.Stats[Stat.MinSC] > 0 || targetItem.Info.Stats[Stat.MaxSC] > 0)
                    {
                        targetItem.AddStat(Stat.MaxSC, 1, StatSource.Refine);
                        result.NewStats[Stat.MaxSC] = 1;
                    }
                    break;
                case RefineType.Health:
                    targetItem.AddStat(Stat.Health, 10, StatSource.Refine);
                    result.NewStats[Stat.Health] = 10;
                    break;
                case RefineType.Mana:
                    targetItem.AddStat(Stat.Mana, 10, StatSource.Refine);
                    result.NewStats[Stat.Mana] = 10;
                    break;
                case RefineType.DCPercent:
                    targetItem.AddStat(Stat.DCPercent, 1, StatSource.Refine);
                    result.NewStats[Stat.DCPercent] = 1;
                    break;
                case RefineType.SPPercent:
                    if (targetItem.Info.Stats[Stat.MinMC] == 0 && targetItem.Info.Stats[Stat.MaxMC] == 0 && targetItem.Info.Stats[Stat.MinSC] == 0 && targetItem.Info.Stats[Stat.MaxSC] == 0)
                    {
                        targetItem.AddStat(Stat.MCPercent, 1, StatSource.Refine);
                        result.NewStats[Stat.MCPercent] = 1;

                        targetItem.AddStat(Stat.SCPercent, 1, StatSource.Refine);
                        result.NewStats[Stat.SCPercent] = 1;
                    }

                    if (targetItem.Info.Stats[Stat.MinMC] > 0 || targetItem.Info.Stats[Stat.MaxMC] > 0)
                    {
                        targetItem.AddStat(Stat.MCPercent, 1, StatSource.Refine);
                        result.NewStats[Stat.MCPercent] = 1;
                    }

                    if (targetItem.Info.Stats[Stat.MinSC] > 0 || targetItem.Info.Stats[Stat.MaxSC] > 0)
                    {
                        targetItem.AddStat(Stat.SCPercent, 1, StatSource.Refine);
                        result.NewStats[Stat.SCPercent] = 1;
                    }
                    break;
                case RefineType.HealthPercent:
                    targetItem.AddStat(Stat.HealthPercent, 1, StatSource.Refine);
                    result.NewStats[Stat.HealthPercent] = 1;
                    break;
                case RefineType.ManaPercent:
                    targetItem.AddStat(Stat.ManaPercent, 1, StatSource.Refine);
                    result.NewStats[Stat.ManaPercent] = 1;
                    break;
                case RefineType.Fire:
                    targetItem.AddStat(Stat.FireAttack, 1, StatSource.Refine);
                    result.NewStats[Stat.FireAttack] = 1;
                    break;
                case RefineType.Ice:
                    targetItem.AddStat(Stat.IceAttack, 1, StatSource.Refine);
                    result.NewStats[Stat.IceAttack] = 1;
                    break;
                case RefineType.Lightning:
                    targetItem.AddStat(Stat.LightningAttack, 1, StatSource.Refine);
                    result.NewStats[Stat.LightningAttack] = 1;
                    break;
                case RefineType.Wind:
                    targetItem.AddStat(Stat.WindAttack, 1, StatSource.Refine);
                    result.NewStats[Stat.WindAttack] = 1;
                    break;
                case RefineType.Holy:
                    targetItem.AddStat(Stat.HolyAttack, 1, StatSource.Refine);
                    result.NewStats[Stat.HolyAttack] = 1;
                    break;
                case RefineType.Dark:
                    targetItem.AddStat(Stat.DarkAttack, 1, StatSource.Refine);
                    result.NewStats[Stat.DarkAttack] = 1;
                    break;
                case RefineType.Phantom:
                    targetItem.AddStat(Stat.PhantomAttack, 1, StatSource.Refine);
                    result.NewStats[Stat.PhantomAttack] = 1;
                    break;
                case RefineType.AC:
                    targetItem.AddStat(Stat.MinAC, 1, StatSource.Refine);
                    result.NewStats[Stat.MinAC] = 1;
                    targetItem.AddStat(Stat.MaxAC, 1, StatSource.Refine);
                    result.NewStats[Stat.MaxAC] = 1;
                    break;
                case RefineType.MR:
                    targetItem.AddStat(Stat.MinMR, 1, StatSource.Refine);
                    result.NewStats[Stat.MinMR] = 1;
                    targetItem.AddStat(Stat.MaxMR, 1, StatSource.Refine);
                    result.NewStats[Stat.MaxMR] = 1;
                    break;
                case RefineType.Accuracy:
                    targetItem.AddStat(Stat.Accuracy, 1, StatSource.Refine);
                    result.NewStats[Stat.Accuracy] = 1;
                    break;
                case RefineType.Agility:
                    targetItem.AddStat(Stat.Agility, 1, StatSource.Refine);
                    result.NewStats[Stat.Agility] = 1;
                    break;
                default:
                    Character.Account.Banned = true;
                    Character.Account.BanReason = "Attempted to Exploit refine, Accessory Refine Type.";
                    Character.Account.BanExpiry = SEnvir.Now.AddYears(10);
                    return;
            }

            targetItem.Flags &= ~UserItemFlags.Refinable;
            targetItem.StatsChanged();

            RefreshStats();

            if (targetItem.Experience >= Globals.AccessoryExperienceList[targetItem.Level])
            {
                targetItem.Experience -= Globals.AccessoryExperienceList[targetItem.Level];
                targetItem.Level++;

                targetItem.Flags |= UserItemFlags.Refinable;
            }

            Enqueue(new S.ItemExperience { Target = p.Target, Experience = targetItem.Experience, Level = targetItem.Level, Flags = targetItem.Flags });
        }
        public void NPCAccessoryReset(C.NPCAccessoryReset p)
        {
            Enqueue(new S.ItemChanged { Link = p.Cell }); //Unlock Item

            if (Dead || NPC == null || NPCPage == null || NPCPage.DialogType != NPCDialogType.AccessoryReset) return;

            if (!ParseLinks(p.Cell)) return;


            UserItem[] targetArray = null;

            switch (p.Cell.GridType)
            {
                case GridType.Inventory:
                    targetArray = Inventory;
                    break;
                case GridType.Equipment:
                    targetArray = Equipment;
                    break;
                case GridType.Storage:
                    targetArray = Storage;
                    break;
                case GridType.CompanionInventory:
                    if (Companion == null) return;

                    targetArray = Companion.Inventory;
                    break;
                default:
                    return;
            }

            if (Globals.AccessoryResetCost > Gold.Amount)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.NPCRefinementGold, MessageType.System);
                return;
            }

            if (p.Cell.Slot < 0 || p.Cell.Slot >= targetArray.Length) return;
            UserItem targetItem = targetArray[p.Cell.Slot];

            if (targetItem == null || p.Cell.Count > targetItem.Count) return; //Already Leveled.
            if ((targetItem.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;


            switch (targetItem.Level)
            {
                case 1:
                    return;
                case 2:
                    if ((targetItem.Flags & UserItemFlags.Refinable) == UserItemFlags.Refinable) return; //Not Refuned.
                    break;
                default:
                    break;
            }

            switch (targetItem.Info.ItemType)
            {
                case ItemType.Ring:
                case ItemType.Bracelet:
                case ItemType.Necklace:
                    break;
                default: return;
            }

            S.ItemStatsRefreshed result = new S.ItemStatsRefreshed { GridType = p.Cell.GridType, Slot = p.Cell.Slot };
            Enqueue(result);

            for (int i = targetItem.AddedStats.Count - 1; i >= 0; i--)
            {
                if (targetItem.AddedStats[i].StatSource != StatSource.Refine) continue;

                targetItem.AddedStats[i].Delete();
            }

            targetItem.StatsChanged();

            result.NewStats = new Stats(targetItem.Stats);

            RefreshStats();

            Gold.Amount -= Globals.AccessoryResetCost;
            GoldChanged();

            while (targetItem.Level > 1)
            {
                targetItem.Experience += Globals.AccessoryExperienceList[targetItem.Level - 1];
                targetItem.Level--;
            }

            if (targetItem.Experience >= Globals.AccessoryExperienceList[targetItem.Level])
            {
                targetItem.Experience -= Globals.AccessoryExperienceList[targetItem.Level];
                targetItem.Level++;

                targetItem.Flags |= UserItemFlags.Refinable;
            }

            Enqueue(new S.ItemExperience { Target = p.Cell, Experience = targetItem.Experience, Level = targetItem.Level, Flags = targetItem.Flags });
        }

        public void NPCAccessoryRefine(C.NPCAccessoryRefine p)
        {
            Enqueue(new S.NPCAccessoryRefine { Target = p.Target, OreTarget = p.OreTarget, Links = p.Links });

            if (Dead || NPC == null || NPCPage == null || NPCPage.DialogType != NPCDialogType.AccessoryRefine) return;

            if (!ParseLinks(p.Target)) return;

            if (Gold.Amount < 50000) return;

            UserItem[] targetArray = null;

            switch (p.Target.GridType)
            {
                case GridType.Inventory:
                    targetArray = Inventory;
                    break;
                case GridType.Equipment:
                    targetArray = Equipment;
                    break;
                case GridType.Storage:
                    targetArray = Storage;
                    break;
                case GridType.CompanionInventory:
                    if (Companion == null) return;

                    targetArray = Companion.Inventory;
                    break;
                default:
                    return;
            }

            if (p.Target.Slot < 0 || p.Target.Slot >= targetArray.Length) return;
            UserItem targetItem = targetArray[p.Target.Slot];

            if (targetItem == null || p.Target.Count > targetItem.Count) return; //Already Leveled.
            if ((targetItem.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;
            if (targetItem.Level > 1) return; //No refine on levelled items

            switch (targetItem.Info.ItemType)
            {
                case ItemType.Ring:
                case ItemType.Bracelet:
                case ItemType.Necklace:
                    break;
                default: return; //only refined accessories
            }

            UserItem[] targetOreArray = null;

            switch (p.OreTarget.GridType)
            {
                case GridType.Inventory:
                    targetOreArray = Inventory;
                    break;
                case GridType.Equipment:
                    targetOreArray = Equipment;
                    break;
                case GridType.Storage:
                    targetOreArray = Storage;
                    break;
                case GridType.CompanionInventory:
                    if (Companion == null) return;

                    targetOreArray = Companion.Inventory;
                    break;
                default:
                    return;
            }
            if (p.OreTarget.Slot < 0 || p.OreTarget.Slot >= targetArray.Length) return;
            UserItem oretargetItem = targetOreArray[p.OreTarget.Slot];

            S.ItemsChanged result = new S.ItemsChanged { Links = new List<CellLinkInfo>(), Success = true };
            Enqueue(result);

            foreach (CellLinkInfo link in p.Links) //loop through to check level and added stats on each material vs target
            {
                UserItem[] fromArray = null;

                switch (link.GridType)
                {
                    case GridType.Inventory:
                        fromArray = Inventory;
                        break;
                    case GridType.Storage:
                        fromArray = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) continue;

                        fromArray = Companion.Inventory;
                        break;
                    default:
                        continue;
                }

                if (link.Slot < 0 || link.Slot >= fromArray.Length) continue;
                UserItem refItem = fromArray[link.Slot];

                if (refItem.Level > 1) return; //if material is levelled dont refine
                if (targetItem.AddedStats.Count != refItem.AddedStats.Count) return; //if material has different amount of added stats to target dont refine

                if (targetItem.AddedStats.Count > 1) //if target has added stats loop through to check material has same stats
                {
                    int count = 0;
                    foreach (UserItemStat addStat in targetItem.AddedStats)
                    {
                        foreach (UserItemStat raddStat in refItem.AddedStats)
                        {
                            if (addStat.Stat == raddStat.Stat && addStat.StatSource == raddStat.StatSource && addStat.Amount == raddStat.Amount)
                            {
                                count++;
                            }
                        }
                    }
                    if (count != targetItem.AddedStats.Count) return;

                }

            }

            foreach (CellLinkInfo link in p.Links) //now loop through to remove materials
            {

                UserItem[] fromArray = null;

                switch (link.GridType)
                {
                    case GridType.Inventory:
                        fromArray = Inventory;
                        break;
                    case GridType.Storage:
                        fromArray = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) continue;

                        fromArray = Companion.Inventory;
                        break;
                    default:
                        continue;
                }

                if (link.Slot < 0 || link.Slot >= fromArray.Length) continue;
                UserItem item = fromArray[link.Slot];

                if (item == null || link.Count > item.Count || (item.Flags & UserItemFlags.Locked) == UserItemFlags.Locked) continue;
                if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) continue;
                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) continue;
                if (item.Info != targetItem.Info) continue;
                if ((item.Flags & UserItemFlags.Bound) == UserItemFlags.Bound && (targetItem.Flags & UserItemFlags.Bound) != UserItemFlags.Bound) continue;

                result.Links.Add(link);

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    fromArray[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;

            }

            int chance = 100 - (oretargetItem.CurrentDurability / 1000);
            int success = 30;
            if (targetItem.Info.Rarity != Rarity.Common)
            {
                success = 40;
            }
            if (SEnvir.Random.Next(chance) < success)
            #region refineworked
            {

                S.ItemAcessoryRefined presult = new S.ItemAcessoryRefined { GridType = p.Target.GridType, Slot = p.Target.Slot, NewStats = new Stats() };
                Enqueue(presult);
                int amount = 1;
                if (SEnvir.Random.Next(chance) == 0)
                {
                    amount = 2;
                }
                switch (p.RefineType)
                {

                    case RefineType.DC:
                        targetItem.AddStat(Stat.MaxDC, amount, StatSource.Added);
                        presult.NewStats[Stat.MaxDC] = amount;
                        break;
                    case RefineType.SpellPower:
                        if (targetItem.Info.Stats[Stat.MinMC] == 0 && targetItem.Info.Stats[Stat.MaxMC] == 0 && targetItem.Info.Stats[Stat.MinSC] == 0 && targetItem.Info.Stats[Stat.MaxSC] == 0)
                        {
                            targetItem.AddStat(Stat.MaxMC, amount, StatSource.Added);
                            presult.NewStats[Stat.MaxMC] = amount;

                            targetItem.AddStat(Stat.MaxSC, amount, StatSource.Added);
                            presult.NewStats[Stat.MaxSC] = amount;
                        }

                        if (targetItem.Info.Stats[Stat.MinMC] > 0 || targetItem.Info.Stats[Stat.MaxMC] > 0)
                        {
                            targetItem.AddStat(Stat.MaxMC, amount, StatSource.Added);
                            presult.NewStats[Stat.MaxMC] = amount;
                        }

                        if (targetItem.Info.Stats[Stat.MinSC] > 0 || targetItem.Info.Stats[Stat.MaxSC] > 0)
                        {
                            targetItem.AddStat(Stat.MaxSC, amount, StatSource.Added);
                            presult.NewStats[Stat.MaxSC] = amount;
                        }
                        break;
                    case RefineType.Health:
                        amount *= 10;
                        targetItem.AddStat(Stat.Health, amount, StatSource.Added);
                        presult.NewStats[Stat.Health] = amount;
                        break;
                    case RefineType.Mana:
                        amount *= 10;
                        targetItem.AddStat(Stat.Mana, amount, StatSource.Added);
                        presult.NewStats[Stat.Mana] = amount;
                        break;
                    case RefineType.DCPercent:
                        targetItem.AddStat(Stat.DCPercent, amount, StatSource.Added);
                        presult.NewStats[Stat.DCPercent] = amount;
                        break;
                    case RefineType.SPPercent:
                        if (targetItem.Info.Stats[Stat.MinMC] == 0 && targetItem.Info.Stats[Stat.MaxMC] == 0 && targetItem.Info.Stats[Stat.MinSC] == 0 && targetItem.Info.Stats[Stat.MaxSC] == 0)
                        {
                            targetItem.AddStat(Stat.MCPercent, amount, StatSource.Added);
                            presult.NewStats[Stat.MCPercent] = amount;

                            targetItem.AddStat(Stat.SCPercent, amount, StatSource.Added);
                            presult.NewStats[Stat.SCPercent] = amount;
                        }

                        if (targetItem.Info.Stats[Stat.MinMC] > 0 || targetItem.Info.Stats[Stat.MaxMC] > 0)
                        {
                            targetItem.AddStat(Stat.MCPercent, amount, StatSource.Added);
                            presult.NewStats[Stat.MCPercent] = amount;
                        }

                        if (targetItem.Info.Stats[Stat.MinSC] > 0 || targetItem.Info.Stats[Stat.MaxSC] > 0)
                        {
                            targetItem.AddStat(Stat.SCPercent, amount, StatSource.Added);
                            presult.NewStats[Stat.SCPercent] = amount;
                        }
                        break;
                    case RefineType.HealthPercent:
                        targetItem.AddStat(Stat.HealthPercent, amount, StatSource.Added);
                        presult.NewStats[Stat.HealthPercent] = amount;
                        break;
                    case RefineType.ManaPercent:
                        targetItem.AddStat(Stat.ManaPercent, amount, StatSource.Added);
                        presult.NewStats[Stat.ManaPercent] = amount;
                        break;
                    case RefineType.Fire:
                        targetItem.AddStat(Stat.FireAttack, amount, StatSource.Added);
                        presult.NewStats[Stat.FireAttack] = amount;
                        break;
                    case RefineType.Ice:
                        targetItem.AddStat(Stat.IceAttack, amount, StatSource.Added);
                        presult.NewStats[Stat.IceAttack] = amount;
                        break;
                    case RefineType.Lightning:
                        targetItem.AddStat(Stat.LightningAttack, amount, StatSource.Added);
                        presult.NewStats[Stat.LightningAttack] = amount;
                        break;
                    case RefineType.Wind:
                        targetItem.AddStat(Stat.WindAttack, amount, StatSource.Added);
                        presult.NewStats[Stat.WindAttack] = amount;
                        break;
                    case RefineType.Holy:
                        targetItem.AddStat(Stat.HolyAttack, amount, StatSource.Added);
                        presult.NewStats[Stat.HolyAttack] = amount;
                        break;
                    case RefineType.Dark:
                        targetItem.AddStat(Stat.DarkAttack, amount, StatSource.Added);
                        presult.NewStats[Stat.DarkAttack] = amount;
                        break;
                    case RefineType.Phantom:
                        targetItem.AddStat(Stat.PhantomAttack, amount, StatSource.Added);
                        presult.NewStats[Stat.PhantomAttack] = amount;
                        break;
                    case RefineType.AC:
                        targetItem.AddStat(Stat.MaxAC, amount, StatSource.Added);
                        presult.NewStats[Stat.MaxAC] = amount;
                        break;
                    case RefineType.MR:
                        targetItem.AddStat(Stat.MaxMR, amount, StatSource.Added);
                        presult.NewStats[Stat.MaxMR] = amount;
                        break;
                    case RefineType.Accuracy:
                        targetItem.AddStat(Stat.Accuracy, amount, StatSource.Added);
                        presult.NewStats[Stat.Accuracy] = amount;
                        break;
                    case RefineType.Agility:
                        targetItem.AddStat(Stat.Agility, amount, StatSource.Added);
                        presult.NewStats[Stat.Agility] = amount;
                        break;
                    default:
                        Character.Account.Banned = true;
                        Character.Account.BanReason = "Attempted to Exploit refine, Accessory Refine Type.";
                        Character.Account.BanExpiry = SEnvir.Now.AddYears(10);
                        return;
                }
                targetItem.StatsChanged();
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.AccessoryRefineSuccess, targetItem.Info.ItemName, p.RefineType, amount), MessageType.System);
                RefreshStats();
            }
            #endregion
            else
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.AccessoryRefineFailed, targetItem.Info.ItemName), MessageType.System);
                targetArray[targetItem.Slot] = null;
                result.Links.Add(p.Target);
                RemoveItem(targetItem);
                targetItem.Delete();

            }

            Gold.Amount -= 50000;
            GoldChanged();
            targetOreArray[oretargetItem.Slot] = null;
            result.Links.Add(p.OreTarget);
            RemoveItem(oretargetItem);
            oretargetItem.Delete();
            Companion?.RefreshWeight();
            RefreshStats();
        }

        public void NPCRepair(C.NPCRepair p)
        {
            S.NPCRepair result = new S.NPCRepair { Links = p.Links, Special = p.Special, SpecialRepairDelay = Config.SpecialRepairDelay };
            Enqueue(result);

            if (Dead || NPC == null || NPCPage == null || NPCPage.DialogType != NPCDialogType.Repair) return;

            if (!ParseLinks(result.Links, 0, 100)) return;

            long cost = 0;
            int count = 0;

            foreach (CellLinkInfo link in p.Links)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Equipment:
                        array = Equipment;
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

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || !item.Info.CanRepair || item.Info.Durability == 0) return;
                if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return;

                switch (item.Info.ItemType)
                {
                    case ItemType.Weapon:
                    case ItemType.Armour:
                    case ItemType.Helmet:
                    case ItemType.Necklace:
                    case ItemType.Bracelet:
                    case ItemType.Ring:
                    case ItemType.Shoes:
                    case ItemType.Shield:
                        break;
                    default:
                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.RepairFail, item.Info.ItemName), MessageType.System);
                        return;
                }

                if (item.CurrentDurability >= item.MaxDurability)
                {
                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.RepairFailRepaired, item.Info.ItemName), MessageType.System);
                    return;
                }
                if (NPCPage.Types.FirstOrDefault(x => x.ItemType == item.Info.ItemType) == null)
                {
                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.RepairFailLocation, item.Info.ItemName), MessageType.System);
                    return;
                }
                if (p.Special && SEnvir.Now < item.SpecialRepairCoolDown)
                {
                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.RepairFailCooldown, item.Info.ItemName, Functions.ToString(item.SpecialRepairCoolDown - SEnvir.Now, false)), MessageType.System);

                    return;
                }


                count++;
                cost += array[link.Slot].RepairCost(p.Special);
            }

            if (p.GuildFunds)
            {
                if (Character.Account.GuildMember == null)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.NPCRepairGuild, MessageType.System);
                    return;
                }
                if ((Character.Account.GuildMember.Permission & GuildPermission.FundsRepair) != GuildPermission.FundsRepair)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.NPCRepairPermission, MessageType.System);
                    return;
                }

                if (cost > Character.Account.GuildMember.Guild.GuildFunds)
                {
                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.NPCRepairGuildCost, cost - Character.Account.GuildMember.Guild.GuildFunds), MessageType.System);
                    return;
                }
            }
            else
            {
                if (cost > Gold.Amount)
                {
                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.NPCRepairCost, Gold.Amount - cost), MessageType.System);
                    return;
                }
            }

            bool refresh = false;
            foreach (CellLinkInfo link in p.Links)
            {
                UserItem[] array = null;

                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Equipment:
                        array = Equipment;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.GuildStorage:
                        array = Character.Account.GuildMember.Guild.Storage;
                        break;
                    case GridType.CompanionInventory:
                        array = Companion.Inventory;
                        break;
                }

                UserItem item = array[link.Slot];

                if (item.CurrentDurability == 0 && link.GridType == GridType.Equipment)
                    refresh = true;

                if (p.Special)
                {
                    item.CurrentDurability = item.MaxDurability;

                    if (item.Info.ItemType != ItemType.Weapon)
                        item.SpecialRepairCoolDown = SEnvir.Now + Config.SpecialRepairDelay;
                }
                else
                {
                    item.MaxDurability = Math.Max(0, item.MaxDurability - (item.MaxDurability - item.CurrentDurability) / Globals.DuraLossRate);
                    item.CurrentDurability = item.MaxDurability;
                }
            }

            Connection.ReceiveChat(string.Format(p.Special ? Connection.Language.NPCRepairSpecialResult : Connection.Language.NPCRepairResult, count, cost), MessageType.System);

            result.Success = true;

            if (p.GuildFunds)
            {
                Character.Account.GuildMember.Guild.GuildFunds -= cost;
                Character.Account.GuildMember.Guild.DailyGrowth -= cost;

                foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                {
                    member.Account.Connection?.Player?.Enqueue(new S.GuildFundsChanged { Change = -cost, ObserverPacket = false });
                    member.Account.Connection?.ReceiveChat(string.Format(member.Account.Connection.Language.NPCRepairGuildResult, Name, cost, count), MessageType.System);
                }
            }
            else
            {
                Gold.Amount -= cost;
                GoldChanged();
            }

            if (refresh)
                RefreshStats();
        }
        public void NPCRefinementStone(C.NPCRefinementStone p)
        {
            S.ItemsChanged result = new S.ItemsChanged
            {
                Links = new List<CellLinkInfo>()
            };
            Enqueue(result);

            if (p.IronOres != null) result.Links.AddRange(p.IronOres);
            if (p.SilverOres != null) result.Links.AddRange(p.SilverOres);
            if (p.DiamondOres != null) result.Links.AddRange(p.DiamondOres);
            if (p.GoldOres != null) result.Links.AddRange(p.GoldOres);
            if (p.Crystal != null) result.Links.AddRange(p.Crystal);

            if (Dead || NPC == null || NPCPage == null || NPCPage.DialogType != NPCDialogType.RefinementStone) return;

            if (SEnvir.RefinementStoneInfo == null) return;

            if (!ParseLinks(p.IronOres, 4, 4)) return;
            if (!ParseLinks(p.SilverOres, 4, 4)) return;
            if (!ParseLinks(p.DiamondOres, 4, 4)) return;
            if (!ParseLinks(p.GoldOres, 2, 2)) return;
            if (!ParseLinks(p.Crystal, 1, 1)) return;

            if (p.Gold < 0) return;

            if (p.Gold > Gold.Amount)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.NPCRefinementGold, MessageType.System);
                return;
            }

            ItemCheck check = new ItemCheck(SEnvir.RefinementStoneInfo, 1, UserItemFlags.None, TimeSpan.Zero);
            if (!CanGainItems(false, check))
            {
                Connection.ReceiveChatWithObservers(con => con.Language.NPCRefinementStoneFailedRoom, MessageType.System);
                return;
            }

            int ironPurity = 0;
            int silverPurity = 0;
            int diamondPurity = 0;
            int goldPurity = 0;

            foreach (CellLinkInfo link in p.IronOres)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemEffect != ItemEffect.IronOre) return;

                ironPurity += item.CurrentDurability;
            }
            foreach (CellLinkInfo link in p.SilverOres)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemEffect != ItemEffect.SilverOre) return;

                silverPurity += item.CurrentDurability;
            }
            foreach (CellLinkInfo link in p.DiamondOres)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemEffect != ItemEffect.Diamond) return;

                diamondPurity += item.CurrentDurability;
            }
            foreach (CellLinkInfo link in p.GoldOres)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemEffect != ItemEffect.GoldOre) return;

                goldPurity += item.CurrentDurability;
            }
            foreach (CellLinkInfo link in p.Crystal)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemEffect != ItemEffect.Crystal) return;
            }

            long chance = p.Gold / 25000; // 250k / 10%, 2,500,000 for 100%

            chance += Math.Min(23, ironPurity / 4350); // Need 100 Purity
            chance += Math.Min(23, silverPurity / 3475); // Need 80 Purity
            chance += Math.Min(23, diamondPurity / 2600); //Need 60 Purity
            chance += Math.Min(31, goldPurity / 1600); //Need 50 Purity

            foreach (CellLinkInfo link in p.IronOres)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                UserItem item = array[link.Slot];

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    array[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;
            }
            foreach (CellLinkInfo link in p.SilverOres)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                UserItem item = array[link.Slot];

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    array[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;
            }
            foreach (CellLinkInfo link in p.DiamondOres)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                UserItem item = array[link.Slot];

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    array[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;
            }
            foreach (CellLinkInfo link in p.GoldOres)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                UserItem item = array[link.Slot];

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    array[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;
            }
            foreach (CellLinkInfo link in p.Crystal)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                UserItem item = array[link.Slot];

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    array[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;
            }

            Gold.Amount -= p.Gold;
            GoldChanged();
            result.Success = true;

            if (SEnvir.Random.Next(100) >= chance)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.NPCRefinementStoneFailed, MessageType.System);
                return;
            }


            UserItem stone = SEnvir.CreateFreshItem(check);
            GainItem(stone);
        }
        public void NPCRefine(C.NPCRefine p)
        {
            S.NPCRefine result = new S.NPCRefine
            {
                RefineQuality = p.RefineQuality,
                RefineType = p.RefineType,
                Ores = p.Ores,
                Items = p.Items,
                Specials = p.Specials,
            };
            Enqueue(result);

            switch (p.RefineQuality)
            {
                case RefineQuality.Rush:
                case RefineQuality.Quick:
                case RefineQuality.Standard:
                case RefineQuality.Careful:
                case RefineQuality.Precise:
                    break;
                default:
                    Character.Account.Banned = true;
                    Character.Account.BanReason = "Attempted to Exploit refine, Weapon Refine Quality.";
                    Character.Account.BanExpiry = SEnvir.Now.AddYears(10);
                    return;
            }

            switch (p.RefineType)
            {
                case RefineType.Durability:
                case RefineType.DC:
                case RefineType.SpellPower:
                case RefineType.Fire:
                case RefineType.Ice:
                case RefineType.Lightning:
                case RefineType.Wind:
                case RefineType.Holy:
                case RefineType.Dark:
                case RefineType.Phantom:
                    break;
                default:
                    Character.Account.Banned = true;
                    Character.Account.BanReason = "Attempted to Exploit refine, Weapon Refine Type.";
                    Character.Account.BanExpiry = SEnvir.Now.AddYears(10);
                    return;
            }



            if (Dead || NPC == null || NPCPage == null || NPCPage.DialogType != NPCDialogType.Refine) return;

            if (!ParseLinks(p.Ores, 0, 5)) return;
            if (!ParseLinks(p.Items, 0, 3)) return;
            if (!ParseLinks(p.Specials, 0, 1)) return;

            int RefineCost = 50000;

            UserItem weapon = Equipment[(int)EquipmentSlot.Weapon];

            if (weapon == null || (weapon.Flags & UserItemFlags.Refinable) != UserItemFlags.Refinable) return;

            if ((weapon.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;

            if (Gold.Amount < RefineCost)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.NPCRefinementGold, MessageType.System);
                return;
            }

            int ore = 0;
            int items = 0;
            int quality = 0;
            int special = 0;
            //Check Ores

            foreach (CellLinkInfo link in p.Ores)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemEffect != ItemEffect.BlackIronOre) return;

                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;

                ore += item.CurrentDurability;
            }

            foreach (CellLinkInfo link in p.Items)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null) return;
                if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return;

                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;

                switch (item.Info.ItemType)
                {
                    case ItemType.Necklace:
                    case ItemType.Bracelet:
                    case ItemType.Ring:
                        break;
                    default:
                        return;
                }

                items += item.Info.RequiredAmount;

                if (item.Info.Rarity != Rarity.Common)
                    quality++;
            }

            foreach (CellLinkInfo link in p.Specials)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemType != ItemType.RefineSpecial) return;

                if (item.Info.Shape != 1) return;
                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;

                link.Count = 1;

                special += item.Info.Stats[Stat.MaxRefineChance];
            }


            /*
             * BaseChance  90% - Weapon Level
             * Max Chance  -5% | 0% | +5% | +10% | +20% = (Rush | Quick | Standard | Careful | Precise)  
             * 5 Ore 1% per 2 Dura Max
             * Items 1% per 6 Item Levels, 5% for Quality
             * Base Chance = 60% -Weapon Level  * 5%
             */

            int maxChance = 90 - weapon.Level + special;
            int chance = 60 - weapon.Level * 5;

            switch (p.RefineQuality)
            {
                case RefineQuality.Rush:
                    maxChance -= 5;
                    break;
                case RefineQuality.Quick:
                    break;
                case RefineQuality.Standard:
                    maxChance += 5;
                    break;
                case RefineQuality.Careful:
                    maxChance += 10;
                    break;
                case RefineQuality.Precise:
                    maxChance += 20;
                    break;
                default:
                    return;
            }

            //Special + Max Chance

            chance += ore / 2000;
            chance += items / 6;
            chance += quality * 25;

            maxChance = Math.Min(100, maxChance);
            chance = Math.Min(maxChance, chance);

            foreach (CellLinkInfo link in p.Ores)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                UserItem item = array[link.Slot];

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    array[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;
            }

            foreach (CellLinkInfo link in p.Items)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                UserItem item = array[link.Slot];

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    array[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;
            }

            foreach (CellLinkInfo link in p.Specials)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                UserItem item = array[link.Slot];

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    array[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;
            }

            RemoveItem(weapon);
            Equipment[(int)EquipmentSlot.Weapon] = null;

            Gold.Amount -= RefineCost;
            GoldChanged();

            RefineInfo info = SEnvir.RefineInfoList.CreateNewObject();

            info.Character = Character;
            info.Weapon = weapon;
            info.Chance = chance;
            info.MaxChance = maxChance;
            info.Quality = p.RefineQuality;
            info.Type = p.RefineType;
            info.RetrieveTime = SEnvir.Now + Globals.RefineTimes[p.RefineQuality];

            result.Success = true;
            SendShapeUpdate();
            RefreshStats();

            Enqueue(new S.RefineList { List = new List<ClientRefineInfo> { info.ToClientInfo() } });
        }
        public void NPCRefineRetrieve(int index)
        {
            if (Dead || NPC == null || NPCPage == null || NPCPage.DialogType != NPCDialogType.RefineRetrieve) return;

            RefineInfo info = Character.Refines.FirstOrDefault(x => x.Index == index);

            if (info == null) return;

            if (SEnvir.Now < info.RetrieveTime && !Character.Account.TempAdmin)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.NPCRefineNotReady, MessageType.System);
                return;
            }

            ItemCheck check = new ItemCheck(info.Weapon, info.Weapon.Count, info.Weapon.Flags, info.Weapon.ExpireTime);

            if (!CanGainItems(false, check))
            {
                Connection.ReceiveChatWithObservers(con => con.Language.NPCRefineNoRoom, MessageType.System);
                return;
            }

            UserItem weapon = info.Weapon;

            if (SEnvir.Random.Next(100) < info.Chance)
            {
                switch (info.Type)
                {
                    case RefineType.Durability:
                        weapon.MaxDurability += 2000;
                        break;
                    case RefineType.DC:
                        weapon.AddStat(Stat.MaxDC, 1, StatSource.Refine);
                        break;
                    case RefineType.SpellPower:
                        if (weapon.Info.Stats[Stat.MinMC] == 0 && weapon.Info.Stats[Stat.MaxMC] == 0 && weapon.Info.Stats[Stat.MinSC] == 0 && weapon.Info.Stats[Stat.MaxSC] == 0)
                        {
                            weapon.AddStat(Stat.MaxMC, 1, StatSource.Refine);
                            weapon.AddStat(Stat.MaxSC, 1, StatSource.Refine);
                        }

                        if (weapon.Info.Stats[Stat.MinMC] > 0 || weapon.Info.Stats[Stat.MaxMC] > 0)
                            weapon.AddStat(Stat.MaxMC, 1, StatSource.Refine);

                        if (weapon.Info.Stats[Stat.MinSC] > 0 || weapon.Info.Stats[Stat.MaxSC] > 0)
                            weapon.AddStat(Stat.MaxSC, 1, StatSource.Refine);
                        break;
                    case RefineType.Fire:
                        weapon.AddStat(Stat.FireAttack, 1, StatSource.Refine);
                        weapon.AddStat(Stat.WeaponElement, 1 - weapon.Stats[Stat.WeaponElement], StatSource.Refine);
                        break;
                    case RefineType.Ice:
                        weapon.AddStat(Stat.IceAttack, 1, StatSource.Refine);
                        weapon.AddStat(Stat.WeaponElement, 2 - weapon.Stats[Stat.WeaponElement], StatSource.Refine);
                        break;
                    case RefineType.Lightning:
                        weapon.AddStat(Stat.LightningAttack, 1, StatSource.Refine);
                        weapon.AddStat(Stat.WeaponElement, 3 - weapon.Stats[Stat.WeaponElement], StatSource.Refine);
                        break;
                    case RefineType.Wind:
                        weapon.AddStat(Stat.WindAttack, 1, StatSource.Refine);
                        weapon.AddStat(Stat.WeaponElement, 4 - weapon.Stats[Stat.WeaponElement], StatSource.Refine);
                        break;
                    case RefineType.Holy:
                        weapon.AddStat(Stat.HolyAttack, 1, StatSource.Refine);
                        weapon.AddStat(Stat.WeaponElement, 5 - weapon.Stats[Stat.WeaponElement], StatSource.Refine);
                        break;
                    case RefineType.Dark:
                        weapon.AddStat(Stat.DarkAttack, 1, StatSource.Refine);
                        weapon.AddStat(Stat.WeaponElement, 6 - weapon.Stats[Stat.WeaponElement], StatSource.Refine);
                        break;
                    case RefineType.Phantom:
                        weapon.AddStat(Stat.PhantomAttack, 1, StatSource.Refine);
                        weapon.AddStat(Stat.WeaponElement, 7 - weapon.Stats[Stat.WeaponElement], StatSource.Refine);
                        break;
                    case RefineType.Reset:
                        weapon.Level = 1;
                        weapon.ResetCoolDown = SEnvir.Now.AddDays(14);

                        weapon.MergeRefineElements(out Stat element);

                        for (int i = weapon.AddedStats.Count - 1; i >= 0; i--)
                        {
                            UserItemStat stat = weapon.AddedStats[i];
                            if (stat.StatSource != StatSource.Refine || stat.Stat == Stat.WeaponElement) continue;

                            int amount = stat.Amount / 5;

                            stat.Delete();
                            weapon.AddStat(stat.Stat, amount, StatSource.Enhancement);
                        }

                        for (int i = weapon.AddedStats.Count - 1; i >= 0; i--)
                        {
                            UserItemStat stat = weapon.AddedStats[i];
                            if (stat.StatSource != StatSource.Enhancement) continue;

                            switch (stat.Stat)
                            {
                                case Stat.MaxDC:
                                case Stat.MaxMC:
                                case Stat.MaxSC:
                                    stat.Amount = Math.Min(stat.Amount, 200);
                                    break;
                                case Stat.FireAttack:
                                case Stat.LightningAttack:
                                case Stat.IceAttack:
                                case Stat.WindAttack:
                                case Stat.DarkAttack:
                                case Stat.HolyAttack:
                                case Stat.PhantomAttack:
                                    stat.Amount = Math.Min(stat.Amount, 200);
                                    break;
                                case Stat.EvasionChance:
                                case Stat.BlockChance:
                                    stat.Amount = Math.Min(stat.Amount, 10);
                                    break;
                            }

                        }

                        break;
                }
                weapon.StatsChanged();

                Connection.ReceiveChatWithObservers(con => con.Language.NPCRefineSuccess, MessageType.System);
            }
            else
            {
                Connection.ReceiveChatWithObservers(con => con.Language.NPCRefineFailed, MessageType.System);
            }

            weapon.Flags &= ~UserItemFlags.Refinable;

            weapon.Flags |= UserItemFlags.Locked;

            Enqueue(new S.NPCRefineRetrieve { Index = info.Index });
            info.Weapon = null;
            info.Character = null;
            info.Delete();


            GainItem(weapon);
        }
        public void NPCResetWeapon()
        {
            UserItem weapon = Equipment[(int)EquipmentSlot.Weapon];
            RemoveItem(weapon);
            Equipment[(int)EquipmentSlot.Weapon] = null;
            Enqueue(new S.ItemChanged
            {
                Link = new CellLinkInfo { Slot = (int)EquipmentSlot.Weapon, GridType = GridType.Equipment },
                Success = true
            });

            RefineInfo info = SEnvir.RefineInfoList.CreateNewObject();

            info.Character = Character;
            info.Weapon = weapon;
            info.Chance = 100;
            info.MaxChance = 100;
            info.Quality = RefineQuality.Precise;
            info.Type = RefineType.Reset;
            info.RetrieveTime = SEnvir.Now + Globals.RefineTimes[RefineQuality.Precise];

            SendShapeUpdate();
            RefreshStats();

            Enqueue(new S.RefineList { List = new List<ClientRefineInfo> { info.ToClientInfo() } });
        }

        public void NPCRebirth()
        {
            AdjustRebirth(Character.Rebirth + 1);
        }

        public void AdjustRebirth(int rebirth)
        {
            Level = 1;
            Experience = Experience / 200;

            Enqueue(new S.LevelChanged { Level = Level, Experience = Experience, MaxExperience = MaxExperience });
            Broadcast(new S.ObjectLeveled { ObjectID = ObjectID });

            Character.Rebirth = rebirth;
            Character.SpentPoints = 0;
            Character.HermitStats.Clear();

            LogMilestone(MilestoneType.Rebirth, rebirth, true);

            if (Character.Discipline != null)
            {
                Character.Discipline.Delete();
                Character.Discipline = null;

                Enqueue(new S.DisciplineUpdate { Discipline = null });
            }

            RefreshStats();
        }

        public void PromoteFame()
        {
            var nextFame = GetNextFameTitle();

            if (nextFame == null) return;

            var currency = SEnvir.CurrencyInfoList.Binding.FirstOrDefault(x => x.Type == CurrencyType.FP);

            if (currency == null) return;

            var userCurrency = GetCurrency(currency);

            if (userCurrency.Amount < nextFame.Cost) return;

            List<ItemCheck> checks = new List<ItemCheck>();

            foreach (var reward in nextFame.ItemRewards)
            {
                ItemCheck check = new ItemCheck(reward.Item, reward.Amount, UserItemFlags.None, TimeSpan.Zero);

                checks.Add(check);
            }

            if (!CanGainItems(false, checks.ToArray()))
            {
                Connection.ReceiveChatWithObservers(con => con.Language.FameNeedSpace, MessageType.System);

                return;
            }

            userCurrency.Amount -= nextFame.Cost;
            CurrencyChanged(userCurrency);

            Character.Fame = nextFame.Index;

            foreach (ItemCheck check in checks)
            {
                while (check.Count > 0)
                    GainItem(SEnvir.CreateFreshItem(check));
            }

            ApplyFameBuff();

            RefreshStats();
        }

        public FameInfo GetNextFameTitle()
        {
            int order = -1;

            var currentFame = SEnvir.FameInfoList.Binding.FirstOrDefault(x => x.Index == Character.Fame);

            if (currentFame != null)
            {
                order = currentFame.Order;
            }

            var nextFame = SEnvir.FameInfoList.Binding.Where(x => x.Order > order).OrderBy(x => x.Order).FirstOrDefault();

            return nextFame;
        }

        public void NPCMasterRefine(C.NPCMasterRefine p)
        {
            S.NPCMasterRefine result = new S.NPCMasterRefine
            {
                Fragment1s = p.Fragment1s,
                Fragment2s = p.Fragment2s,
                Fragment3s = p.Fragment3s,
                Stones = p.Stones,
                Specials = p.Specials,
            };
            Enqueue(result);


            switch (p.RefineType)
            {
                case RefineType.DC:
                case RefineType.SpellPower:
                case RefineType.Fire:
                case RefineType.Ice:
                case RefineType.Lightning:
                case RefineType.Wind:
                case RefineType.Holy:
                case RefineType.Dark:
                case RefineType.Phantom:
                    break;
                default:
                    Character.Account.Banned = true;
                    Character.Account.BanReason = "Attempted to Exploit Master refine, Weapon Refine Type.";
                    Character.Account.BanExpiry = SEnvir.Now.AddYears(10);
                    return;
            }

            if (Dead || NPC == null || NPCPage == null || NPCPage.DialogType != NPCDialogType.MasterRefine) return;

            if (!ParseLinks(p.Fragment1s, 1, 1)) return;
            if (!ParseLinks(p.Fragment2s, 1, 1)) return;
            if (!ParseLinks(p.Fragment3s, 1, 1)) return;
            if (!ParseLinks(p.Stones, 1, 1)) return;
            if (!ParseLinks(p.Specials, 0, 1)) return;

            UserItem weapon = Equipment[(int)EquipmentSlot.Weapon];

            if (weapon == null) return;

            if ((weapon.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;

            if (weapon.Level != Globals.WeaponExperienceList.Count) return;

            long fragmentCount = 0;
            int special = 0;
            int fragmentRate = 2;
            //Check Ores

            foreach (CellLinkInfo link in p.Fragment1s)
            {
                if (link.Count != 10) return;

                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemEffect != ItemEffect.Fragment1) return;
                if (item.Count < 10) return;

                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;
            }
            foreach (CellLinkInfo link in p.Fragment2s)
            {
                if (link.Count != 10) return;

                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemEffect != ItemEffect.Fragment2) return;
                if (item.Count < 10) return;

                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;
            }
            foreach (CellLinkInfo link in p.Fragment3s)
            {
                if (link.Count < 1) return;

                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemEffect != ItemEffect.Fragment3) return;
                if (item.Count < link.Count) return;

                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;

                fragmentCount += link.Count;
            }

            foreach (CellLinkInfo link in p.Stones)
            {
                if (link.Count != 1) return;

                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemEffect != ItemEffect.RefinementStone) return;
                if (item.Count < link.Count) return;

                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;
            }
            foreach (CellLinkInfo link in p.Specials)
            {
                if (link.Count != 1) return;

                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemType != ItemType.RefineSpecial) return;

                if (item.Info.Shape != 5) return;
                if (item.Count < link.Count) return;
                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;


                special += item.Info.Stats[Stat.MaxRefineChance];
                fragmentRate += item.Info.Stats[Stat.FragmentRate];
            }


            int maxChance = 80 + special;
            int statValue = 0;
            bool sucess = false;

            switch (p.RefineType)
            {
                case RefineType.DC:
                    foreach (UserItemStat stat in weapon.AddedStats)
                    {
                        if (stat.Stat != Stat.MaxDC || stat.StatSource != StatSource.Refine) continue;

                        statValue = stat.Amount;
                        break;
                    }

                    sucess = SEnvir.Random.Next(100) < Math.Min(maxChance, 80 - statValue * 4 + fragmentCount * fragmentRate);

                    if (sucess)
                        weapon.AddStat(Stat.MaxDC, 5, StatSource.Refine);
                    else
                        weapon.AddStat(Stat.MaxDC, -Math.Min(statValue, 5), StatSource.Refine);

                    break;
                case RefineType.SpellPower:
                    foreach (UserItemStat stat in weapon.AddedStats)
                    {
                        if (stat.StatSource != StatSource.Refine) continue;

                        if (stat.Stat != Stat.MaxMC && stat.Stat != Stat.MaxSC) continue;

                        statValue = Math.Max(statValue, stat.Amount);
                    }

                    sucess = SEnvir.Random.Next(100) < Math.Min(maxChance, 80 - statValue * 4 + fragmentCount * fragmentRate);

                    if (sucess)
                    {
                        if (weapon.Info.Stats[Stat.MinMC] == 0 && weapon.Info.Stats[Stat.MaxMC] == 0 && weapon.Info.Stats[Stat.MinSC] == 0 && weapon.Info.Stats[Stat.MaxSC] == 0)
                        {
                            weapon.AddStat(Stat.MaxMC, 5, StatSource.Refine);
                            weapon.AddStat(Stat.MaxSC, 5, StatSource.Refine);
                        }

                        if (weapon.Info.Stats[Stat.MinMC] > 0 || weapon.Info.Stats[Stat.MaxMC] > 0)
                            weapon.AddStat(Stat.MaxMC, 5, StatSource.Refine);

                        if (weapon.Info.Stats[Stat.MinSC] > 0 || weapon.Info.Stats[Stat.MaxSC] > 0)
                            weapon.AddStat(Stat.MaxSC, 5, StatSource.Refine);
                    }
                    else
                    {
                        if (weapon.Info.Stats[Stat.MinMC] == 0 && weapon.Info.Stats[Stat.MaxMC] == 0 && weapon.Info.Stats[Stat.MinSC] == 0 && weapon.Info.Stats[Stat.MaxSC] == 0)
                        {
                            weapon.AddStat(Stat.MaxMC, -Math.Min(statValue, 5), StatSource.Refine);
                            weapon.AddStat(Stat.MaxSC, -Math.Min(statValue, 5), StatSource.Refine);
                        }

                        if (weapon.Info.Stats[Stat.MinMC] > 0 || weapon.Info.Stats[Stat.MaxMC] > 0)
                            weapon.AddStat(Stat.MaxMC, -Math.Min(statValue, 5), StatSource.Refine);

                        if (weapon.Info.Stats[Stat.MinSC] > 0 || weapon.Info.Stats[Stat.MaxSC] > 0)
                            weapon.AddStat(Stat.MaxSC, -Math.Min(statValue, 5), StatSource.Refine);
                    }
                    break;
                case RefineType.Fire:
                case RefineType.Ice:
                case RefineType.Lightning:
                case RefineType.Wind:
                case RefineType.Holy:
                case RefineType.Dark:
                case RefineType.Phantom:
                    statValue = weapon.MergeRefineElements(out Stat element);

                    sucess = SEnvir.Random.Next(100) < Math.Min(maxChance, 80 - statValue * 4 + fragmentCount * fragmentRate);

                    if (element == Stat.None)
                        element = Stat.FireAttack; //Could be any

                    if (sucess)
                    {

                        weapon.AddStat(element, 5, StatSource.Refine);
                        switch (p.RefineType)
                        {
                            case RefineType.Fire:
                                weapon.AddStat(Stat.WeaponElement, 1 - weapon.Stats[Stat.WeaponElement], StatSource.Refine);
                                break;
                            case RefineType.Ice:
                                weapon.AddStat(Stat.WeaponElement, 2 - weapon.Stats[Stat.WeaponElement], StatSource.Refine);
                                break;
                            case RefineType.Lightning:
                                weapon.AddStat(Stat.WeaponElement, 3 - weapon.Stats[Stat.WeaponElement], StatSource.Refine);
                                break;
                            case RefineType.Wind:
                                weapon.AddStat(Stat.WeaponElement, 4 - weapon.Stats[Stat.WeaponElement], StatSource.Refine);
                                break;
                            case RefineType.Holy:
                                weapon.AddStat(Stat.WeaponElement, 5 - weapon.Stats[Stat.WeaponElement], StatSource.Refine);
                                break;
                            case RefineType.Dark:
                                weapon.AddStat(Stat.WeaponElement, 6 - weapon.Stats[Stat.WeaponElement], StatSource.Refine);
                                break;
                            case RefineType.Phantom:
                                weapon.AddStat(Stat.WeaponElement, 7 - weapon.Stats[Stat.WeaponElement], StatSource.Refine);
                                break;
                        }
                    }
                    else
                        weapon.AddStat(element, -Math.Min(statValue, 5), StatSource.Refine);
                    break;
            }


            foreach (CellLinkInfo link in p.Fragment1s)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                UserItem item = array[link.Slot];

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    array[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;
            }

            foreach (CellLinkInfo link in p.Fragment2s)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                UserItem item = array[link.Slot];

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    array[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;
            }

            foreach (CellLinkInfo link in p.Fragment3s)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                UserItem item = array[link.Slot];

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    array[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;
            }

            foreach (CellLinkInfo link in p.Stones)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                UserItem item = array[link.Slot];

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    array[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;
            }

            foreach (CellLinkInfo link in p.Specials)
            {
                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                UserItem item = array[link.Slot];

                if (item.Count == link.Count)
                {
                    RemoveItem(item);
                    array[link.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= link.Count;
            }

            result.Success = true;

            Connection.ReceiveChat(sucess ? Connection.Language.NPCRefineSuccess : Connection.Language.NPCRefineFailed, MessageType.System);

            weapon.StatsChanged();
            SendShapeUpdate();
            RefreshStats();

            Enqueue(new S.ItemStatsRefreshed { GridType = GridType.Equipment, Slot = (int)EquipmentSlot.Weapon, NewStats = new Stats(weapon.Stats) });
        }

        public void NPCSpecialRefine(Stat stat, int amount)
        {
            UserItem weapon = Equipment[(int)EquipmentSlot.Weapon];

            if (weapon == null) return;

            if (weapon.Level != Globals.WeaponExperienceList.Count) return;

            weapon.AddStat(stat, amount, StatSource.Refine);


            Connection.ReceiveChatWithObservers(con => con.Language.NPCRefineSuccess, MessageType.System);

            weapon.StatsChanged();
            SendShapeUpdate();
            RefreshStats();

            Enqueue(new S.ItemStatsRefreshed { GridType = GridType.Equipment, Slot = (int)EquipmentSlot.Weapon, NewStats = new Stats(weapon.Stats) });
        }

        public void NPCMasterRefineEvaluate(C.NPCMasterRefineEvaluate p)
        {
            switch (p.RefineType)
            {
                case RefineType.DC:
                case RefineType.SpellPower:
                case RefineType.Fire:
                case RefineType.Ice:
                case RefineType.Lightning:
                case RefineType.Wind:
                case RefineType.Holy:
                case RefineType.Dark:
                case RefineType.Phantom:
                    break;
                default:
                    return;
            }

            if (Dead || NPC == null || NPCPage == null || NPCPage.DialogType != NPCDialogType.MasterRefine) return;

            if (!ParseLinks(p.Fragment1s, 1, 1)) return;
            if (!ParseLinks(p.Fragment2s, 1, 1)) return;
            if (!ParseLinks(p.Fragment3s, 1, 1)) return;
            if (!ParseLinks(p.Stones, 1, 1)) return;
            if (!ParseLinks(p.Specials, 0, 1)) return;

            if (Gold.Amount < Globals.MasterRefineEvaluateCost)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.NPCMasterRefineGold, Globals.MasterRefineEvaluateCost), MessageType.System);
                return;
            }

            UserItem weapon = Equipment[(int)EquipmentSlot.Weapon];

            if (weapon == null) return;

            if ((weapon.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;

            if (weapon.Level != Globals.WeaponExperienceList.Count) return;

            long fragmentCount = 0;
            int special = 0;
            int fragmentRate = 2;
            //Check Ores

            foreach (CellLinkInfo link in p.Fragment1s)
            {
                if (link.Count != 10) return;

                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemEffect != ItemEffect.Fragment1) return;
                if (item.Count < 10) return;

                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;
            }
            foreach (CellLinkInfo link in p.Fragment2s)
            {
                if (link.Count != 10) return;

                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemEffect != ItemEffect.Fragment2) return;
                if (item.Count < 10) return;

                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;
            }
            foreach (CellLinkInfo link in p.Fragment3s)
            {
                if (link.Count < 1) return;

                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemEffect != ItemEffect.Fragment3) return;
                if (item.Count < link.Count) return;

                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;

                fragmentCount += link.Count;
            }
            foreach (CellLinkInfo link in p.Stones)
            {
                if (link.Count != 1) return;

                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemEffect != ItemEffect.RefinementStone) return;
                if (item.Count < link.Count) return;

                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;
            }
            foreach (CellLinkInfo link in p.Specials)
            {
                if (link.Count != 1) return;

                UserItem[] array;
                switch (link.GridType)
                {
                    case GridType.Inventory:
                        array = Inventory;
                        break;
                    case GridType.Storage:
                        array = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        array = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= array.Length) return;
                UserItem item = array[link.Slot];

                if (item == null || item.Info.ItemType != ItemType.RefineSpecial) return;

                if (item.Info.Shape != 5) return;
                if (item.Count < link.Count) return;
                if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;


                special += item.Info.Stats[Stat.MaxRefineChance];
                fragmentRate += item.Info.Stats[Stat.FragmentRate];
            }


            int maxChance = 80 + special;
            int statValue = 0;
            bool sucess = false;

            switch (p.RefineType)
            {
                case RefineType.DC:
                    foreach (UserItemStat stat in weapon.AddedStats)
                    {
                        if (stat.Stat != Stat.MaxDC || stat.StatSource != StatSource.Refine) continue;

                        statValue = stat.Amount;
                        break;
                    }

                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.NPCMasterRefineChance, Math.Min(maxChance, Math.Max(80 - statValue * 4 + fragmentCount * fragmentRate, 0))), MessageType.System);
                    break;
                case RefineType.SpellPower:
                    foreach (UserItemStat stat in weapon.AddedStats)
                    {
                        if (stat.StatSource != StatSource.Refine) continue;

                        if (stat.Stat != Stat.MaxMC && stat.Stat != Stat.MaxSC) continue;

                        statValue = Math.Max(statValue, stat.Amount);
                    }
                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.NPCMasterRefineChance, Math.Min(maxChance, Math.Max(80 - statValue * 4 + fragmentCount * fragmentRate, 0))), MessageType.System);
                    break;
                case RefineType.Fire:
                case RefineType.Ice:
                case RefineType.Lightning:
                case RefineType.Wind:
                case RefineType.Holy:
                case RefineType.Dark:
                case RefineType.Phantom:
                    statValue = weapon.MergeRefineElements(out Stat element);
                    weapon.StatsChanged();

                    sucess = SEnvir.Random.Next(100) >= Math.Min(maxChance, 80 - statValue * 4 + fragmentCount * 2);
                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.NPCMasterRefineChance, Math.Min(maxChance, Math.Max(80 - statValue * 4 + fragmentCount * fragmentRate, 0))), MessageType.System);
                    break;
            }

            Gold.Amount -= Globals.MasterRefineEvaluateCost;
            GoldChanged();
        }
        public void NPCWeaponCraft(C.NPCWeaponCraft p)
        {
            S.NPCWeaponCraft result = new S.NPCWeaponCraft
            {
                Template = p.Template,
                Yellow = p.Yellow,
                Blue = p.Blue,
                Red = p.Red,
                Purple = p.Purple,
                Green = p.Green,
                Grey = p.Grey,
            };
            Enqueue(result);


            int statCount = 0;

            bool isTemplate = false;

            #region Tempate Check

            if (p.Template == null) return;

            if (p.Template.GridType != GridType.Inventory) return;

            if (p.Template.Slot < 0 || p.Template.Slot >= Inventory.Length) return;

            if (p.Template.Count != 1) return;

            if (Inventory[p.Template.Slot] == null) return;

            if (Inventory[p.Template.Slot].Info.ItemEffect == ItemEffect.WeaponTemplate)
            {
                isTemplate = true;
            }
            else if (Inventory[p.Template.Slot].Info.ItemType != ItemType.Weapon || Inventory[p.Template.Slot].Info.ItemEffect == ItemEffect.SpiritBlade) return;

            #endregion

            long cost = Globals.CraftWeaponPercentCost;

            if (!isTemplate)
            {
                switch (Inventory[p.Template.Slot].Info.Rarity)
                {
                    case Rarity.Common:
                        cost = Globals.CommonCraftWeaponPercentCost;
                        break;
                    case Rarity.Superior:
                        cost = Globals.SuperiorCraftWeaponPercentCost;
                        break;
                    case Rarity.Elite:
                        cost = Globals.EliteCraftWeaponPercentCost;
                        break;
                }
            }


            #region Yellow Check

            if (p.Yellow != null)
            {
                if (p.Yellow.GridType != GridType.Inventory) return;

                if (p.Yellow.Slot < 0 || p.Yellow.Slot >= Inventory.Length) return;

                if (p.Yellow.Count != 1) return;

                if (Inventory[p.Yellow.Slot] == null || Inventory[p.Yellow.Slot].Info.ItemEffect != ItemEffect.YellowSlot) return;

                statCount += Inventory[p.Yellow.Slot].Info.Shape;
            }

            #endregion

            #region Blue Check

            if (p.Blue != null)
            {
                if (p.Blue.GridType != GridType.Inventory) return;

                if (p.Blue.Slot < 0 || p.Blue.Slot >= Inventory.Length) return;

                if (p.Blue.Count != 1) return;

                if (Inventory[p.Blue.Slot] == null || Inventory[p.Blue.Slot].Info.ItemEffect != ItemEffect.BlueSlot) return;

                statCount += Inventory[p.Blue.Slot].Info.Shape;
            }

            #endregion

            #region Red Check

            if (p.Red != null)
            {
                if (p.Red.GridType != GridType.Inventory) return;

                if (p.Red.Slot < 0 || p.Red.Slot >= Inventory.Length) return;

                if (p.Red.Count != 1) return;

                if (Inventory[p.Red.Slot] == null || Inventory[p.Red.Slot].Info.ItemEffect != ItemEffect.RedSlot) return;

                statCount += Inventory[p.Red.Slot].Info.Shape;
            }

            #endregion

            #region Purple Check

            if (p.Purple != null)
            {
                if (p.Purple.GridType != GridType.Inventory) return;

                if (p.Purple.Slot < 0 || p.Purple.Slot >= Inventory.Length) return;

                if (p.Purple.Count != 1) return;

                if (Inventory[p.Purple.Slot] == null || Inventory[p.Purple.Slot].Info.ItemEffect != ItemEffect.PurpleSlot) return;

                statCount += Inventory[p.Purple.Slot].Info.Shape;
            }

            #endregion

            #region Green Check

            if (p.Green != null)
            {
                if (p.Green.GridType != GridType.Inventory) return;

                if (p.Green.Slot < 0 || p.Green.Slot >= Inventory.Length) return;

                if (p.Green.Count != 1) return;

                if (Inventory[p.Green.Slot] == null || Inventory[p.Green.Slot].Info.ItemEffect != ItemEffect.GreenSlot) return;

                statCount += Inventory[p.Green.Slot].Info.Shape;
            }

            #endregion

            #region Grey Check

            if (p.Grey != null)
            {
                if (p.Grey.GridType != GridType.Inventory) return;

                if (p.Grey.Slot < 0 || p.Grey.Slot >= Inventory.Length) return;

                if (p.Grey.Count != 1) return;

                if (Inventory[p.Grey.Slot] == null || Inventory[p.Grey.Slot].Info.ItemEffect != ItemEffect.GreySlot) return;

                statCount += Inventory[p.Grey.Slot].Info.Shape;
            }

            #endregion

            ItemInfo weap = null;

            if (isTemplate)
            {

                switch (p.Class)
                {
                    case RequiredClass.Warrior:
                        weap = SEnvir.ItemInfoList.Binding.First(x => x.ItemEffect == ItemEffect.WarriorWeapon);
                        break;
                    case RequiredClass.Wizard:
                        weap = SEnvir.ItemInfoList.Binding.First(x => x.ItemEffect == ItemEffect.WizardWeapon);
                        break;
                    case RequiredClass.Taoist:
                        weap = SEnvir.ItemInfoList.Binding.First(x => x.ItemEffect == ItemEffect.TaoistWeapon);
                        break;
                    case RequiredClass.Assassin:
                        weap = SEnvir.ItemInfoList.Binding.First(x => x.ItemEffect == ItemEffect.AssassinWeapon);
                        break;
                    default:
                        return;
                }

                if (!CanGainItems(false, new ItemCheck(weap, 1, UserItemFlags.None, TimeSpan.Zero)))
                {
                    Connection.ReceiveChat("Not enough bag space available.", MessageType.System);
                    return;
                }
            }

            result.Success = true;

            UserItem item;

            #region Tempate

            if (isTemplate)
            {
                item = Inventory[p.Template.Slot];
                if (item.Count == 1)
                {
                    RemoveItem(item);
                    Inventory[p.Template.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= 1;
            }
            #endregion

            #region Yellow

            if (p.Yellow != null)
            {
                item = Inventory[p.Yellow.Slot];
                if (item.Count == 1)
                {
                    RemoveItem(item);
                    Inventory[p.Yellow.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= 1;
            }

            #endregion

            #region Blue

            if (p.Blue != null)
            {
                item = Inventory[p.Blue.Slot];
                if (item.Count == 1)
                {
                    RemoveItem(item);
                    Inventory[p.Blue.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= 1;
            }

            #endregion

            #region Red

            if (p.Red != null)
            {
                item = Inventory[p.Red.Slot];
                if (item.Count == 1)
                {
                    RemoveItem(item);
                    Inventory[p.Red.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= 1;
            }

            #endregion

            #region Purple

            if (p.Purple != null)
            {
                item = Inventory[p.Purple.Slot];
                if (item.Count == 1)
                {
                    RemoveItem(item);
                    Inventory[p.Purple.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= 1;
            }

            #endregion

            #region Green

            if (p.Green != null)
            {
                item = Inventory[p.Green.Slot];
                if (item.Count == 1)
                {
                    RemoveItem(item);
                    Inventory[p.Green.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= 1;
            }

            #endregion

            #region Grey

            if (p.Grey != null)
            {
                item = Inventory[p.Grey.Slot];
                if (item.Count == 1)
                {
                    RemoveItem(item);
                    Inventory[p.Grey.Slot] = null;
                    item.Delete();
                }
                else
                    item.Count -= 1;
            }

            #endregion

            Gold.Amount -= cost;
            GoldChanged();

            int total = 0;

            foreach (WeaponCraftStatInfo stat in SEnvir.WeaponCraftStatInfoList.Binding)
            {
                if ((stat.RequiredClass & p.Class) != p.Class) continue;

                total += stat.Weight;
            }

            if (isTemplate)
            {
                item = SEnvir.CreateFreshItem(weap);
            }
            else
            {
                item = Inventory[p.Template.Slot];

                RemoveItem(item);
                Inventory[p.Template.Slot] = null;

                item.Level = 1;
                item.Flags &= ~UserItemFlags.Refinable;

                for (int i = item.AddedStats.Count - 1; i >= 0; i--)
                {
                    UserItemStat stat = item.AddedStats[i];
                    if (stat.StatSource == StatSource.Enhancement) continue;

                    stat.Delete();
                }

                item.StatsChanged();
            }

            for (int i = 0; i < statCount; i++)
            {
                int value = SEnvir.Random.Next(total);

                foreach (WeaponCraftStatInfo stat in SEnvir.WeaponCraftStatInfoList.Binding)
                {
                    if ((stat.RequiredClass & p.Class) != p.Class) continue;

                    value -= stat.Weight;

                    if (value >= 0) continue;

                    item.AddStat(stat.Stat, SEnvir.Random.Next(stat.MinValue, stat.MaxValue + 1), StatSource.Added);
                    break;
                }
            }

            item.StatsChanged();

            GainItem(item);
        }
        #endregion
    }
}
