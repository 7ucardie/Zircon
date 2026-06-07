using Client.Controls;
using Client.Envir;
using Client.Models;
using Client.Scenes.Views;
using Client.UserModels;
using Library;
using Library.SystemModels;
using MirDB;
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Drawing;
using System.Linq;
using System.Reflection;
using System.Text;
using System.Windows.Forms;
using C = Library.Network.ClientPackets;

namespace Client.Scenes
{
    public sealed partial class GameScene
    {
        public void UseMagic(SpellKey key)
        {
            if (Game.Observer || User == null || User.Horse != HorseType.None || MagicBarBox == null) return;

            ClientUserMagic magic = null;

            foreach (KeyValuePair<MagicInfo, ClientUserMagic> pair in User.Magics)
            {
                switch (MagicBarBox.SpellSet)
                {
                    case 1:
                        if (pair.Value.Set1Key == key)
                            magic = pair.Value;
                        break;
                    case 2:
                        if (pair.Value.Set2Key == key)
                            magic = pair.Value;
                        break;
                    case 3:
                        if (pair.Value.Set3Key == key)
                            magic = pair.Value;
                        break;
                    case 4:
                        if (pair.Value.Set4Key == key)
                            magic = pair.Value;
                        break;
                }

                if (magic != null) break;
            }

            if (magic == null) return;

            if (magic.ItemRequired)
            {
                var magicItem = Equipment.FirstOrDefault(x => x != null && x.Info.ItemEffect == ItemEffect.MagicRing && x.Info.Shape == magic.Info.Index);

                if (magicItem == null) return;
            }
            else
            {
                if (User.Level < magic.Info.NeedLevel1) return;
            }

            switch (magic.Info.Magic)
            {
                case MagicType.Swordsmanship:
                case MagicType.SpiritSword:
                case MagicType.VineTreeDance:
                case MagicType.WillowDance:
                    return;
                case MagicType.Thrusting:
                    if (CEnvir.Now < ToggleTime) return;
                    ToggleTime = CEnvir.Now.AddSeconds(1);
                    CEnvir.Enqueue(new C.MagicToggle { Magic = magic.Info.Magic, CanUse = !User.CanThrusting });
                    return;
                case MagicType.HalfMoon:
                    if (CEnvir.Now < ToggleTime) return;
                    ToggleTime = CEnvir.Now.AddSeconds(1);
                    CEnvir.Enqueue(new C.MagicToggle { Magic = magic.Info.Magic, CanUse = !User.CanHalfMoon });
                    return;
                case MagicType.DestructiveSurge:
                    if (CEnvir.Now < ToggleTime) return;
                    ToggleTime = CEnvir.Now.AddSeconds(1);
                    CEnvir.Enqueue(new C.MagicToggle { Magic = magic.Info.Magic, CanUse = !User.CanDestructiveSurge });
                    return;
                case MagicType.FlamingSword:
                case MagicType.DragonRise:
                case MagicType.BladeStorm:
                case MagicType.DemonicRecovery:
                case MagicType.DefensiveBlow:
                case MagicType.OffensiveBlow:
                    if (CEnvir.Now < magic.NextCast || magic.Cost > User.CurrentMP) return;
                    magic.NextCast = CEnvir.Now.AddSeconds(0.5D); //Act as an anti spam
                    CEnvir.Enqueue(new C.MagicToggle { Magic = magic.Info.Magic });
                    return;
                case MagicType.FlameSplash:
                    if (CEnvir.Now < ToggleTime) return;
                    ToggleTime = CEnvir.Now.AddSeconds(1);
                    CEnvir.Enqueue(new C.MagicToggle { Magic = magic.Info.Magic, CanUse = !User.CanFlameSplash });
                    return;
                case MagicType.FullBloom:
                case MagicType.WhiteLotus:
                case MagicType.RedLotus:
                case MagicType.SweetBrier:
                    if (CEnvir.Now < ToggleTime || CEnvir.Now < magic.NextCast) return;

                    if (User.AttackMagic != magic.Info.Magic)
                    {
                        ReceiveChat(string.Format(CEnvir.Language.GameSceneSkillReady, magic.Info.Name), MessageType.Hint);
                        int attackDelay = Globals.AttackDelay - MapObject.User.Stats[Stat.AttackSpeed] * Globals.ASpeedRate;
                        attackDelay = Math.Max(800, attackDelay);

                        ToggleTime = CEnvir.Now + TimeSpan.FromMilliseconds(attackDelay + 200);

                        User.AttackMagic = magic.Info.Magic;
                    }
                    return;
                case MagicType.Karma:
                    if (CEnvir.Now < ToggleTime || CEnvir.Now < magic.NextCast || User.Buffs.All(x => x.Type != BuffType.Cloak)) return;

                    if (User.AttackMagic != magic.Info.Magic)
                    {
                        ReceiveChat(string.Format(CEnvir.Language.GameSceneSkillReady, magic.Info.Name), MessageType.Hint);
                        ToggleTime = CEnvir.Now + TimeSpan.FromMilliseconds(500);

                        User.AttackMagic = magic.Info.Magic;
                    }
                    return;
            }

            if (CEnvir.Now < User.NextMagicTime || User.Dead ||
                User.Buffs.Any(x => x.Type == BuffType.DragonRepulse) ||
                (User.Buffs.Any(x => x.Type == BuffType.ElementalHurricane) && magic.Info.Magic != MagicType.ElementalHurricane) ||
                (User.Poison & PoisonType.Paralysis) == PoisonType.Paralysis ||
                (User.Poison & PoisonType.Silenced) == PoisonType.Silenced) return;

            if (CEnvir.Now < magic.NextCast)
            {
                if (CEnvir.Now >= OutputTime)
                {
                    OutputTime = CEnvir.Now.AddSeconds(1);
                    ReceiveChat(string.Format(CEnvir.Language.GameSceneCastInCooldown, magic.Info.Name), MessageType.Hint);
                }
                return;
            }

            switch (magic.Info.Magic)
            {
                case MagicType.Cloak:
                    if (User.VisibleBuffs.ContainsKey(BuffType.Cloak)) break;
                    if (CEnvir.Now < User.CombatTime.AddSeconds(10))
                    {
                        if (CEnvir.Now >= OutputTime)
                        {
                            OutputTime = CEnvir.Now.AddSeconds(1);
                            ReceiveChat(string.Format(CEnvir.Language.GameSceneCastInCombat, magic.Info.Name), MessageType.Hint);
                        }
                        return;
                    }

                    if (User.Stats[Stat.Health] * magic.Cost / 1000 >= User.CurrentHP || User.CurrentHP < User.Stats[Stat.Health] / 10)
                    {
                        if (CEnvir.Now >= OutputTime)
                        {
                            OutputTime = CEnvir.Now.AddSeconds(1);
                            ReceiveChat(string.Format(CEnvir.Language.GameSceneCastNoEnoughHealth, magic.Info.Name), MessageType.Hint);
                        }
                        return;
                    }
                    break;
                case MagicType.DarkConversion:
                    if (User.VisibleBuffs.ContainsKey(BuffType.DarkConversion)) break;

                    if (magic.Cost > User.CurrentMP)
                    {
                        if (CEnvir.Now >= OutputTime)
                        {
                            OutputTime = CEnvir.Now.AddSeconds(1);
                            ReceiveChat(string.Format(CEnvir.Language.GameSceneCastNoEnoughMana, magic.Info.Name), MessageType.Hint);
                        }
                        return;
                    }
                    break;
                case MagicType.DragonRepulse:
                    if (User.Stats[Stat.Health] * magic.Cost / 1000 >= User.CurrentHP || User.CurrentHP < User.Stats[Stat.Health] / 10)
                    {
                        if (CEnvir.Now >= OutputTime)
                        {
                            OutputTime = CEnvir.Now.AddSeconds(1);
                            ReceiveChat(string.Format(CEnvir.Language.GameSceneCastNoEnoughHealth, magic.Info.Name), MessageType.Hint);
                        }
                        return;
                    }
                    if (User.Stats[Stat.Mana] * magic.Cost / 1000 >= User.CurrentMP || User.CurrentMP < User.Stats[Stat.Mana] / 10)
                    {
                        if (CEnvir.Now >= OutputTime)
                        {
                            OutputTime = CEnvir.Now.AddSeconds(1);
                            ReceiveChat(string.Format(CEnvir.Language.GameSceneCastNoEnoughMana, magic.Info.Name), MessageType.Hint);
                        }
                        return;
                    }
                    break;
                case MagicType.ElementalHurricane:
                    int cost = magic.Cost;
                    if (MapObject.User.VisibleBuffs.ContainsKey(BuffType.ElementalHurricane))
                        cost = 0;

                    if (cost > User.CurrentMP)
                    {
                        if (CEnvir.Now >= OutputTime)
                        {
                            OutputTime = CEnvir.Now.AddSeconds(1);
                            ReceiveChat($"Unable to cast {magic.Info.Name}, You do not have enough Mana.", MessageType.Hint);
                        }
                        return;
                    }
                    break;
                default:

                    if (magic.Cost > User.CurrentMP)
                    {
                        if (CEnvir.Now >= OutputTime)
                        {
                            OutputTime = CEnvir.Now.AddSeconds(1);
                            ReceiveChat(string.Format(CEnvir.Language.GameSceneCastNoEnoughMana, magic.Info.Name), MessageType.Hint);
                        }
                        return;
                    }
                    break;
            }
            MapObject target = null;
            MirDirection direction = MapControl.MouseDirection();

            switch (magic.Info.Magic)
            {
                case MagicType.ShoulderDash:
                    if (CEnvir.Now < User.ServerTime) return;
                    if ((User.Poison & PoisonType.WraithGrip) == PoisonType.WraithGrip) return;

                    User.ServerTime = CEnvir.Now.AddSeconds(5);
                    User.NextMagicTime = CEnvir.Now + Globals.MagicDelay;
                    CEnvir.Enqueue(new C.Magic { Direction = direction, Action = MirAction.Spell, Type = magic.Info.Magic });
                    return;

                case MagicType.DanceOfSwallow:
                    if (CEnvir.Now < User.ServerTime) return;
                    if (CanAttackTarget(MouseObject))
                        target = MouseObject;

                    if (target == null) return;

                    if (!Functions.InRange(target.CurrentLocation, User.CurrentLocation, Globals.MagicRange))
                    {
                        if (CEnvir.Now < OutputTime) return;
                        OutputTime = CEnvir.Now.AddSeconds(1);
                        ReceiveChat(string.Format(CEnvir.Language.GameSceneCastTooFar, magic.Info.Name), MessageType.Hint);
                        return;
                    }

                    User.ServerTime = CEnvir.Now.AddSeconds(5);
                    User.NextMagicTime = CEnvir.Now + Globals.MagicDelay;

                    MapObject.TargetObject = target;
                    MapObject.MagicObject = target;

                    CEnvir.Enqueue(new C.Magic { Action = MirAction.Spell, Type = magic.Info.Magic, Target = target.ObjectID });
                    return;

                case MagicType.FireBall:
                case MagicType.IceBolt:
                case MagicType.LightningBall:
                case MagicType.GustBlast:
                case MagicType.ElectricShock:
                case MagicType.AdamantineFireBall:
                case MagicType.FireBounce:
                case MagicType.ThunderBolt:
                case MagicType.ChainLightning:
                case MagicType.IceBlades:
                case MagicType.Cyclone:
                case MagicType.ExpelUndead:
                case MagicType.LightningStrike:
                case MagicType.IceRain:
                case MagicType.IceDragon:

                case MagicType.PoisonDust:
                case MagicType.ExplosiveTalisman:
                case MagicType.EvilSlayer:
                case MagicType.GreaterEvilSlayer:
                case MagicType.ImprovedExplosiveTalisman:
                case MagicType.Parasite:
                case MagicType.Neutralize:
                case MagicType.SearingLight:
                case MagicType.BindingTalisman:
                case MagicType.BrainStorm:

                case MagicType.Hemorrhage:
                case MagicType.FlamingDaggers:
                case MagicType.Shredding:
                    if (CanAttackTarget(MagicObject))
                        target = MagicObject;

                    if (CanAttackTarget(MouseObject))
                    {
                        target = MouseObject;

                        if (MouseObject.Race == ObjectType.Monster && ((MonsterObject)MouseObject).MonsterInfo.AI >= 0)
                            MapObject.MagicObject = target;
                        else
                            MapObject.MagicObject = null;
                    }
                    break;
                case MagicType.HundredFist:
                    if (CanAttackTarget(MagicObject))
                        target = MagicObject;
                    if (CanAttackTarget(MouseObject))
                    {
                        target = MouseObject;

                        if (MouseObject.Race == ObjectType.Monster && ((MonsterObject)MouseObject).MonsterInfo.AI >= 0)
                            MapObject.MagicObject = target;
                        else
                            MapObject.MagicObject = null;
                    }
                    if (target == null || !Functions.IsStraightEightDirection(User.CurrentLocation, target.CurrentLocation))
                        return;

                    break;

                case MagicType.WraithGrip:
                case MagicType.HellFire:
                case MagicType.Abyss:
                    if (CanAttackTarget(MouseObject))
                        target = MouseObject;
                    break;
                case MagicType.Interchange:
                case MagicType.Beckon:
                    if (CanAttackTarget(MouseObject))
                        target = MouseObject;
                    break;
                case MagicType.MagicCombustion:
                    if (!CanAttackTarget(MouseObject) || MouseObject.Race != ObjectType.Player) return;

                    target = MouseObject;
                    break;

                case MagicType.Heal:
                case MagicType.Purification:
                    target = MouseObject ?? User;
                    break;
                case MagicType.CelestialLight:
                    if (User.Buffs.All(x => x.Type == BuffType.CelestialLight)) return;
                    break;

                case MagicType.Resurrection:
                    if (MouseObject == null || !MouseObject.Dead || MouseObject.Race != ObjectType.Player) return;

                    target = MouseObject;
                    break;
                case MagicType.SoulResonance:
                    if (MouseObject == null || MouseObject.Dead || MouseObject.Race != ObjectType.Player || !IsAlly(MouseObject.ObjectID)) return;

                    target = MouseObject;
                    break;
                case MagicType.CursedDoll:
                    if (CanAttackTarget(MouseObject))
                        target = MouseObject;
                    break;
                case MagicType.CorpseExploder:
                case MagicType.SummonDead:
                    if (MouseObject != null && MouseObject.Dead && (MouseObject.Race == ObjectType.Player || MouseObject.Race == ObjectType.Monster))
                        target = MouseObject;
                    break;

                case MagicType.Spiritualism:
                    if (Equipment[(int)EquipmentSlot.Amulet] == null || Equipment[(int)EquipmentSlot.Amulet].Info.Shape != 0 || Equipment[(int)EquipmentSlot.Amulet].Count < 1) return;

                    direction = MirDirection.Down;
                    break;

                case MagicType.Rake:
                    if (!User.VisibleBuffs.ContainsKey(BuffType.Cloak)) return;
                    break;

                case MagicType.Chain:
                    if (CanAttackTarget(MouseObject) && MouseObject.Race == ObjectType.Monster)
                        target = MouseObject;
                    break;

                case MagicType.Defiance:
                case MagicType.Invincibility:
                    direction = MirDirection.Down;
                    break;
                case MagicType.Might:
                    direction = MirDirection.Down;
                    break;
                case MagicType.MassBeckon:
                    direction = MirDirection.Down;
                    break;
                case MagicType.ReflectDamage:
                    if (User.Buffs.Any(x => x.Type == BuffType.ReflectDamage)) return;
                    direction = MirDirection.Down;
                    break;
                case MagicType.Endurance:
                    direction = MirDirection.Down;
                    break;
                case MagicType.Renounce:
                    break;
                case MagicType.StrengthOfFaith:
                    break;
                case MagicType.MagicShield:
                    if (User.Buffs.Any(x => x.Type == BuffType.MagicShield || x.Type == BuffType.SuperiorMagicShield)) return;
                    break;
                case MagicType.SuperiorMagicShield:
                    if (User.Buffs.Any(x => x.Type == BuffType.SuperiorMagicShield)) return;
                    break;
                case MagicType.FrostBite:
                    if (User.Buffs.Any(x => x.Type == BuffType.FrostBite)) return;
                    break;
                case MagicType.JudgementOfHeaven:
                    break;

                case MagicType.SeismicSlam:
                case MagicType.CrushingWave:
                case MagicType.ElementalSwords:
                case MagicType.TaecheonSword:
                case MagicType.FireSword:

                case MagicType.Repulsion:
                case MagicType.ScortchedEarth:
                case MagicType.LightningBeam:
                case MagicType.Teleportation:
                case MagicType.FrozenEarth:
                case MagicType.BlowEarth:
                case MagicType.GreaterFrozenEarth:
                case MagicType.ThunderStrike:
                case MagicType.MirrorImage:
                case MagicType.ElementalHurricane:
                case MagicType.IceAura:
                case MagicType.IceBreaker:
                case MagicType.FrozenDragon:

                case MagicType.Invisibility:
                case MagicType.CombatKick:
                case MagicType.ThunderKick:
                case MagicType.Fetter:
                case MagicType.SummonSkeleton:
                case MagicType.SummonShinsu:
                case MagicType.SummonJinSkeleton:
                case MagicType.SummonDemonicCreature:
                case MagicType.DemonExplosion:
                case MagicType.DarkSoulPrison:
                case MagicType.HeavenlySky:
                case MagicType.PoisonCloud:

                case MagicType.PoisonousCloud:
                case MagicType.Cloak:
                case MagicType.SummonPuppet:
                case MagicType.TheNewBeginning:
                case MagicType.DarkConversion:
                case MagicType.DragonRepulse:
                case MagicType.FlashOfLight:
                case MagicType.Evasion:
                case MagicType.RagingWind:
                case MagicType.Concentration:
                case MagicType.Containment:
                case MagicType.FourWheels:
                case MagicType.CrescentMoon:
                    break;

                case MagicType.SwiftBlade:

                case MagicType.FireWall:
                case MagicType.FireStorm:
                case MagicType.LightningWave:
                case MagicType.IceStorm:
                case MagicType.DragonTornado:
                case MagicType.GeoManipulation:
                case MagicType.Transparency:
                case MagicType.MeteorShower:
                case MagicType.Tempest:
                case MagicType.Asteroid:
                case MagicType.Tornado:

                case MagicType.MagicResistance:
                case MagicType.Resilience:
                case MagicType.LifeSteal:
                case MagicType.MassInvisibility:
                case MagicType.TrapOctagon:
                case MagicType.ElementalSuperiority:
                case MagicType.BloodLust:
                case MagicType.MassHeal:

                case MagicType.BurningFire:
                    if (!Functions.InRange(MapControl.MapLocation, User.CurrentLocation, Globals.MagicRange))
                    {
                        if (CEnvir.Now < OutputTime) return;
                        OutputTime = CEnvir.Now.AddSeconds(1);
                        ReceiveChat(string.Format(CEnvir.Language.GameSceneCastTooFar, magic.Info.Name), MessageType.Hint);
                        return;
                    }
                    break;
                default:
                    return;
            }

            if (target != null && !Functions.InRange(target.CurrentLocation, User.CurrentLocation, Globals.MagicRange))
            {
                if (CEnvir.Now < OutputTime) return;
                OutputTime = CEnvir.Now.AddSeconds(1);
                ReceiveChat(string.Format(CEnvir.Language.GameSceneCastTooFar, magic.Info.Name), MessageType.Hint);
                return;
            }

            //Check Attack Range.

            if (target != null && target != User)
                direction = Functions.DirectionFromPoint(User.CurrentLocation, target.CurrentLocation);

            uint targetID = target?.ObjectID ?? 0;
            Point targetLocation;

            //Allows area casting instead of direct lock on (augmentations required)
            //targetID and separate maplocation passed through - allowing for target lock and area lookup to work
            switch (magic.Info.Magic)
            {
                case MagicType.Purification:
                case MagicType.EvilSlayer:
                case MagicType.GreaterEvilSlayer:
                case MagicType.ExplosiveTalisman:
                case MagicType.ImprovedExplosiveTalisman:
                case MagicType.PoisonDust:
                case MagicType.Neutralize:
                case MagicType.BindingTalisman:
                case MagicType.BrainStorm:
                    targetLocation = MapControl.MapLocation;
                    break;
                default:
                    targetLocation = target?.CurrentLocation ?? MapControl.MapLocation;
                    break;
            }

            //switch spell type.

            if (MouseObject != null && MouseObject.Race == ObjectType.Monster)
                FocusObject = (MonsterObject)MouseObject;

            User.MagicAction = new ObjectAction(MirAction.Spell, direction, MapObject.User.CurrentLocation, magic.Info.Magic, new List<uint> { targetID }, new List<Point> { targetLocation }, false, Element.None);
        }

