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
        public void GainExperience(decimal amount, bool huntGold, int gainLevel = Int32.MaxValue, bool rateEffected = true)
        {
            if (rateEffected)
            {
                amount *= 1M + Stats[Stat.ExperienceRate] / 100M;

                amount *= 1M + Stats[Stat.BaseExperienceRate] / 100M;

                for (int i = 0; i < Character.Rebirth; i++)
                    amount *= 0.5M;
            }

            /*
            if (Level >= 60)
            {
    
                if (Level > gainLevel)
                    amount -= Math.Min(amount, amount * Math.Min(0.9M, (Level - gainLevel) * 0.10M));
            }
            else
            {
                if (Level > gainLevel)
                    amount -= Math.Min(amount, amount * Math.Min(0.3M, (Level - gainLevel) * 0.06M));
            }
            */

            if (amount == 0) return;

            Experience += amount;
            ExperienceAccumulated += amount;

            UserItem weapon = Equipment[(int)EquipmentSlot.Weapon];

            if (weapon != null && weapon.Info.ItemEffect != ItemEffect.PickAxe && (weapon.Flags & UserItemFlags.Refinable) != UserItemFlags.Refinable && (weapon.Flags & UserItemFlags.NonRefinable) != UserItemFlags.NonRefinable && weapon.Level < Globals.WeaponExperienceList.Count && rateEffected)
            {
                weapon.Experience += amount / 10;

                if (weapon.Experience >= Globals.WeaponExperienceList[weapon.Level])
                {
                    weapon.Experience = 0;
                    weapon.Level++;

                    if (weapon.Level < Globals.WeaponExperienceList.Count)
                        weapon.Flags |= UserItemFlags.Refinable;
                }
            }

            if (huntGold)
            {
                BuffInfo buff = Buffs.First(x => x.Type == BuffType.HuntGold);

                if (buff.Stats[Stat.AvailableHuntGold] > 0)
                {
                    buff.Stats[Stat.AvailableHuntGold]--;
                    HuntGold.Amount++;
                    HuntGoldChanged();
                    Enqueue(new S.BuffChanged { Index = buff.Index, Stats = buff.Stats });
                }
            }

            if (Level >= Config.MaxLevel || Experience < MaxExperience)
            {
                //SEnvir.RankingSort(Character);
                return;
            }

            ProcessExperience(true);

            Experience -= MaxExperience;

            Level++;
            LevelUp();
        }

        public void LevelUp()
        {
            RefreshStats();

            SetHP(Stats[Stat.Health]);
            SetMP(Stats[Stat.Mana]);
            SetFP(CurrentFP);

            Enqueue(new S.LevelChanged { Level = Level, Experience = Experience, MaxExperience = MaxExperience });
            Broadcast(new S.ObjectLeveled { ObjectID = ObjectID });

            SEnvir.RankingSort(Character);

            if (Character.Account.Characters.Max(x => x.Level) <= Level)
                BuffRemove(BuffType.Veteran);

            LogMilestone(MilestoneType.Level, Level, true);

            ApplyGuildBuff();
        }

        public void RefreshWeight()
        {
            BagWeight = 0;

            foreach (UserItem item in Inventory)
            {
                if (item == null) continue;

                BagWeight += item.Weight;
            }

            WearWeight = 0;
            HandWeight = 0;

            foreach (UserItem item in Equipment)
            {
                if (item == null) continue;

                switch (item.Info.ItemType)
                {
                    case ItemType.Weapon:
                    case ItemType.Torch:
                        HandWeight += item.Weight;
                        break;
                    default:
                        WearWeight += item.Weight;
                        break;
                }
            }

            Enqueue(new S.WeightUpdate { BagWeight = BagWeight, WearWeight = WearWeight, HandWeight = HandWeight });
        }
        public override void RefreshStats()
        {
            int tracking = Stats[Stat.BossTracker] + Stats[Stat.PlayerTracker];

            Stats.Clear();

            AddBaseStats();

            switch (Character.Account.Horse)
            {
                case HorseType.Brown:
                    Stats[Stat.BagWeight] += 50;
                    break;
                case HorseType.White:
                    Stats[Stat.Comfort] += 2;
                    Stats[Stat.BagWeight] += 100;
                    Stats[Stat.MaxAC] += 5;
                    Stats[Stat.MaxMR] += 5;
                    Stats[Stat.MaxDC] += 5;
                    Stats[Stat.MaxMC] += 5;
                    Stats[Stat.MaxSC] += 5;
                    break;
                case HorseType.Red:
                    Stats[Stat.Comfort] += 5;
                    Stats[Stat.BagWeight] += 150;
                    Stats[Stat.MaxAC] += 12;
                    Stats[Stat.MaxMR] += 12;
                    Stats[Stat.MaxDC] += 12;
                    Stats[Stat.MaxMC] += 12;
                    Stats[Stat.MaxSC] += 12;
                    break;
                case HorseType.Black:
                    Stats[Stat.Comfort] += 7;
                    Stats[Stat.BagWeight] += 200;
                    Stats[Stat.MaxAC] += 25;
                    Stats[Stat.MaxMR] += 25;
                    Stats[Stat.MaxDC] += 25;
                    Stats[Stat.MaxMC] += 25;
                    Stats[Stat.MaxSC] += 25;
                    break;
                case HorseType.WhiteUnicorn:
                case HorseType.RedUnicorn:
                    Stats[Stat.Comfort] += 9;
                    Stats[Stat.BagWeight] += 250;
                    Stats[Stat.MaxAC] += 30;
                    Stats[Stat.MaxMR] += 30;
                    Stats[Stat.MaxDC] += 30;
                    Stats[Stat.MaxMC] += 30;
                    Stats[Stat.MaxSC] += 30;
                    break;
            }

            Dictionary<SetInfo, List<ItemInfo>> sets = new Dictionary<SetInfo, List<ItemInfo>>();

            foreach (UserItem item in Equipment)
            {
                if (item == null || (item.CurrentDurability == 0 && item.Info.Durability > 0)) continue;

                if (item.Info.Set != null)
                {
                    List<ItemInfo> items;
                    if (!sets.TryGetValue(item.Info.Set, out items))
                        sets[item.Info.Set] = items = new List<ItemInfo>();

                    if (!items.Contains(item.Info))
                        items.Add(item.Info);
                }

                if (item.Info.ItemType == ItemType.HorseArmour && Character.Account.Horse == HorseType.None) continue;

                Stats.Add(item.Info.Stats, item.Info.ItemType != ItemType.Weapon);
                Stats.Add(item.Stats, item.Info.ItemType != ItemType.Weapon);

                if (item.Info.ItemType == ItemType.Weapon)
                {
                    Stat ele = item.Stats.GetWeaponElement();

                    if (ele == Stat.None)
                        ele = item.Info.Stats.GetWeaponElement();

                    if (ele != Stat.None)
                        Stats[ele] += item.Stats.GetWeaponElementValue() + item.Info.Stats.GetWeaponElementValue();
                }

                if (item.Info.ItemEffect == ItemEffect.MagicRing)
                {
                    MagicInfo info = SEnvir.GetMagicInfo(item.Info.Shape);

                    if (info != null && info.School != MagicSchool.None)
                    {
                        if (!GetMagic(info.Magic, out MagicObject magicObject))
                        {
                            var magic = SEnvir.UserMagicList.CreateNewObject();
                            magic.Character = Character;
                            magic.Info = info;
                            magic.ItemRequired = true;

                            magicObject = SetupMagic(magic);
                            Enqueue(new S.NewMagic { Magic = magic.ToClientInfo() });
                            Connection.ReceiveChatWithObservers(con => string.Format(con.Language.LearnBookSuccess, magic.Info.Name), MessageType.System);
                        }
                    }
                }
            }

            if (GroupMembers != null && GroupMembers.Count >= 8)
            {
                int warrior = 0, wizard = 0, taoist = 0, assassin = 0;

                foreach (PlayerObject ob in GroupMembers)
                {
                    switch (ob.Class)
                    {
                        case MirClass.Warrior:
                            warrior++;
                            break;
                        case MirClass.Wizard:
                            wizard++;
                            break;
                        case MirClass.Taoist:
                            taoist++;
                            break;
                        case MirClass.Assassin:
                            assassin++;
                            break;
                    }
                }

                if (warrior >= 2 && wizard >= 2 && taoist >= 2 && assassin >= 2)
                {
                    Stats[Stat.Health] += Stats[Stat.BaseHealth] / 10;
                    Stats[Stat.Mana] += Stats[Stat.BaseMana] / 10;
                }
            }

            foreach (MagicType type in MagicObjects.Keys)
            {
                var magicObject = MagicObjects[type];

                if (!magicObject.CanUseMagic()) continue;

                Stats.Add(magicObject.GetPassiveStats());
            }

            foreach (BuffInfo buff in Buffs)
            {
                if (buff.Pause) continue;

                if (buff.Type == BuffType.ItemBuff)
                {
                    Stats.Add(SEnvir.ItemInfoList.Binding.First(x => x.Index == buff.ItemIndex).Stats);
                    continue;
                }

                if (buff.Stats == null) continue;

                Stats.Add(buff.Stats);
            }

            foreach (KeyValuePair<SetInfo, List<ItemInfo>> pair in sets)
            {
                if (pair.Key.Items.Count != pair.Value.Count) continue;

                foreach (SetInfoStat stat in pair.Key.SetStats)
                {
                    if (Level < stat.Level) continue;

                    switch (Class)
                    {
                        case MirClass.Warrior:
                            if ((stat.Class & RequiredClass.Warrior) != RequiredClass.Warrior) continue;
                            break;
                        case MirClass.Wizard:
                            if ((stat.Class & RequiredClass.Wizard) != RequiredClass.Wizard) continue;
                            break;
                        case MirClass.Taoist:
                            if ((stat.Class & RequiredClass.Taoist) != RequiredClass.Taoist) continue;
                            break;
                        case MirClass.Assassin:
                            if ((stat.Class & RequiredClass.Assassin) != RequiredClass.Assassin) continue;
                            break;
                    }

                    Stats[stat.Stat] += stat.Amount;
                }
            }

            if (Buffs.Any(x => x.Type == BuffType.RagingWind) && GetMagic(MagicType.RagingWind, out RagingWind ragingWind))
            {
                int power = Stats[Stat.MinAC] + Stats[Stat.MaxAC] + 4 + ragingWind.Magic.Level * 6;

                Stats[Stat.MinAC] = power * 3 / 10;
                Stats[Stat.MaxAC] = power - Stats[Stat.MinAC];

                power = Stats[Stat.MinMR] + Stats[Stat.MaxMR] + 4 + ragingWind.Magic.Level * 6;

                Stats[Stat.MinMR] = power * 3 / 10;
                Stats[Stat.MaxMR] = power - Stats[Stat.MinMR];
            }

            Stats[Stat.AttackSpeed] += Math.Min(3, Level / 15);

            Stats[Stat.FireResistance] = Math.Min(5, Stats[Stat.FireResistance]);
            Stats[Stat.IceResistance] = Math.Min(5, Stats[Stat.IceResistance]);
            Stats[Stat.LightningResistance] = Math.Min(5, Stats[Stat.LightningResistance]);
            Stats[Stat.WindResistance] = Math.Min(5, Stats[Stat.WindResistance]);
            Stats[Stat.HolyResistance] = Math.Min(5, Stats[Stat.HolyResistance]);
            Stats[Stat.DarkResistance] = Math.Min(5, Stats[Stat.DarkResistance]);
            Stats[Stat.PhantomResistance] = Math.Min(5, Stats[Stat.PhantomResistance]);
            Stats[Stat.PhysicalResistance] = Math.Min(5, Stats[Stat.PhysicalResistance]);

            Stats[Stat.Comfort] = Math.Min(20, Stats[Stat.Comfort]);
            Stats[Stat.AttackSpeed] = Math.Min(15, Stats[Stat.AttackSpeed]);

            RegenDelay = TimeSpan.FromMilliseconds(15000 - Stats[Stat.Comfort] * 650);

            Stats[Stat.Health] += (Stats[Stat.Health] * Stats[Stat.HealthPercent]) / 100;
            Stats[Stat.Mana] += (Stats[Stat.Mana] * Stats[Stat.ManaPercent]) / 100;

            Stats[Stat.MinDC] += (Stats[Stat.MinDC] * Stats[Stat.DCPercent]) / 100;
            Stats[Stat.MaxDC] += (Stats[Stat.MaxDC] * Stats[Stat.DCPercent]) / 100;

            Stats[Stat.MinMC] += (Stats[Stat.MinMC] * Stats[Stat.MCPercent]) / 100;
            Stats[Stat.MaxMC] += (Stats[Stat.MaxMC] * Stats[Stat.MCPercent]) / 100;

            Stats[Stat.MinSC] += (Stats[Stat.MinSC] * Stats[Stat.SCPercent]) / 100;
            Stats[Stat.MaxSC] += (Stats[Stat.MaxSC] * Stats[Stat.SCPercent]) / 100;

            Stats[Stat.Health] = Math.Max(10, Stats[Stat.Health]);
            Stats[Stat.Mana] = Math.Max(10, Stats[Stat.Mana]);

            if (Buffs.Any(x => x.Type == BuffType.MagicWeakness))
            {
                Stats[Stat.MinMR] = 0;
                Stats[Stat.MaxMR] = 0;
            }

            Stats[Stat.MinAC] += (Stats[Stat.MinAC] * Stats[Stat.PhysicalDefencePercent]) / 100;
            Stats[Stat.MaxAC] += (Stats[Stat.MaxAC] * Stats[Stat.PhysicalDefencePercent]) / 100;

            Stats[Stat.MinMR] += (Stats[Stat.MinMR] * Stats[Stat.MagicDefencePercent]) / 100;
            Stats[Stat.MaxMR] += (Stats[Stat.MaxMR] * Stats[Stat.MagicDefencePercent]) / 100;

            Stats[Stat.MinAC] = Math.Max(0, Stats[Stat.MinAC]);
            Stats[Stat.MaxAC] = Math.Max(0, Stats[Stat.MaxAC]);
            Stats[Stat.MinMR] = Math.Max(0, Stats[Stat.MinMR]);
            Stats[Stat.MaxMR] = Math.Max(0, Stats[Stat.MaxMR]);
            Stats[Stat.MinDC] = Math.Max(0, Stats[Stat.MinDC]);
            Stats[Stat.MaxDC] = Math.Max(0, Stats[Stat.MaxDC]);
            Stats[Stat.MinMC] = Math.Max(0, Stats[Stat.MinMC]);
            Stats[Stat.MaxMC] = Math.Max(0, Stats[Stat.MaxMC]);
            Stats[Stat.MinSC] = Math.Max(0, Stats[Stat.MinSC]);
            Stats[Stat.MaxSC] = Math.Max(0, Stats[Stat.MaxSC]);

            Stats[Stat.MinDC] = Math.Min(Stats[Stat.MinDC], Stats[Stat.MaxDC]);
            Stats[Stat.MinMC] = Math.Min(Stats[Stat.MinMC], Stats[Stat.MaxMC]);
            Stats[Stat.MinSC] = Math.Min(Stats[Stat.MinSC], Stats[Stat.MaxSC]);

            Stats[Stat.HandWeight] += Stats[Stat.HandWeight] * Stats[Stat.WeightRate];
            Stats[Stat.WearWeight] += Stats[Stat.WearWeight] * Stats[Stat.WeightRate];
            Stats[Stat.BagWeight] += Stats[Stat.BagWeight] * Stats[Stat.WeightRate];

            Stats[Stat.Rebirth] = Character.Rebirth;

            Stats[Stat.Fame] = Character.Fame;

            Stats[Stat.DropRate] += 20 * Stats[Stat.Rebirth];
            Stats[Stat.GoldRate] += 20 * Stats[Stat.Rebirth];

            Enqueue(new S.StatsUpdate
            {
                Stats = Stats,
                HermitStats = Config.EnableHermit ? Character.HermitStats : new Stats(),
                HermitPoints = Math.Max(0, Level - 39 - Character.SpentPoints)
            });

            S.DataObjectMaxHealthMana p = new S.DataObjectMaxHealthMana { ObjectID = ObjectID, MaxHealth = Stats[Stat.Health], MaxMana = Stats[Stat.Mana] };

            foreach (PlayerObject player in DataSeenByPlayers)
                player.Enqueue(p);

            RefreshWeight();

            if (CurrentHP > Stats[Stat.Health]) SetHP(Stats[Stat.Health]);
            if (CurrentMP > Stats[Stat.Mana]) SetMP(Stats[Stat.Mana]);

            if (Spawned && tracking != Stats[Stat.PlayerTracker] + Stats[Stat.BossTracker])
            {
                RemoveAllObjects();
                AddAllObjects();
            }
        }
        public void AddBaseStats()
        {
            MaxExperience = Level < Globals.ExperienceList.Count ? Globals.ExperienceList[Level] : 0;

            BaseStat stat = null;

            //Get best possible match.
            foreach (BaseStat bStat in SEnvir.BaseStatList.Binding)
            {
                if (bStat.Class != Class) continue;
                if (bStat.Level > Level) continue;
                if (stat != null && bStat.Level < stat.Level) continue;

                stat = bStat;

                if (bStat.Level == Level) break;
            }

            if (stat == null) return;

            Stats[Stat.Health] = stat.Health;
            Stats[Stat.Mana] = stat.Mana;

            Stats[Stat.Focus] = Character.Discipline?.Info.FocusPoints ?? 0;

            Stats[Stat.BagWeight] = stat.BagWeight;
            Stats[Stat.WearWeight] = stat.WearWeight;
            Stats[Stat.HandWeight] = stat.HandWeight;

            Stats[Stat.Accuracy] = stat.Accuracy;

            Stats[Stat.Agility] = stat.Agility;

            Stats[Stat.MinAC] = stat.MinAC;
            Stats[Stat.MaxAC] = stat.MaxAC;

            Stats[Stat.MinMR] = stat.MinMR;
            Stats[Stat.MaxMR] = stat.MaxMR;

            Stats[Stat.MinDC] = stat.MinDC;
            Stats[Stat.MaxDC] = stat.MaxDC;

            Stats[Stat.MinMC] = stat.MinMC;
            Stats[Stat.MaxMC] = stat.MaxMC;

            Stats[Stat.MinSC] = stat.MinSC;
            Stats[Stat.MaxSC] = stat.MaxSC;

            Stats[Stat.PickUpRadius] = 1;
            Stats[Stat.SkillRate] = 1;
            Stats[Stat.CriticalChance] = 1;

            if (Config.EnableHermit)
            {
                Stats.Add(Character.HermitStats);
            }

            Stats[Stat.BaseHealth] = Stats[Stat.Health];
            Stats[Stat.BaseMana] = Stats[Stat.Mana];
        }

        public void AssignHermit(Stat stat)
        {
            if (!Config.EnableHermit) return;

            if (Level - 39 - Character.SpentPoints <= 0) return;

            switch (stat)
            {
                case Stat.MaxDC:
                case Stat.MaxMC:
                case Stat.MaxSC:
                    Character.HermitStats[stat] += 2 + Character.SpentPoints / 10;
                    break;
                case Stat.MaxAC:
                    Character.HermitStats[Stat.MinAC] += 2;
                    Character.HermitStats[Stat.MaxAC] += 2;
                    break;
                case Stat.MaxMR:
                    Character.HermitStats[Stat.MinMR] += 2;
                    Character.HermitStats[Stat.MaxMR] += 2;
                    break;
                case Stat.Health:
                    Character.HermitStats[stat] += 10 + (Character.SpentPoints / 10) * 10;
                    break;
                case Stat.Mana:
                    Character.HermitStats[stat] += 15 + (Character.SpentPoints / 10) * 15;
                    break;
                case Stat.WeaponElement:

                    if (Character.SpentPoints >= 20) return;

                    int count = 2 + Character.SpentPoints / 10;

                    List<Stat> Elements = new List<Stat>();

                    if (Stats[Stat.FireAttack] > 0) Elements.Add(Stat.FireAttack);
                    if (Stats[Stat.IceAttack] > 0) Elements.Add(Stat.IceAttack);
                    if (Stats[Stat.LightningAttack] > 0) Elements.Add(Stat.LightningAttack);
                    if (Stats[Stat.WindAttack] > 0) Elements.Add(Stat.WindAttack);
                    if (Stats[Stat.HolyAttack] > 0) Elements.Add(Stat.HolyAttack);
                    if (Stats[Stat.DarkAttack] > 0) Elements.Add(Stat.DarkAttack);
                    if (Stats[Stat.PhantomAttack] > 0) Elements.Add(Stat.PhantomAttack);

                    if (Elements.Count == 0)
                        Elements.AddRange(new[]
                        {
                            Stat.FireAttack,
                            Stat.IceAttack,
                            Stat.LightningAttack,
                            Stat.WindAttack,
                            Stat.HolyAttack,
                            Stat.DarkAttack,
                            Stat.PhantomAttack,
                        });

                    for (int i = 0; i < count; i++)
                        Character.HermitStats[Elements[SEnvir.Random.Next(Elements.Count)]]++;
                    break;
                default:
                    Character.Account.Banned = true;
                    Character.Account.BanReason = "Attempted to Exploit hermit.";
                    Character.Account.BanExpiry = SEnvir.Now.AddYears(10);
                    return;
            }

            Character.SpentPoints++;
            RefreshStats();
        }

        #region Buffs

        public void ApplyMapBuff()
        {
            BuffRemove(BuffType.MapEffect);
            BuffRemove(BuffType.InstanceEffect);

            if (CurrentMap == null) return;

            if (CurrentMap.Info.Stats.Count != 0)
            {
                BuffAdd(BuffType.MapEffect, TimeSpan.MaxValue, CurrentMap.Info.Stats, false, false, TimeSpan.Zero);
            }

            if (CurrentMap.Instance != null && CurrentMap.Instance.Stats.Count != 0)
            {
                BuffAdd(BuffType.InstanceEffect, TimeSpan.MaxValue, CurrentMap.Instance.Stats, false, false, TimeSpan.Zero);
            }
        }

        public void ApplyServerBuff()
        {
            BuffRemove(BuffType.Server);

            Stats stats = new Stats();

            stats[Stat.BaseExperienceRate] += Config.ExperienceRate;
            stats[Stat.BaseDropRate] += Config.DropRate;
            stats[Stat.BaseGoldRate] += Config.GoldRate;
            stats[Stat.SkillRate] = Config.SkillRate;
            stats[Stat.CompanionRate] = Config.CompanionRate;

            if (stats.Count == 0) return;

            BuffAdd(BuffType.Server, TimeSpan.MaxValue, stats, false, false, TimeSpan.Zero);
        }

        public void ApplyObserverBuff()
        {
            BuffRemove(BuffType.Observable);

            if (!Character.Observable) return;

            if (!Config.AllowObservation) return;

            Stats stats = new Stats();

            stats[Stat.ExperienceRate] += 15;
            stats[Stat.DropRate] += 15;
            stats[Stat.GoldRate] += 15;

            BuffAdd(BuffType.Observable, TimeSpan.MaxValue, stats, false, false, TimeSpan.Zero);
        }

        public void ApplyFameBuff()
        {
            BuffRemove(BuffType.Fame);

            if (Character.Fame <= 0) return;

            var fame = SEnvir.FameInfoList.Binding.FirstOrDefault(x => x.Index == Character.Fame);

            if (fame == null) return;

            Stats stats = new();

            foreach (var stat in fame.BuffStats)
            {
                stats[stat.Stat] = stat.Amount;
            }

            if (stats.Count == 0) return;

            stats[Stat.Fame] = fame.Index;

            BuffAdd(BuffType.Fame, TimeSpan.MaxValue, stats, false, false, TimeSpan.Zero);
        }

        public void ApplyCastleBuff()
        {
            BuffRemove(BuffType.Castle);

            if (Character.Account.GuildMember?.Guild.Castle == null) return;

            Stats stats = new Stats();

            stats[Stat.ExperienceRate] += 10;
            stats[Stat.DropRate] += 10;
            stats[Stat.GoldRate] += 10;

            BuffAdd(BuffType.Castle, TimeSpan.MaxValue, stats, false, false, TimeSpan.Zero);
        }
        public void ApplyGuildBuff()
        {
            BuffRemove(BuffType.Guild);

            if (Character.Account.GuildMember == null) return;

            Stats stats = new Stats();

            if (Character.Account.GuildMember.Guild.StarterGuild)
            {
                if (Level < 50)
                {
                    stats[Stat.ExperienceRate] += 50;
                    stats[Stat.DropRate] += 50;
                    stats[Stat.GoldRate] += 50;
                }
                else
                {
                    stats[Stat.ExperienceRate] -= 50;
                    stats[Stat.DropRate] -= 50;
                    stats[Stat.GoldRate] -= 50;
                }
            }
            else if (Character.Account.GuildMember.Guild.Members.Count <= 15)
            {
                stats[Stat.ExperienceRate] += 30;
                stats[Stat.DropRate] += 30;
                stats[Stat.GoldRate] += 30;
            }
            else if (Character.Account.GuildMember.Guild.Members.Count <= 30)
            {
                stats[Stat.ExperienceRate] += 23;
                stats[Stat.DropRate] += 23;
                stats[Stat.GoldRate] += 23;
            }
            else if (Character.Account.GuildMember.Guild.Members.Count <= 45)
            {
                stats[Stat.ExperienceRate] += 18;
                stats[Stat.DropRate] += 18;
                stats[Stat.GoldRate] += 18;
            }
            else
            {
                stats[Stat.ExperienceRate] += 13;
                stats[Stat.DropRate] += 13;
                stats[Stat.GoldRate] += 13;
            }

            if (stats.Count == 0) return;

            if (!Character.Account.GuildMember.Guild.StarterGuild && GroupMembers != null)
            {
                foreach (PlayerObject member in GroupMembers)
                {
                    if (member.Character.Account.GuildMember != null && member.Character.Account.GuildMember.Guild.StarterGuild) continue;

                    if (member.Character.Account.GuildMember?.Guild != Character.Account.GuildMember.Guild) return;
                }
            }

            BuffAdd(BuffType.Guild, TimeSpan.MaxValue, stats, false, false, TimeSpan.Zero);
        }


        public bool ItemBuffAdd(ItemInfo info)
        {
            switch (info.ItemEffect)
            {
                case ItemEffect.DestructionElixir:
                case ItemEffect.HasteElixir:
                case ItemEffect.LifeElixir:
                case ItemEffect.ManaElixir:
                case ItemEffect.NatureElixir:
                case ItemEffect.SpiritElixir:

                    for (int i = Buffs.Count - 1; i >= 0; i--)
                    {
                        BuffInfo buff = Buffs[i];
                        if (buff.Type != BuffType.ItemBuff || info.Index == buff.ItemIndex) continue; //Same Item don't remove, extend instead 

                        ItemInfo buffItemInfo = SEnvir.ItemInfoList.Binding.First(x => x.Index == buff.ItemIndex);

                        if (buffItemInfo.ItemEffect == info.ItemEffect)
                            BuffRemove(buff);
                    }
                    break;
            }

            BuffInfo currentBuff = Buffs.FirstOrDefault(x => x.Type == BuffType.ItemBuff && x.ItemIndex == info.Index);

            if (currentBuff != null) //Extend buff
            {
                if (info.Stats[Stat.Duration] >= 0)
                {
                    TimeSpan duration = TimeSpan.FromSeconds(info.Stats[Stat.Duration]);

                    long ticks = currentBuff.RemainingTime.Ticks - long.MaxValue + duration.Ticks; //Check for Overflow (Probably never going to happen) 403x MaxValue durations refreshes.

                    if (ticks >= 0)
                        currentBuff.RemainingTime = TimeSpan.MaxValue;
                    else
                        currentBuff.RemainingTime += duration;
                }
                else
                    currentBuff.RemainingTime = TimeSpan.MaxValue;

                Enqueue(new S.BuffTime { Index = currentBuff.Index, Time = currentBuff.RemainingTime });
                return true;
            }

            currentBuff = SEnvir.BuffInfoList.CreateNewObject();

            currentBuff.Type = BuffType.ItemBuff;
            currentBuff.ItemIndex = info.Index;
            currentBuff.RemainingTime = info.Stats[Stat.Duration] > 0 ? TimeSpan.FromSeconds(info.Stats[Stat.Duration]) : TimeSpan.MaxValue;

            if (info.RequiredAmount == 0 && info.RequiredClass == RequiredClass.All)
                currentBuff.Account = Character.Account;
            else
                currentBuff.Character = Character;

            currentBuff.Pause = InSafeZone;
            Buffs.Add(currentBuff);
            Enqueue(new S.BuffAdd { Buff = currentBuff.ToClientInfo() });

            RefreshStats();
            AddAllObjects();

            return true;
        }

        public override BuffInfo BuffAdd(BuffType type, TimeSpan remainingTicks, Stats stats, bool visible, bool pause, TimeSpan tickRate, bool hidden = false, int extra = 0)
        {
            BuffInfo info = base.BuffAdd(type, remainingTicks, stats, visible, pause, tickRate, hidden, extra);

            info.Character = Character;

            switch (type)
            {
                case BuffType.ItemBuff:
                    info.Pause = InSafeZone;
                    break;
            }

            info.Hidden = hidden;

            if (!hidden)
            {
                Enqueue(new S.BuffAdd { Buff = info.ToClientInfo() });
            }

            switch (type)
            {
                case BuffType.StrengthOfFaith:
                    foreach (MonsterObject pet in Pets)
                    {
                        if (GetMagic(MagicType.StrengthOfFaith, out StrengthOfFaith strengthOfFaith))
                        {
                            pet.Magics.Add(strengthOfFaith.Magic);
                        }
                        pet.RefreshStats();
                    }
                    break;
                case BuffType.DragonRepulse:
                case BuffType.Companion:
                case BuffType.Server:
                case BuffType.MapEffect:
                case BuffType.Guild:
                case BuffType.Ranking:
                case BuffType.Developer:
                case BuffType.Castle:
                case BuffType.ElementalHurricane:
                case BuffType.SuperiorMagicShield:
                case BuffType.ElementalSwords:
                    info.IsTemporary = true;
                    break;
            }

            return info;
        }

        public override void BuffRemove(BuffInfo info)
        {
            int oldHealth = Stats[Stat.Health];

            base.BuffRemove(info);

            if (!info.Hidden)
            {
                Enqueue(new S.BuffRemove { Index = info.Index });
            }

            switch (info.Type)
            {
                case BuffType.StrengthOfFaith:
                    foreach (MonsterObject pet in Pets)
                    {
                        if (GetMagic(MagicType.StrengthOfFaith, out StrengthOfFaith strengthOfFaith))
                        {
                            pet.Magics.Remove(strengthOfFaith.Magic);
                        }

                        pet.RefreshStats();
                    }
                    break;
                case BuffType.Renounce:
                    if (Dead) return;
                    ChangeHP(info.Stats[Stat.RenounceHPLost]);
                    break;

                case BuffType.ItemBuff:
                    RefreshStats();
                    RemoveAllObjects();
                    break;
            }
        }

        public void PauseBuffs()
        {
            if (CurrentCell == null) return;

            bool change = false;

            bool pause = InSafeZone || Fishing;

            foreach (MapObject ob in CurrentCell.Objects)
            {
                if (ob.Race != ObjectType.Spell) continue;

                SpellObject spell = (SpellObject)ob;

                if (spell.Effect != SpellEffect.Rubble) continue;

                pause = true;
                break;
            }

            foreach (BuffInfo buff in Buffs)
            {
                bool buffPause = pause;

                switch (buff.Type)
                {
                    case BuffType.ItemBuff:
                        buffPause = buff.RemainingTime != TimeSpan.MaxValue && pause;
                        break;
                    case BuffType.HuntGold:
                        break;
                    default:
                        continue;
                }

                if (buff.Pause == buffPause) continue;

                buff.Pause = buffPause;
                change = true;

                Enqueue(new S.BuffPaused { Index = buff.Index, Paused = buffPause });
            }

            if (change)
                RefreshStats();
        }
        #endregion
    }
}
