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
        #region Packet Actions
        public void Turn(MirDirection direction)
        {
            if (SEnvir.Now < ActionTime || SEnvir.Now < MoveTime)
            {
                if (!PacketWaiting)
                {
                    ActionList.Add(new DelayedAction(ActionTime, ActionType.Turn, direction));
                    PacketWaiting = true;
                }
                else
                    Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });

                return;
            }

            if (!CanMove)
            {
                Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                return;
            }

            if (direction != Direction)
                TradeClose();

            Direction = direction;


            ActionTime = SEnvir.Now + Globals.TurnTime;

            if ((Poison & PoisonType.Neutralize) == PoisonType.Neutralize)
                ActionTime += Globals.TurnTime;

            Poison poison = PoisonList.FirstOrDefault(x => x.Type == PoisonType.Slow);
            TimeSpan slow = TimeSpan.Zero;
            if (poison != null)
            {
                slow = TimeSpan.FromMilliseconds(poison.Value * 100);
                ActionTime += slow;
            }

            Broadcast(new S.ObjectTurn { ObjectID = ObjectID, Direction = Direction, Location = CurrentLocation, Slow = slow });
        }
        public void Harvest(MirDirection direction)
        {
            if (SEnvir.Now < ActionTime || SEnvir.Now < MoveTime)
            {
                if (!PacketWaiting)
                {
                    ActionList.Add(new DelayedAction(ActionTime, ActionType.Harvest, direction));
                    PacketWaiting = true;
                }
                else
                    Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });

                return;
            }

            if (!CanMove || Horse != HorseType.None)
            {
                Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                return;
            }

            Direction = direction;
            ActionTime = SEnvir.Now + Globals.HarvestTime;

            if ((Poison & PoisonType.Neutralize) == PoisonType.Neutralize)
                ActionTime += Globals.TurnTime;

            Poison poison = PoisonList.FirstOrDefault(x => x.Type == PoisonType.Slow);
            TimeSpan slow = TimeSpan.Zero;
            if (poison != null)
            {
                slow = TimeSpan.FromMilliseconds(poison.Value * 100);
                ActionTime += slow;
            }

            Broadcast(new S.ObjectHarvest { ObjectID = ObjectID, Direction = Direction, Location = CurrentLocation, Slow = slow });

            Point front = Functions.Move(CurrentLocation, Direction, 1);
            int range = Stats[Stat.PickUpRadius];
            bool send = false;

            for (int d = 0; d <= range; d++)
            {
                for (int y = front.Y - d; y <= front.Y + d; y++)
                {
                    if (y < 0) continue;
                    if (y >= CurrentMap.Height) break;

                    for (int x = front.X - d; x <= front.X + d; x += Math.Abs(y - front.Y) == d ? 1 : d * 2)
                    {
                        if (x < 0) continue;
                        if (x >= CurrentMap.Width) break;

                        Cell cell = CurrentMap.Cells[x, y]; //Direct Access we've checked the boudaries.

                        if (cell?.Objects == null) continue;

                        foreach (MapObject cellObject in cell.Objects)
                        {
                            if (cellObject.Race != ObjectType.Monster) continue;

                            MonsterObject ob = (MonsterObject)cellObject;

                            if (ob.Drops == null) continue;

                            List<UserItem> items;

                            if (!ob.Drops.TryGetValue(Character.Account, out items))
                            {
                                send = true;
                                continue;
                            }

                            if (ob.HarvestCount > 0)
                            {
                                ob.HarvestCount--;
                                continue;
                            }

                            LogMilestone(MilestoneType.Harvest, 1);

                            if (items != null)
                            {
                                for (int i = items.Count - 1; i >= 0; i--)
                                {
                                    UserItem item = items[i];
                                    if (item.UserTask == null) continue;

                                    if (!item.UserTask.Completed &&
                                        ((item.UserTask.Quest.Character != null && item.UserTask.Quest.Character == Character) ||
                                        (item.UserTask.Quest.Account != null && item.UserTask.Quest.Account == Character.Account))) continue;

                                    items.Remove(item);
                                    item.Delete();
                                }

                                if (items.Count == 0) items = null;
                            }

                            if (items == null)
                            {
                                ob.Drops.Remove(Character.Account);

                                if (ob.Drops.Count == 0) ob.Drops = null;
                                ob.HarvestChanged();
                                Connection.ReceiveChatWithObservers(con => con.Language.HarvestNothing, MessageType.System);
                                continue;
                            }

                            for (int i = items.Count - 1; i >= 0; i--)
                            {
                                UserItem item = items[i];

                                ItemCheck check = new ItemCheck(item, item.Count, item.Flags, item.ExpireTime);

                                if (!CanGainItems(false, check)) continue;

                                GainItem(item);
                                items.Remove(item);
                            }

                            if (items.Count == 0)
                            {
                                ob.Drops.Remove(Character.Account);

                                if (ob.Drops.Count == 0) ob.Drops = null;
                                ob.HarvestChanged();

                                continue;
                            }

                            Connection.ReceiveChatWithObservers(con => con.Language.HarvestCarry, MessageType.System);

                            continue;
                        }
                    }
                }
            }

            if (send)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.HarvestOwner, MessageType.System);
            }
        }
        public void Mount()
        {
            if (SEnvir.Now < ActionTime)
            {
                if (!PacketWaiting)
                {
                    ActionList.Add(new DelayedAction(ActionTime, ActionType.Mount));
                    PacketWaiting = true;
                }
                else
                    Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });

                return;
            }

            if (Dead)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.HorseDead, MessageType.System);

                Enqueue(new S.MountFailed { Horse = Horse });
                return;
            }

            if (Character.Account.Horse == HorseType.None)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.HorseOwner, MessageType.System);

                Enqueue(new S.MountFailed { Horse = Horse });
                return;
            }

            if (!CurrentMap.Info.CanHorse)
            {
                Connection.ReceiveChatWithObservers(con => con.Language.HorseMap, MessageType.System);

                Enqueue(new S.MountFailed { Horse = Horse });
                return;
            }

            ActionTime = SEnvir.Now + Globals.TurnTime;

            if (Horse == HorseType.None)
                Horse = Character.Account.Horse;
            else
                Horse = HorseType.None;

            BuffRemove(BuffType.Cloak);
            BuffRemove(BuffType.Transparency);

            Broadcast(new S.ObjectMount { ObjectID = ObjectID, Horse = Horse });
        }
        public void FishingCast(FishingState state, MirDirection castDirection, Point floatLocation, bool caught = false)
        {
            if (SEnvir.Now < ActionTime || SEnvir.Now < AttackTime)
            {
                if (!PacketWaiting)
                {
                    ActionList.Add(new DelayedAction(ActionTime, ActionType.Fishing, state, castDirection, floatLocation, caught));
                    PacketWaiting = true;
                }
                else
                    Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });

                return;
            }

            #region Validation Checks

            if (!CanAttack)
            {
                Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                return;
            }

            var throwDistance = Functions.Distance(CurrentLocation, floatLocation);

            if (Functions.FishingZone(SEnvir.FishingInfoList, CurrentMap.Info, CurrentMap.Width, CurrentMap.Height, floatLocation) == null || !Functions.ValidFishingDistance(throwDistance, Stats[Stat.ThrowDistance]))
            {
                Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                return;
            }

            if (Equipment[(int)EquipmentSlot.Weapon]?.Info.ItemEffect != ItemEffect.FishingRod || Equipment[(int)EquipmentSlot.Armour]?.Info.ItemEffect != ItemEffect.FishingRobe)
            {
                return;
            }

            if (Equipment[(int)EquipmentSlot.Weapon].CurrentDurability > 0 || Equipment[(int)EquipmentSlot.Weapon].Info.Durability == 0)
            {
                DamageItem(GridType.Equipment, (int)EquipmentSlot.Weapon, 1);
            }
            else
            {
                state = FishingState.Reel;
            }

            //TODO - Client - Change cursor (hold Alt to change cursor icon on valid cell)

            #endregion

            Direction = castDirection;

            ActionTime = SEnvir.Now + Globals.AttackTime;
            AttackTime = SEnvir.Now.AddMilliseconds(Globals.AttackDelay);

            if (state == FishingState.Cast)
            {
                FishingCastTime = SEnvir.Now.AddMilliseconds(Globals.AttackDelay + 100);

                bool canAutoCast = Stats[Stat.AutoCast] > 0;

                int fishRequiredAccuracy = -1;

                if (!Fishing)
                {
                    #region Calculate Throw Quality (Rod, ThrowDistance Stat)

                    FishThrowQuality = Functions.FishingThrowQuality(throwDistance);

                    #endregion

                    #region Calculate Start Points (Finder, Finder Stat)

                    FishPointsCurrent = SEnvir.Random.Next(Config.FishPointsRequired - 10);
                    FishPointsCurrent += Math.Min(Config.FishPointsRequired, (FishPointsCurrent * Stats[Stat.FinderChance]) / 100);

                    #endregion

                    #region Calculate Accuracy Required (Hook, Flexibility Stat)

                    fishRequiredAccuracy = 10 + Math.Max(0, Math.Min(Stats[Stat.Flexibility], 15));

                    #endregion

                    Fishing = true;
                    FishingDirection = castDirection;
                    FishingLocation = floatLocation;

                    LogMilestone(MilestoneType.FishingCast, 1);

                    PauseBuffs();

                    DamageItem(GridType.Equipment, (int)EquipmentSlot.Hook, 4);
                    DamageItem(GridType.Equipment, (int)EquipmentSlot.Float, 4);
                    DamageItem(GridType.Equipment, (int)EquipmentSlot.Finder, 4);
                    DamageItem(GridType.Equipment, (int)EquipmentSlot.Reel, 4);

                    if (!UseBait(1, 0, out _))
                    {
                        state = FishingState.Reel;

                        Connection.ReceiveChat("Not enough bait.", MessageType.System);
                    }
                }

                if (!FishFound && state == FishingState.Cast)
                {
                    #region Calculate Fish Find Chance (Bait , NibbleChance Stat)

                    FishFound = SEnvir.Random.Next(100) < Math.Max(0, Config.FishNibbleChanceBase + Stats[Stat.NibbleChance]);

                    #endregion
                }

                if (FishFound && state == FishingState.Cast)
                {
                    FishAttempts++;

                    if (caught)
                    {
                        LogMilestone(MilestoneType.FishingCatch, 1);

                        #region Calculate Success Point Increase (Reel, ReelBonus Stat)

                        FishPointsCurrent += Math.Max(Config.FishPointSuccessRewardMin, Math.Min(Config.FishPointSuccessRewardMax, Config.FishPointSuccessRewardMin + Stats[Stat.ReelBonus]));

                        #endregion
                    }
                    else
                    {
                        LogMilestone(MilestoneType.FishingFail, 1);

                        #region Calculate Failure Point Deduction (Float, FloatStrength Stat)

                        FishPointsCurrent -= Math.Max(Config.FishPointFailureRewardMin, Math.Min(Config.FishPointFailureRewardMax, Config.FishPointFailureRewardMax - Stats[Stat.FloatStrength]));

                        #endregion

                        FishFails++;
                    }

                    if (FishPointsCurrent >= Config.FishPointsRequired)
                    {
                        //success
                        state = FishingState.Reel;

                        var perfectCatch = false;

                        if (Config.FishEnablePerfectCatch && FishAttempts > 1 && FishFails == 0)
                        {
                            perfectCatch = true;

                            Connection.ReceiveChat("Perfect Catch!", MessageType.System);
                        }

                        var zone = Functions.FishingZone(SEnvir.FishingInfoList, CurrentMap.Info, CurrentMap.Width, CurrentMap.Height, floatLocation);

                        foreach (FishingDropInfo info in zone.Drops.OrderByDescending(x => x.Chance))
                        {
                            if (info.Item == null) continue;

                            if (info.PerfectCatch && !perfectCatch) continue;

                            if (info.ThrowQuality != 0 && info.ThrowQuality != FishThrowQuality) continue;

                            if (SEnvir.Random.Next(info.Chance) > 0) continue;

                            ItemCheck check = new ItemCheck(info.Item, 1, UserItemFlags.Bound, TimeSpan.Zero);

                            if (!CanGainItems(false, check)) continue;

                            UserItem item = SEnvir.CreateDropItem(check);
                            GainItem(item);
                            break; //One item gained, so stop rewarding any more

                            //TODO - Limit drops by bait type used?
                        }

                        //TODO - Log Attempts taken, perfect catches etc
                    }
                    else if (FishPointsCurrent <= 0)
                    {
                        //fail
                        state = FishingState.Reel;
                    }
                }

                Enqueue(new S.FishingStats
                {
                    CanAutoCast = canAutoCast,
                    CurrentPoints = FishPointsCurrent,

                    ThrowQuality = FishThrowQuality,
                    RequiredPoints = Config.FishPointsRequired,
                    MovementSpeed = 2,
                    RequiredAccuracy = fishRequiredAccuracy
                });
            }

            if (state != FishingState.Cast)
            {
                ResetFishing();
            }

            Broadcast(new S.ObjectFishing
            {
                ObjectID = ObjectID,
                State = state,
                Direction = FishingDirection,
                FloatLocation = FishingLocation,
                FishFound = FishFound
            });
        }

        public bool UseBait(int count, int shape, out Stats stats)
        {
            stats = null;
            UserItem bait = Equipment[(int)EquipmentSlot.Bait];

            if (bait == null || bait.Info.ItemType != ItemType.Bait || bait.Count < count || bait.Info.Shape != shape) return false;

            bait.Count -= count;

            Enqueue(new S.ItemChanged
            {
                Link = new CellLinkInfo { GridType = GridType.Equipment, Slot = (int)EquipmentSlot.Bait, Count = bait.Count },
                Success = true
            });

            stats = new Stats(bait.Info.Stats);

            if (bait.Count != 0) return true;

            RemoveItem(bait);
            Equipment[(int)EquipmentSlot.Bait] = null;
            bait.Delete();

            RefreshStats();
            RefreshWeight();

            return true;
        }

        private void ResetFishing()
        {
            Fishing = false;
            FishFound = false;
            FishThrowQuality = 0;
            FishPointsCurrent = 0;
            FishAttempts = 0;
            FishFails = 0;

            PauseBuffs();
        }

        public void Move(MirDirection direction, int distance)
        {
            if (SEnvir.Now < ActionTime || SEnvir.Now < MoveTime)
            {
                if (!PacketWaiting)
                {
                    ActionList.Add(new DelayedAction(ActionTime, ActionType.Move, direction, distance));
                    PacketWaiting = true;
                }
                else
                    Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });

                return;
            }

            if (!CanMove)
            {
                Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                return;
            }

            if (distance <= 0 || distance > 3)
            {
                Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                return;
            }

            if (distance == 3 && Horse == HorseType.None)
            {
                Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                return;
            }

            Cell cell = null;
            SafeZoneInfo traversedSafeZone = null;

            for (int i = 1; i <= distance; i++)
            {
                cell = CurrentMap.GetCell(Functions.Move(CurrentLocation, direction, i));
                if (cell == null)
                {
                    Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                    return;
                }

                if (cell.IsBlocking(this, true))
                {
                    Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                    return;
                }

                if (CanBindToSafeZone(cell.SafeZone))
                    traversedSafeZone = cell.SafeZone;
            }

            BuffRemove(BuffType.Invisibility);
            BuffRemove(BuffType.Transparency);

            if (distance > 1)
            {
                if (Stats[Stat.Comfort] < 12)
                    RegenTime = SEnvir.Now + RegenDelay;

                if (!GetMagic(MagicType.Stealth, out Stealth stealth) || !stealth.CheckCloak())
                {
                    BuffRemove(BuffType.Cloak);
                }
            }

            if (Horse != HorseType.None)
            {
                LogMilestone(MilestoneType.Ride, distance);
            }
            else
            {
                switch (distance)
                {
                    case 1:
                        LogMilestone(MilestoneType.Walk, 1);
                        break;
                    case 2:
                        LogMilestone(MilestoneType.Run, 2);
                        break;
                }
            }

            Direction = direction;

            ActionTime = SEnvir.Now + Globals.MoveTime;
            MoveTime = SEnvir.Now + Globals.MoveTime;

            var previousCell = CurrentCell;

            PreventSpellCheck = true;
            CurrentCell = cell.GetMovement(this);
            PreventSpellCheck = false;

            UpdateBindPoint(traversedSafeZone);

            RemoveAllObjects();
            AddAllObjects();

            Poison poison = PoisonList.FirstOrDefault(x => x.Type == PoisonType.Slow);
            TimeSpan slow = TimeSpan.Zero;
            if (poison != null)
            {
                slow = TimeSpan.FromMilliseconds(poison.Value * 100);
                ActionTime += slow;
            }

            Broadcast(new S.ObjectMove { ObjectID = ObjectID, Direction = direction, Location = CurrentLocation, Slow = slow, Distance = distance });
            CheckSpellObjects();
        }

        private bool CanBindToSafeZone(SafeZoneInfo safeZone)
        {
            return safeZone != null &&
                   safeZone.ValidBindPoints.Count > 0 &&
                   Stats[Stat.PKPoint] < Config.RedPoint;
        }

        private void UpdateBindPoint(SafeZoneInfo safeZone)
        {
            if (!CanBindToSafeZone(safeZone)) return;

            Character.BindPoint = safeZone;
        }

        public void Attack(MirDirection direction, MagicType attackMagic)
        {
            if (SEnvir.Now < ActionTime || SEnvir.Now < AttackTime)
            {
                if (!PacketWaiting)
                {
                    ActionList.Add(new DelayedAction(ActionTime, ActionType.Attack, direction, attackMagic));
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

            MagicType validMagic = MagicType.None;
            List<MagicType> magics = new List<MagicType>();

            foreach (var key in MagicObjects.OrderedKeys)
            {
                var magicObject = MagicObjects[key];

                if (magicObject.AttackSkill)
                {
                    if (!magicObject.CanUseMagic())
                    {
                        continue;
                    }

                    var response = magicObject.AttackCast(attackMagic);

                    if (response.Cast)
                        validMagic = magicObject.Type;

                    magics.AddRange(response.Magics);
                }
            }

            if (attackMagic != validMagic)
            {
                SEnvir.Log($"[ERROR] {Name} requested Attack Skill '{attackMagic}' but valid magic was '{validMagic}'.");
                Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                return;
            }

            Element element = Functions.GetAttackElement(Stats);

            if (Equipment[(int)EquipmentSlot.Amulet]?.Info.ItemType == ItemType.DarkStone)
            {
                element = Equipment[(int)EquipmentSlot.Amulet].Info.Stats.GetAffinityElement();
            }

            bool attackSuccess = AttackLocation(Functions.Move(CurrentLocation, Direction), magics, true);

            if (GetMagic(attackMagic, out MagicObject attackMagicObject))
            {
                if (attackSuccess)
                    attackMagicObject.AttackLocationSuccess(attackDelay);

                attackMagicObject.SecondaryAttackLocation(magics);
            }

            BuffRemove(BuffType.Transparency);

            if (!GetMagic(MagicType.Stealth, out Stealth stealth) || !stealth.CheckCloak())
            {
                BuffRemove(BuffType.Cloak);
            }

            Broadcast(new S.ObjectAttack { ObjectID = ObjectID, Direction = Direction, Location = CurrentLocation, Slow = slow, AttackMagic = validMagic, AttackElement = element });
        }

        public void Magic(C.Magic p)
        {
            if (!GetMagic(p.Type, out MagicObject magicObject))
            {
                Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                return;
            }

            if (SEnvir.Now < ActionTime || SEnvir.Now < MagicTime || SEnvir.Now < magicObject.Magic.Cooldown)
            {
                if (!PacketWaiting)
                {
                    ActionList.Add(new DelayedAction(ActionTime, ActionType.Magic, p));
                    PacketWaiting = true;
                }
                else
                    Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });

                return;
            }

            if (!CanCast)
            {
                Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                return;
            }

            if (!magicObject.CheckCost())
            {
                Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                return;
            }

            //Combat time

            MapObject ob = VisibleObjects.FirstOrDefault(x => x.ObjectID == p.Target);

            if (ob != null && !Functions.InRange(CurrentLocation, ob.CurrentLocation, Globals.MagicRange))
                ob = null;

            bool cast = true;

            List<uint> targets = new List<uint>();
            List<Point> locations = new List<Point>();

            var element = magicObject.GetElement(Element.None);

            var castObject = magicObject.MagicCast(ob, p.Location, p.Direction);

            if (castObject.Return)
            {
                return;
            }

            cast = castObject.Cast;
            locations = castObject.Locations;
            targets = castObject.Targets;
            ob = castObject.Ob;

            p.Direction = castObject.Direction ?? p.Direction;

            magicObject.MagicConsume();

            magicObject.MagicFinalise();

            magicObject.ResetCombatTime();

            if (cast)
            {
                magicObject.MagicCooldown();
            }

            Direction = ob == null || ob == this ? p.Direction : Functions.DirectionFromPoint(CurrentLocation, ob.CurrentLocation);

            if (Stats[Stat.Comfort] < 15)
                RegenTime = SEnvir.Now + RegenDelay;

            ActionTime = SEnvir.Now + Globals.CastTime;
            MagicTime = SEnvir.Now + Globals.MagicDelay;

            if (BagWeight > Stats[Stat.BagWeight] || (Poison & PoisonType.Neutralize) == PoisonType.Neutralize)
                MagicTime += Globals.MagicDelay;

            Poison poison = PoisonList.FirstOrDefault(x => x.Type == PoisonType.Slow);
            TimeSpan slow = TimeSpan.Zero;

            if (poison != null)
            {
                slow = TimeSpan.FromMilliseconds(poison.Value * 100);
                ActionTime += slow;
            }

            Broadcast(new S.ObjectMagic
            {
                ObjectID = ObjectID,
                Direction = Direction,
                CurrentLocation = CurrentLocation,
                Type = p.Type,
                Targets = targets,
                Locations = locations,
                Cast = cast,
                Slow = slow,
                AttackElement = element
            });
        }

        public void MagicToggle(C.MagicToggle p)
        {
            if (Horse != HorseType.None) return;

            if (!GetMagic(p.Magic, out MagicObject magic))
            {
                Connection.ReceiveChat("Spell Not Implemented", MessageType.System);
                Enqueue(new S.UserLocation { Direction = Direction, Location = CurrentLocation });
                return;
            }

            magic.Toggle(p.CanUse);
        }

        public void Mining(MirDirection direction)
        {
            if (SEnvir.Now < ActionTime || SEnvir.Now < AttackTime)
            {
                if (!PacketWaiting)
                {
                    ActionList.Add(new DelayedAction(ActionTime, ActionType.Mining, direction));
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

            if (BagWeight > Stats[Stat.BagWeight])
                AttackTime += TimeSpan.FromMilliseconds(attackDelay);

            var front = Functions.Move(CurrentLocation, Direction);

            bool result = false;
            if (CurrentMap.Info.CanMine && CurrentMap.GetCell(front) == null)
            {
                UserItem weap = Equipment[(int)EquipmentSlot.Weapon];

                if (weap != null && weap.Info.ItemEffect == ItemEffect.PickAxe && (weap.CurrentDurability > 0 || weap.Info.Durability == 0))
                {
                    DamageItem(GridType.Equipment, (int)EquipmentSlot.Weapon, 4);

                    LogMilestone(MilestoneType.MineCast, 1);

                    foreach (MineInfo info in CurrentMap.Info.Mining)
                    {
                        if (SEnvir.Random.Next(info.Chance) > 0) continue;

                        if (info.Region != null && !info.Region.PointList.Contains(front)) continue;

                        if (info.Quantity == 0) continue;

                        if (info.Quantity > 0 && info.RemainingQuantity == 0)
                        {
                            if (info.NextRestock > SEnvir.Now) continue;

                            info.RemainingQuantity = info.Quantity;
                        }

                        ItemCheck check = new ItemCheck(info.Item, 1, UserItemFlags.Bound, TimeSpan.Zero);

                        if (!CanGainItems(false, check)) continue;

                        UserItem item = SEnvir.CreateDropItem(check);
                        GainItem(item);

                        LogMilestone(MilestoneType.MineCatch, item.Count, item: item.Info);

                        if (info.Quantity > 0)
                        {
                            info.RemainingQuantity--;

                            // sets restock time when quantity reaches zero. Unless restock time is less than zero, then never restock
                            if (info.RemainingQuantity == 0 && info.RestockTimeInMinutes >= 0)
                            {
                                info.NextRestock = SEnvir.Now.AddMinutes(info.RestockTimeInMinutes);
                            }
                        }
                    }

                    bool hasRubble = false;

                    foreach (MapObject ob in CurrentCell.Objects)
                    {
                        if (ob.Race != ObjectType.Spell) continue;

                        SpellObject rubble = (SpellObject)ob;
                        if (rubble.Effect != SpellEffect.Rubble) continue;

                        hasRubble = true;

                        rubble.Power++;
                        rubble.Broadcast(new S.ObjectSpellChanged { ObjectID = ob.ObjectID, Power = rubble.Power });
                        rubble.TickTime = SEnvir.Now.AddMinutes(1);
                        break;
                    }

                    if (!hasRubble)
                    {
                        SpellObject ob = new SpellObject
                        {
                            DisplayLocation = CurrentLocation,
                            TickCount = 1,
                            TickFrequency = TimeSpan.FromMinutes(1),
                            TickTime = SEnvir.Now.AddMinutes(1),
                            Owner = this,
                            Effect = SpellEffect.Rubble,
                        };

                        ob.Spawn(CurrentMap, CurrentLocation);

                        PauseBuffs();
                    }

                    result = true;
                }
            }

            BuffRemove(BuffType.Transparency);
            BuffRemove(BuffType.Cloak);
            Broadcast(new S.ObjectMining { ObjectID = ObjectID, Direction = Direction, Location = CurrentLocation, Slow = slow, Effect = result });
        }
        #endregion
    }
}
