using Library;
using Library.SystemModels;
using Server.Envir;
using Server.Models.Monsters;
using System;
using System.Linq;

namespace Server.Models
{
    /// <summary>
    /// One-time registration of all AI id → MonsterObject factory mappings.
    /// To add a new monster AI, add a single Register() call here.
    /// </summary>
    internal static class MonsterRegistrations
    {
        public static void RegisterAll()
        {
            MonsterRegistry.Register(-1, info => new Guard { MonsterInfo = info });

            MonsterRegistry.Register(1, info => new MonsterObject { MonsterInfo = info, Passive = true, NeedHarvest = true, HarvestCount = 2 });
            MonsterRegistry.Register(2, info => new MonsterObject { MonsterInfo = info, Passive = true, NeedHarvest = true, HarvestCount = 3 });
            MonsterRegistry.Register(3, info => new MonsterObject { MonsterInfo = info, NeedHarvest = true, HarvestCount = 3 });
            MonsterRegistry.Register(4, info => new TreeMonster { MonsterInfo = info });
            MonsterRegistry.Register(5, info => new CarnivorousPlant { MonsterInfo = info, NeedHarvest = true, HarvestCount = 2 });
            MonsterRegistry.Register(6, info => new SpittingSpider { MonsterInfo = info, NeedHarvest = true, HarvestCount = 2, PoisonType = PoisonType.Green });
            MonsterRegistry.Register(7, info => new SkeletonAxeThrower { MonsterInfo = info });
            MonsterRegistry.Register(8, info => new MonsterObject { MonsterInfo = info, NeedHarvest = true, HarvestCount = 2, PoisonType = PoisonType.Paralysis, PoisonTicks = 1, PoisonFrequency = 5, PoisonRate = 12 });
            MonsterRegistry.Register(9, info => new GhostSorcerer { MonsterInfo = info });
            MonsterRegistry.Register(10, info => new GhostMage { MonsterInfo = info });
            MonsterRegistry.Register(11, info => new VoraciousGhost { MonsterInfo = info });
            MonsterRegistry.Register(12, info => new HealerAnt { MonsterInfo = info });
            MonsterRegistry.Register(13, info => new LordNiJae { MonsterInfo = info });
            MonsterRegistry.Register(14, info => new SpittingSpider { MonsterInfo = info, PoisonType = PoisonType.Green });
            MonsterRegistry.Register(15, info => new MonsterObject { MonsterInfo = info });
            MonsterRegistry.Register(16, info => new UmaKing { MonsterInfo = info });
            MonsterRegistry.Register(17, info => new ArachnidGrazer
            {
                MonsterInfo = info,
                SpawnList = { [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.Larva)] = 1 }
            });
            MonsterRegistry.Register(18, info => new Larva { MonsterInfo = info, PoisonType = PoisonType.Green });
            MonsterRegistry.Register(19, info => new RedMoonTheFallen { MonsterInfo = info });
            MonsterRegistry.Register(20, info => new SkeletonAxeThrower { MonsterInfo = info, FearRate = 2, FearDuration = 4 });
            MonsterRegistry.Register(21, info => new ZumaGuardian { MonsterInfo = info });
            MonsterRegistry.Register(22, info => new ZumaKing
            {
                MonsterInfo = info,
                SpawnList =
                {
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.ZumaArcherMonster)] = 50,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.ZumaFanaticMonster)] = 25,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.ZumaGuardianMonster)] = 25,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.ZumaKeeperMonster)] = 1
                }
            });
            MonsterRegistry.Register(23, info => new Monkey { MonsterInfo = info, PoisonType = PoisonType.Green });
            MonsterRegistry.Register(24, info => new Monkey { MonsterInfo = info, PoisonType = PoisonType.Red });
            MonsterRegistry.Register(25, info => new EvilElephant { MonsterInfo = info });
            MonsterRegistry.Register(26, info => new NumaMage { MonsterInfo = info });
            MonsterRegistry.Register(27, info => new GhostMage { MonsterInfo = info });
            MonsterRegistry.Register(28, info => new WindfurySorcerer { MonsterInfo = info });
            MonsterRegistry.Register(29, info => new SkeletonAxeThrower { MonsterInfo = info });
            MonsterRegistry.Register(30, info => new NetherworldGate { MonsterInfo = info });
            MonsterRegistry.Register(31, info => new SonicLizard { MonsterInfo = info });
            MonsterRegistry.Register(33, info => new GiantLizard { MonsterInfo = info, AttackRange = 9, IgnoreShield = true });
            MonsterRegistry.Register(34, info => new SkeletonAxeThrower { MonsterInfo = info, AttackRange = 9 });
            MonsterRegistry.Register(35, info => new MonsterObject { MonsterInfo = info });
            MonsterRegistry.Register(36, info => new NumaMage { MonsterInfo = info });
            MonsterRegistry.Register(37, info => new MonsterObject { MonsterInfo = info });
            MonsterRegistry.Register(38, info => new BanyaLeftGuard { MonsterInfo = info });
            MonsterRegistry.Register(39, info => new MonsterObject { MonsterInfo = info });
            MonsterRegistry.Register(40, info => new MonsterObject { MonsterInfo = info });
            MonsterRegistry.Register(41, info => new EmperorSaWoo { MonsterInfo = info });
            MonsterRegistry.Register(42, info => new SpittingSpider { MonsterInfo = info });
            MonsterRegistry.Register(43, info => new ArchLichTaedu
            {
                MonsterInfo = info,
                SpawnList =
                {
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.BoneArcher)] = 90,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.BoneSoldier)] = 15,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.BoneBladesman)] = 15,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.BoneCaptain)] = 15,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.SkeletonEnforcer)] = 1
                }
            });
            MonsterRegistry.Register(44, info => new WedgeMothLarva
            {
                MonsterInfo = info,
                SpawnList = { [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.LesserWedgeMoth)] = 1 }
            });
            MonsterRegistry.Register(45, info => new RazorTusk { MonsterInfo = info });
            MonsterRegistry.Register(46, info => new SpittingSpider { MonsterInfo = info, PoisonType = PoisonType.Red, PoisonTicks = 1, PoisonFrequency = 10, PoisonRate = 25 });
            MonsterRegistry.Register(47, info => new SpittingSpider { MonsterInfo = info, PoisonType = PoisonType.Green, PoisonTicks = 7, PoisonRate = 15 });
            MonsterRegistry.Register(48, info => new SonicLizard { MonsterInfo = info, IgnoreShield = true });
            MonsterRegistry.Register(49, info => new GiantLizard { MonsterInfo = info, AttackRange = 8, PoisonType = PoisonType.Paralysis, PoisonTicks = 1, PoisonFrequency = 5 });
            MonsterRegistry.Register(50, info => new GiantLizard { MonsterInfo = info, AttackRange = 8 });
            MonsterRegistry.Register(52, info => new WhiteBone { MonsterInfo = info });
            MonsterRegistry.Register(53, info => new Shinsu { MonsterInfo = info });
            MonsterRegistry.Register(54, info => new GiantLizard { MonsterInfo = info, RangeCooldown = TimeSpan.FromSeconds(5) });
            MonsterRegistry.Register(56, info => new CorrosivePoisonSpitter { MonsterInfo = info, PoisonType = PoisonType.Green, PoisonTicks = 7, PoisonRate = 15, IgnoreShield = true });
            MonsterRegistry.Register(57, info => new CorrosivePoisonSpitter { MonsterInfo = info });
            MonsterRegistry.Register(58, info => new Stomper { MonsterInfo = info });
            MonsterRegistry.Register(59, info => new CrimsonNecromancer { MonsterInfo = info });
            MonsterRegistry.Register(60, info => new ChaosKnight { MonsterInfo = info });
            MonsterRegistry.Register(61, info => new PachontheChaosbringer { MonsterInfo = info });
            MonsterRegistry.Register(62, info => new NumaHighMage { MonsterInfo = info });
            MonsterRegistry.Register(63, info => new NumaStoneThrower { MonsterInfo = info });
            MonsterRegistry.Register(64, info => new Monkey { MonsterInfo = info });
            MonsterRegistry.Register(65, info => new IcyGoddess { MonsterInfo = info, FindRange = 3 });
            MonsterRegistry.Register(66, info => new IcySpiritWarrior { MonsterInfo = info, PoisonType = PoisonType.Paralysis, PoisonTicks = 1, PoisonFrequency = 5, PoisonRate = 25 });
            MonsterRegistry.Register(67, info => new IcySpiritGeneral { MonsterInfo = info, IgnoreShield = true });
            MonsterRegistry.Register(68, info => new Warewolf { MonsterInfo = info, IgnoreShield = true });
            MonsterRegistry.Register(69, info => new JinamStoneGate { MonsterInfo = info });
            MonsterRegistry.Register(70, info => new FrostLordHwa { MonsterInfo = info });
            MonsterRegistry.Register(71, info => new BanyoWarrior { MonsterInfo = info });
            MonsterRegistry.Register(72, info => new BanyoCaptain { MonsterInfo = info });
            MonsterRegistry.Register(74, info => new BanyoLordGuzak
            {
                MonsterInfo = info,
                SpawnList = { [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.BanyoCaptain)] = 2 }
            });
            MonsterRegistry.Register(75, info => new DepartedMonster
            {
                MonsterInfo = info,
                SpawnList = { [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.MatureEarwig)] = 1 }
            });
            MonsterRegistry.Register(76, info => new DepartedMonster
            {
                MonsterInfo = info,
                SpawnList = { [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.GoldenArmouredBeetle)] = 1 }
            });
            MonsterRegistry.Register(77, info => new EnragedLordNiJae
            {
                MonsterInfo = info,
                SpawnList = { [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.Millipede)] = 1 },
                MaxMinions = 200
            });
            MonsterRegistry.Register(78, info => new JinchonDevil { MonsterInfo = info });
            MonsterRegistry.Register(79, info => new GiantLizard { MonsterInfo = info, AttackRange = 10, RangeCooldown = TimeSpan.FromSeconds(5) });
            MonsterRegistry.Register(80, info => new SunFeralWarrior
            {
                MonsterInfo = info,
                SpawnList =
                {
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.FerociousFlameDemon)] = 5,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.FlameDemon)] = 1
                }
            });
            MonsterRegistry.Register(81, info => new MoonFeralWarrior { MonsterInfo = info });
            MonsterRegistry.Register(82, info => new OxFeralGeneral { MonsterInfo = info, IgnoreShield = true });
            MonsterRegistry.Register(83, info => new FlameDemon { MonsterInfo = info, Min = -2, Max = 2 });
            MonsterRegistry.Register(84, info => new WingedHorror { MonsterInfo = info, RangeChance = 1 });
            MonsterRegistry.Register(85, info => new EmperorSaWoo { MonsterInfo = info, PoisonType = PoisonType.Paralysis, PoisonTicks = 1, PoisonFrequency = 5, PoisonRate = 8 });
            MonsterRegistry.Register(86, info => new FlameDemon { MonsterInfo = info, Passive = true, Min = 0, Max = 8 });
            MonsterRegistry.Register(87, info => new OmaWarlord { MonsterInfo = info, PoisonType = PoisonType.Abyss, PoisonTicks = 1, PoisonFrequency = 7, PoisonRate = 15 });
            MonsterRegistry.Register(88, info => new GoruSpearman { MonsterInfo = info });
            MonsterRegistry.Register(89, info => new GoruArcher { MonsterInfo = info, PoisonType = PoisonType.Silenced, PoisonTicks = 1, PoisonFrequency = 5, PoisonRate = 10 });
            MonsterRegistry.Register(90, info => new OmaWarlord { MonsterInfo = info, PoisonType = PoisonType.Paralysis, PoisonTicks = 1, PoisonFrequency = 5, PoisonRate = 25 });
            MonsterRegistry.Register(91, info => new EnragedArchLichTaedu
            {
                MonsterInfo = info,
                MinSpawn = 5,
                RandomSpawn = 5,
                PoisonType = PoisonType.Red,
                PoisonTicks = 1,
                PoisonFrequency = 25,
                PoisonRate = 5,
                SpawnList =
                {
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.GoruArcher)] = 10,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.GoruGeneral)] = 5,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.GoruSpearman)] = 5
                }
            });
            MonsterRegistry.Register(92, info => new GiantLizard { MonsterInfo = info, AttackRange = 9 });
            MonsterRegistry.Register(93, info => new EscortCommander { MonsterInfo = info });
            MonsterRegistry.Register(94, info => new FieryDancer { MonsterInfo = info });
            MonsterRegistry.Register(95, info => new FieryDancer { MonsterInfo = info, PoisonType = PoisonType.Paralysis, PoisonTicks = 1, PoisonFrequency = 5, PoisonRate = 15 });
            MonsterRegistry.Register(96, info => new QueenOfDawn { MonsterInfo = info });
            MonsterRegistry.Register(97, info => new SonicLizard { MonsterInfo = info, IgnoreShield = true, Range = 5 });
            MonsterRegistry.Register(98, info => new YumgonWitch { MonsterInfo = info, AoEElement = Element.Lightning });
            MonsterRegistry.Register(99, info => new JinhwanSpirit { MonsterInfo = info, SpawnList = { [info] = 1 } });
            MonsterRegistry.Register(100, info => new YumgonWitch { MonsterInfo = info });
            MonsterRegistry.Register(101, info => new DragonQueen
            {
                MonsterInfo = info,
                DragonLordInfo = SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.DragonLord),
                SpawnList =
                {
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.OYoungBeast)] = 2,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.YumgonWitch)] = 2,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.MaWarden)] = 2,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.MaWarlord)] = 2,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.JinhwanSpirit)] = 2,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.JinhwanGuardian)] = 2,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.OyoungGeneral)] = 2,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.YumgonGeneral)] = 2
                }
            });
            MonsterRegistry.Register(102, info => new DragonLord
            {
                MonsterInfo = info,
                SpawnList =
                {
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.OYoungBeast)] = 10000,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.YumgonWitch)] = 10000,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.MaWarden)] = 10000,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.MaWarlord)] = 10000,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.JinhwanSpirit)] = 10000,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.JinhwanGuardian)] = 10000,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.OyoungGeneral)] = 10000,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.YumgonGeneral)] = 10000,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.DragonLord)] = 1
                }
            });
            MonsterRegistry.Register(103, info => new InfernalSoldier { MonsterInfo = info, AttackRange = 5 });
            MonsterRegistry.Register(104, info => new FerociousIceTiger { MonsterInfo = info });
            MonsterRegistry.Register(105, info => new GiantLizard { MonsterInfo = info, AttackRange = 5, IgnoreShield = true, CanPvPRange = true });
            MonsterRegistry.Register(106, info => new GiantLizard { MonsterInfo = info, AttackRange = 7, CanPvPRange = true });
            MonsterRegistry.Register(107, info => new SamaFireGuardian { MonsterInfo = info });
            MonsterRegistry.Register(108, info => new SamaIceGuardian { MonsterInfo = info });
            MonsterRegistry.Register(109, info => new SamaLightningGuardian { MonsterInfo = info });
            MonsterRegistry.Register(110, info => new SamaWindGuardian { MonsterInfo = info });
            MonsterRegistry.Register(111, info => new SamaPhoenix { MonsterInfo = info });
            MonsterRegistry.Register(112, info => new SamaBlack { MonsterInfo = info });
            MonsterRegistry.Register(113, info => new SamaBlue { MonsterInfo = info });
            MonsterRegistry.Register(114, info => new SamaWhite { MonsterInfo = info });
            MonsterRegistry.Register(115, info => new SamaProphet
            {
                MonsterInfo = info,
                SpawnList = { [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.SamaSorcerer)] = 1 }
            });
            MonsterRegistry.Register(116, info => new SamaScorcer { MonsterInfo = info });
            MonsterRegistry.Register(117, info => new BanyoWarrior { MonsterInfo = info, DoubleDamage = true });
            MonsterRegistry.Register(118, info => new OmaMage { MonsterInfo = info });
            MonsterRegistry.Register(119, info => new MonsterObject { MonsterInfo = info, PoisonType = PoisonType.Silenced, PoisonTicks = 1, PoisonFrequency = 5, PoisonRate = 10 });
            MonsterRegistry.Register(120, info => new DoomClaw { MonsterInfo = info });
            MonsterRegistry.Register(121, info => new PinkBat { MonsterInfo = info });
            MonsterRegistry.Register(122, info => new QuartzTurtleSub
            {
                MonsterInfo = info,
                SpawnList = { [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.QuartzMiniTurtle)] = 2 }
            });
            MonsterRegistry.Register(123, info => new Larva { MonsterInfo = info, Range = 3 });
            MonsterRegistry.Register(124, info => new QuartzTree
            {
                MonsterInfo = info,
                SubBossInfo = SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.QuartzTurtleSub),
                SpawnList =
                {
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.QuartzBlueBat)] = 20,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.QuartzPinkBat)] = 20,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.QuartzBlueCrystal)] = 20,
                    [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.QuartzRedHood)] = 2
                }
            });
            MonsterRegistry.Register(125, info => new CarnivorousPlant { MonsterInfo = info, HideRange = 1, FindRange = 1 });
            MonsterRegistry.Register(126, info => new MonasteryBoss
            {
                MonsterInfo = info,
                SpawnList = { [SEnvir.MonsterInfoList.Binding.First(x => x.Flag == MonsterFlag.Sacrifice)] = 1 }
            });
            MonsterRegistry.Register(127, info => new JinchonDevil { MonsterInfo = info, CastDelay = TimeSpan.FromSeconds(8), DeathCloudDurationMin = 2000, DeathCloudDurationRandom = 5000 });
            MonsterRegistry.Register(128, info => new Doll { MonsterInfo = info });
            MonsterRegistry.Register(129, info => new Monsters.Tornado { MonsterInfo = info, Passive = true });
            MonsterRegistry.Register(130, info => new UndeadSoul { MonsterInfo = info });
            MonsterRegistry.Register(131, info => new Terracotta { MonsterInfo = info });
            MonsterRegistry.Register(132, info => new Terracotta { MonsterInfo = info, CanPhase = true });
            MonsterRegistry.Register(133, info => new TerracottaSub { MonsterInfo = info, PoisonType = PoisonType.Paralysis, PoisonTicks = 1, PoisonFrequency = 5, PoisonRate = 15 });
            MonsterRegistry.Register(134, info => new TerracottaBoss { MonsterInfo = info, PoisonType = PoisonType.Paralysis, PoisonTicks = 1, PoisonFrequency = 5, PoisonRate = 15 });

            MonsterRegistry.Register(1001, info => new CastleFlag { MonsterInfo = info });
            MonsterRegistry.Register(1002, info => new CastleGate { MonsterInfo = info });
            MonsterRegistry.Register(1003, info => new CastleGuard { MonsterInfo = info });
        }
    }
}
