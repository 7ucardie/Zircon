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
        #region Quests

        public IEnumerable<UserQuest> Quests
        {
            get
            {
                return Character.Quests.Concat(Character.Account.Quests);
            }
        }

        public void QuestAccept(int index)
        {
            if (Dead || NPC == null) return;

            foreach (QuestInfo quest in NPC.NPCInfo.StartQuests)
            {
                if (quest.Index != index) continue;

                if (!QuestCanAccept(quest)) return;

                UserQuest userQuest = SEnvir.UserQuestList.CreateNewObject();

                userQuest.QuestInfo = quest;

                if (quest.QuestType == QuestType.Account)
                    userQuest.Account = Character.Account;
                else
                    userQuest.Character = Character;

                userQuest.DateTaken = SEnvir.Now;

                Enqueue(new S.QuestChanged { Quest = userQuest.ToClientInfo() });
                break;
            }
        }
        public bool QuestCanAccept(QuestInfo quest)
        {
            if (Quests.Any(x => x.QuestInfo == quest)) return false;

            foreach (QuestRequirement requirement in quest.Requirements)
            {
                switch (requirement.Requirement)
                {
                    case QuestRequirementType.MinLevel:
                        if (Level < requirement.IntParameter1) return false;
                        break;
                    case QuestRequirementType.MaxLevel:
                        if (Level > requirement.IntParameter1) return false;
                        break;
                    case QuestRequirementType.NotAccepted:
                        if (Quests.Any(x => x.QuestInfo == requirement.QuestParameter)) return false;

                        break;
                    case QuestRequirementType.HaveCompleted:
                        if (Quests.Any(x => x.QuestInfo == requirement.QuestParameter && x.Completed)) break;

                        return false;
                    case QuestRequirementType.HaveNotCompleted:
                        if (Quests.Any(x => x.QuestInfo == requirement.QuestParameter && x.Completed)) return false;

                        break;
                    case QuestRequirementType.Class:
                        switch (Class)
                        {
                            case MirClass.Warrior:
                                if ((requirement.Class & RequiredClass.Warrior) != RequiredClass.Warrior) return false;

                                break;
                            case MirClass.Wizard:
                                if ((requirement.Class & RequiredClass.Wizard) != RequiredClass.Wizard) return false;
                                break;
                            case MirClass.Taoist:
                                if ((requirement.Class & RequiredClass.Taoist) != RequiredClass.Taoist) return false;
                                break;
                            case MirClass.Assassin:
                                if ((requirement.Class & RequiredClass.Assassin) != RequiredClass.Assassin) return false;
                                break;
                        }
                        break;
                }

            }
            return true;
        }

        public void QuestComplete(C.QuestComplete p)
        {
            if (Dead) return;
            if (Dead || NPC == null) return;

            foreach (QuestInfo quest in NPC.NPCInfo.FinishQuests)
            {
                if (quest.Index != p.Index) continue;

                UserQuest userQuest = Quests.FirstOrDefault(x => x.QuestInfo == quest);

                if (userQuest == null || userQuest.Completed || !userQuest.IsComplete) return;

                List<ItemCheck> checks = new List<ItemCheck>();

                bool hasChoice = false;
                bool hasChosen = false;

                foreach (QuestReward reward in quest.Rewards)
                {
                    switch (Class)
                    {
                        case MirClass.Warrior:
                            if ((reward.Class & RequiredClass.Warrior) != RequiredClass.Warrior) continue;
                            break;
                        case MirClass.Wizard:
                            if ((reward.Class & RequiredClass.Wizard) != RequiredClass.Wizard) continue;
                            break;
                        case MirClass.Taoist:
                            if ((reward.Class & RequiredClass.Taoist) != RequiredClass.Taoist) continue;
                            break;
                        case MirClass.Assassin:
                            if ((reward.Class & RequiredClass.Assassin) != RequiredClass.Assassin) continue;
                            break;
                    }

                    if (reward.Choice)
                    {
                        hasChoice = true;
                        if (reward.Index != p.ChoiceIndex) continue;

                        hasChosen = true;
                    }

                    UserItemFlags flags = UserItemFlags.None;
                    TimeSpan duration = TimeSpan.FromSeconds(reward.Duration);

                    if (reward.Bound)
                        flags |= UserItemFlags.Bound;

                    if (duration != TimeSpan.Zero)
                        flags |= UserItemFlags.Expirable;

                    ItemCheck check = new ItemCheck(reward.Item, reward.Amount, flags, duration);

                    checks.Add(check);
                }

                if (hasChoice && !hasChosen)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.QuestSelectReward, MessageType.System);
                    return;
                }

                if (!CanGainItems(false, checks.ToArray()))
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.QuestNeedSpace, MessageType.System);
                    return;
                }

                foreach (ItemCheck check in checks)
                {
                    while (check.Count > 0)
                        GainItem(SEnvir.CreateFreshItem(check));
                }

                userQuest.Track = false;
                userQuest.Completed = true;
                userQuest.DateCompleted = SEnvir.Now;

                LogMilestone(MilestoneType.QuestComplete, 1, quest: quest);

                if (hasChosen)
                    userQuest.SelectedReward = p.ChoiceIndex;

                Enqueue(new S.QuestChanged { Quest = userQuest.ToClientInfo() });
                break;
            }
        }

        public void QuestTrack(C.QuestTrack p)
        {
            UserQuest quest = Quests.FirstOrDefault(x => x.Index == p.Index);

            if (quest == null || quest.Completed) return;

            quest.Track = p.Track;
        }

        public void QuestAbandon(C.QuestAbandon p)
        {
            UserQuest quest = Quests.FirstOrDefault(x => x.Index == p.Index);

            if (quest == null || quest.Completed) return;

            Character.Quests.Remove(quest);

            Enqueue(new S.QuestCancelled { Index = quest.Index });
        }

        #endregion

        #region Mail

        public void MailGetItem(C.MailGetItem p)
        {
            MailInfo mail = Character.Account.Mail.FirstOrDefault(x => x.Index == p.Index);

            if (mail == null)
            {
                Enqueue(new S.MailDelete { Index = p.Index, ObserverPacket = true });
                return;
            }

            UserItem item = mail.Items.FirstOrDefault(x => x.Slot == p.Slot);

            if (item == null)
            {
                Enqueue(new S.MailItemDelete { Index = p.Index, Slot = p.Slot, ObserverPacket = true });
                return;
            }

            if (!InSafeZone && !Character.Account.TempAdmin)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MailSafeZone, MessageType.System);
                return;
            }

            if (!CanGainItems(false, new ItemCheck(item, item.Count, item.Flags, item.ExpireTime)))
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MailNeedSpace, MessageType.System);
                return;
            }

            item.Mail = null;
            GainItem(item);

            Enqueue(new S.MailItemDelete { Index = p.Index, Slot = p.Slot, ObserverPacket = true });
        }
        public void MailDelete(int index)
        {
            MailInfo mail = Character.Account.Mail.FirstOrDefault(x => x.Index == index);

            if (mail == null)
            {
                Enqueue(new S.MailDelete { Index = index, ObserverPacket = true });
                return;
            }
            ;

            if (mail.Items.Count > 0)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MailHasItems, MessageType.System);
                return;
            }

            mail.Delete();

            Enqueue(new S.MailDelete { Index = index, ObserverPacket = true });
        }
        public void MailSend(C.MailSend p)
        {
            Enqueue(new S.MailSend { ObserverPacket = false });

            if (MailTime > SEnvir.Now) return;

            MailTime = SEnvir.Now.AddSeconds(10);

            S.ItemsChanged result = new S.ItemsChanged { Links = p.Links };

            Enqueue(result);

            if (!ParseLinks(p.Links, 0, 5)) return;

            if (p.Recipient == null || p.Recipient.Length > Globals.MaxCharacterNameLength)
            {
                return;
            }

            AccountInfo account = SEnvir.GetCharacter(p.Recipient)?.Account;

            if (account == null || SEnvir.IsBlocking(Character.Account, account))
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.MailNotFound, p.Recipient), MessageType.System);
                return;
            }

            if (account == Character.Account && !Character.Account.TempAdmin)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MailSelfMail, MessageType.System);
                return;
            }

            if (p.Links.Count > 0 && account.Mail.Sum(x => x.Items.Count) >= Globals.MaxMailStorage)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MailStorageFull, MessageType.System);
                return;
            }

            if (p.Gold < 0 || p.Gold > Gold.Amount)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MailMailCost, MessageType.System);
                return;
            }

            if (p.Subject == null || p.Subject.Length > 30)
            {
                return;
            }

            if (p.Message == null || p.Message.Length > 300)
            {
                return;
            }

            UserItem item;
            foreach (CellLinkInfo link in p.Links)
            {
                UserItem[] fromArray;

                switch (link.GridType)
                {
                    case GridType.Inventory:
                        if (!InSafeZone && !Character.Account.TempAdmin)
                        {
                            Connection.ReceiveChatWithObservers(con => con.Language.MailSendSafeZone, MessageType.System);
                            return;
                        }
                        fromArray = Inventory;
                        break;
                    case GridType.PartsStorage:
                        fromArray = PartsStorage;
                        break;
                    case GridType.Storage:
                        fromArray = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null) return;

                        if (!InSafeZone && !Character.Account.TempAdmin)
                        {
                            Connection.ReceiveChatWithObservers(con => con.Language.MailSendSafeZone, MessageType.System);
                            return;
                        }

                        fromArray = Companion.Inventory;
                        break;
                    default:
                        return;
                }

                if (link.Slot < 0 || link.Slot >= fromArray.Length) return;

                item = fromArray[link.Slot];

                if (item == null || link.Count > item.Count) return;
                if (((item.Flags & UserItemFlags.Bound) == UserItemFlags.Bound || !item.Info.CanTrade) && !account.IsAdmin(true) && !Character.Account.IsAdmin(true)) return;
                if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return;
                //Success
            }

            MailInfo mail = SEnvir.MailInfoList.CreateNewObject();

            mail.Account = account;
            mail.Sender = Name;
            mail.Subject = p.Subject;
            mail.Message = p.Message;

            result.Success = true;

            if (p.Gold > 0)
            {
                Gold.Amount -= p.Gold;
                GoldChanged();

                item = SEnvir.CreateFreshItem(SEnvir.GoldInfo);
                item.Count = p.Gold;
                item.Slot = mail.Items.Count;
                item.Mail = mail;
            }

            foreach (CellLinkInfo link in p.Links)
            {
                UserItem[] fromArray = null;

                switch (link.GridType)
                {
                    case GridType.Inventory:
                        fromArray = Inventory;
                        break;
                    case GridType.PartsStorage:
                        fromArray = PartsStorage;
                        break;
                    case GridType.Storage:
                        fromArray = Storage;
                        break;
                    case GridType.CompanionInventory:
                        fromArray = Companion.Inventory;
                        break;
                }

                item = fromArray[link.Slot];

                if (link.Count == item.Count)
                {
                    RemoveItem(item);
                    fromArray[link.Slot] = null;
                }
                else
                {
                    item.Count -= link.Count;

                    item = SEnvir.CreateFreshItem(item);
                    item.Count = link.Count;
                }

                item.Slot = mail.Items.Count;
                item.Mail = mail;
            }

            if (p.Links.Count > 0)
            {
                Companion?.RefreshWeight();
                RefreshWeight();
            }

            mail.HasItem = mail.Items.Count > 0;

            if (account.Connection?.Player != null)
                account.Connection.Enqueue(new S.MailNew
                {
                    Mail = mail.ToClientInfo(),
                    ObserverPacket = false,
                });

            LogMilestone(MilestoneType.MailSend, 1);
        }

        #endregion

        #region MarketPlace

        public void MarketPlaceConsign(C.MarketPlaceConsign p)
        {
            S.ItemChanged result = new S.ItemChanged
            {
                Link = p.Link,
            };
            Enqueue(result);

            if (!ParseLinks(p.Link)) return;

            if (string.IsNullOrEmpty(p.Message) || p.Message.Length > 150) return;

            UserItem[] array;
            switch (p.Link.GridType)
            {
                case GridType.Inventory:
                    array = Inventory;
                    if (!InSafeZone && !Character.Account.TempAdmin)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.ConsignSafeZone, MessageType.System);
                        return;
                    }
                    break;
                case GridType.PartsStorage:
                    array = PartsStorage;
                    break;
                case GridType.Storage:
                    array = Storage;
                    break;
                case GridType.CompanionInventory:
                    if (Companion == null) return;

                    array = Companion.Inventory;
                    if (!InSafeZone && !Character.Account.TempAdmin)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.ConsignSafeZone, MessageType.System);
                        return;
                    }
                    break;
                default:
                    return;
            }

            if (p.Link.Slot < 0 || p.Link.Slot >= array.Length) return;
            UserItem item = array[p.Link.Slot];

            if (item == null || p.Link.Count > item.Count) return; //trying to sell more than owned.

            if ((item.Flags & UserItemFlags.Bound) == UserItemFlags.Bound) return;
            if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return;
            if ((item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) return;

            if (p.Price <= 0) return; // Buy Out Less than 1

            int cost = 0;//(int) Math.Min(int.MaxValue, p.Price*Globals.MarketPlaceTax*p.Link.Count + Globals.MarketPlaceFee);

            if (Character.Account.Auctions.Count >= Character.Account.HighestLevel() * 3 + Character.Account.StorageSize - Globals.StorageSize)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.ConsignLimit, MessageType.System);
                return;
            }

            if (p.GuildFunds)
            {
                if (Character.Account.GuildMember == null)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.ConsignGuildFundsGuild, MessageType.System);
                    return;
                }
                if ((Character.Account.GuildMember.Permission & GuildPermission.FundsMarket) != GuildPermission.FundsMarket)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.ConsignGuildFundsPermission, MessageType.System);
                    return;
                }

                if (cost > Character.Account.GuildMember.Guild.GuildFunds)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.ConsignGuildFundsCost, MessageType.System);
                    return;
                }

                Character.Account.GuildMember.Guild.GuildFunds -= cost;
                Character.Account.GuildMember.Guild.DailyGrowth -= cost;

                foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                {
                    member.Account.Connection?.Player?.Enqueue(new S.GuildFundsChanged { Change = -cost, ObserverPacket = false });
                    member.Account.Connection?.ReceiveChat(string.Format(Connection.Language.ConsignGuildFundsUsed, Name, cost, item.Info.ItemName, result.Link.Count, p.Price), MessageType.System);
                }
            }
            else
            {
                if (cost > Gold.Amount)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.ConsignCost, MessageType.System);
                    return;
                }

                Gold.Amount -= cost;
                GoldChanged();
            }

            UserItem auctionItem;

            if (p.Link.Count == item.Count)
            {
                auctionItem = item;
                RemoveItem(item);
                array[p.Link.Slot] = null;

                result.Link.Count = 0;
            }
            else
            {
                auctionItem = SEnvir.CreateFreshItem(item);
                auctionItem.Count = p.Link.Count;
                item.Count -= p.Link.Count;

                result.Link.Count = item.Count;
            }

            RefreshWeight();
            Companion?.RefreshWeight();

            AuctionInfo auction = SEnvir.AuctionInfoList.CreateNewObject();

            auction.Account = Character.Account;

            auction.Price = p.Price;

            auction.Item = auctionItem;
            auction.Character = Character;
            auction.Message = p.Message;

            result.Success = true;

            LogMilestone(MilestoneType.MarketConsign, auctionItem.Count, item: auctionItem.Info);

            Enqueue(new S.MarketPlaceConsign { Consignments = new List<ClientMarketPlaceInfo> { auction.ToClientInfo(Character.Account) }, ObserverPacket = false });
            Connection.ReceiveChatWithObservers(con => con.Language.ConsignComplete, MessageType.System);
        }
        public void MarketPlaceCancelConsign(C.MarketPlaceCancelConsign p)
        {
            if (p.Count <= 0) return;

            AuctionInfo info = Character.Account.Auctions?.FirstOrDefault(x => x.Index == p.Index);

            if (info == null) return;

            if (info.Item == null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.ConsignAlreadySold, MessageType.System);
                return;
            }

            if (info.Item.Count < p.Count)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.ConsignNotEnough, MessageType.System);
                return;
            }

            UserItem item = info.Item;

            if (info.Item.Count > p.Count)
            {
                info.Item.Count -= p.Count;

                item = SEnvir.CreateFreshItem(info.Item);
                item.Count = p.Count;
            }
            else
                info.Item = null;

            if (!InSafeZone || !CanGainItems(false, new ItemCheck(item, item.Count, item.Flags, item.ExpireTime)))
            {
                MailInfo mail = SEnvir.MailInfoList.CreateNewObject();

                mail.Account = Character.Account;
                mail.Subject = "Listing Cancelled";
                mail.Message = string.Format("You cancelled your sale of '{0}{1}' and was not able to collect the item.", item.Info.ItemName, item.Count == 1 ? "" : "x" + item.Count);
                mail.Sender = "Market Place";
                item.Mail = mail;
                item.Slot = 0;
                mail.HasItem = true;

                Enqueue(new S.MailNew
                {
                    Mail = mail.ToClientInfo(),
                    ObserverPacket = false,
                });
            }
            else
            {
                GainItem(item);
            }


            if (info.Item == null)
                info.Delete();

            Enqueue(new S.MarketPlaceConsignChanged { Index = info.Index, Count = info.Item?.Count ?? 0, ObserverPacket = false, });
        }

        public void MarketPlaceBuy(C.MarketPlaceBuy p)
        {
            if (p.Count <= 0) return;

            S.MarketPlaceBuy result = new S.MarketPlaceBuy
            {
                ObserverPacket = false,
            };

            Enqueue(result);

            AuctionInfo info = Connection.MPSearchResults.FirstOrDefault(x => x.Index == p.Index);

            if (info == null) return;

            if (info.Item == null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.ConsignAlreadySold, MessageType.System);
                return;
            }

            if (info.Account == Character.Account && !Character.Account.TempAdmin)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.ConsignBuyOwnItem, MessageType.System);
                return;
            }

            if (info.Item.Count < p.Count)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.ConsignNotEnough, MessageType.System);
                return;
            }

            long cost = p.Count;

            cost *= info.Price;

            if (p.GuildFunds)
            {
                if (Character.Account.GuildMember == null)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.ConsignBuyGuildFundsGuild, MessageType.System);
                    return;
                }
                if ((Character.Account.GuildMember.Permission & GuildPermission.FundsMarket) != GuildPermission.FundsMarket)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.ConsignBuyGuildFundsPermission, MessageType.System);
                    return;
                }

                if (cost > Character.Account.GuildMember.Guild.GuildFunds)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.ConsignBuyGuildFundsCost, MessageType.System);
                    return;
                }

                Character.Account.GuildMember.Guild.GuildFunds -= cost;
                Character.Account.GuildMember.Guild.DailyGrowth -= cost;

                foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                {
                    member.Account.Connection?.Player?.Enqueue(new S.GuildFundsChanged { Change = -cost, ObserverPacket = false });
                    member.Account.Connection?.ReceiveChat(string.Format(member.Account.Connection.Language.ConsignBuyGuildFundsUsed, Name, cost, info.Item.Info.ItemName, p.Count, info.Price), MessageType.System);
                }
            }
            else
            {
                if (cost > Gold.Amount)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.ConsignBuyCost, MessageType.System);
                    return;
                }

                Gold.Amount -= cost;
                GoldChanged();
            }


            UserItem item = info.Item;

            if (info.Item.Count > p.Count)
            {
                info.Item.Count -= p.Count;

                item = SEnvir.CreateFreshItem(info.Item);
                item.Count = p.Count;
            }
            else
                info.Item = null;

            MailInfo mail = SEnvir.MailInfoList.CreateNewObject();

            mail.Account = info.Account;

            long tax = (long)(cost * Globals.MarketPlaceTax);

            mail.Subject = "Listing Sale";
            mail.Sender = "Market Place";

            ItemInfo itemInfo = item.Info;
            int partIndex = item.Stats[Stat.ItemIndex];

            string itemName;

            if (item.Info.ItemEffect == ItemEffect.ItemPart)
                itemName = SEnvir.ItemInfoList.Binding.First(x => x.Index == partIndex).ItemName + " - [Part]";
            else
                itemName = item.Info.ItemName;

            mail.Message = $"You have sold an item\n\n" +
                           string.Format("Buyer: {0}\n", Name) +
                           string.Format("Item: {0} x{1}\n", itemName, p.Count) +
                           string.Format("Price: {0:#,##0} each\n", info.Price) +
                           string.Format("Sub Total: {0:#,##0}\n\n", cost) +
                           string.Format("Tax: {0:#,##0} ({1:p0})\n\n", tax, Globals.MarketPlaceTax) +
                           string.Format("Total: {0:#,##0}", cost - tax);

            UserItem gold = SEnvir.CreateFreshItem(SEnvir.GoldInfo);
            gold.Count = (long)(cost - tax);

            gold.Mail = mail;
            gold.Slot = 0;
            mail.HasItem = true;

            if (info.Account.Connection?.Player != null)
                info.Account.Connection.Enqueue(new S.MailNew
                {
                    Mail = mail.ToClientInfo(),
                    ObserverPacket = false,
                });


            item.Flags |= UserItemFlags.Locked;

            if (!InSafeZone || !CanGainItems(false, new ItemCheck(item, item.Count, item.Flags, item.ExpireTime)))
            {
                mail = SEnvir.MailInfoList.CreateNewObject();

                mail.Account = Character.Account;

                mail.Subject = "Item Purchase";
                mail.Sender = "Market Place";
                mail.Message = string.Format("You purchased '{0}{1}' and was not able to collect the item.", itemName, item.Count == 1 ? "" : "x" + item.Count);

                item.Mail = mail;
                item.Slot = 0;
                mail.HasItem = true;

                Enqueue(new S.MailNew
                {
                    Mail = mail.ToClientInfo(),
                    ObserverPacket = false,
                });
            }
            else
            {
                GainItem(item);
            }

            result.Index = info.Index;
            result.Count = info.Item?.Count ?? 0;
            result.Success = true;

            LogMilestone(MilestoneType.MarketPurchase, result.Count, item: info.Item.Info);
            SEnvir.LogMilestone(info.Character, MilestoneType.MarketSell, result.Count, item: info.Item.Info);

            AuctionHistoryInfo history = SEnvir.AuctionHistoryInfoList.Binding.FirstOrDefault(x => x.Info == itemInfo.Index && x.PartIndex == partIndex) ?? SEnvir.AuctionHistoryInfoList.CreateNewObject();

            history.Info = itemInfo.Index;
            history.PartIndex = partIndex;
            history.SaleCount += p.Count;
            history.LastPrice = info.Price;

            for (int i = history.Average.Length - 2; i >= 0; i--)
                history.Average[i + 1] = history.Average[i];

            history.Average[0] = info.Price; //Only care about the price per transaction


            if (info.Account.Connection?.Player != null)
                info.Account.Connection.Enqueue(new S.MarketPlaceConsignChanged { Index = info.Index, Count = info.Item?.Count ?? 0, ObserverPacket = false, });

            if (info.Item == null)
                info.Delete();
        }

        public void MarketPlaceStoreBuy(C.MarketPlaceStoreBuy p)
        {
            if (p.Count <= 0) return;

            S.MarketPlaceStoreBuy result = new S.MarketPlaceStoreBuy
            {
                ObserverPacket = false,
            };

            Enqueue(result);

            StoreInfo info = SEnvir.StoreInfoList.Binding.FirstOrDefault(x => x.Index == p.Index);

            if (info?.Item == null) return;

            if (!info.Available)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.StoreNotAvailable, MessageType.System);
                return;
            }

            p.Count = Math.Min(p.Count, info.Item.StackSize);

            long cost = p.Count;

            int price = p.UseHuntGold ? (info.HuntGoldPrice == 0 ? info.Price : info.HuntGoldPrice) : info.Price;

            cost *= price;


            UserItemFlags flags = UserItemFlags.Worthless;
            TimeSpan duration = TimeSpan.FromSeconds(info.Duration);

            if (p.UseHuntGold || Character.Account.HighestLevel() < 40)
                flags |= UserItemFlags.Bound;

            if (duration != TimeSpan.Zero)
                flags |= UserItemFlags.Expirable;

            flags |= UserItemFlags.Locked;

            ItemCheck check = new ItemCheck(info.Item, p.Count, flags, duration);

            if (!CanGainItems(false, check))
            {
                Connection.ReceiveChatWithObservers(con => con.Language.StoreNeedSpace, MessageType.System);
                return;
            }

            if (!Config.TestServer)
            {
                if (p.UseHuntGold)
                {
                    if (cost > HuntGold.Amount)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.StoreCost, MessageType.System);
                        return;
                    }

                    HuntGold.Amount -= (int)cost;

                    HuntGoldChanged();
                }
                else
                {
                    if (cost > GameGold.Amount)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.StoreCost, MessageType.System);
                        return;
                    }

                    GameGold.Amount -= (int)cost;

                    GameGoldChanged();
                }
            }


            UserItem item = SEnvir.CreateFreshItem(check);

            GainItem(item);



            GameStoreSale sale = SEnvir.GameStoreSaleList.CreateNewObject();

            sale.Item = info.Item;
            sale.Account = Character.Account;
            sale.Count = p.Count;
            sale.Price = price;
            sale.HuntGold = p.UseHuntGold;
        }

        public void MarketPlaceCancelSuperior()
        {
            for (int i = SEnvir.AuctionInfoList.Count - 1; i >= 0; i--)
            {
                AuctionInfo info = SEnvir.AuctionInfoList[i];

                if (info.Item == null) continue;

                if ((info.Item.Info.ItemType != ItemType.ItemPart) &&
                   (info.Item.Info.RequiredType != RequiredType.Level || info.Item.Info.RequiredAmount < 40 || info.Item.Info.RequiredAmount > 56)) continue;

                UserItem item = info.Item;

                info.Item = null;

                MailInfo mail = SEnvir.MailInfoList.CreateNewObject();

                mail.Account = info.Account;
                mail.Subject = "Listing Cancelled";
                mail.Message = "Your listing was cancelled because of Item change(s).";
                mail.Sender = "System";
                item.Mail = mail;
                item.Slot = 0;
                mail.HasItem = true;

                info.Account.Connection?.Player?.Enqueue(new S.MailNew
                {
                    Mail = mail.ToClientInfo(),
                    ObserverPacket = false,
                });

                if (info.Item == null)
                    info.Delete();
            }
        }

        #endregion
    }
}
