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
        #region Combat

        public bool AttackLocation(Point location, List<MagicType> types, bool primary)
        {
            Cell cell = CurrentMap.GetCell(location);

            if (cell?.Objects == null) return false;

            bool result = false;

            foreach (MapObject ob in cell.Objects)
            {
                if (!CanAttackTarget(ob)) continue;

                int delay = 300;

                if (types.Contains(MagicType.DragonRise))
                    delay = 600;

                ActionList.Add(new DelayedAction(SEnvir.Now.AddMilliseconds(delay), ActionType.DelayAttack, ob, types, primary, 0));

                result = true;
            }

            return result;
        }

        public void RangeAttack(MirDirection direction, uint target)
        {
            UserItem weapon = Equipment[(int)EquipmentSlot.Weapon];

            // shukiran
            if (weapon is null || weapon.Info.Shape != Globals.ShurikenLibraryWeaponShape)
                return;


            MapObject ob = VisibleObjects.FirstOrDefault(x => x.ObjectID == target);

            if (ob is null) return;


            if (SEnvir.Now < ActionTime || SEnvir.Now < AttackTime)
            {
                if (!PacketWaiting)
                {
                    ActionList.Add(new DelayedAction(ActionTime, ActionType.RangeAttack, direction, target));
                    PacketWaiting = true;
                }
                else
                    Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });

                return;
            }

            if (!CanAttack)
            {
                Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                return;
            }

            if (!CanAttackTarget(ob))
            {
                ob = null;
                Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                return;
            }

            CombatTime = SEnvir.Now;

            if (Stats[Stat.Comfort] < 15)
                RegenTime = SEnvir.Now + RegenDelay;
            Direction = direction;
            ActionTime = SEnvir.Now + Globals.AttackTime;

            int aspeed = Stats[Stat.AttackSpeed];
            int attackDelay = Globals.AttackDelay - aspeed * Globals.ASpeedRate;
            attackDelay = Math.Max(800, attackDelay);
            AttackTime = SEnvir.Now.AddMilliseconds(attackDelay);

            if (BagWeight > Stats[Stat.BagWeight] || (Poison & PoisonType.Neutralize) == PoisonType.Neutralize)
                AttackTime += TimeSpan.FromMilliseconds(attackDelay);

            Poison poison = PoisonList.FirstOrDefault(x => x.Type == PoisonType.Slow);
            TimeSpan slow = TimeSpan.Zero;
            if (poison != null)
            {
                slow = TimeSpan.FromMilliseconds(poison.Value * 100);
                ActionTime += slow;
            }

            Direction = ob == null || ob == this ? direction : Functions.DirectionFromPoint(CurrentLocation, ob.CurrentLocation);

            Element element = Functions.GetAttackElement(Stats);

            if (Equipment[(int)EquipmentSlot.Amulet]?.Info.ItemType == ItemType.DarkStone)
            {
                element = Equipment[(int)EquipmentSlot.Amulet].Info.Stats.GetAffinityElement();
            }

            Broadcast(new S.ObjectRangeAttack
            {
                ObjectID = ObjectID,
                Direction = Direction,
                Location = CurrentLocation,
                AttackMagic = MagicType.Shuriken,
                AttackElement = element,
                Targets = new List<uint> { ob.ObjectID }
            });

            int projectileDelay = Math.Max(100, Math.Min(750, Functions.Distance(CurrentLocation, ob.CurrentLocation) * 50));

            ActionList.Add(new DelayedAction(SEnvir.Now.AddMilliseconds(projectileDelay), ActionType.DelayAttack, ob, new List<MagicType>() { MagicType.Shuriken }, true, 50));

            DamageItem(GridType.Equipment, (int)EquipmentSlot.Weapon);
        }

        public void Attack(MapObject ob, List<MagicType> types, bool primary, int extra)
        {
            if (ob?.Node == null || ob.Dead) return;

            for (int i = Pets.Count - 1; i >= 0; i--)
                if (Pets[i].Target == null)
                    Pets[i].Target = ob;

            int power = GetDC();

            bool ignoreAccuracy = false, ignoreDefense = false, hasFlameSplash = false;
            bool hasMassacre = false;

            int maxLifeSteal = 750;

            foreach (MagicType type in types)
            {
                if (GetMagic(type, out MagicObject magicObject))
                {
                    if (magicObject.IgnoreAccuracy) ignoreAccuracy = true;
                    if (magicObject.IgnorePhysicalDefense) ignoreDefense = true;

                    if (magicObject.MaxLifeSteal > maxLifeSteal) maxLifeSteal = magicObject.MaxLifeSteal;

                    if (magicObject.HasMassacre) hasMassacre = true;
                    if (magicObject.HasFlameSplash(primary)) hasFlameSplash = true;
                }
            }

            int accuracy = Stats[Stat.Accuracy];

            int res;

            if (types.Contains(MagicType.Karma))
            {
                if (GetMagic(MagicType.Resolution, out Resolution resolution))
                {
                    accuracy += (accuracy * resolution.Magic.GetPower() / 100);
                }
            }

            if (!ignoreAccuracy && SEnvir.Random.Next(ob.Stats[Stat.Agility]) > accuracy)
            {
                ob.Dodged();
                return;
            }

            bool hasStone = Equipment[(int)EquipmentSlot.Amulet]?.Info.ItemType == ItemType.DarkStone;

            foreach (MagicType type in types)
            {
                if (GetMagic(type, out MagicObject magicObject))
                {
                    power = magicObject.ModifyPowerAdditionner(primary, power, ob, null, extra);
                }
            }

            Element element = Element.None;

            if (!hasMassacre)
            {
                if (!ignoreDefense)
                {
                    var resistance = ob.GetAC();

                    if (types.Contains(MagicType.Karma))
                    {
                        if (GetMagic(MagicType.Resolution, out Resolution resolution))
                        {
                            resistance -= (resistance * resolution.Magic.GetPower() / 100);
                        }
                    }

                    power -= resistance;

                    if (ob.Race == ObjectType.Player)
                        res = ob.Stats.GetResistanceValue(hasStone ? Equipment[(int)EquipmentSlot.Amulet].Info.Stats.GetAffinityElement() : Element.None);
                    else
                        res = ob.Stats.GetResistanceValue(Element.None);

                    if (res > 0)
                        power -= power * res / 10;
                    else if (res < 0)
                        power -= power * res / 5;
                }

                if (power < 0) power = 0;

                for (Element ele = Element.Fire; ele <= Element.Phantom; ele++)
                {
                    if (hasFlameSplash && ele > Element.Fire) break;

                    int value = Stats.GetElementValue(ele);

                    if (hasStone)
                    {
                        value += Equipment[(int)EquipmentSlot.Amulet].Info.Stats.GetAffinityValue(ele);
                        element = ele;
                    }

                    power += value;

                    res = ob.Stats.GetResistanceValue(ele);

                    if (res <= 0)
                        power -= value * res * 3 / 10;
                    else
                        power -= value * res * 2 / 10;
                }

                if (hasStone && (!hasFlameSplash || element == Element.Fire))
                    DamageDarkStone();

                if (hasFlameSplash)
                    element = Element.Fire;
            }
            else
            {
                power = extra;

                if (ob.Race == ObjectType.Player)
                    res = ob.Stats.GetResistanceValue(hasStone ? Equipment[(int)EquipmentSlot.Amulet].Info.Stats.GetAffinityElement() : Element.None);
                else
                    res = ob.Stats.GetResistanceValue(Element.None);

                if (res > 0)
                    power -= power * res / 10;
                else if (res < 0)
                    power -= power * res / 5;
            }

            if (power <= 0)
            {
                ob.Blocked();
                return;
            }

            int damage = 0;

            if (types.Contains(MagicType.BladeStorm) && GetMagic(MagicType.BladeStorm, out BladeStorm _))
            {
                power /= 2;
                ActionList.Add(new DelayedAction(SEnvir.Now.AddMilliseconds(300), ActionType.DelayedAttackDamage, ob, power, element, true, true, ob.Stats[Stat.MagicShield] == 0, true));
            }

            if (types.Contains(MagicType.Karma) && GetMagic(MagicType.Karma, out Karma karma))
            {
                var karmaDamage = ob.CurrentHP * karma.Magic.GetPower() / 100;

                if (ob.Race == ObjectType.Monster)
                {
                    if (((MonsterObject)ob).MonsterInfo.IsBoss)
                        karmaDamage = karma.Magic.GetPower() * 20;
                    else
                        karmaDamage /= 4;
                }

                if (karmaDamage > 0)
                    damage += ob.Attacked(this, karmaDamage, Element.None, false, true, false);
            }

            damage += ob.Attacked(this, power, element, true, false, !hasMassacre);

            if (damage <= 0) return;

            CheckBrown(ob);

            DamageItem(GridType.Equipment, (int)EquipmentSlot.Weapon, SEnvir.Random.Next(2) + 1);

            decimal lifestealAmount = damage * Stats[Stat.LifeSteal] / 100M;

            foreach (MagicType type in types)
            {
                if (GetMagic(type, out MagicObject magicObject))
                {
                    lifestealAmount = magicObject.LifeSteal(primary, lifestealAmount);
                }
            }

            if (primary || Class == MirClass.Warrior || hasFlameSplash)
                LifeSteal += lifestealAmount;

            if (LifeSteal > 1)
            {
                int heal = (int)Math.Floor(LifeSteal);
                LifeSteal -= heal;
                ChangeHP(Math.Min(maxLifeSteal, heal));
            }

            int psnRate = Globals.PhysicalPoisonRate;

            if (ob.Level >= 250)
                psnRate = Globals.PhysicalPoisonRate * 10;

            if (SEnvir.Random.Next(psnRate) < Stats[Stat.ParalysisChance])
            {
                ob.ApplyPoison(new Poison
                {
                    Owner = this,
                    Type = PoisonType.Paralysis,
                    TickFrequency = TimeSpan.FromSeconds(3),
                    TickCount = 1,
                });
            }

            if (ob.Race != ObjectType.Player && SEnvir.Random.Next(psnRate) < Stats[Stat.SlowChance])
            {
                ob.ApplyPoison(new Poison
                {
                    Owner = this,
                    Type = PoisonType.Slow,
                    Value = 20,
                    TickFrequency = TimeSpan.FromSeconds(5),
                    TickCount = 1,
                });
            }

            if (SEnvir.Random.Next(psnRate) < Stats[Stat.SilenceChance])
            {
                ob.ApplyPoison(new Poison
                {
                    Owner = this,
                    Type = PoisonType.Silenced,
                    TickFrequency = TimeSpan.FromSeconds(5),
                    TickCount = 1,
                });
            }

            foreach (var type in MagicObjects.Keys)
            {
                var magicObject = MagicObjects[type];

                if (types.Contains(type))
                {
                    magicObject.AttackComplete(ob);
                }

                magicObject.AttackCompletePassive(ob, types);
            }
        }

        public int MagicAttack(List<MagicType> types, MapObject ob, bool primary = true, Stats stats = null, int extra = 0)
        {
            if (ob?.Node == null || ob.Dead) return 0;

            if (PetMode == PetMode.PvP)
            {
                for (int i = Pets.Count - 1; i >= 0; i--)
                    if (Pets[i].CanAttackTarget(ob))
                        Pets[i].Target = ob;
            }
            else
                for (int i = Pets.Count - 1; i >= 0; i--)
                    if (Pets[i].Target == null)
                        Pets[i].Target = ob;

            int power = 0;
            int slow = 0, slowLevel = 0, repel = 0, silence = 0;
            int shock = 0, burn = 0, burnLevel = 0;

            bool canStruck = true;

            Element element = Element.None;

            foreach (MagicType type in types)
            {
                if (GetMagic(type, out MagicObject magicObject))
                {
                    power = magicObject.ModifyPowerAdditionner(primary, power, ob, stats, extra);

                    slow = magicObject.GetSlow(slow, stats);
                    slowLevel = magicObject.GetSlowLevel(slowLevel, stats);
                    repel = magicObject.GetRepel(repel, stats);
                    silence = magicObject.GetSilence(silence, stats);
                    shock = magicObject.GetShock(shock, stats);
                    burn = magicObject.GetBurn(burn, stats);
                    burnLevel = magicObject.GetBurnLevel(burnLevel, stats);

                    canStruck = magicObject.CanStruck;

                    element = magicObject.GetElement(element);
                }
            }

            foreach (MagicType type in types)
            {
                if (GetMagic(type, out MagicObject magicObject))
                {
                    power = magicObject.ModifyPowerMultiplier(primary, power, stats, extra);
                }
            }

            power -= ob.GetMR();

            switch (element)
            {
                case Element.None:
                    power -= power * ob.Stats[Stat.PhysicalResistance] / 10;
                    break;
                case Element.Fire:
                    power += GetElementPower(ob.Race, Stat.FireAttack) * 2;
                    power -= power * ob.Stats[Stat.FireResistance] / 10;
                    break;
                case Element.Ice:
                    power += GetElementPower(ob.Race, Stat.IceAttack) * 2;
                    power -= power * ob.Stats[Stat.IceResistance] / 10;
                    break;
                case Element.Lightning:
                    power += GetElementPower(ob.Race, Stat.LightningAttack) * 2;
                    power -= power * ob.Stats[Stat.LightningResistance] / 10;
                    break;
                case Element.Wind:
                    power += GetElementPower(ob.Race, Stat.WindAttack) * 2;
                    power -= power * ob.Stats[Stat.WindResistance] / 10;
                    break;
                case Element.Holy:
                    power += GetElementPower(ob.Race, Stat.HolyAttack) * 2;
                    power -= power * ob.Stats[Stat.HolyResistance] / 10;
                    break;
                case Element.Dark:
                    power += GetElementPower(ob.Race, Stat.DarkAttack) * 2;
                    power -= power * ob.Stats[Stat.DarkResistance] / 10;
                    break;
                case Element.Phantom:
                    power += GetElementPower(ob.Race, Stat.PhantomAttack) * 2;
                    power -= power * ob.Stats[Stat.PhantomResistance] / 10;
                    break;
            }

            if (power <= 0)
            {
                ob.Blocked();
                return 0;
            }

            int damage = ob.Attacked(this, power, element, false, false, true, canStruck);

            if (damage <= 0) return damage;

            if (shock > 0)
            {
                DateTime shockTime = SEnvir.Now.AddSeconds(shock);

                if (shockTime > ob.ShockTime)
                {
                    ob.ShockTime = shockTime;
                }
            }

            if (burn > 0)
            {
                ob.ApplyPoison(new Poison
                {
                    Owner = this,
                    Type = PoisonType.Burn,
                    Value = damage * burnLevel / 10,
                    TickFrequency = TimeSpan.FromSeconds(2),
                    TickCount = burn,
                });
            }

            int psnRate = Globals.MagicalPoisonRate;

            if (ob.Level >= 250)
            {
                psnRate = Globals.MagicalPoisonRate * 10;
            }

            if (SEnvir.Random.Next(psnRate) < Stats[Stat.ParalysisChance])
            {
                ob.ApplyPoison(new Poison
                {
                    Owner = this,
                    Type = PoisonType.Paralysis,
                    TickFrequency = TimeSpan.FromSeconds(2),
                    TickCount = 1,
                });
            }

            if (ob.Race != ObjectType.Player && SEnvir.Random.Next(psnRate) < Stats[Stat.SlowChance])
            {
                ob.ApplyPoison(new Poison
                {
                    Owner = this,
                    Type = PoisonType.Slow,
                    Value = 20,
                    TickFrequency = TimeSpan.FromSeconds(5),
                    TickCount = 1,
                });
            }

            if (SEnvir.Random.Next(psnRate) < Stats[Stat.SilenceChance])
            {
                ob.ApplyPoison(new Poison
                {
                    Owner = this,
                    Type = PoisonType.Silenced,
                    TickFrequency = TimeSpan.FromSeconds(5),
                    TickCount = 1,
                });
            }

            foreach (var type in MagicObjects.Keys)
            {
                var magicObject = MagicObjects[type];

                if (types.Contains(type))
                {
                    magicObject.MagicAttackSuccess(ob, damage);
                }

                magicObject.MagicAttackSuccessPassive(ob, types);
            }

            switch (ob.Race)
            {
                case ObjectType.Player:
                    break;
                case ObjectType.Monster:
                    if (slow > 0 && SEnvir.Random.Next(slow) == 0 && !((MonsterObject)ob).MonsterInfo.IsBoss)
                    {
                        TimeSpan duration = TimeSpan.FromSeconds(3 + SEnvir.Random.Next(3));

                        slowLevel *= 2;
                        duration += duration;

                        ob.ApplyPoison(new Poison
                        {
                            Type = PoisonType.Slow,
                            Value = slowLevel,
                            TickCount = 1,
                            TickFrequency = duration,
                            Owner = this,
                        });
                    }

                    if (repel > 0 && SEnvir.Random.Next(repel) == 0)
                    {
                        if (ob.CurrentMap == CurrentMap && Level > ob.Level)
                        {
                            MirDirection dir = Functions.DirectionFromPoint(CurrentLocation, ob.CurrentLocation);
                            if (ob.Pushed(dir, 1) == 0)
                            {
                                int rotation = SEnvir.Random.Next(2) == 0 ? 1 : -1;

                                for (int i = 1; i < 2; i++)
                                {
                                    if (ob.Pushed(Functions.ShiftDirection(dir, i * rotation), 1) > 0) break;
                                    if (ob.Pushed(Functions.ShiftDirection(dir, i * -rotation), 1) > 0) break;
                                }
                            }
                        }
                    }

                    if (silence > 0 && !((MonsterObject)ob).MonsterInfo.IsBoss)
                    {
                        ob.ApplyPoison(new Poison
                        {
                            Type = PoisonType.Silenced,
                            TickCount = 1,
                            TickFrequency = TimeSpan.FromSeconds(silence),
                            Owner = this,
                        });
                    }

                    break;
            }

            CheckBrown(ob);

            return damage;
        }

        public override int Attacked(MapObject attacker, int power, Element element, bool canReflect = true, bool ignoreShield = false, bool canCrit = true, bool canStruck = true)
        {
            if (attacker?.Node == null || power == 0 || Dead || attacker.CurrentMap != CurrentMap || !Functions.InRange(attacker.CurrentLocation, CurrentLocation, Config.MaxViewRange) || Stats[Stat.Invincibility] > 0) return 0;

            if (element != Element.None)
            {
                if (SEnvir.Random.Next(attacker.Race == ObjectType.Player ? 200 : 100) <= Stats[Stat.EvasionChance])// 4 + magic.Level * 2)
                {
                    if (Buffs.Any(x => x.Type == BuffType.Evasion) && GetMagic(MagicType.Evasion, out Evasion evasion))
                        LevelMagic(evasion.Magic);

                    DisplayMiss = true;
                    return 0;
                }

                if (GetMagic(MagicType.MagicImmunity, out MagicImmunity magicImmunity))
                {
                    power -= power * magicImmunity.Magic.GetPower() / 100;

                    if (power <= 0)
                    {
                        DisplayMiss = true;
                        return 0;
                    }

                    LevelMagic(magicImmunity.Magic);
                }
            }
            else
            {
                if (SEnvir.Random.Next(attacker.Race == ObjectType.Player ? 200 : 100) <= Stats[Stat.BlockChance])
                {
                    DisplayMiss = true;
                    return 0;
                }

                if (GetMagic(MagicType.PhysicalImmunity, out PhysicalImmunity physicalImmunity))
                {
                    power -= power * physicalImmunity.Magic.GetPower() / 100;

                    if (power <= 0)
                    {
                        DisplayMiss = true;
                        return 0;
                    }

                    LevelMagic(physicalImmunity.Magic);
                }
            }

            CombatTime = SEnvir.Now;

            if (attacker.Race == ObjectType.Player)
            {
                PvPTime = SEnvir.Now;
                ((PlayerObject)attacker).PvPTime = SEnvir.Now;
            }

            if (Stats[Stat.Comfort] < 20)
                RegenTime = SEnvir.Now + RegenDelay;

            if ((Poison & PoisonType.Red) == PoisonType.Red)
                power = (int)(power * 1.2F);

            for (int i = 0; i < attacker.Stats[Stat.Rebirth]; i++)
                power = (int)(power * 1.2F);

            if (SEnvir.Random.Next(100) < attacker.Stats[Stat.CriticalChance] && canCrit)
            {
                if (!canReflect)
                    power = (int)(power * 1.2F);
                else if (attacker.Race == ObjectType.Player)
                    power = (int)(power * 1.3F);
                else
                    power += power;

                Critical();
            }

            int psnRate = Globals.PhysicalPoisonRate;

            if (attacker.Level >= 250)
                psnRate = Globals.PhysicalPoisonRate * 10;

            BuffInfo buff = Buffs.FirstOrDefault(x => x.Type == BuffType.FrostBite);

            if (buff != null)
            {
                buff.Stats[Stat.FrostBiteDamage] += power;
                Enqueue(new S.BuffChanged() { Index = buff.Index, Stats = new Stats(buff.Stats) });

                if (SEnvir.Random.Next(psnRate) < buff.Stats[Stat.FrostBiteChance])
                {
                    attacker.ApplyPoison(new Poison
                    {
                        Type = PoisonType.Slow,
                        TickCount = 5,
                        Owner = this,
                        TickFrequency = TimeSpan.FromSeconds(2),
                    });
                }
            }

            if (attacker.Race == ObjectType.Monster && SEnvir.Now < FrostBiteImmunity) return 0;

            if (!ignoreShield)
            {
                if (Buffs.Any(x => x.Type == BuffType.Cloak))
                    power -= power / 2;

                buff = Buffs.FirstOrDefault(x => x.Type == BuffType.MagicShield);

                if (buff != null)
                {
                    buff.RemainingTime -= TimeSpan.FromMilliseconds(power * 25);
                    Enqueue(new S.BuffTime { Index = buff.Index, Time = buff.RemainingTime });
                }

                power -= power * Stats[Stat.MagicShield] / 100;
            }

            if (StruckTime != DateTime.MaxValue && SEnvir.Now > StruckTime.AddMilliseconds(500) && canStruck)
            {
                bool ignore = false;

                if (Buffs.Any(x => x.Type == BuffType.ElementalHurricane))
                {
                    BuffRemove(BuffType.ElementalHurricane);
                }

                if (Buffs.Any(x => x.Type == BuffType.DragonRepulse))
                {
                    ignore = true;
                }

                if (!ignore)
                {
                    StruckTime = SEnvir.Now;

                    if (!Buffs.Any(x => x.Type == BuffType.Dash))
                    {
                        if (Config.EnableStruck)
                        {
                            if (StruckTime.AddMilliseconds(300) > ActionTime) ActionTime = StruckTime.AddMilliseconds(300);
                        }

                        Broadcast(new S.ObjectStruck { ObjectID = ObjectID, Direction = Direction, Location = CurrentLocation, AttackerID = attacker.ObjectID, Element = element });
                    }

                    bool update = false;
                    for (int i = 0; i < Equipment.Length; i++)
                    {
                        switch ((EquipmentSlot)i)
                        {
                            case EquipmentSlot.Amulet:
                            case EquipmentSlot.Poison:
                            case EquipmentSlot.Torch:
                                continue;
                        }

                        update = DamageItem(GridType.Equipment, i, SEnvir.Random.Next(2) + 1, true) || update;
                    }

                    if (update)
                    {
                        SendShapeUpdate();
                        RefreshStats();
                    }
                }
            }

            #region Conquest Stats

            UserConquestStats conquest = SEnvir.GetConquestStats(this);

            if (conquest != null)
            {
                switch (attacker.Race)
                {
                    case ObjectType.Player:
                        conquest.PvPDamageTaken += power;

                        conquest = SEnvir.GetConquestStats((PlayerObject)attacker);

                        if (conquest != null)
                            conquest.PvPDamageDealt += power;

                        break;
                    case ObjectType.Monster:
                        MonsterObject mob = (MonsterObject)attacker;

                        if (mob is CastleLord)
                            conquest.BossDamageTaken += power;
                        else if (mob.PetOwner != null)
                        {
                            conquest.PvPDamageTaken += power;

                            conquest = SEnvir.GetConquestStats(mob.PetOwner);

                            if (conquest != null)
                                conquest.PvPDamageDealt += power;
                        }
                        break;
                }
            }

            #endregion

            LastHitter = attacker;

            if (!ignoreShield && Buffs.Any(x => x.Type == BuffType.SuperiorMagicShield))
            {
                buff = Buffs.FirstOrDefault(x => x.Type == BuffType.SuperiorMagicShield);

                if (buff != null)
                {
                    buff.Stats[Stat.SuperiorMagicShield] -= power;
                    if (buff.Stats[Stat.SuperiorMagicShield] <= 0)
                        BuffRemove(buff);
                    else
                        Enqueue(new S.BuffChanged() { Index = buff.Index, Stats = new Stats(buff.Stats) });
                }
            }
            else
                ChangeHP(-power);

            if (attacker is MonsterObject monsterAttacker)
                LogMilestone(MilestoneType.MonsterDamageTake, power, monster: monsterAttacker.MonsterInfo);
            else if (attacker is PlayerObject playerAttacker)
            {
                LogMilestone(MilestoneType.PlayerDamageTake, power, player: playerAttacker.Character);
                playerAttacker.LogMilestone(MilestoneType.PlayerDamageDone, power, player: Character);
            }

            LastHitter = null;

            if (canReflect && CanAttackTarget(attacker) && attacker.Race != ObjectType.Player)
            {
                attacker.Attacked(this, power * Stats[Stat.ReflectDamage] / 100, Element.None, false);

                if (Buffs.Any(x => x.Type == BuffType.ReflectDamage) && GetMagic(MagicType.ReflectDamage, out ReflectDamage reflectDamage))
                    LevelMagic(reflectDamage.Magic);
            }

            if (canReflect && CanAttackTarget(attacker) && SEnvir.Random.Next(100) < Stats[Stat.JudgementOfHeaven] && !(attacker is CastleLord))
            {
                int damagePvE = GetMC() / 5 + GetElementPower(ObjectType.Monster, Stat.LightningAttack) * 2;
                int damagePvP = Math.Min(50, GetMC() / 5 + GetElementPower(ObjectType.Monster, Stat.LightningAttack) / 2);

                Broadcast(new S.ObjectEffect { ObjectID = attacker.ObjectID, Effect = Effect.ThunderBolt });
                ActionList.Add(new DelayedAction(SEnvir.Now.AddMilliseconds(300), ActionType.DelayedAttackDamage, attacker, attacker.Race == ObjectType.Player ? damagePvP : damagePvE, Element.Lightning, false, false, true, true));

                if (Buffs.Any(x => x.Type == BuffType.JudgementOfHeaven) && GetMagic(MagicType.JudgementOfHeaven, out JudgementOfHeaven judgementOfHeaven))
                    LevelMagic(judgementOfHeaven.Magic);
            }

            if (Buffs.Any(x => x.Type == BuffType.Defiance) && GetMagic(MagicType.Defiance, out Defiance defiance))
                LevelMagic(defiance.Magic);

            if (GetMagic(MagicType.DefensiveMastery, out DefensiveMastery defensiveMastery))
                LevelMagic(defensiveMastery.Magic);

            if (Buffs.Any(x => x.Type == BuffType.RagingWind) && GetMagic(MagicType.RagingWind, out RagingWind ragingWind))
                LevelMagic(ragingWind.Magic);

            if (GetMagic(MagicType.AdventOfDemon, out AdventOfDemon adventOfDemon) && element == Element.None)
                LevelMagic(adventOfDemon.Magic);

            if (GetMagic(MagicType.AdventOfDevil, out AdventOfDevil adventOfDevil) && element != Element.None)
                LevelMagic(adventOfDevil.Magic);

            if (Buffs.Any(x => x.Type == BuffType.Invincibility) && GetMagic(MagicType.Invincibility, out Invincibility invincibility))
                LevelMagic(invincibility.Magic);

            return power;
        }

        public override bool CanAttackTarget(MapObject ob)
        {
            if (ob?.Node == null || ob == this || ob.Dead || !ob.Visible || ob is Guard) return false;

            switch (ob.Race)
            {
                case ObjectType.Monster:
                    MonsterObject mob = (MonsterObject)ob;

                    if (mob.PetOwner == null) return true; //Wild Monster

                    // Player vs Pet
                    if (mob.PetOwner == this)
                        return AttackMode == AttackMode.All || mob is Puppet;

                    if (mob is Puppet) return false; //Don't hit other person's puppet

                    if (mob.InSafeZone || InSafeZone) return false;

                    switch (AttackMode)
                    {
                        case AttackMode.Peace:
                            return false;
                        case AttackMode.Group:
                            if (InGroup(mob.PetOwner))
                                return false;
                            break;
                        case AttackMode.Guild:
                            if (InGuild(mob.PetOwner))
                                return false;
                            break;
                        case AttackMode.WarRedBrown:
                            if (mob.PetOwner.Stats[Stat.Brown] == 0 && mob.PetOwner.Stats[Stat.PKPoint] < Config.RedPoint && !AtWar(mob.PetOwner))
                                return false;
                            break;
                    }

                    return true;
                case ObjectType.Player:
                    PlayerObject player = (PlayerObject)ob;

                    if (player.GameMaster) return false;

                    if (InSafeZone || player.InSafeZone) return false; //Login Time?

                    switch (AttackMode)
                    {
                        case AttackMode.Peace:
                            return false;
                        case AttackMode.Group:
                            if (InGroup(player))
                                return false;
                            break;
                        case AttackMode.Guild:
                            if (InGuild(player))
                                return false;
                            break;
                        case AttackMode.WarRedBrown:
                            if (player.Stats[Stat.Brown] == 0 && player.Stats[Stat.PKPoint] < Config.RedPoint && !AtWar(player))
                                return false;
                            break;
                    }

                    return true;
                default:
                    return false;
            }
        }
        public override bool CanHelpTarget(MapObject ob)
        {
            if (ob?.Node == null || ob.Dead || !ob.Visible || ob is Guard || ob is CastleLord) return false;
            if (ob == this) return true;

            switch (ob.Race)
            {
                case ObjectType.Player:
                    PlayerObject player = (PlayerObject)ob;

                    switch (AttackMode)
                    {
                        case AttackMode.Peace:
                            return true;

                        case AttackMode.Group:
                            if (InGroup(player))
                                return true;
                            break;

                        case AttackMode.Guild:
                            if (InGuild(player))
                                return true;
                            break;

                        case AttackMode.WarRedBrown:
                            if (player.Stats[Stat.Brown] == 0 && player.Stats[Stat.PKPoint] < Config.RedPoint && !AtWar(player))
                                return true;
                            break;
                    }

                    return false;

                case ObjectType.Monster:
                    MonsterObject mob = (MonsterObject)ob;

                    if (mob.PetOwner == this) return true;
                    if (mob.PetOwner == null) return false;

                    switch (AttackMode)
                    {
                        case AttackMode.Peace:
                            return true;

                        case AttackMode.Group:
                            if (InGroup(mob.PetOwner))
                                return true;
                            break;

                        case AttackMode.Guild:
                            if (InGuild(mob.PetOwner))
                                return true;
                            break;

                        case AttackMode.WarRedBrown:
                            if (mob.PetOwner.Stats[Stat.Brown] == 0 && mob.PetOwner.Stats[Stat.PKPoint] < Config.RedPoint && !AtWar(mob.PetOwner))
                                return true;
                            break;
                    }

                    return false;

                default:
                    return false;
            }
        }

        public bool GetMagic<T>(MagicType type, out T magic) where T : MagicObject
        {
            var hasMagic = MagicObjects.TryGetValue(type, out var retrievedMagic);

            if (hasMagic && retrievedMagic.CanUseMagic())
            {
                magic = (T)retrievedMagic;
                return true;
            }

            magic = null;
            return false;
        }

        public void LevelMagic(UserMagic magic)
        {
            if (magic == null) return;

            int experience = SEnvir.Random.Next(Config.SkillExp) + 1;

            experience *= Stats[Stat.SkillRate];

            int maxExperience;
            switch (magic.Level)
            {
                case 0:
                    if (Level < magic.Info.NeedLevel1) return;

                    maxExperience = magic.Info.Experience1;
                    break;
                case 1:
                    if (Level < magic.Info.NeedLevel2) return;

                    maxExperience = magic.Info.Experience2;
                    break;
                case 2:
                    if (Level < magic.Info.NeedLevel3) return;

                    maxExperience = magic.Info.Experience3;
                    break;
                default:
                    return;
            }

            magic.Experience += experience;

            if (magic.Experience >= maxExperience)
            {
                magic.Experience -= maxExperience;
                magic.Level++;
                RefreshStats();

                LogMilestone(MilestoneType.SkillLevel, magic.Level, true, magic: magic.Info);

                for (int i = Pets.Count - 1; i >= 0; i--)
                    Pets[i].RefreshStats();
            }

            Enqueue(new S.MagicLeveled { InfoIndex = magic.Info.Index, Level = magic.Level, Experience = magic.Experience });
        }

        public override int Pushed(MirDirection direction, int distance)
        {
            if (Buffs.Any(x => x.Type == BuffType.Endurance))
            {
                if (GetMagic(MagicType.Endurance, out Endurance endurance))
                    LevelMagic(endurance.Magic);

                return 0;
            }

            RemoveMount();

            return base.Pushed(direction, distance);
        }

        public override bool ApplyPoison(Poison p)
        {
            if (p.Owner != null && p.Owner.Race == ObjectType.Player)
            {
                PvPTime = SEnvir.Now;
                ((PlayerObject)p.Owner).PvPTime = SEnvir.Now;
            }

            if (Buffs.Any(x => x.Type == BuffType.Endurance))
            {
                if (GetMagic(MagicType.Endurance, out Endurance endurance))
                    LevelMagic(endurance.Magic);

                return false;
            }

            bool res = base.ApplyPoison(p);

            if (res)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.Poisoned, MessageType.System);

                if (p.Owner != null && p.Owner.Race == ObjectType.Player)
                    ((PlayerObject)p.Owner).CheckBrown(this);
            }

            return res;
        }

        public override void Die()
        {
            RevivalTime = SEnvir.Now + Config.AutoReviveDelay;

            RemoveMount();

            TradeClose();

            HashSet<MonsterObject> clearList = new HashSet<MonsterObject>(TaggedMonsters);

            foreach (MonsterObject ob in clearList)
                ob.EXPOwner = null;

            TaggedMonsters.Clear();

            base.Die();

            for (int i = SpellList.Count - 1; i >= 0; i--)
                SpellList[i].Despawn();

            for (int i = Pets.Count - 1; i >= 0; i--)
                Pets[i].Die();

            Pets.Clear();

            if (Buffs.Any(x => x.Type == BuffType.SoulResonance))
                SoulResonance.Activate(this);

            LogMilestone(MilestoneType.Die, 1);

            SEnvir.EventHandler.Process(this, "PLAYERDIE");

            #region Conquest Stats

            UserConquestStats conquest = SEnvir.GetConquestStats(this);

            if (conquest != null && LastHitter != null)
            {
                switch (LastHitter.Race)
                {
                    case ObjectType.Player:
                        conquest.PvPDeathCount++;

                        conquest = SEnvir.GetConquestStats((PlayerObject)LastHitter);

                        if (conquest != null)
                            conquest.PvPKillCount++;
                        break;
                    case ObjectType.Monster:
                        MonsterObject mob = (MonsterObject)LastHitter;

                        if (mob is CastleLord)
                            conquest.BossDeathCount++;
                        else if (mob.PetOwner != null)
                        {
                            conquest.PvPDeathCount++;

                            conquest = SEnvir.GetConquestStats(mob.PetOwner);
                            if (conquest != null)
                                conquest.PvPKillCount++;
                        }
                        break;
                }
            }

            #endregion

            switch (CurrentMap.Info.Fight)
            {
                case FightSetting.Safe:
                case FightSetting.Fight:
                    return;
            }

            if (InSafeZone) return;

            PlayerObject attacker = null;

            if (LastHitter != null)
            {
                switch (LastHitter.Race)
                {
                    case ObjectType.Player:
                        var playerAttacker = (PlayerObject)LastHitter;
                        attacker = playerAttacker;
                        LogMilestone(MilestoneType.PlayerDeath, 1, player: playerAttacker.Character);
                        attacker.LogMilestone(MilestoneType.PlayerKill, 1, player: Character);
                        break;
                    case ObjectType.Monster:
                        var monsterAttacker = (MonsterObject)LastHitter;
                        attacker = monsterAttacker.PetOwner;
                        LogMilestone(MilestoneType.MonsterDeath, 1, monster: monsterAttacker.MonsterInfo);
                        attacker?.LogMilestone(MilestoneType.PlayerPetKill, 1, player: Character);
                        break;
                }
            }

            if (Stats[Stat.Rebirth] > 0 && (LastHitter == null || LastHitter.Race != ObjectType.Player))
            {
                //Level = Math.Max(Level - Stats[Stat.Rebirth] * 3, 1);
                decimal expbonus = Experience;
                Enqueue(new S.GainedExperience { Amount = -expbonus });
                Experience = 0;

                if (expbonus > 0)
                {
                    List<PlayerObject> targets = new List<PlayerObject>();

                    foreach (PlayerObject player in SEnvir.Players)
                    {
                        if (player.Character.Rebirth > 0 || player.Character.Level >= 86) continue;

                        targets.Add(player);
                    }

                    PlayerObject target = null;
                    if (targets.Count > 0)
                    {
                        target = targets[SEnvir.Random.Next(targets.Count)];

                        target.GainExperience(expbonus, false, int.MaxValue, false);
                    }

                    SEnvir.Broadcast(new S.Chat { Text = $"{Name} has died and lost {expbonus:##,##0} Experience, {target?.Name ?? "No one"} has won the experience.", Type = MessageType.System });
                }

                // Enqueue(new S.LevelChanged { Level = Level, Experience = Experience });
                // Broadcast(new S.ObjectLeveled { ObjectID = ObjectID });
            }

            BuffInfo buff;
            int rate;
            TimeSpan time;

            if (attacker != null)
            {
                if (AtWar(attacker))
                {
                    foreach (GuildMemberInfo member in Character.Account.GuildMember.Guild.Members)
                    {
                        if (member.Account.Connection == null) continue;

                        member.Account.Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildWarDeath, Name, Character.Account.GuildMember.Guild.GuildName, attacker.Name, attacker.Character.Account.GuildMember.Guild.GuildName), MessageType.System);
                    }
                    foreach (GuildMemberInfo member in attacker.Character.Account.GuildMember.Guild.Members)
                    {
                        if (member.Account.Connection == null) continue;

                        member.Account.Connection.ReceiveChatWithObservers(con => string.Format(con.Language.GuildWarDeath, Name, Character.Account.GuildMember.Guild.GuildName, attacker.Name, attacker.Character.Account.GuildMember.Guild.GuildName), MessageType.System);
                    }
                }
                else
                {
                    if (Stats[Stat.PKPoint] < Config.RedPoint && Stats[Stat.Brown] == 0)
                    {
                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.MurderedBy, attacker.Name), MessageType.System);

                        //PvP death

                        if (attacker.Stats[Stat.PKPoint] >= Config.RedPoint && SEnvir.Random.Next(Config.PvPCurseRate) == 0)
                        {
                            rate = -1;
                            time = Config.PvPCurseDuration;
                            buff = Buffs.FirstOrDefault(x => x.Type == BuffType.PvPCurse);

                            if (buff != null)
                            {
                                rate += buff.Stats[Stat.Luck];
                                time += buff.RemainingTime;
                            }

                            attacker.BuffAdd(BuffType.PvPCurse, time, new Stats { [Stat.Luck] = rate }, false, false, TimeSpan.Zero);

                            attacker.Connection.ReceiveChatWithObservers(con => string.Format(con.Language.Curse, Name), MessageType.System);
                        }
                        else
                        {
                            attacker.Connection.ReceiveChatWithObservers(con => string.Format(con.Language.Murdered, Name), MessageType.System);
                        }

                        attacker.IncreasePKPoints(Config.PKPointRate);
                    }
                    else
                    {
                        attacker.Connection.ReceiveChatWithObservers(con => con.Language.Protected, MessageType.System);

                        Connection.ReceiveChatWithObservers(con => string.Format(con.Language.Killed, attacker.Name), MessageType.System);
                    }
                }
            }
            else
            {
                if (Stats[Stat.PKPoint] >= Config.RedPoint)
                {
                    bool update = false;
                    for (int i = 0; i < Equipment.Length; i++)
                    {
                        UserItem item = Equipment[i];
                        if (item == null) continue;

                        update = DamageItem(GridType.Equipment, i, item.Info.Durability / 10) || update;
                    }

                    if (update)
                    {
                        SendShapeUpdate();
                        RefreshStats();
                    }
                }

                Connection.ReceiveChatWithObservers(con => con.Language.Died, MessageType.System);
            }

            if (Stats[Stat.DeathDrops] > 0)
                DeathDrop();
        }

        public void DeathDrop()
        {
            for (int i = 0; i < Inventory.Length; i++)
            {
                UserItem item = Inventory[i];

                if (item == null) continue;
                if (!item.Info.CanDeathDrop) continue;
                if ((item.Flags & UserItemFlags.Bound) == UserItemFlags.Bound) continue;
                if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) continue;
                if ((item.Flags & UserItemFlags.Worthless) == UserItemFlags.Worthless) continue;
                if (SEnvir.Random.Next(10) > 0) continue;

                Cell cell = GetDropLocation(4, null);

                if (cell == null) break;

                long count;

                count = 1 + SEnvir.Random.Next((int)item.Count);

                UserItem dropItem;
                if (count == item.Count)
                {
                    dropItem = item;
                    RemoveItem(item);
                    Inventory[i] = null;
                    count = 0;
                }
                else
                {
                    dropItem = SEnvir.CreateFreshItem(item);
                    dropItem.Count = count;
                    item.Count -= count;

                    count = item.Count;
                }

                Companion?.RefreshWeight();
                dropItem.IsTemporary = true;

                ItemObject ob = new ItemObject
                {
                    Item = dropItem,
                };

                ob.Spawn(CurrentMap, cell.Location);

                Enqueue(new S.ItemChanged { Link = new CellLinkInfo { GridType = GridType.Inventory, Slot = i, Count = count }, Success = true });
            }


            if (Companion != null)
            {
                for (int i = 0; i < Companion.Inventory.Length; i++)
                {
                    UserItem item = Companion.Inventory[i];

                    if (item == null) continue;
                    if (!item.Info.CanDeathDrop) continue;
                    if ((item.Flags & UserItemFlags.Bound) == UserItemFlags.Bound) continue;
                    if ((item.Flags & UserItemFlags.Worthless) == UserItemFlags.Worthless) continue;
                    if (SEnvir.Random.Next(7) > 0) continue;

                    Cell cell = GetDropLocation(4, null);

                    if (cell == null) break;

                    long count;

                    count = 1 + SEnvir.Random.Next((int)item.Count);

                    UserItem dropItem;
                    if (count == item.Count)
                    {
                        dropItem = item;
                        RemoveItem(item);
                        Companion.Inventory[i] = null;
                        count = 0;
                    }
                    else
                    {
                        dropItem = SEnvir.CreateFreshItem(item);
                        dropItem.Count = count;
                        item.Count -= count;

                        count = item.Count;
                    }

                    Companion?.RefreshWeight();
                    dropItem.IsTemporary = true;

                    ItemObject ob = new ItemObject
                    {
                        Item = dropItem,
                    };

                    ob.Spawn(CurrentMap, cell.Location);

                    Enqueue(new S.ItemChanged { Link = new CellLinkInfo { GridType = GridType.CompanionInventory, Slot = i, Count = count }, Success = true });
                }
            }

            bool botter = Character.Account.ItemBot || Character.Account.GoldBot;

            if (SEnvir.Random.Next((botter ? 10 : 100)) == 0)
            {
                List<int> dropList = new List<int>();

                for (int i = 0; i < Equipment.Length; i++)
                {
                    UserItem item = Equipment[i];

                    if (item == null) continue;
                    if (!item.Info.CanDeathDrop) continue;
                    if ((item.Flags & UserItemFlags.Bound) == UserItemFlags.Bound) continue;
                    if ((item.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage) continue;
                    if ((item.Flags & UserItemFlags.Worthless) == UserItemFlags.Worthless) continue;


                    dropList.Add(i);

                    if (botter && dropList.Count > 0) break;

                }

                if (dropList.Count > 0)
                {
                    int index = dropList[SEnvir.Random.Next(dropList.Count)];

                    UserItem item = Equipment[index];

                    Cell cell = GetDropLocation(4, null);

                    if (cell != null)
                    {
                        UserItem dropItem;

                        dropItem = item;
                        RemoveItem(item);
                        Equipment[index] = null;

                        dropItem.IsTemporary = true;

                        ItemObject ob = new ItemObject
                        {
                            Item = dropItem,
                        };

                        ob.Spawn(CurrentMap, cell.Location);

                        Enqueue(new S.ItemChanged { Link = new CellLinkInfo { GridType = GridType.Equipment, Slot = index, Count = 0 }, Success = true });

                    }
                }
            }

            RefreshWeight();
            RefreshStats();
        }

        public bool InGuild(PlayerObject player)
        {
            if (player == null) return false;

            if (Character.Account.GuildMember == null || player.Character.Account.GuildMember == null) return false;

            return Character.Account.GuildMember.Guild == player.Character.Account.GuildMember.Guild;
        }

        public void CheckBrown(MapObject ob)
        {
            //if in fight map
            PlayerObject player;
            switch (ob.Race)
            {
                case ObjectType.Player:
                    player = (PlayerObject)ob;
                    break;
                case ObjectType.Monster:
                    player = ((MonsterObject)ob).PetOwner;
                    break;
                default:
                    return;
            }

            if (player == null || player == this) return;

            switch (CurrentMap.Info.Fight)
            {
                case FightSetting.Safe:
                case FightSetting.Fight:
                    return;
            }


            if (InSafeZone || player.InSafeZone) return;

            switch (player.CurrentMap.Info.Fight)
            {
                case FightSetting.Safe:
                case FightSetting.Fight:
                    return;
            }

            if (player.Stats[Stat.Brown] > 0 || player.Stats[Stat.PKPoint] >= Config.RedPoint) return;

            if (AtWar(player)) return;

            BuffAdd(BuffType.Brown, Config.BrownDuration, new Stats { [Stat.Brown] = 1 }, false, false, TimeSpan.Zero);
        }

        public void IncreasePKPoints(int count)
        {
            BuffInfo buff = Buffs.FirstOrDefault(x => x.Type == BuffType.PKPoint);

            if (buff != null)
                count += buff.Stats[Stat.PKPoint];

            if (count >= Config.RedPoint && !Character.BindPoint.RedZone)
            {
                SafeZoneInfo info = SEnvir.SafeZoneInfoList.Binding.FirstOrDefault(x => x.RedZone && x.ValidBindPoints.Count > 0);

                if (info != null)
                    Character.BindPoint = info;
            }

            LogMilestone(MilestoneType.PKPoint, count, true);

            BuffAdd(BuffType.PKPoint, TimeSpan.MaxValue, new Stats { [Stat.PKPoint] = count }, false, false, Config.PKPointTickRate);
        }

        public int GetLotusMana(ObjectType race)
        {
            if (race != ObjectType.Player) return Stats[Stat.Mana];

            int min = 0;
            int max = Stats[Stat.Mana];

            int luck = Stats[Stat.Luck];

            if (min < 0) min = 0;
            if (min >= max) return max;

            if (luck > 0)
            {
                if (luck >= 10) return max;

                if (SEnvir.Random.Next(10) < luck) return max;
            }
            else if (luck < 0)
            {
                if (luck < -SEnvir.Random.Next(10)) return min;
            }

            return SEnvir.Random.Next(min, max + 1);
        }
        public int GetElementPower(ObjectType race, Stat element)
        {
            if (race != ObjectType.Player) return Stats[element];

            int min = 0;
            int max = Stats[element];

            int luck = Stats[Stat.Luck];

            if (min < 0) min = 0;
            if (min >= max) return max;

            if (luck > 0)
            {
                if (luck >= 10) return max;

                if (SEnvir.Random.Next(10) < luck) return max;
            }
            else if (luck < 0)
            {
                if (luck < -SEnvir.Random.Next(10)) return min;
            }

            return SEnvir.Random.Next(min, max + 1);
        }
        #endregion
    }
}
