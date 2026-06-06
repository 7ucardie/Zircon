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
        #region Marriage

        public void MarriageRequest()
        {
            if (Character.Partner != null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryAlreadyMarried, MessageType.System);
                return;
            }

            if (Level < 22)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryNeedLevel, MessageType.System);
                return;
            }

            if (Gold.Amount < 500000)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryNeedGold, MessageType.System);
                return;
            }

            Cell cell = CurrentMap.GetCell(Functions.Move(CurrentLocation, Direction));

            if (cell?.Objects == null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryNotFacing, MessageType.System);
                return;
            }

            PlayerObject player = null;
            foreach (MapObject ob in cell.Objects)
            {
                if (ob.Race != ObjectType.Player) continue;
                player = (PlayerObject)ob;
                break;
            }

            if (player == null || player.Direction != Functions.ShiftDirection(Direction, 4))
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryNotFacing, MessageType.System);
                return;
            }

            if (player.Character.Partner != null)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.MarryTargetAlreadyMarried, player.Character.CharacterName), MessageType.System);
                return;
            }

            if (player.MarriageInvitation != null)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.MarryTargetHasProposal, player.Character.CharacterName), MessageType.System);
                return;
            }

            if (player.Level < 22)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.MarryTargetNeedLevel, player.Character.CharacterName), MessageType.System);
                player.Connection.ReceiveChatWithObservers(con => con.Language.MarryNeedLevel, MessageType.System);
                return;
            }

            if (player.Gold.Amount < 500000)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.MarryTargetNeedGold, player.Character.CharacterName), MessageType.System);
                player.Connection.ReceiveChatWithObservers(con => con.Language.MarryNeedGold, MessageType.System);
                return;
            }
            if (player.Dead || Dead)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryDead, MessageType.System);
                player.Connection.ReceiveChatWithObservers(con => con.Language.MarryDead, MessageType.System);
                return;
            }

            player.MarriageInvitation = this;
            player.Enqueue(new S.MarriageInvite { Name = Name });
        }
        public void MarriageJoin()
        {
            if (MarriageInvitation != null && MarriageInvitation.Node == null) MarriageInvitation = null;

            if (MarriageInvitation == null || Character.Partner != null || MarriageInvitation.Character.Partner != null) return;

            const int cost = 500000;

            if (Gold.Amount < cost)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryNeedGold, MessageType.System);
                MarriageInvitation.Connection.ReceiveChatWithObservers(con => string.Format(con.Language.MarryTargetNeedGold, Character.CharacterName), MessageType.System);
                return;
            }

            if (MarriageInvitation.Gold.Amount < cost)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.MarryTargetNeedGold, MarriageInvitation.Character.CharacterName), MessageType.System);
                MarriageInvitation.Connection.ReceiveChatWithObservers(con => con.Language.MarryNeedGold, MessageType.System);
                return;
            }

            Character.Partner = MarriageInvitation.Character;

            Connection.ReceiveChatWithObservers(con => string.Format(con.Language.MarryComplete, MarriageInvitation.Character.CharacterName), MessageType.System);
            MarriageInvitation.Connection.ReceiveChatWithObservers(con => string.Format(con.Language.MarryComplete, Character.CharacterName), MessageType.System);

            Gold.Amount -= cost;
            MarriageInvitation.Gold.Amount -= cost;

            LogMilestone(MilestoneType.Marry, 1);
            MarriageInvitation.LogMilestone(MilestoneType.Marry, 1);

            GoldChanged();
            MarriageInvitation.GoldChanged();

            AddAllObjects();

            Enqueue(GetMarriageInfo());
            MarriageInvitation.Enqueue(MarriageInvitation.GetMarriageInfo());
        }
        public void MarriageLeave()
        {
            if (Character.Partner == null) return;

            CharacterInfo partner = Character.Partner;

            Character.Partner = null;

            MarriageRemoveRing();
            Connection.ReceiveChatWithObservers(con => string.Format(con.Language.MarryDivorce, partner.CharacterName), MessageType.System);

            Enqueue(GetMarriageInfo());

            LogMilestone(MilestoneType.Divorce, 1);

            if (partner.Player != null)
            {
                partner.Player.MarriageRemoveRing();
                partner.Player.Connection.ReceiveChatWithObservers(con => string.Format(con.Language.MarryDivorce, Character.CharacterName), MessageType.System);
                partner.Player.Enqueue(partner.Player.GetMarriageInfo());
                partner.Player.LogMilestone(MilestoneType.Divorce, 1);
            }
            else
            {
                foreach (UserItem item in partner.Items)
                {
                    if (item.Slot != Globals.EquipmentOffSet + (int)EquipmentSlot.RingL) continue;

                    item.Flags &= ~UserItemFlags.Marriage;
                }
            }
        }
        public void MarriageMakeRing(int index)
        {
            if (Character.Partner == null) return; // Not Married

            if (Equipment[(int)EquipmentSlot.RingL] != null && (Equipment[(int)EquipmentSlot.RingL].Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return;

            if (index < 0 || index >= Inventory.Length) return;

            UserItem ring = Inventory[index];

            if (ring == null || ring.Info.ItemType != ItemType.Ring) return;

            if (!(CanWearItem(ring, EquipmentSlot.RingL) || CanWearItem(ring, EquipmentSlot.RingR))) return;

            ring.Flags |= UserItemFlags.Marriage;

            Inventory[index] = Equipment[(int)EquipmentSlot.RingL];

            if (Inventory[index] != null)
                Inventory[index].Slot = index;

            Equipment[(int)EquipmentSlot.RingL] = ring;
            ring.Slot = Globals.EquipmentOffSet + (int)EquipmentSlot.RingL;

            Enqueue(new S.ItemMove { FromGrid = GridType.Inventory, FromSlot = index, ToGrid = GridType.Equipment, ToSlot = (int)EquipmentSlot.RingL, Success = true });
            Enqueue(new S.MarriageMakeRing());
            RefreshStats();
            Enqueue(new S.NPCClose());
        }
        public void MarriageTeleport()
        {
            if (Character.Partner == null)
            {
                Connection.ReceiveChat(Connection.Language.NotMarried, MessageType.System);
                return;
            }

            if (Equipment[(int)EquipmentSlot.RingL] == null || (Equipment[(int)EquipmentSlot.RingL].Flags & UserItemFlags.Marriage) != UserItemFlags.Marriage)
            {
                Connection.ReceiveChat(Connection.Language.MarryNotRing, MessageType.System);
                return;
            }

            if (Dead)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryTeleportDead, MessageType.System);
                return;
            }

            if (Stats[Stat.PKPoint] >= Config.RedPoint)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryTeleportPK, MessageType.System);
                return;
            }

            if (SEnvir.Now < Character.MarriageTeleportTime)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.MarryTeleportDelay, Functions.ToString(Character.MarriageTeleportTime - SEnvir.Now, true)), MessageType.System);
                return;
            }

            if (Character.Partner.Player?.Node == null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryTeleportOffline, MessageType.System);
                return;
            }
            if (Character.Partner.Player.Dead)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryTeleportPartnerDead, MessageType.System);
                return;
            }

            if (!Character.Partner.Player.CurrentMap.Info.CanMarriageRecall)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryTeleportMap, MessageType.System);
                return;
            }

            if (Character.Partner.Player.CurrentMap.Instance != null && !Character.Partner.Player.CurrentMap.Instance.AllowTeleport)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryTeleportMap, MessageType.System);
                return;
            }

            if (CurrentMap.Instance != null && !CurrentMap.Instance.AllowTeleport)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryTeleportMapEscape, MessageType.System);
                return;
            }

            if (!CurrentMap.Info.AllowTT)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.MarryTeleportMapEscape, MessageType.System);
                return;
            }

            if (Teleport(Character.Partner.Player.CurrentMap, Character.Partner.Player.CurrentMap.GetRandomLocation(Character.Partner.Player.CurrentLocation, 10)))
                Character.MarriageTeleportTime = SEnvir.Now.AddSeconds(120);
        }

        public void MarriageRemoveRing()
        {
            if (Equipment[(int)EquipmentSlot.RingL] == null || (Equipment[(int)EquipmentSlot.RingL].Flags & UserItemFlags.Marriage) != UserItemFlags.Marriage) return;

            Equipment[(int)EquipmentSlot.RingL].Flags &= ~UserItemFlags.Marriage;
            Enqueue(new S.MarriageRemoveRing());
        }
        public S.MarriageInfo GetMarriageInfo()
        {
            return new S.MarriageInfo
            {
                Partner = new ClientPlayerInfo { Name = Character.Partner?.CharacterName, ObjectID = Character.Partner?.Player != null ? Character.Partner.Player.ObjectID : 0 }
            };
        }
        #endregion

        #region Companions

        public void CompanionUnlock(int index)
        {
            S.CompanionUnlock result = new S.CompanionUnlock();
            Enqueue(result);

            CompanionInfo info = SEnvir.CompanionInfoList.Binding.FirstOrDefault(x => x.Index == index);

            if (info == null) return;

            if (info.Available || Character.Account.CompanionUnlocks.Any(x => x.CompanionInfo == info))
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.CompanionAppearanceAlready, info.MonsterInfo.MonsterName), MessageType.System);
                return;
            }

            var unlockItem = info.UnlockItem;

            unlockItem ??= SEnvir.ItemInfoList.Binding.FirstOrDefault(x => x.ItemEffect == ItemEffect.CompanionTicket);

            if (unlockItem == null) 
            { 
                return; 
            }

            UserItem item = null;
            int slot = 0;

            for (int i = 0; i < Inventory.Length; i++)
            {
                if (Inventory[i] == null || Inventory[i].Info != unlockItem) continue;

                item = Inventory[i];
                slot = i;
                break;
            }

            if (item == null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.CompanionNeedItem, MessageType.System);
                return;
            }

            S.ItemChanged changed = new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = GridType.Inventory, Slot = slot },

                Success = true
            };
            Enqueue(changed);
            if (item.Count > 1)
            {
                item.Count--;
                changed.Link.Count = item.Count;
            }
            else
            {
                RemoveItem(item);
                Inventory[slot] = null;
                item.Delete();
            }

            RefreshWeight();

            result.Index = info.Index;

            UserCompanionUnlock unlock = SEnvir.UserCompanionUnlockList.CreateNewObject();
            unlock.Account = Character.Account;
            unlock.CompanionInfo = info;
        }

        public void CompanionAdopt(C.CompanionAdopt p)
        {
            S.CompanionAdopt result = new S.CompanionAdopt();
            Enqueue(result);

            if (Dead || NPC == null || NPCPage == null) return;

            if (NPCPage.DialogType != NPCDialogType.CompanionManage) return;

            CompanionInfo info = SEnvir.CompanionInfoList.Binding.FirstOrDefault(x => x.Index == p.Index);

            if (info == null) return;

            if (!info.Available && Character.Account.CompanionUnlocks.All(x => x.CompanionInfo != info))
            {
                Connection.ReceiveChatWithObservers(con => con.Language.CompanionAppearanceAlready, MessageType.System);
                return;
            }

            var userCurrency = GetCurrency(info.Currency);

            if (info.Price > userCurrency.Amount)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.CompanionNeedCurrency, MessageType.System);
                return;
            }

            if (!Globals.GuildNameRegex.IsMatch(p.Name))
            {
                Connection.ReceiveChatWithObservers(con => con.Language.CompanionBadName, MessageType.System);
                return;
            }

            userCurrency.Amount -= info.Price;
            CurrencyChanged(userCurrency);

            UserCompanion companion = SEnvir.UserCompanionList.CreateNewObject();

            companion.Account = Character.Account;
            companion.Info = info;
            companion.Level = 1;
            companion.Hunger = 100;
            companion.Name = p.Name;

            LogMilestone(MilestoneType.CompanionAdopt, 1);

            result.UserCompanion = companion.ToClientInfo();
        }

        public void SetFilters(C.SendCompanionFilters p)
        {
            Character.FiltersClass = String.Join(",", p.FilterClass);
            Character.FiltersRarity = String.Join(",", p.FilterRarity);
            Character.FiltersItemType = String.Join(",", p.FilterItemType);

            FiltersClass = Character.FiltersClass;
            FiltersItemType = Character.FiltersItemType;
            FiltersRarity = Character.FiltersRarity;

            Enqueue(new S.SendCompanionFilters { FilterClass = p.FilterClass, FilterRarity = p.FilterRarity, FilterItemType = p.FilterItemType });
            Connection.ReceiveChat(Connection.Language.CompanionFiltersUpdated, MessageType.System);
        }

        public void CompanionRetrieve(int index)
        {
            if (Dead || NPC == null || NPCPage == null) return;

            if (NPCPage.DialogType != NPCDialogType.CompanionManage) return;

            UserCompanion info = Character.Account.Companions.FirstOrDefault(x => x.Index == index);

            if (info == null) return;

            if (info.Character != null)
            {
                if (info.Character != Character)
                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.CompanionRetrieveFailed, info.Name, info.Character.CharacterName), MessageType.System);
                return;
            }

            info.Character = Character;

            Enqueue(new S.CompanionStore());
            Enqueue(new S.CompanionRetrieve { Index = index });

            CompanionDespawn();
            CompanionSpawn();
        }
        public void CompanionRelease(int index)
        {
            if (Dead || NPC == null || NPCPage == null) return;

            if (NPCPage.DialogType != NPCDialogType.CompanionManage) return;

            UserCompanion info = Character.Account.Companions.FirstOrDefault(x => x.Index == index);

            if (info == null) return;

            if (info.Character != null)
            {
                if (info.Character != Character)
                {
                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.CompanionReleaseFailed, info.Name, info.Character.CharacterName), MessageType.System);
                    return;
                }
            }

            if (Character.Companion == info)
            {
                Character.Companion = null;
            }

            Character.Account.Companions.Remove(info);

            Enqueue(new S.CompanionRelease { Index = index });

            CompanionDespawn();
        }

        public void CompanionStore(int index)
        {
            if (Dead || NPC == null || NPCPage == null) return;

            if (NPCPage.DialogType != NPCDialogType.CompanionManage) return;

            if (Character.Companion == null) return;

            Character.Companion = null;

            Enqueue(new S.CompanionStore());

            CompanionDespawn();
        }

        public void CompanionSpawn()
        {
            if (Companion != null) return;

            if (Character.Companion == null) return;

            // Fix incase companion was removed from account but still linked to character
            if (!Character.Account.Companions.Contains(Character.Companion))
            {
                Character.Companion = null;
                return;
            }

            Companion companion = new Companion(Character.Companion)
            {
                CompanionOwner = this,
            };

            if (companion.Spawn(CurrentMap, CurrentLocation))
            {
                Companion = companion;
                CompanionApplyBuff();
            }
        }
        public void CompanionApplyBuff()
        {
            if (Companion.UserCompanion.Hunger <= 0) return;

            Stats buffStats = new Stats();

            if (Companion.UserCompanion.Level >= 3)
                buffStats.Add(Companion.UserCompanion.Level3);

            if (Companion.UserCompanion.Level >= 5)
                buffStats.Add(Companion.UserCompanion.Level5);

            if (Companion.UserCompanion.Level >= 7)
                buffStats.Add(Companion.UserCompanion.Level7);

            if (Companion.UserCompanion.Level >= 10)
                buffStats.Add(Companion.UserCompanion.Level10);

            if (Companion.UserCompanion.Level >= 11)
                buffStats.Add(Companion.UserCompanion.Level11);

            if (Companion.UserCompanion.Level >= 13)
                buffStats.Add(Companion.UserCompanion.Level13);

            if (Companion.UserCompanion.Level >= 15)
                buffStats.Add(Companion.UserCompanion.Level15);

            BuffInfo buff = BuffAdd(BuffType.Companion, TimeSpan.MaxValue, buffStats, false, false, TimeSpan.FromMinutes(1));
            buff.TickTime = TimeSpan.FromMinutes(1); //set to Full Minute
        }
        public void CompanionDespawn()
        {
            if (Companion == null) return;

            BuffRemove(BuffType.Companion);

            Companion.CompanionOwner = null;
            Companion.Despawn();
            Companion = null;
        }
        public void CompanionRefreshBuff()
        {
            if (Companion.UserCompanion.Hunger <= 0) return;

            BuffInfo buff = Buffs.FirstOrDefault(x => x.Type == BuffType.Companion);

            if (buff == null) return;

            Stats buffStats = new Stats();

            if (Companion.UserCompanion.Level >= 3)
                buffStats.Add(Companion.UserCompanion.Level3);

            if (Companion.UserCompanion.Level >= 5)
                buffStats.Add(Companion.UserCompanion.Level5);

            if (Companion.UserCompanion.Level >= 7)
                buffStats.Add(Companion.UserCompanion.Level7);

            if (Companion.UserCompanion.Level >= 10)
                buffStats.Add(Companion.UserCompanion.Level10);

            if (Companion.UserCompanion.Level >= 11)
                buffStats.Add(Companion.UserCompanion.Level11);

            if (Companion.UserCompanion.Level >= 13)
                buffStats.Add(Companion.UserCompanion.Level13);

            if (Companion.UserCompanion.Level >= 15)
                buffStats.Add(Companion.UserCompanion.Level15);

            buff.Stats = buffStats;
            RefreshStats();

            Enqueue(new S.BuffChanged { Index = buff.Index, Stats = buffStats });
        }

        #endregion

        #region Guild

        public void GuildCreate(C.GuildCreate p)
        {
            Enqueue(new S.GuildCreate { ObserverPacket = false });

            if (Character.Account.GuildMember != null) return;

            if (string.IsNullOrWhiteSpace(p.Name)) return;
            if (p.Members < 0 || p.Members > 100) return;
            if (p.Storage < 0 || p.Storage > 500) return;

            long cost = p.Members * Globals.GuildMemberCost + p.Storage * Globals.GuildStorageCost;

            if (p.UseGold)
                cost += Globals.GuildCreationCost;
            else
            {
                bool result = false;
                for (int i = 0; i < Inventory.Length; i++)
                {
                    if (Inventory[i] == null || Inventory[i].Info.ItemEffect != ItemEffect.UmaKingHorn) continue;

                    result = true;
                    break;
                }

                if (!result)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.GuildNeedHorn, MessageType.System);
                    return;
                }
            }

            if (cost > Gold.Amount)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildNeedGold, MessageType.System);
                return;
            }


            if (!Globals.GuildNameRegex.IsMatch(p.Name))
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildBadName, MessageType.System);
                return;
            }

            GuildInfo info = SEnvir.GuildInfoList.Binding.FirstOrDefault(x => string.Compare(x.GuildName, p.Name, StringComparison.OrdinalIgnoreCase) == 0);

            if (info != null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildNameTaken, MessageType.System);
                return;
            }

            info = SEnvir.GuildInfoList.CreateNewObject();

            info.GuildName = p.Name;
            info.MemberLimit = 10 + p.Members;
            info.StorageSize = 10 + p.Storage;
            //info.GuildFunds = Globals.GuildCreationCost;
            info.GuildLevel = 1;

            GuildMemberInfo memberInfo = SEnvir.GuildMemberInfoList.CreateNewObject();

            memberInfo.Account = Character.Account;
            memberInfo.Guild = info;
            memberInfo.Rank = "Guild Leader";
            memberInfo.JoinDate = SEnvir.Now;
            memberInfo.Permission = GuildPermission.Leader;

            if (!p.UseGold)
            {
                for (int i = 0; i < Inventory.Length; i++)
                {
                    UserItem item = Inventory[i];
                    if (Inventory[i] == null || Inventory[i].Info.ItemEffect != ItemEffect.UmaKingHorn) continue;

                    if (item.Count > 1)
                    {
                        item.Count -= 1;

                        Enqueue(new S.ItemChanged { Link = new CellLinkInfo { GridType = GridType.Inventory, Slot = i, Count = item.Count }, Success = true });
                        break;
                    }

                    RemoveItem(item);
                    Inventory[i] = null;
                    item.Delete();

                    Enqueue(new S.ItemChanged { Link = new CellLinkInfo { GridType = GridType.Inventory, Slot = i }, Success = true });
                    break;
                }
            }

            LogMilestone(MilestoneType.GuildCreate, 1);

            Gold.Amount -= cost;
            GoldChanged();

            SendGuildInfo();
        }
        public void GuildEditNotice(C.GuildEditNotice p)
        {
            if (Character.Account.GuildMember == null) return;

            if ((Character.Account.GuildMember.Permission & GuildPermission.EditNotice) != GuildPermission.EditNotice)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildNoticePermission, MessageType.System);
                return;
            }

            if (p.Notice == null || p.Notice.Length > Globals.MaxGuildNoticeLength) return;

            Character.Account.GuildMember.Guild.GuildNotice = p.Notice;

            foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
            {
                member.Account.Connection?.Player?.Enqueue(new S.GuildNoticeChanged { Notice = p.Notice, ObserverPacket = false });
            }
        }
        public void GuildEditMember(C.GuildEditMember p)
        {
            if (Character.Account.GuildMember == null) return;

            if ((Character.Account.GuildMember.Permission & GuildPermission.Leader) != GuildPermission.Leader)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildEditMemberPermission, MessageType.System);
                return;
            }

            if (p.Rank == null || p.Rank.Length > Globals.MaxCharacterNameLength)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildMemberLength, MessageType.System);
                return;
            }


            if (p.Index > 0)
            {
                GuildMemberInfo info = Character.Account.GuildMember.Guild.Members.FirstOrDefault(x => x.Index == p.Index);

                if (info == null)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.GuildMemberNotFound, MessageType.System);
                    return;
                }

                if (info != Character.Account.GuildMember) //Don't Change ones own permission.
                    info.Permission = p.Permission;
                info.Rank = p.Rank;

                S.GuildUpdate update = Character.Account.GuildMember.Guild.GetUpdatePacket();

                update.Members.Add(info.ToClientInfo());

                foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                    member.Account.Connection?.Player?.Enqueue(update);

                info.Account.Connection?.Player?.Broadcast(new S.GuildChanged { ObjectID = info.Account.Connection.Player.ObjectID, GuildName = info.Guild.GuildName, GuildRank = info.Rank });
            }
            else
            {
                Character.Account.GuildMember.Guild.DefaultRank = p.Rank;
                Character.Account.GuildMember.Guild.DefaultPermission = p.Permission;

                S.GuildUpdate update = Character.Account.GuildMember.Guild.GetUpdatePacket();

                foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                    member.Account.Connection?.Player?.Enqueue(update);
            }
        }
        public void GuildKickMember(C.GuildKickMember p)
        {
            if (Character.Account.GuildMember == null) return;

            if ((Character.Account.GuildMember.Permission & GuildPermission.Leader) != GuildPermission.Leader)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildKickPermission, MessageType.System);
                return;
            }

            GuildMemberInfo info = Character.Account.GuildMember.Guild.Members.FirstOrDefault(x => x.Index == p.Index);

            if (info == null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildMemberNotFound, MessageType.System);
                return;
            }

            if (info == Character.Account.GuildMember)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildKickSelf, MessageType.System);
                return;
            }

            GuildInfo guild = info.Guild;
            PlayerObject player = info.Account.Connection?.Player;
            string memberName = info.Account.LastCharacter.CharacterName;

            info.Account.GuildTime = SEnvir.Now.AddDays(1);

            info.Guild = null;
            info.Account = null;
            info.Delete();


            if (player != null)
            {
                player.Connection.ReceiveChat(string.Format(player.Connection.Language.GuildKicked, Name), MessageType.System);
                player.Enqueue(new S.GuildInfo { ObserverPacket = false });
                player.Broadcast(new S.GuildChanged { ObjectID = player.ObjectID });
                player.RemoveAllObjects();
                player.ApplyGuildBuff();
            }

            foreach (GuildMemberInfo member in guild.Members)
            {
                if (member.Account.Connection?.Player == null) continue;

                member.Account.Connection.ReceiveChat(string.Format(member.Account.Connection.Language.GuildMemberKicked, memberName, Name), MessageType.System);
                member.Account.Connection.Player.Enqueue(new S.GuildKick { Index = info.Index, ObserverPacket = false });
                member.Account.Connection.Player.RemoveAllObjects();
                member.Account.Connection.Player.ApplyGuildBuff();
            }

        }
        public void GuildTax(C.GuildTax p)
        {
            Enqueue(new S.GuildTax { ObserverPacket = false });

            if (Character.Account.GuildMember == null) return;

            if ((Character.Account.GuildMember.Permission & GuildPermission.Leader) != GuildPermission.Leader)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildManagePermission, MessageType.System);
                return;
            }

            if (p.Tax < 0 || p.Tax > 100) return;

            Character.Account.GuildMember.Guild.GuildTax = p.Tax / 100M;

            S.GuildUpdate update = Character.Account.GuildMember.Guild.GetUpdatePacket();

            foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                member.Account.Connection?.Player?.Enqueue(update);
        }
        public void GuildIncreaseMember(C.GuildIncreaseMember p)
        {
            Enqueue(new S.GuildIncreaseMember { ObserverPacket = false });

            if (Character.Account.GuildMember == null) return;

            GuildInfo guild = Character.Account.GuildMember.Guild;

            if ((Character.Account.GuildMember.Permission & GuildPermission.Leader) != GuildPermission.Leader)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildManagePermission, MessageType.System);
                return;
            }

            if (guild.MemberLimit >= 100)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildMemberLimit, MessageType.System);
                return;
            }

            if (guild.GuildFunds < Globals.GuildMemberCost)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildMemberCost, MessageType.System);
                return;
            }

            guild.GuildFunds -= Globals.GuildMemberCost;
            guild.DailyGrowth -= Globals.GuildMemberCost;

            Character.Account.GuildMember.Guild.MemberLimit++;

            S.GuildUpdate update = Character.Account.GuildMember.Guild.GetUpdatePacket();

            foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                member.Account.Connection?.Player?.Enqueue(update);
        }
        public void GuildIncreaseStorage(C.GuildIncreaseStorage p)
        {
            Enqueue(new S.GuildIncreaseStorage { ObserverPacket = false });

            if (Character.Account.GuildMember == null) return;

            if ((Character.Account.GuildMember.Permission & GuildPermission.Leader) != GuildPermission.Leader)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildManagePermission, MessageType.System);
                return;
            }

            GuildInfo guild = Character.Account.GuildMember.Guild;
            if (guild.StorageSize >= 500)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildStorageLimit, MessageType.System);
                return;
            }

            if (guild.GuildFunds < Globals.GuildStorageCost)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildStorageCost, MessageType.System);
                return;
            }

            guild.GuildFunds -= Globals.GuildStorageCost;
            guild.DailyGrowth -= Globals.GuildStorageCost;
            Character.Account.GuildMember.Guild.StorageSize++;

            S.GuildUpdate update = Character.Account.GuildMember.Guild.GetUpdatePacket();

            foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                member.Account.Connection?.Player?.Enqueue(update);
        }
        public void GuildInviteMember(C.GuildInviteMember p)
        {
            Enqueue(new S.GuildInviteMember { ObserverPacket = false });

            if (Character.Account.GuildMember == null) return;

            if ((Character.Account.GuildMember.Permission & GuildPermission.AddMember) != GuildPermission.AddMember)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildInvitePermission, MessageType.System);
                return;
            }

            PlayerObject player = SEnvir.GetPlayerByCharacter(p.Name);

            if (player == null)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.CannotFindPlayer, p.Name), MessageType.System);
                return;
            }

            if (player.Character.Account.GuildMember != null)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildInviteGuild, player.Name), MessageType.System);
                return;
            }

            if (player.GuildInvitation != null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildInviteInvited, MessageType.System);
                return;
            }

            if (SEnvir.IsBlocking(Character.Account, player.Character.Account))
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildInviteNotAllowed, player.Name), MessageType.System);
                return;
            }
            if (!player.Character.Account.AllowGuild)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildInviteNotAllowed, player.Name), MessageType.System);
                player.Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildInvitedNotAllowed, Character.CharacterName, Character.Account.GuildMember.Guild.GuildName), MessageType.System);
                return;
            }


            if (Character.Account.GuildMember.Guild.Members.Count >= Character.Account.GuildMember.Guild.MemberLimit)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildInviteRoom, player.Name), MessageType.System);
                return;
            }

            player.GuildInvitation = this;
            player.Enqueue(new S.GuildInvite { Name = Name, GuildName = Character.Account.GuildMember.Guild.GuildName, ObserverPacket = false });
        }
        public void GuildWar(string guildName)
        {
            S.GuildWar result = new S.GuildWar { ObserverPacket = false };
            Enqueue(result);

            if (Character.Account.GuildMember == null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildNoGuild, MessageType.System);
                return;
            }

            if ((Character.Account.GuildMember.Permission & GuildPermission.StartWar) != GuildPermission.StartWar)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildWarPermission, MessageType.System);
                return;
            }

            GuildInfo guild = SEnvir.GuildInfoList.Binding.FirstOrDefault(x => string.Compare(x.GuildName, guildName, StringComparison.OrdinalIgnoreCase) == 0);

            if (guild == null)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildNotFoundGuild, guildName), MessageType.System);
                return;
            }

            if (guild == Character.Account.GuildMember.Guild)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildWarOwnGuild, MessageType.System);
                result.Success = true;
                return;
            }

            if (SEnvir.GuildWarInfoList.Binding.Any(x => (x.Guild1 == guild && x.Guild2 == Character.Account.GuildMember.Guild) ||
                                                         (x.Guild2 == guild && x.Guild1 == Character.Account.GuildMember.Guild)))
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildAlreadyWar, guild.GuildName), MessageType.System);
                return;
            }

            if (Globals.GuildWarCost > Character.Account.GuildMember.Guild.GuildFunds)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildWarCost, MessageType.System);
                return;
            }

            result.Success = true;

            Character.Account.GuildMember.Guild.GuildFunds -= Globals.GuildWarCost;
            Character.Account.GuildMember.Guild.DailyGrowth -= Globals.GuildWarCost;

            GuildWarInfo warInfo = SEnvir.GuildWarInfoList.CreateNewObject();

            warInfo.Guild1 = Character.Account.GuildMember.Guild;
            warInfo.Guild2 = guild;
            warInfo.Duration = TimeSpan.FromHours(2);

            foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
            {
                member.Account.Connection?.Player?.Enqueue(new S.GuildFundsChanged { Change = -Globals.GuildWarCost, ObserverPacket = false });
                member.Account.Connection?.Player?.Enqueue(new S.GuildWarStarted { GuildName = guild.GuildName, Duration = warInfo.Duration });
            }

            foreach (GuildMemberInfo member in guild.Members)
            {
                member.Account.Connection?.Player?.Enqueue(new S.GuildWarStarted { GuildName = Character.Account.GuildMember.Guild.GuildName, Duration = warInfo.Duration });
            }
        }
        public void GuildConquest(int index)
        {
            if (Character.Account.GuildMember == null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildNoGuild, MessageType.System);
                return;
            }

            if ((Character.Account.GuildMember.Permission & GuildPermission.Leader) != GuildPermission.Leader)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildWarPermission, MessageType.System);
                return;
            }

            if (Character.Account.GuildMember.Guild.Castle != null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildConquestCastle, MessageType.System);
                return;
            }

            if (Character.Account.GuildMember.Guild.Conquest != null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildConquestExists, MessageType.System);
                return;
            }

            CastleInfo castle = SEnvir.CastleInfoList.Binding.FirstOrDefault(x => x.Index == index);

            if (castle == null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildConquestBadCastle, MessageType.System);
                return;
            }

            if (SEnvir.ConquestWars.Count > 0)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildConquestProgress, MessageType.System);
                return;
            }

            if (castle.Item != null)
            {
                if (GetItemCount(castle.Item) == 0)
                {
                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildConquestNeedItem, castle.Item.ItemName, castle.Name), MessageType.System);
                    return;
                }

                TakeItem(castle.Item, 1);
            }

            DateTime now = SEnvir.Now;
            DateTime date = new DateTime(now.Ticks - now.TimeOfDay.Ticks + TimeSpan.TicksPerDay * 2);

            if (now.TimeOfDay.Ticks >= castle.StartTime.Ticks)
                date = date.AddTicks(TimeSpan.TicksPerDay);

            UserConquest conquest = SEnvir.UserConquestList.CreateNewObject();
            conquest.Guild = Character.Account.GuildMember.Guild;
            conquest.Castle = castle;
            conquest.WarDate = date;

            GuildInfo ownerGuild = SEnvir.GuildInfoList.Binding.FirstOrDefault(x => x.Castle == castle);

            if (ownerGuild != null)
            {
                foreach (GuildMemberInfo member in ownerGuild.Members)
                {
                    if (member.Account.Connection?.Player == null) continue; //Offline

                    member.Account.Connection.ReceiveChat(member.Account.Connection.Language.GuildConquestSuccess, MessageType.System);
                    member.Account.Connection.Enqueue(new S.GuildConquestDate { Index = castle.Index, WarTime = (date + castle.StartTime) - SEnvir.Now, ObserverPacket = false });
                }
            }

            //Send War Date to guild.
            foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
            {
                if (member.Account.Connection?.Player == null) continue; //Offline

                member.Account.Connection.ReceiveChat(string.Format(member.Account.Connection.Language.GuildConquestDate, castle.Name), MessageType.System);
                member.Account.Connection.Enqueue(new S.GuildConquestDate { Index = castle.Index, WarTime = (date + castle.StartTime) - SEnvir.Now, ObserverPacket = false });
            }
        }

        public void GuildColour(Color colour)
        {
            if (Character.Account.GuildMember == null) return;

            if ((Character.Account.GuildMember.Permission & GuildPermission.Leader) != GuildPermission.Leader)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildManagePermission, MessageType.System);
                return;
            }

            Character.Account.GuildMember.Guild.Colour = colour;

            if (Character.Account.GuildMember.Guild.Castle != null)
            {
                var map = SEnvir.GetMap(Character.Account.GuildMember.Guild.Castle.Map);
                map.RefreshFlags();
            }

            S.GuildUpdate update = Character.Account.GuildMember.Guild.GetUpdatePacket();

            foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                member.Account.Connection?.Player?.Enqueue(update);
        }

        public void GuildFlag(int flag)
        {
            if (Character.Account.GuildMember == null) return;

            if ((Character.Account.GuildMember.Permission & GuildPermission.Leader) != GuildPermission.Leader)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildManagePermission, MessageType.System);
                return;
            }

            if (flag < 0 || flag > 9) return;

            Character.Account.GuildMember.Guild.Flag = flag;

            if (Character.Account.GuildMember.Guild.Castle != null)
            {
                var map = SEnvir.GetMap(Character.Account.GuildMember.Guild.Castle.Map);
                map.RefreshFlags();
            }

            S.GuildUpdate update = Character.Account.GuildMember.Guild.GetUpdatePacket();

            foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                member.Account.Connection?.Player?.Enqueue(update);
        }

        public void GuildToggleCastleGates()
        {
            if (Character.Account.GuildMember == null) return;

            if (Character.Account.GuildMember.Guild.Castle == null)
            {
                return;
            }

            var castle = Character.Account.GuildMember.Guild.Castle;

            var castleRegion = castle.CastleRegion;
            var map = SEnvir.GetMap(castleRegion.Map);
            if (map == null) return;

            foreach (var gate in map.CastleGates)
            {
                var closeDoor = false;

                if (gate.Node == null || gate.Dead) continue;

                if (gate.Closed)
                    closeDoor = false;
                else
                    closeDoor = true;

                if (closeDoor)
                {
                    gate.CloseDoor();

                    foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                        member.Account.Connection?.ReceiveChat(con => string.Format(con.Language.GuildGateClosed, castle.Name, gate.MonsterInfo.MonsterName), MessageType.System);
                }
                else
                {
                    gate.OpenDoor();

                    foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                        member.Account.Connection?.ReceiveChat(con => string.Format(con.Language.GuildGateOpened, castle.Name, gate.MonsterInfo.MonsterName), MessageType.System);
                }
            }
        }

        public void GuildRepairCastleGates()
        {
            if (Character.Account.GuildMember == null) return;

            if (Character.Account.GuildMember.Guild.Castle == null)
            {
                return;
            }

            if ((Character.Account.GuildMember.Permission & GuildPermission.Leader) != GuildPermission.Leader)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildCastleRepairPermission, MessageType.System);
                return;
            }

            var castle = Character.Account.GuildMember.Guild.Castle;
            var castleRegion = castle.CastleRegion;
            var map = SEnvir.GetMap(castleRegion.Map);
            if (map == null) return;

            int cost = 0;

            foreach (var gate in castle.Gates)
            {
                if (gate.RepairCost <= 0) continue;

                var mob = map.CastleGates.FirstOrDefault(x => x.GateInfo == gate);

                if (mob == null || mob.Dead)
                {
                    cost += gate.RepairCost;
                }
                else
                {
                    var percent = Math.Abs(mob.CurrentHP) * 100 / mob.Stats[Stat.Health];

                    cost += (gate.RepairCost * percent / 100);
                }
            }

            if (cost > Character.Account.GuildMember.Guild.GuildFunds)
            {
                Connection.ReceiveChat(string.Format(Connection.Language.GuildRepairCastleGatesCost, cost - Character.Account.GuildMember.Guild.GuildFunds), MessageType.System);
                return;
            }

            Character.Account.GuildMember.Guild.GuildFunds -= cost;
            Character.Account.GuildMember.Guild.DailyGrowth -= cost;

            foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
            {
                member.Account.Connection?.Player?.Enqueue(new S.GuildFundsChanged { Change = -cost, ObserverPacket = false });
            }

            foreach (var gate in castle.Gates)
            {
                var mob = map.CastleGates.FirstOrDefault(x => x.GateInfo == gate);

                if (mob == null)
                {
                    mob = MonsterObject.GetMonster(gate.Monster) as CastleGate;

                    mob.Spawn(castle, gate);
                }
                else
                {
                    mob.RepairGate();
                }
            }
        }

        public void GuildRepairCastleGuards()
        {
            if (Character.Account.GuildMember == null) return;

            if (Character.Account.GuildMember.Guild.Castle == null)
            {
                return;
            }

            if ((Character.Account.GuildMember.Permission & GuildPermission.Leader) != GuildPermission.Leader)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildCastleRepairPermission, MessageType.System);
                return;
            }

            var castle = Character.Account.GuildMember.Guild.Castle;
            var castleRegion = castle.CastleRegion;
            var map = SEnvir.GetMap(castleRegion.Map);
            if (map == null) return;

            int cost = 0;

            foreach (var guard in castle.Guards)
            {
                if (guard.RepairCost <= 0) continue;

                var mob = map.CastleGuards.FirstOrDefault(x => x.GuardInfo == guard);

                if (mob == null || mob.Dead)
                {
                    cost += guard.RepairCost;
                }
                else
                {
                    var percent = Math.Abs(mob.CurrentHP) * 100 / mob.Stats[Stat.Health];

                    cost += (guard.RepairCost * percent / 100);
                }
            }

            if (cost > Character.Account.GuildMember.Guild.GuildFunds)
            {
                Connection.ReceiveChat(string.Format(Connection.Language.GuildRepairCastleGuardsCost, cost - Character.Account.GuildMember.Guild.GuildFunds), MessageType.System);
                return;
            }

            Character.Account.GuildMember.Guild.GuildFunds -= cost;
            Character.Account.GuildMember.Guild.DailyGrowth -= cost;

            foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
            {
                member.Account.Connection?.Player?.Enqueue(new S.GuildFundsChanged { Change = -cost, ObserverPacket = false });
            }

            foreach (var guard in castle.Guards)
            {
                var mob = map.CastleGuards.FirstOrDefault(x => x.GuardInfo == guard);

                if (mob == null)
                {
                    mob = MonsterObject.GetMonster(guard.Monster) as CastleGuard;

                    mob.Spawn(castle, guard);
                }
                else
                {
                    mob.RepairGuard();
                }
            }
        }

        public void GuildJoin()
        {
            if (GuildInvitation != null && GuildInvitation.Node == null) GuildInvitation = null;

            if (GuildInvitation == null) return;

            if (Character.Account.GuildMember != null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildJoinGuild, MessageType.System);
                return;
            }

            if (Character.Account.GuildTime > SEnvir.Now)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildJoinTime, Functions.ToString(Character.Account.GuildTime - SEnvir.Now, true)), MessageType.System);
                return;
            }

            if (GuildInvitation.Character.Account.GuildMember == null)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildJoinGuild, GuildInvitation.Name), MessageType.System);
                return;
            }

            if ((GuildInvitation.Character.Account.GuildMember.Permission & GuildPermission.AddMember) != GuildPermission.AddMember)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildJoinPermission, GuildInvitation.Name), MessageType.System);
                return;
            }

            if (GuildInvitation.Character.Account.GuildMember.Guild.Members.Count >= GuildInvitation.Character.Account.GuildMember.Guild.MemberLimit)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildJoinNoRoom, GuildInvitation.Name), MessageType.System);
                return;
            }

            GuildMemberInfo memberInfo = SEnvir.GuildMemberInfoList.CreateNewObject();

            memberInfo.Account = Character.Account;
            memberInfo.Guild = GuildInvitation.Character.Account.GuildMember.Guild;
            memberInfo.Rank = GuildInvitation.Character.Account.GuildMember.Guild.DefaultRank;
            memberInfo.JoinDate = SEnvir.Now;
            memberInfo.Permission = GuildInvitation.Character.Account.GuildMember.Guild.DefaultPermission;

            SendGuildInfo();
            Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildJoinWelcome, Name), MessageType.System);

            Broadcast(new S.GuildChanged { ObjectID = ObjectID, GuildName = memberInfo.Guild.GuildName, GuildRank = memberInfo.Rank });
            AddAllObjects();

            S.GuildUpdate update = memberInfo.Guild.GetUpdatePacket();

            update.Members.Add(memberInfo.ToClientInfo());

            foreach (GuildMemberInfo member in memberInfo.Guild.Members)
            {
                if (member == memberInfo || member.Account.Connection?.Player == null) continue;

                member.Account.Connection.ReceiveChat(string.Format(member.Account.Connection.Language.GuildMemberJoined, GuildInvitation.Name, Name), MessageType.System);
                member.Account.Connection.Player.Enqueue(update);

                member.Account.Connection.Player.AddAllObjects();
                member.Account.Connection.Player.ApplyGuildBuff();
            }

            LogMilestone(MilestoneType.GuildJoin, 1);

            ApplyCastleBuff();
            ApplyGuildBuff();
        }

        public void GuildLeave()
        {
            if (Character.Account.GuildMember == null) return;

            GuildMemberInfo info = Character.Account.GuildMember;

            if ((Character.Account.GuildMember.Permission & GuildPermission.Leader) == GuildPermission.Leader && info.Guild.Members.Count > 1 && info.Guild.Members.FirstOrDefault(x => x.Index != info.Index && (x.Permission & GuildPermission.Leader) == GuildPermission.Leader) == null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.GuildLeaveFailed, MessageType.System);
                return;
            }

            GuildInfo guild = info.Guild;
            int index = info.Index;

            info.Guild = null;
            info.Account = null;
            info.Delete();

            if (!guild.StarterGuild)
                Character.Account.GuildTime = SEnvir.Now.AddDays(1);

            Connection.ReceiveChatWithObservers(con => con.Language.GuildLeave, MessageType.System);
            Enqueue(new S.GuildInfo { ObserverPacket = false });

            Broadcast(new S.GuildChanged { ObjectID = ObjectID });
            RemoveAllObjects();

            foreach (GuildMemberInfo member in guild.Members)
            {
                if (member.Account.Connection?.Player == null) continue;

                member.Account.Connection.Player.Enqueue(new S.GuildKick { Index = index, ObserverPacket = false });
                member.Account.Connection.ReceiveChat(string.Format(member.Account.Connection.Language.GuildMemberLeave, Name), MessageType.System);
                member.Account.Connection.Player.RemoveAllObjects();
                member.Account.Connection.Player.ApplyGuildBuff();
            }

            ApplyCastleBuff();
            ApplyGuildBuff();
        }

        public bool AtWar(PlayerObject player)
        {
            foreach (ConquestWar conquest in SEnvir.ConquestWars)
            {
                if (conquest.Map != CurrentMap) continue;

                if (Character.Account.GuildMember == null || player.Character.Account.GuildMember == null) return true;

                return Character.Account.GuildMember.Guild != player.Character.Account.GuildMember.Guild;
            }

            if (player.Character.Account.GuildMember == null) return false;
            if (Character.Account.GuildMember == null) return false;


            foreach (GuildWarInfo warInfo in SEnvir.GuildWarInfoList.Binding)
            {
                if (warInfo.Guild1 == Character.Account.GuildMember.Guild && warInfo.Guild2 == player.Character.Account.GuildMember.Guild) return true;
                if (warInfo.Guild2 == Character.Account.GuildMember.Guild && warInfo.Guild1 == player.Character.Account.GuildMember.Guild) return true;
            }

            return false;
        }

        public void SendGuildInfo()
        {
            if (Character.Account.GuildMember == null) return;

            S.GuildInfo result = new S.GuildInfo
            {
                Guild = Character.Account.GuildMember.Guild.ToClientInfo(),
                ObserverPacket = false,
            };

            result.Guild.UserIndex = Character.Account.GuildMember.Index;

            Enqueue(result);

            foreach (GuildWarInfo warInfo in SEnvir.GuildWarInfoList.Binding)
            {
                if (warInfo.Guild1 == Character.Account.GuildMember.Guild)
                    Enqueue(new S.GuildWarStarted { GuildName = warInfo.Guild2.GuildName, Duration = warInfo.Duration });

                if (warInfo.Guild2 == Character.Account.GuildMember.Guild)
                    Enqueue(new S.GuildWarStarted { GuildName = warInfo.Guild1.GuildName, Duration = warInfo.Duration });
            }

            //Send War Date to guild.
            foreach (CastleInfo castle in SEnvir.CastleInfoList.Binding)
            {
                UserConquest conquest = SEnvir.UserConquestList.Binding.FirstOrDefault(x => x.Castle == castle && (x.Guild == Character.Account.GuildMember.Guild || x.Castle == Character.Account.GuildMember.Guild.Castle));

                TimeSpan warTime = TimeSpan.MinValue;
                if (conquest != null)
                    warTime = (conquest.WarDate + conquest.Castle.StartTime) - SEnvir.Now;

                Enqueue(new S.GuildConquestDate { Index = castle.Index, WarTime = warTime, ObserverPacket = false });
            }
        }

        public void JoinStarterGuild()
        {
            if (Character.Account.GuildMember != null) return;

            GuildMemberInfo memberInfo = SEnvir.GuildMemberInfoList.CreateNewObject();

            memberInfo.Account = Character.Account;
            memberInfo.Guild = SEnvir.StarterGuild;
            memberInfo.Rank = SEnvir.StarterGuild.DefaultRank;
            memberInfo.JoinDate = SEnvir.Now;
            memberInfo.Permission = SEnvir.StarterGuild.DefaultPermission;

            SendGuildInfo();

            Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildJoinWelcome, memberInfo.Guild.GuildName), MessageType.System);

            Broadcast(new S.GuildChanged { ObjectID = ObjectID, GuildName = memberInfo.Guild.GuildName, GuildRank = memberInfo.Rank });
            AddAllObjects();

            S.GuildUpdate update = memberInfo.Guild.GetUpdatePacket();

            update.Members.Add(memberInfo.ToClientInfo());

            foreach (GuildMemberInfo member in memberInfo.Guild.Members)
            {
                if (member == memberInfo || member.Account.Connection?.Player == null) continue;

                member.Account.Connection.ReceiveChat(string.Format(member.Account.Connection.Language.GuildMemberJoined, SEnvir.StarterGuild, Name), MessageType.System);
                member.Account.Connection.Player.Enqueue(update);

                member.Account.Connection.Player.AddAllObjects();
            }

            ApplyCastleBuff();
            ApplyGuildBuff();
        }

        #endregion

        #region Group

        public void GroupSwitch(bool allowGroup)
        {
            if (Character.Account.AllowGroup == allowGroup) return;

            if (GroupMembers != null && GroupMembers.Any(x => x.CurrentMap.Instance != null))
            {
                Connection.ReceiveChat(Connection.Language.InstanceNoAction, MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(con.Language.InstanceNoAction, MessageType.System);
                return;
            }

            Character.Account.AllowGroup = allowGroup;

            Enqueue(new S.GroupSwitch { Allow = Character.Account.AllowGroup });

            if (GroupMembers != null)
                GroupLeave();

            UpdateLFGStatus(false);
        }

        public void GroupRemove(string name)
        {
            if (GroupMembers == null)
            {
                Connection.ReceiveChat(Connection.Language.GroupNoGroup, MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(con.Language.GroupNoGroup, MessageType.System);
                return;
            }

            if (GroupMembers[0] != this)
            {
                Connection.ReceiveChat(Connection.Language.GroupNotLeader, MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(con.Language.GroupNotLeader, MessageType.System);
                return;
            }

            if (GroupMembers.Any(x => x.CurrentMap.Instance != null))
            {
                Connection.ReceiveChat(Connection.Language.InstanceNoAction, MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(con.Language.InstanceNoAction, MessageType.System);
                return;
            }

            foreach (PlayerObject member in GroupMembers)
            {
                if (string.Compare(member.Name, name, StringComparison.OrdinalIgnoreCase) != 0) continue;

                member.GroupLeave();
                return;
            }

            Connection.ReceiveChat(string.Format(Connection.Language.GroupMemberNotFound, name), MessageType.System);

            foreach (SConnection con in Connection.Observers)
                con.ReceiveChat(string.Format(con.Language.GroupMemberNotFound, name), MessageType.System);

        }

        public void GroupInvite(string name)
        {
            if (GroupMembers != null && GroupMembers[0] != this)
            {
                Connection.ReceiveChat(Connection.Language.GroupNotLeader, MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(con.Language.GroupNotLeader, MessageType.System);
                return;
            }

            PlayerObject player = SEnvir.GetPlayerByCharacter(name);

            if (player == null)
            {
                Connection.ReceiveChat(string.Format(Connection.Language.CannotFindPlayer, name), MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(string.Format(con.Language.CannotFindPlayer, name), MessageType.System);
                return;
            }

            if (player.GroupMembers != null)
            {
                Connection.ReceiveChat(string.Format(Connection.Language.GroupAlreadyGrouped, name), MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(string.Format(con.Language.GroupAlreadyGrouped, name), MessageType.System);
                return;
            }

            if (player.GroupInvitation != null)
            {
                Connection.ReceiveChat(string.Format(Connection.Language.GroupAlreadyInvited, name), MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(string.Format(con.Language.GroupAlreadyInvited, name), MessageType.System);
                return;
            }

            if (!player.Character.Account.AllowGroup || SEnvir.IsBlocking(Character.Account, player.Character.Account))
            {
                Connection.ReceiveChat(string.Format(Connection.Language.GroupInviteNotAllowed, name), MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(string.Format(con.Language.GroupInviteNotAllowed, name), MessageType.System);
                return;
            }

            if (player == this)
            {
                Connection.ReceiveChat(Connection.Language.GroupSelf, MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(con.Language.GroupSelf, MessageType.System);
                return;
            }

            if (GroupMembers != null && CurrentMap.Instance != null)
            {
                if (!CurrentMap.Instance.UserRecord.TryGetValue(player.Name, out byte instanceSequence) || CurrentMap.InstanceSequence != instanceSequence)
                {
                    Connection.ReceiveChat(Connection.Language.InstanceNoAction, MessageType.System);

                    foreach (SConnection con in Connection.Observers)
                        con.ReceiveChat(con.Language.InstanceNoAction, MessageType.System);

                    return;
                }
            }

            if (GroupInvitationRequest.Contains(player))
            {
                player.GroupInvitation = this;
                player.GroupJoin();
                player.GroupInvitation = null;
                GroupInvitationRequest.Remove(player);
                return;
            }

            player.GroupInvitation = this;
            player.Enqueue(new S.GroupInvite { Name = Name, ObserverPacket = false });
        }

        public void GroupRequest(string groupLeader)
        {
            if (GroupMembers != null || !Character.Account.AllowGroup)
            {
                return;
            }

            PlayerObject leader = SEnvir.GetPlayerByCharacter(groupLeader);

            if (leader == null)
            {
                Connection.ReceiveChat(string.Format(Connection.Language.CannotFindPlayer, groupLeader), MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(string.Format(con.Language.CannotFindPlayer, groupLeader), MessageType.System);
                return;
            }

            if (!leader.LFGSettings.Enabled)
            {
                return;
            }

            if (leader == this)
            {
                Connection.ReceiveChat(Connection.Language.GroupSelf, MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(con.Language.GroupSelf, MessageType.System);
                return;
            }

            if (!leader.Character.Account.AllowGroup || SEnvir.IsBlocking(leader.Character.Account, Character.Account))
            {
                Connection.ReceiveChat(string.Format(Connection.Language.GroupInviteNotAllowed, groupLeader), MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(string.Format(con.Language.GroupInviteNotAllowed, groupLeader), MessageType.System);
                return;
            }

            if (leader.CurrentMap.Instance != null)
            {
                if (!leader.CurrentMap.Instance.UserRecord.TryGetValue(Name, out byte instanceSequence) || leader.CurrentMap.InstanceSequence != instanceSequence)
                {
                    Connection.ReceiveChat(Connection.Language.InstanceNoAction, MessageType.System);

                    foreach (SConnection con in Connection.Observers)
                        con.ReceiveChat(con.Language.InstanceNoAction, MessageType.System);

                    return;
                }
            }

            if (GroupMembers != null && GroupMembers.Count >= leader.LFGSettings.MaxCount)
            {
                Connection.ReceiveChat(string.Format(Connection.Language.GroupMemberLimit, GroupInvitation.Name), MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(string.Format(con.Language.GroupMemberLimit, GroupInvitation.Name), MessageType.System);

                return;
            }

            if (leader.GroupInvitationRequest.Contains(this))
            {
                return;
            }

            leader.GroupInvitationRequest.Add(this);
            leader.Enqueue(new S.GroupRequest { Name = Name, Level = Level, Class = Class, ObserverPacket = false });
        }

        public void GroupJoin()
        {
            if (GroupInvitation != null && GroupInvitation.Node == null) GroupInvitation = null;

            if (GroupInvitation == null || GroupMembers != null) return;

            if (GroupInvitation.GroupMembers == null)
            {
                LogMilestone(MilestoneType.GroupCreate, 1);

                GroupInvitation.GroupSwitch(true);
                GroupInvitation.GroupMembers = new List<PlayerObject> { GroupInvitation };
                GroupInvitation.Enqueue(new S.GroupMember { ObjectID = GroupInvitation.ObjectID, Name = GroupInvitation.Name }); //<-- Setting group leader?
            }
            else if (GroupInvitation.GroupMembers[0] != GroupInvitation)
            {
                Connection.ReceiveChat(string.Format(Connection.Language.GroupAlreadyGrouped, GroupInvitation.Name), MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(string.Format(con.Language.GroupAlreadyGrouped, GroupInvitation.Name), MessageType.System);
                return;
            }
            else if (GroupInvitation.GroupMembers.Count >= Globals.GroupLimit)
            {
                Connection.ReceiveChat(string.Format(Connection.Language.GroupMemberLimit, GroupInvitation.Name), MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(string.Format(con.Language.GroupMemberLimit, GroupInvitation.Name), MessageType.System);
                return;
            }

            if (CurrentMap.Instance != null)
            {
                Connection.ReceiveChat(Connection.Language.InstanceNoAction, MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(con.Language.InstanceNoAction, MessageType.System);
                return;
            }

            GroupMembers = GroupInvitation.GroupMembers;
            GroupMembers.Add(this);

            foreach (PlayerObject ob in GroupMembers)
            {
                if (ob == this) continue;

                ob.Enqueue(new S.GroupMember { ObjectID = ObjectID, Name = Name });
                Enqueue(new S.GroupMember { ObjectID = ob.ObjectID, Name = ob.Name });

                ob.AddAllObjects();
                ob.RefreshStats();
                ob.ApplyGuildBuff();
            }

            AddAllObjects();
            ApplyGuildBuff();

            if (GroupMembers[0] != this)
            {
                LogMilestone(MilestoneType.GroupJoin, 1);
            }

            GroupInvitation.LFGSettings.NeedUpdate = true;

            //Disable own LFG as joined another group
            UpdateLFGStatus(false);

            RefreshStats();
            Enqueue(new S.GroupMember { ObjectID = ObjectID, Name = Name });
        }
        public void GroupDecline(string name)
        {
            PlayerObject player = SEnvir.GetPlayerByCharacter(name);

            if (player == null)
            {
                return;
            }

            if (!GroupInvitationRequest.Contains(player))
            {
                return;
            }

            GroupInvitationRequest.Remove(player);

            player.Connection?.ReceiveChat(player.Connection.Language.GroupRequestDeclined, MessageType.System);
        }

        public void GroupLeave(bool disableLFG = true)
        {
            Packet p = new S.GroupRemove { ObjectID = ObjectID };

            GroupMembers.Remove(this);
            List<PlayerObject> oldGroup = GroupMembers;
            GroupMembers = null;

            if (Buffs.Any(x => x.Type == BuffType.SoulResonance))
                SoulResonance.Remove(this);

            foreach (PlayerObject ob in oldGroup)
            {
                ob.Enqueue(p);
                ob.RemoveAllObjects();
                ob.RefreshStats();
                ob.ApplyGuildBuff();
            }

            if (oldGroup.Count > 0)
                oldGroup[0].LFGSettings.NeedUpdate = true;

            if (oldGroup.Count == 1) oldGroup[0].GroupLeave(false);

            GroupMembers = null;

            if (disableLFG)
            {
                //Disable your own LFG if you leave group. Mainly for if group leader has left.
                UpdateLFGStatus(false, true);
            }

            Enqueue(p);
            RemoveAllObjects();
            RefreshStats();
            ApplyGuildBuff();
        }

        #endregion

        #region Looking For Group

        #region Properties

        public class LookingForGroupSettings
        {
            public string Name { get; set; }
            public string Type { get; set; }
            public int MaxCount { get; set; }
            public bool Enabled { get; set; }
            public DateTime EnabledDateTime { get; set; }
            public bool ReceiveUpdates { get; set; }
            public bool NeedUpdate { get; set; }
        }

        public LookingForGroupSettings LFGSettings = new();

        #endregion

        public void ProcessGroup()
        {
            if (LFGSettings.Enabled && SEnvir.Now > LFGSettings.EnabledDateTime)
            {
                LFGSettings.Enabled = false;
                LFGSettings.NeedUpdate = true;

                Connection.ReceiveChat(Connection.Language.GroupLFGExpired, MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(Connection.Language.GroupLFGExpired, MessageType.System);
            }

            if (LFGSettings.NeedUpdate)
            {
                BroadcastLFG();

                LFGSettings.NeedUpdate = false;
            }
        }

        public void BroadcastLFG()
        {
            for (int i = SEnvir.Players.Count - 1; i >= 0; i--)
            {
                var player = SEnvir.Players[i];

                if (player.Connection == null || player.Node == null) continue;

                if (!player.LFGSettings.ReceiveUpdates) continue;

                player.Enqueue(new S.GroupUpdate { Group = ToClientGroup() });
            }
        }

        public void SendLFGList()
        {
            var list = new List<ClientLookingForGroup>();

            for (int i = SEnvir.Players.Count - 1; i >= 0; i--)
            {
                var player = SEnvir.Players[i];

                if (player.Connection == null || player.Node == null) continue;

                if (!player.LFGSettings.Enabled) continue;

                list.Add(player.ToClientGroup());
            }

            Enqueue(new S.GroupLFG { List = list });
        }

        public void LFGUpdate(C.GroupLFGUpdate p)
        {
            LFGSettings.Name = p.Name;
            LFGSettings.MaxCount = p.MaxCount;
            LFGSettings.Type = p.Type;

            GroupSwitch(true);

            UpdateLFGStatus(p.Enabled);

            LFGSettings.NeedUpdate = true;
        }

        public void UpdateLFGStatus(bool enabled, bool immediate = false)
        {
            bool old = LFGSettings.Enabled;

            LFGSettings.Enabled = enabled;
            if (LFGSettings.Enabled)
            {
                LFGSettings.EnabledDateTime = SEnvir.Now.AddMinutes(Globals.LookingForGroupMinutes);

                Connection.ReceiveChat(string.Format(Connection.Language.GroupLFGEnabled, Globals.LookingForGroupMinutes), MessageType.System);

                foreach (SConnection con in Connection.Observers)
                    con.ReceiveChat(string.Format(con.Language.GroupLFGEnabled, Globals.LookingForGroupMinutes), MessageType.System);
            }

            if (old != LFGSettings.Enabled)
            {
                LFGSettings.NeedUpdate = true;
            }

            if (immediate)
            {
                ProcessGroup();
            }
        }

        public ClientLookingForGroup ToClientGroup()
        {
            var members = GroupMembers?.ToList() ?? [this];

            return new ClientLookingForGroup
            {
                GroupName = LFGSettings.Name,
                LeaderName = Name,
                GroupType = LFGSettings.Type,
                MemberInfo = members.Select(x => $"{x.Name} [Level {x.Level} {x.Class}]").ToList(),
                MaxCount = LFGSettings.MaxCount,
                Enabled = LFGSettings.Enabled
            };
        }

        #endregion

        #region Trade

        public void TradeClose()
        {
            if (TradePartner == null) return;

            Enqueue(new S.TradeClose());

            if (TradePartner?.Node != null)
                TradePartner.Enqueue(new S.TradeClose());

            TradePartner.TradePartner = null;
            TradePartner.TradeItems.Clear();
            TradePartner.TradeGold = 0;
            TradePartner.TradeConfirmed = false;

            TradePartner = null;
            TradeItems.Clear();
            TradeGold = 0;
            TradeConfirmed = false;
        }

        public void TradeRequest()
        {
            if (TradePartner != null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.TradeAlreadyTrading, MessageType.System);
                return;
            }
            if (TradePartnerRequest != null)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.TradeAlreadyHaveRequest, MessageType.System);
                return;
            }

            Cell cell = CurrentMap.GetCell(Functions.Move(CurrentLocation, Direction));

            if (cell?.Objects == null) return;

            PlayerObject player = null;
            foreach (MapObject ob in cell.Objects)
            {
                if (ob.Race != ObjectType.Player) continue;
                player = (PlayerObject)ob;
                break;
            }

            if (player == null || player.Direction != Functions.ShiftDirection(Direction, 4))
            {
                Connection.ReceiveChatWithObservers(con => con.Language.TradeNeedFace, MessageType.System);
                return;
            }

            if (SEnvir.IsBlocking(Character.Account, player.Character.Account))
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.TradeTargetNotAllowed, player.Character.CharacterName), MessageType.System);
                return;
            }

            if (player.TradePartner != null)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.TradeTargetAlreadyTrading, player.Character.CharacterName), MessageType.System);
                return;
            }

            if (player.TradePartnerRequest != null)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.TradeTargetAlreadyHaveRequest, player.Character.CharacterName), MessageType.System);
                return;
            }


            if (!player.Character.Account.AllowTrade)
            {
                Connection.ReceiveChatWithObservers(con => string.Format(con.Language.TradeTargetNotAllowed, player.Character.CharacterName), MessageType.System);
                player.Connection.ReceiveChatWithObservers(con => string.Format(con.Language.TradeNotAllowed, Character.CharacterName), MessageType.System);

                return;
            }

            if (player.Dead || Dead)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.TradeTargetDead, MessageType.System);

                return;
            }


            player.TradePartnerRequest = this;
            player.Enqueue(new S.TradeRequest { Name = Name, ObserverPacket = false });

            Connection.ReceiveChatWithObservers(con => string.Format(con.Language.TradeRequested, player.Character.CharacterName), MessageType.System);
        }
        public void TradeAccept()
        {
            if (TradePartnerRequest?.Node == null || TradePartnerRequest.TradePartner != null || TradePartnerRequest.Dead ||
                Functions.Distance(CurrentLocation, TradePartnerRequest.CurrentLocation) != 1 || TradePartnerRequest.Direction != Functions.ShiftDirection(Direction, 4))
                return;

            TradePartner = TradePartnerRequest;
            TradePartnerRequest.TradePartner = this;

            TradePartner.Enqueue(new S.TradeOpen { Name = Name });
            Enqueue(new S.TradeOpen { Name = TradePartner.Name });
        }

        public void TradeAddItem(CellLinkInfo cell)
        {
            S.TradeAddItem result = new S.TradeAddItem
            {
                Cell = cell,
            };

            Enqueue(result);

            if (!ParseLinks(cell) || TradePartner == null || TradeItems.Count >= 15) return;

            UserItem[] fromArray;

            switch (cell.GridType)
            {
                case GridType.Inventory:
                    fromArray = Inventory;
                    break;
                case GridType.Equipment:
                    fromArray = Equipment;
                    break;
                case GridType.CompanionInventory:
                    if (Companion == null) return;

                    fromArray = Companion.Inventory;
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
                    break; /*
                case GridType.GuildStorage:
                    if (Character.GuildMemberInfo == null) return;

                    if (!Character.GuildMemberInfo.Permissions.HasFlag(GuildPermissions.GetItem))
                    {
                        ReceiveChat("You do no have the permissions to take from the guild storage", ChatType.System);
                        return;
                    }

                    if (!CurrentCell.IsSafeZone)
                    {
                        ReceiveChat("You cannot use guild storage unless you are in a safe zone", ChatType.Hint);
                        return;
                    }

                    fromArray = Character.GuildMemberInfo.GuildInfo.StorageArray;
                    break;*/
                default:
                    return;
            }

            if (cell.Slot < 0 || cell.Slot >= fromArray.Length) return;

            UserItem fromItem = fromArray[cell.Slot];

            if (fromItem == null || cell.Count > fromItem.Count || (!TradePartner.Character.Account.IsAdmin(true) && !Character.Account.IsAdmin(true) && ((fromItem.Flags & UserItemFlags.Bound) == UserItemFlags.Bound || !fromItem.Info.CanTrade))) return;
            if ((fromItem.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return;

            if (TradeItems.ContainsKey(fromItem)) return;

            //All is Well
            result.Success = true;
            TradeItems[fromItem] = cell;
            S.TradeItemAdded packet = new S.TradeItemAdded
            {
                Item = fromItem.ToClientInfo()
            };
            packet.Item.Count = cell.Count;
            TradePartner.Enqueue(packet);
        }
        public void TradeAddGold(long gold)
        {
            S.TradeAddGold p = new S.TradeAddGold
            {
                Gold = TradeGold,
            };
            Enqueue(p);

            if (TradePartner == null || TradeGold >= gold) return;

            if (gold <= 0 || gold > Gold.Amount) return;

            TradeGold = gold;
            p.Gold = TradeGold;

            //All is Well
            S.TradeGoldAdded packet = new S.TradeGoldAdded
            {
                Gold = TradeGold,
            };

            TradePartner.Enqueue(packet);
        }

        public void TradeConfirm()
        {
            if (TradePartner == null) return;

            TradeConfirmed = true;

            if (!TradePartner.TradeConfirmed)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.TradeWaiting, MessageType.System);
                TradePartner.Connection.ReceiveChatWithObservers(con => con.Language.TradePartnerWaiting, MessageType.System);

                return;
            }

            long gold = Gold.Amount;
            gold += TradePartner.TradeGold - TradeGold;

            if (gold < 0)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.TradeNoGold, MessageType.System);
                TradePartner.Connection.ReceiveChatWithObservers(con => con.Language.TradePartnerNoGold, MessageType.System);

                TradeClose();
                return;
            }


            gold = TradePartner.Gold.Amount;
            gold += TradeGold - TradePartner.TradeGold;

            if (gold < 0)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.TradePartnerNoGold, MessageType.System);
                TradePartner.Connection.ReceiveChatWithObservers(con => con.Language.TradeNoGold, MessageType.System);

                TradeClose();
                return;
            }

            List<ItemCheck> checks = new List<ItemCheck>();

            foreach (KeyValuePair<UserItem, CellLinkInfo> pair in TradeItems)
            {
                UserItem[] fromArray;
                switch (pair.Value.GridType)
                {
                    case GridType.Inventory:
                        fromArray = Inventory;
                        break;
                    case GridType.Equipment:
                        fromArray = Equipment;
                        break;
                    case GridType.PartsStorage:
                        fromArray = PartsStorage;
                        break;
                    case GridType.Storage:
                        fromArray = Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (Companion == null)
                        {
                            Connection.ReceiveChatWithObservers(con => con.Language.TradeFailedItemsChanged, MessageType.System);
                            TradePartner.Connection.ReceiveChatWithObservers(con => string.Format(con.Language.TradeFailedPartnerItemsChanged, Name), MessageType.System);

                            TradeClose();
                            return;
                        }

                        fromArray = Companion.Inventory;
                        break;
                    default:
                        //MAJOR LOGIC FAILURE 
                        return;
                }

                if (fromArray[pair.Value.Slot] != pair.Key || pair.Key.Count < pair.Value.Count)
                {
                    Connection.ReceiveChatWithObservers(con => con.Language.TradeFailedItemsChanged, MessageType.System);
                    TradePartner.Connection.ReceiveChatWithObservers(con => string.Format(con.Language.TradeFailedPartnerItemsChanged, Name), MessageType.System);

                    TradeClose();
                    return;
                }

                UserItem item = fromArray[pair.Value.Slot];

                if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return;

                bool handled = false;

                foreach (ItemCheck check in checks)
                {
                    if (check.Info != item.Info) continue;
                    if ((check.Flags & UserItemFlags.Expirable) == UserItemFlags.Expirable) continue;
                    if ((item.Flags & UserItemFlags.Expirable) == UserItemFlags.Expirable) continue;
                    if ((check.Flags & UserItemFlags.Bound) != (item.Flags & UserItemFlags.Bound)) continue;
                    if ((check.Flags & UserItemFlags.Worthless) != (item.Flags & UserItemFlags.Worthless)) continue;
                    if ((check.Flags & UserItemFlags.NonRefinable) != (item.Flags & UserItemFlags.NonRefinable)) continue;

                    check.Count += pair.Value.Count;
                    handled = true;
                    break;
                }

                if (handled) continue;

                checks.Add(new ItemCheck(item, pair.Value.Count, item.Flags, item.ExpireTime));
            }

            if (!TradePartner.CanGainItems(false, checks.ToArray()))
            {
                Connection.ReceiveChatWithObservers(con => con.Language.TradeWaiting, MessageType.System);
                TradePartner.Connection.ReceiveChatWithObservers(con => con.Language.TradeNotEnoughSpace, MessageType.System);

                TradePartner.TradeConfirmed = false;
                TradePartner.Enqueue(new S.TradeUnlock());
                return;
            }

            checks.Clear();

            foreach (KeyValuePair<UserItem, CellLinkInfo> pair in TradePartner.TradeItems)
            {
                UserItem[] fromArray;
                switch (pair.Value.GridType)
                {
                    case GridType.Inventory:
                        fromArray = TradePartner.Inventory;
                        break;
                    case GridType.Equipment:
                        fromArray = TradePartner.Equipment;
                        break;
                    case GridType.PartsStorage:
                        fromArray = TradePartner.PartsStorage;
                        break;
                    case GridType.Storage:
                        fromArray = TradePartner.Storage;
                        break;
                    case GridType.CompanionInventory:
                        if (TradePartner.Companion == null)
                        {
                            Connection.ReceiveChatWithObservers(con => string.Format(con.Language.TradeFailedPartnerItemsChanged, TradePartner.Name), MessageType.System);
                            TradePartner.Connection.ReceiveChatWithObservers(con => con.Language.TradeFailedItemsChanged, MessageType.System);

                            TradeClose();
                            return;
                        }


                        fromArray = TradePartner.Companion.Inventory;
                        break;
                    default:
                        //MAJOR LOGIC FAILURE 
                        return;
                }

                if (fromArray[pair.Value.Slot] != pair.Key || pair.Key.Count < pair.Value.Count)
                {
                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.TradeFailedPartnerItemsChanged, TradePartner.Name), MessageType.System);
                    TradePartner.Connection.ReceiveChatWithObservers(con => con.Language.TradeFailedItemsChanged, MessageType.System);

                    TradeClose();
                    return;
                }

                UserItem item = fromArray[pair.Value.Slot];
                if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) return;

                bool handled = false;

                foreach (ItemCheck check in checks)
                {
                    if (check.Info != item.Info) continue;
                    if ((check.Flags & UserItemFlags.Expirable) == UserItemFlags.Expirable) continue;
                    if ((item.Flags & UserItemFlags.Expirable) == UserItemFlags.Expirable) continue;
                    if ((check.Flags & UserItemFlags.Bound) != (item.Flags & UserItemFlags.Bound)) continue;
                    if ((check.Flags & UserItemFlags.Worthless) != (item.Flags & UserItemFlags.Worthless)) continue;
                    if ((check.Flags & UserItemFlags.NonRefinable) != (item.Flags & UserItemFlags.NonRefinable)) continue;

                    check.Count += pair.Value.Count;
                    handled = true;
                    break;
                }

                if (handled) continue;

                checks.Add(new ItemCheck(item, pair.Value.Count, item.Flags, item.ExpireTime));
            }

            if (!CanGainItems(false, checks.ToArray()))
            {
                Connection.ReceiveChatWithObservers(con => con.Language.TradeNotEnoughSpace, MessageType.System);
                TradePartner.Connection.ReceiveChatWithObservers(con => con.Language.TradeWaiting, MessageType.System);

                TradeConfirmed = false;
                Enqueue(new S.TradeUnlock());
                return;
            }

            Enqueue(new S.ItemsChanged { Links = TradeItems.Values.ToList(), Success = true });

            //Deal Successful, Both can accept items without issues so send away
            UserItem tempItem;

            foreach (KeyValuePair<UserItem, CellLinkInfo> pair in TradeItems)
            {
                if (pair.Key.Count > pair.Value.Count)
                {
                    pair.Key.Count -= pair.Value.Count;

                    tempItem = SEnvir.CreateFreshItem(pair.Key);
                    tempItem.Count = pair.Value.Count;
                    TradePartner.GainItem(tempItem);
                    continue;
                }


                UserItem[] fromArray;

                switch (pair.Value.GridType)
                {
                    case GridType.Inventory:
                        fromArray = Inventory;
                        break;
                    case GridType.Equipment:
                        fromArray = Equipment;
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
                    default:
                        return;
                }

                fromArray[pair.Value.Slot] = null;
                RemoveItem(pair.Key);
                TradePartner.GainItem(pair.Key);
            }
            TradePartner.Enqueue(new S.ItemsChanged { Links = TradePartner.TradeItems.Values.ToList(), Success = true });

            foreach (KeyValuePair<UserItem, CellLinkInfo> pair in TradePartner.TradeItems)
            {
                if (pair.Key.Count > pair.Value.Count)
                {
                    pair.Key.Count -= pair.Value.Count;

                    tempItem = SEnvir.CreateFreshItem(pair.Key);
                    tempItem.Count = pair.Value.Count;
                    GainItem(tempItem);
                    continue;
                }

                UserItem[] fromArray;

                switch (pair.Value.GridType)
                {
                    case GridType.Inventory:
                        fromArray = TradePartner.Inventory;
                        break;
                    case GridType.Equipment:
                        fromArray = TradePartner.Equipment;
                        break;
                    case GridType.PartsStorage:
                        fromArray = TradePartner.PartsStorage;
                        break;
                    case GridType.Storage:
                        fromArray = TradePartner.Storage;
                        break;
                    case GridType.CompanionInventory:
                        fromArray = TradePartner.Companion.Inventory;
                        break;
                    default:
                        return;
                }

                fromArray[pair.Value.Slot] = null;
                TradePartner.RemoveItem(pair.Key);
                GainItem(pair.Key);
            }

            RefreshStats();
            SendShapeUpdate();
            TradePartner.RefreshStats();
            TradePartner.SendShapeUpdate();

            Gold.Amount += TradePartner.TradeGold - TradeGold;
            GoldChanged();

            TradePartner.Gold.Amount += TradeGold - TradePartner.TradeGold;
            TradePartner.GoldChanged();

            LogMilestone(MilestoneType.Trade, 1);
            TradePartner.LogMilestone(MilestoneType.Trade, 1);

            Connection.ReceiveChatWithObservers(con => con.Language.TradeComplete, MessageType.System);
            TradePartner.Connection.ReceiveChatWithObservers(con => con.Language.TradeComplete, MessageType.System);

            TradeClose();
        }

        #endregion
    }
}
