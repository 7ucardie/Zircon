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
        public void Chat(string text)
        {
            if (string.IsNullOrEmpty(text)) return;
            SEnvir.LogChat($"{Name}: {text}");

            //Item Links

            string[] parts;

            List<ClientUserItem> linkedItems = new List<ClientUserItem>();
            MatchCollection matches = Globals.LinkedItemRegex.Matches(text);
            foreach (Match match in matches)
            {
                if (!int.TryParse(match.Groups["ID"].Value, out int itemIndex)) continue;
                if (string.IsNullOrWhiteSpace(match.Groups["Text"].Value)) continue;

                UserItem item = Inventory.FirstOrDefault(e => e != null && e.Index == itemIndex);

                if (item == null)
                    item = Storage.FirstOrDefault(e => e != null && e.Index == itemIndex);
                if (item == null)
                    item = Equipment.FirstOrDefault(e => e != null && e.Index == itemIndex);
                if (item == null && Companion != null)
                    item = Companion.Inventory.FirstOrDefault(e => e != null && e.Index == itemIndex);
                if (item == null)
                    continue;


                text = text.Replace(match.Groups["Text"].Value, item.Info.ItemName);
                if (!linkedItems.Any(e => e.Index == item.Index))
                    linkedItems.Add(item.ToClientInfo());
            }

            if (text.StartsWith("/"))
            {
                if (SEnvir.Now < Character.Account.ChatBanExpiry) return;

                //Private Message
                text = text.Remove(0, 1);
                parts = text.Split(new[] { ' ' }, StringSplitOptions.RemoveEmptyEntries);

                if (parts.Length == 0) return;

                SConnection con = SEnvir.GetConnectionByCharacter(parts[0]);

                if (con == null || (con.Stage != GameStage.Observer && con.Stage != GameStage.Game) || SEnvir.IsBlocking(Character.Account, con.Account))
                {
                    Connection.ReceiveChatWithObservers(con => string.Format(con.Language.CannotFindPlayer, parts[0]), MessageType.System, linkedItems);
                    return;
                }

                if (!Character.Account.TempAdmin)
                {
                    if (BlockWhisper)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.BlockingWhisper, MessageType.System, linkedItems);
                        return;
                    }

                    if (con.Player != null && con.Player.BlockWhisper)
                    {
                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.PlayerBlockingWhisper, parts[0]), MessageType.System, linkedItems);
                        return;
                    }
                }

                Connection.ReceiveChat($"/{text}", MessageType.WhisperOut, linkedItems);

                con.ReceiveChat($"{Name}=> {text.Remove(0, parts[0].Length)}", Character.Account.TempAdmin ? MessageType.GMWhisperIn : MessageType.WhisperIn, linkedItems);
            }
            else if (text.StartsWith("!!"))
            {
                if (GroupMembers == null) return;

                text = $"{Name}: {text.Remove(0, 2)}";

                foreach (PlayerObject member in GroupMembers)
                {
                    if (SEnvir.IsBlocking(Character.Account, member.Character.Account)) continue;

                    if (member != this && SEnvir.Now < Character.Account.ChatBanExpiry) continue;

                    member.Connection.ReceiveChat(text, MessageType.Group, linkedItems);
                }
            }
            else if (text.StartsWith("!~"))
            {
                if (Character.Account.GuildMember == null) return;

                text = $"{Name}: {text.Remove(0, 2)}";

                foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                {
                    if (member.Account.Connection == null) continue;
                    if (member.Account.Connection.Stage != GameStage.Game && member.Account.Connection.Stage != GameStage.Observer) continue;
                    if (SEnvir.IsBlocking(Character.Account, member.Account)) continue;

                    member.Account.Connection.ReceiveChat(text, MessageType.Guild, linkedItems);
                }
            }
            else if (text.StartsWith("!@"))
            {
                if (!Character.Account.TempAdmin)
                {
                    if (SEnvir.Now < Character.Account.GlobalShoutExpiry)
                    {
                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GlobalDelay, Math.Ceiling((Character.Account.GlobalShoutExpiry - SEnvir.Now).TotalSeconds)), MessageType.System, linkedItems);
                        return;
                    }
                    if (Level < 33 && Stats[Stat.GlobalShout] == 0)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.GlobalLevel, MessageType.System, linkedItems);
                        return;
                    }

                    Character.Account.GlobalShoutExpiry = SEnvir.Now.AddSeconds(30);
                }

                text = string.Format("(!@){0}: {1}", Name, text.Remove(0, 2));

                foreach (SConnection con in SEnvir.Connections)
                {
                    switch (con.Stage)
                    {
                        case GameStage.Game:
                        case GameStage.Observer:
                            if (SEnvir.IsBlocking(Character.Account, con.Account)) continue;

                            con.ReceiveChat(text, MessageType.Global, linkedItems);
                            break;
                        default: continue;
                    }
                }
            }
            else if (text.StartsWith("!"))
            {
                //Shout
                if (!Character.Account.TempAdmin)
                {
                    if (SEnvir.Now < ShoutExpiry)
                    {
                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.ShoutDelay, Math.Ceiling((ShoutExpiry - SEnvir.Now).TotalSeconds)), MessageType.System, linkedItems);
                        return;
                    }
                    if (Level < 2)
                    {
                        Connection.ReceiveChatWithObservers(con => con.Language.ShoutLevel, MessageType.System, linkedItems);
                        return;
                    }
                }

                text = string.Format("(!){0}: {1}", Name, text.Remove(0, 1));
                ShoutExpiry = SEnvir.Now + Config.ShoutDelay;

                foreach (PlayerObject player in CurrentMap.Players)
                {
                    if (player != this && SEnvir.Now < Character.Account.ChatBanExpiry) continue;

                    if (!SEnvir.IsBlocking(Character.Account, player.Character.Account))
                        player.Connection.ReceiveChat(text, MessageType.Shout, linkedItems);

                    foreach (SConnection observer in player.Connection.Observers)
                    {
                        if (SEnvir.IsBlocking(Character.Account, observer.Account)) continue;

                        observer.ReceiveChat(text, MessageType.Shout, linkedItems);
                    }
                }
            }
            else if (text.StartsWith("@!"))
            {
                if (!Character.Account.TempAdmin) return;

                text = string.Format("{0}: {1}", Name, text.Remove(0, 2));

                foreach (SConnection con in SEnvir.Connections)
                {
                    switch (con.Stage)
                    {
                        case GameStage.Game:
                        case GameStage.Observer:
                            con.ReceiveChat(text, MessageType.Announcement, linkedItems);
                            break;
                        default: continue;
                    }
                }
            }
            else if (text.StartsWith("@"))
            {
                text = text.Remove(0, 1);
                parts = text.Split(new[] { ' ' }, StringSplitOptions.RemoveEmptyEntries);

                if (parts.Length == 0) return;

                SEnvir.CommandHandler.Handle(this, parts);
            }
            else if (text.StartsWith("#"))
            {
                text = string.Format("(#){0}: {1}", Name, text.Remove(0, 1));

                Connection.ReceiveChat(text, MessageType.ObserverChat, linkedItems);

                foreach (SConnection target in Connection.Observers)
                {
                    if (SEnvir.IsBlocking(Character.Account, target.Account)) continue;

                    target.ReceiveChat(text, MessageType.ObserverChat, linkedItems);
                }
            }
            else
            {
                text = string.Format("{0}: {1}", Name, text);
                foreach (PlayerObject player in SeenByPlayers)
                {
                    if (!Functions.InRange(CurrentLocation, player.CurrentLocation, Config.MaxViewRange)) continue;

                    if (player != this && SEnvir.Now < Character.Account.ChatBanExpiry) continue;

                    if (!SEnvir.IsBlocking(Character.Account, player.Character.Account))
                        player.Connection.ReceiveChat(text, MessageType.Normal, linkedItems, ObjectID);

                    foreach (SConnection observer in player.Connection.Observers)
                    {
                        if (SEnvir.IsBlocking(Character.Account, observer.Account)) continue;

                        observer.ReceiveChat(text, MessageType.Normal, linkedItems, ObjectID);
                    }
                }
            }
        }
        public void ObserverChat(SConnection con, string text)
        {
            if (string.IsNullOrEmpty(text)) return;

            if (con.Account?.LastCharacter == null)
            {
                con.ReceiveChat(con.Language.ObserverNotLoggedIn, MessageType.System);
                return;
            }
            SEnvir.LogChat($"{con.Account.LastCharacter.CharacterName}: {text}");

            string[] parts;

            if (text.StartsWith("/"))
            {
                //Private Message
                text = text.Remove(0, 1);
                parts = text.Split(new[] { ' ' }, StringSplitOptions.RemoveEmptyEntries);

                if (parts.Length == 0) return;

                SConnection target = SEnvir.GetConnectionByCharacter(parts[0]);

                if (target == null || (target.Stage != GameStage.Observer && target.Stage != GameStage.Game) || SEnvir.IsBlocking(con.Account, target.Account))
                {
                    con.ReceiveChat(string.Format(con.Language.CannotFindPlayer, parts[0]), MessageType.System);
                    return;
                }

                if (!con.Account.TempAdmin)
                {
                    if (target.Player != null && target.Player.BlockWhisper)
                    {
                        con.ReceiveChat(string.Format(con.Language.PlayerBlockingWhisper, parts[0]), MessageType.System);
                        return;
                    }
                }

                con.ReceiveChat($"/{text}", MessageType.WhisperOut);

                if (SEnvir.Now < con.Account.LastCharacter.Account.ChatBanExpiry) return;

                target.ReceiveChat($"{con.Account.LastCharacter.CharacterName}=> {text.Remove(0, parts[0].Length)}", Character.Account.TempAdmin ? MessageType.GMWhisperIn : MessageType.WhisperIn);
            }
            else if (text.StartsWith("!~"))
            {
                if (con.Account.GuildMember == null) return;

                text = string.Format("{0}: {1}", con.Account.LastCharacter.CharacterName, text.Remove(0, 2));

                foreach (GuildMemberInfo member in con.Account.GuildMember.Guild.Members)
                {
                    if (member.Account.Connection == null) continue;
                    if (member.Account.Connection.Stage != GameStage.Game && member.Account.Connection.Stage != GameStage.Observer) continue;

                    if (SEnvir.IsBlocking(con.Account, member.Account)) continue;

                    member.Account.Connection.ReceiveChat(text, MessageType.Guild);
                }
            }
            else if (text.StartsWith("!@"))
            {
                if (!con.Account.LastCharacter.Account.TempAdmin)
                {
                    if (SEnvir.Now < con.Account.LastCharacter.Account.GlobalShoutExpiry)
                    {
                        con.ReceiveChat(string.Format(con.Language.GlobalDelay, Math.Ceiling((con.Account.GlobalShoutExpiry - SEnvir.Now).TotalSeconds)), MessageType.System);
                        return;
                    }

                    if (con.Account.LastCharacter.Level < 33 && con.Account.LastCharacter.LastStats[Stat.GlobalShout] == 0)
                    {
                        con.ReceiveChat(con.Language.GlobalLevel, MessageType.System);
                        return;
                    }

                    con.Account.LastCharacter.Account.GlobalShoutExpiry = SEnvir.Now.AddSeconds(30);
                }

                text = string.Format("(!@){0}: {1}", con.Account.LastCharacter.CharacterName, text.Remove(0, 2));

                foreach (SConnection target in SEnvir.Connections)
                {
                    switch (target.Stage)
                    {
                        case GameStage.Game:
                        case GameStage.Observer:
                            if (SEnvir.IsBlocking(con.Account, target.Account)) continue;

                            target.ReceiveChat(text, MessageType.Global);
                            break;
                        default: continue;
                    }
                }
            }
            else if (text.StartsWith("@!"))
            {
                if (!con.Account.LastCharacter.Account.TempAdmin) return;

                text = string.Format("{0}: {1}", con.Account.LastCharacter.CharacterName, text.Remove(0, 2));

                foreach (SConnection target in SEnvir.Connections)
                {
                    switch (target.Stage)
                    {
                        case GameStage.Game:
                        case GameStage.Observer:
                            target.ReceiveChat(text, MessageType.Announcement);
                            break;
                        default: continue;
                    }
                }
            }
            else
            {
                if (SEnvir.IsBlocking(con.Account, Character.Account)) return;

                text = string.Format("(#){0}: {1}", con.Account.LastCharacter.CharacterName, text);

                Connection.ReceiveChat(text, MessageType.ObserverChat);

                foreach (SConnection target in Connection.Observers)
                {
                    if (SEnvir.IsBlocking(con.Account, target.Account)) continue;

                    target.ReceiveChat(text, MessageType.ObserverChat);
                }
            }
        }

        public void Inspect(int index, bool ranking, SConnection con)
        {
            //if (index == Character.Index) return;

            CharacterInfo target = SEnvir.GetCharacter(index);

            if (target == null) return;

            S.Inspect packet = new S.Inspect
            {
                Name = target.CharacterName,
                Partner = target.Partner?.CharacterName,
                Class = target.Class,
                Gender = target.Gender,
                //Stats = target.LastStats,
                //HermitStats = target.HermitStats,
                //HermitPoints = Math.Max(0, target.Level - 39 - target.SpentPoints),
                Level = target.Level,
                Fame = target.Fame,

                Hair = target.HairType,
                HairColour = target.HairColour,
                Items = new List<ClientUserItem>(),
                ObserverPacket = false,
                Ranking = ranking
            };

            if (target.Account.GuildMember != null)
            {
                packet.GuildName = target.Account.GuildMember.Guild.GuildName;
                packet.GuildRank = target.Account.GuildMember.Rank;
                packet.GuildFlag = target.Account.GuildMember.Guild.Flag;
                packet.GuildColour = target.Account.GuildMember.Guild.Colour;
            }

            //if (target.Player != null)
            //{
            //    packet.WearWeight = target.Player.WearWeight;
            //    packet.HandWeight = target.Player.HandWeight;
            //}


            foreach (UserItem item in target.Items)
            {
                if (item == null || item.Slot < 0 || item.Slot < Globals.EquipmentOffSet) continue;

                ClientUserItem clientItem = item.ToClientInfo();
                clientItem.Slot -= Globals.EquipmentOffSet;

                packet.Items.Add(clientItem);
            }

            con.Enqueue(packet);
        }

        //TODO - Move to MagicObject
        public override void CelestialLightActivate()
        {
            base.CelestialLightActivate();

            if (GetMagic(MagicType.CelestialLight, out CelestialLight celestialLight))
            {
                celestialLight.MagicCooldown(null, 6000);
            }
        }
        
        public override void ItemRevive()
        {
            base.ItemRevive();

            Character.ItemReviveTime = ItemReviveTime;

            UpdateReviveTimers(Connection);
        }

        public void UpdateReviveTimers(SConnection con)
        {
            con.Enqueue(new S.ReviveTimers
            {
                ItemReviveTime = ItemReviveTime > SEnvir.Now ? ItemReviveTime - SEnvir.Now : TimeSpan.Zero,
                ReincarnationPillTime = Character.ReincarnationPillTime > SEnvir.Now ? Character.ReincarnationPillTime - SEnvir.Now : TimeSpan.Zero,
            });
        }
    }
}