        private bool CanAttackTarget(MapObject ob)
        {
            if (ob == null || ob.Dead || !ob.Visible) return false;

            switch (ob.Race)
            {
                case ObjectType.Player:
                    return true;
                case ObjectType.Monster:
                    MonsterObject mob = (MonsterObject)ob;

                    if (mob.MonsterInfo.AI < 0) return false;

                    return true;


                default:
                    return false;
            }
        }
        protected override void OnAfterDraw()
        {
            base.OnAfterDraw();

            int image = -1;
            Color color = Color.Empty;

            if (SelectedCell?.Item != null)
            {
                ItemInfo info = SelectedCell.Item.Info;

                if (info.ItemEffect == ItemEffect.ItemPart)
                    info = Globals.ItemInfoList.Binding.First(x => x.Index == SelectedCell.Item.AddedStats[Stat.ItemIndex]);

                image = info.Image;
                color = SelectedCell.Item.Colour;
            }
            else if (CurrencyPickedUp != null)
            {
                image = CEnvir.CurrencyImage(CurrencyPickedUp.Info.DropItem, CurrencyPickedUp.Amount);
            }

            MirLibrary library;

            if (image >= 0 && CEnvir.LibraryList.TryGetValue(LibraryFile.Inventory, out library))
            {
                Size imageSize = library.GetSize(image);
                Point p = new Point(CEnvir.MouseLocation.X - imageSize.Width / 2, CEnvir.MouseLocation.Y - imageSize.Height / 2);

                if (p.X + imageSize.Width >= Size.Width + Location.X)
                    p.X = Size.Width - imageSize.Width + Location.X;

                if (p.Y + imageSize.Height >= Size.Height + Location.Y)
                    p.Y = Size.Height - imageSize.Height + Location.Y;

                if (p.X < Location.X)
                    p.X = Location.X;

                if (p.Y <= Location.Y)
                    p.Y = Location.Y;


                library.Draw(image, p.X, p.Y, Color.White, false, 1f, ImageType.Image);

                if (color != Color.Empty)
                    library.Draw(image, p.X, p.Y, color, false, 1f, ImageType.Overlay);
            }

            if (ItemLabel != null && !ItemLabel.IsDisposed)
                ItemLabel.Draw();

            if (MagicLabel != null && !MagicLabel.IsDisposed)
                MagicLabel.Draw();

            if (FameLabel != null && !FameLabel.IsDisposed)
                FameLabel.Draw();
        }

        public void Displacement(MirDirection direction, Point location, bool clearQueue = false)
        {
            MapObject.User.ServerTime = DateTime.MinValue;
            MapObject.User.SetAction(new ObjectAction(MirAction.Standing, direction, location));
            MapObject.User.NextActionTime = CEnvir.Now.AddMilliseconds(300);

            if (clearQueue)
            {
                // Queue might contain actions at an old location (causing desync), so clear it out
                MapObject.User.ActionQueue.Clear();
            }
        }

    }
}
