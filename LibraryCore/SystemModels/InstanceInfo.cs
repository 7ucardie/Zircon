using Library;
using MirDB;
using System;
using System.Collections.Generic;
using System.Text.Json.Serialization;

namespace Library.SystemModels
{
    public sealed class InstanceInfo : DBObject
    {
        [IsIdentity]
        public string Name
        {
            get { return _Name; }
            set
            {
                if (_Name == value) return;

                var oldValue = _Name;
                _Name = value;

                OnChanged(oldValue, value, "Name");
            }
        }
        private string _Name;

        public InstanceType Type
        {
            get { return _Type; }
            set
            {
                if (_Type == value) return;

                var oldValue = _Type;
                _Type = value;

                OnChanged(oldValue, value, "Type");
            }
        }
        private InstanceType _Type;

        public byte MaxInstances
        {
            get { return _MaxInstances; }
            set
            {
                if (_MaxInstances == value) return;

                var oldValue = _MaxInstances;
                _MaxInstances = value;

                OnChanged(oldValue, value, "MaximumAllowed");
            }
        }
        private byte _MaxInstances;

        public bool ShowOnDungeonFinder
        {
            get { return _ShowOnDungeonFinder; }
            set
            {
                if (_ShowOnDungeonFinder == value) return;

                var oldValue = _ShowOnDungeonFinder;
                _ShowOnDungeonFinder = value;

                OnChanged(oldValue, value, "ShowOnDungeonFinder");
            }
        }

        private bool _ShowOnDungeonFinder;
        public bool SafeZoneOnly
        {
            get { return _SafeZoneOnly; }
            set
            {
                if (_SafeZoneOnly == value) return;

                var oldValue = _SafeZoneOnly;
                _SafeZoneOnly = value;

                OnChanged(oldValue, value, "SafeZoneOnly");
            }
        }
        private bool _SafeZoneOnly;

        public bool AllowRejoin
        {
            get { return _AllowRejoin; }
            set
            {
                if (_AllowRejoin == value) return;

                var oldValue = _AllowRejoin;
                _AllowRejoin = value;

                OnChanged(oldValue, value, "AllowRejoin");
            }
        }
        private bool _AllowRejoin;

        public bool AllowTeleport
        {
            get { return _AllowTeleport; }
            set
            {
                if (_AllowTeleport == value) return;

                var oldValue = _AllowTeleport;
                _AllowTeleport = value;

                OnChanged(oldValue, value, "AllowTeleport");
            }
        }
        private bool _AllowTeleport;

        public bool SavePlace
        {
            get { return _SavePlace; }
            set
            {
                if (_SavePlace == value) return;

                var oldValue = _SavePlace;
                _SavePlace = value;

                OnChanged(oldValue, value, "SavePlace");
            }
        }
        private bool _SavePlace;

        public byte MinPlayerLevel
        {
            get { return _MinPlayerLevel; }
            set
            {
                if (_MinPlayerLevel == value) return;

                var oldValue = _MinPlayerLevel;
                _MinPlayerLevel = value;

                OnChanged(oldValue, value, "MinimumLevel");
            }
        }
        private byte _MinPlayerLevel;

        public byte MaxPlayerLevel
        {
            get { return _MaxPlayerLevel; }
            set
            {
                if (_MaxPlayerLevel == value) return;

                var oldValue = _MaxPlayerLevel;
                _MaxPlayerLevel = value;

                OnChanged(oldValue, value, "MaximumLevel");
            }
        }
        private byte _MaxPlayerLevel;

        public byte MinPlayerCount
        {
            get { return _MinPlayerCount; }
            set
            {
                if (_MinPlayerCount == value) return;

                var oldValue = _MinPlayerCount;
                _MinPlayerCount = value;

                OnChanged(oldValue, value, "MinimumCount");
            }
        }
        private byte _MinPlayerCount;

        public byte MaxPlayerCount
        {
            get { return _MaxPlayerCount; }
            set
            {
                if (_MaxPlayerCount == value) return;

                var oldValue = _MaxPlayerCount;
                _MaxPlayerCount = value;

                OnChanged(oldValue, value, "MaximumCount");
            }
        }
        private byte _MaxPlayerCount;

        public ItemInfo RequiredItem
        {
            get { return _RequiredItem; }
            set
            {
                if (_RequiredItem == value) return;

                var oldValue = _RequiredItem;
                _RequiredItem = value;

                OnChanged(oldValue, value, "RequiredItem");
            }
        }
        private ItemInfo _RequiredItem;

        public bool RequiredItemSingleUse
        {
            get { return _RequiredItemSingleUse; }
            set
            {
                if (_RequiredItemSingleUse == value) return;

                var oldValue = _RequiredItemSingleUse;
                _RequiredItemSingleUse = value;

                OnChanged(oldValue, value, "RequiredItemSingleUse");
            }
        }

        private bool _RequiredItemSingleUse;

        public MapRegion ConnectRegion
        {
            get { return _ConnectRegion; }
            set
            {
                if (_ConnectRegion == value) return;

                var oldValue = _ConnectRegion;
                _ConnectRegion = value;

                OnChanged(oldValue, value, "ConnectRegion");
            }
        }
        private MapRegion _ConnectRegion;

        public MapRegion ReconnectRegion
        {
            get { return _ReconnectRegion; }
            set
            {
                if (_ReconnectRegion == value) return;

                var oldValue = _ReconnectRegion;
                _ReconnectRegion = value;

                OnChanged(oldValue, value, "ReconnectRegion");
            }
        }
        private MapRegion _ReconnectRegion;

        public int CooldownTimeInMinutes
        {
            get { return _CooldownTimeInMinutes; }
            set
            {
                if (_CooldownTimeInMinutes == value) return;

                var oldValue = _CooldownTimeInMinutes;
                _CooldownTimeInMinutes = value;

                OnChanged(oldValue, value, "CooldownTimeInMinutes");
            }
        }
        private int _CooldownTimeInMinutes;

        public int TimeLimitInMinutes
        {
            get { return _TimeLimitInMinutes; }
            set
            {
                if (_TimeLimitInMinutes == value) return;

                var oldValue = _TimeLimitInMinutes;
                _TimeLimitInMinutes = value;

                OnChanged(oldValue, value, "TimeLimitInMinutes");
            }
        }
        private int _TimeLimitInMinutes;

        public bool ShowTimer
        {
            get { return _ShowTimer; }
            set
            {
                if (_ShowTimer == value) return;

                var oldValue = _ShowTimer;
                _ShowTimer = value;

                OnChanged(oldValue, value, "ShowTimer");
            }
        }

        private bool _ShowTimer;

        public int HardMultiplier
        {
            get { return _HardMultiplier; }
            set
            {
                if (_HardMultiplier == value) return;
                var oldValue = _HardMultiplier;
                _HardMultiplier = value;
                OnChanged(oldValue, value, "HardMultiplier");
            }
        }
        private int _HardMultiplier = 25;

        public int NightmareMultiplier
        {
            get { return _NightmareMultiplier; }
            set
            {
                if (_NightmareMultiplier == value) return;
                var oldValue = _NightmareMultiplier;
                _NightmareMultiplier = value;
                OnChanged(oldValue, value, "NightmareMultiplier");
            }
        }
        private int _NightmareMultiplier = 50;

        [Association("Map", true)]
        public DBBindingList<InstanceMapInfo> Maps { get; set; }

        [Association("InstanceInfoStats", true)]
        public DBBindingList<InstanceInfoStat> BuffStats { get; set; }

        [Association("Phases", true)]
        public DBBindingList<InstancePhase> Phases { get; set; }

        [JsonIgnore]
        [IgnoreProperty]
        public Dictionary<string, byte> UserRecord { get; set; }

        [JsonIgnore]
        [IgnoreProperty]
        public Dictionary<string, DateTime> UserCooldown { get; set; }

        [JsonIgnore]
        [IgnoreProperty]
        public Dictionary<string, DateTime> GuildCooldown { get; set; }

        [JsonIgnore]
        [IgnoreProperty]
        public Dictionary<byte, DifficultyType> SequenceDifficulty { get; set; }

        public Stats Stats = new();

        protected internal override void OnLoaded()
        {
            base.OnLoaded();

            UserRecord = new Dictionary<string, byte>();
            UserCooldown = new Dictionary<string, DateTime>();
            GuildCooldown = new Dictionary<string, DateTime>();
            SequenceDifficulty = new Dictionary<byte, DifficultyType>();

            StatsChanged();
        }

        public void StatsChanged()
        {
            Stats.Clear();
            foreach (InstanceInfoStat stat in BuffStats)
                Stats[stat.Stat] += stat.Amount;
        }

        public override string ToString()
        {
            return Name;
        }
    }

    public class InstanceMapInfo : DBObject
    {
        [IsIdentity]
        [Association("Map")]
        public InstanceInfo Instance
        {
            get { return _Instance; }
            set
            {
                if (_Instance == value) return;

                var oldValue = _Instance;
                _Instance = value;

                OnChanged(oldValue, value, "Instance");
            }
        }
        private InstanceInfo _Instance;

        [IsIdentity]
        public MapInfo Map
        {
            get { return _Map; }
            set
            {
                if (_Map == value) return;

                var oldValue = _Map;
                _Map = value;

                OnChanged(oldValue, value, "Map");
            }
        }
        private MapInfo _Map;

        public int RespawnIndex
        {
            get { return _RespawnIndex; }
            set
            {
                if (_RespawnIndex == value) return;

                var oldValue = _RespawnIndex;
                _RespawnIndex = value;

                OnChanged(oldValue, value, "RespawnIndex");
            }
        }
        private int _RespawnIndex;
    }

    public sealed class InstanceInfoStat : DBObject
    {
        [IsIdentity]
        [Association("InstanceInfoStats")]
        public InstanceInfo Instance
        {
            get { return _Instance; }
            set
            {
                if (_Instance == value) return;

                var oldValue = _Instance;
                _Instance = value;

                OnChanged(oldValue, value, "Instance");
            }
        }
        private InstanceInfo _Instance;

        [IsIdentity]
        public Stat Stat
        {
            get { return _Stat; }
            set
            {
                if (_Stat == value) return;

                var oldValue = _Stat;
                _Stat = value;

                OnChanged(oldValue, value, "Stat");
            }
        }
        private Stat _Stat;

        public int Amount
        {
            get { return _Amount; }
            set
            {
                if (_Amount == value) return;

                var oldValue = _Amount;
                _Amount = value;

                OnChanged(oldValue, value, "Amount");
            }
        }
        private int _Amount;
    }

    /// <summary>
    /// One phase in a dungeon instance. Phases activate sequentially (ascending PhaseIndex).
    /// A phase becomes active when its entry condition is met on the instance where it is running.
    /// </summary>
    public sealed class InstancePhase : DBObject
    {
        [IsIdentity]
        [Association("Phases")]
        public InstanceInfo Instance
        {
            get { return _Instance; }
            set
            {
                if (_Instance == value) return;
                var oldValue = _Instance;
                _Instance = value;
                OnChanged(oldValue, value, "Instance");
            }
        }
        private InstanceInfo _Instance;

        public int PhaseIndex
        {
            get { return _PhaseIndex; }
            set
            {
                if (_PhaseIndex == value) return;
                var oldValue = _PhaseIndex;
                _PhaseIndex = value;
                OnChanged(oldValue, value, "PhaseIndex");
            }
        }
        private int _PhaseIndex;

        public string Description
        {
            get { return _Description; }
            set
            {
                if (_Description == value) return;
                var oldValue = _Description;
                _Description = value;
                OnChanged(oldValue, value, "Description");
            }
        }
        private string _Description;

        public InstancePhaseConditionType ConditionType
        {
            get { return _ConditionType; }
            set
            {
                if (_ConditionType == value) return;
                var oldValue = _ConditionType;
                _ConditionType = value;
                OnChanged(oldValue, value, "ConditionType");
            }
        }
        private InstancePhaseConditionType _ConditionType;

        /// <summary>MonsterClear: spawn group that must be fully cleared to trigger this phase.</summary>
        public RespawnInfo ConditionRespawn
        {
            get { return _ConditionRespawn; }
            set
            {
                if (_ConditionRespawn == value) return;
                var oldValue = _ConditionRespawn;
                _ConditionRespawn = value;
                OnChanged(oldValue, value, "ConditionRespawn");
            }
        }
        private RespawnInfo _ConditionRespawn;

        /// <summary>Timer: minutes elapsed since instance start before this phase triggers.</summary>
        public int ConditionMinutes
        {
            get { return _ConditionMinutes; }
            set
            {
                if (_ConditionMinutes == value) return;
                var oldValue = _ConditionMinutes;
                _ConditionMinutes = value;
                OnChanged(oldValue, value, "ConditionMinutes");
            }
        }
        private int _ConditionMinutes;

        /// <summary>ItemUsed: item a player must use inside the instance to trigger this phase.</summary>
        public ItemInfo ConditionItem
        {
            get { return _ConditionItem; }
            set
            {
                if (_ConditionItem == value) return;
                var oldValue = _ConditionItem;
                _ConditionItem = value;
                OnChanged(oldValue, value, "ConditionItem");
            }
        }
        private ItemInfo _ConditionItem;

        [Association("Actions", true)]
        public DBBindingList<InstancePhaseAction> Actions { get; set; }

        protected override void OnDeleted()
        {
            Instance = null;
            ConditionRespawn = null;
            ConditionItem = null;
            for (int i = Actions.Count - 1; i >= 0; i--)
                Actions[i].Delete();
            base.OnDeleted();
        }
    }

    /// <summary>
    /// One action executed when an <see cref="InstancePhase"/> becomes active.
    /// </summary>
    public sealed class InstancePhaseAction : DBObject
    {
        [IsIdentity]
        [Association("Actions")]
        public InstancePhase Phase
        {
            get { return _Phase; }
            set
            {
                if (_Phase == value) return;
                var oldValue = _Phase;
                _Phase = value;
                OnChanged(oldValue, value, "Phase");
            }
        }
        private InstancePhase _Phase;

        public InstancePhaseActionType ActionType
        {
            get { return _ActionType; }
            set
            {
                if (_ActionType == value) return;
                var oldValue = _ActionType;
                _ActionType = value;
                OnChanged(oldValue, value, "ActionType");
            }
        }
        private InstancePhaseActionType _ActionType;

        /// <summary>SpawnGroup: the RespawnInfo group to force-spawn.</summary>
        public RespawnInfo ActionRespawn
        {
            get { return _ActionRespawn; }
            set
            {
                if (_ActionRespawn == value) return;
                var oldValue = _ActionRespawn;
                _ActionRespawn = value;
                OnChanged(oldValue, value, "ActionRespawn");
            }
        }
        private RespawnInfo _ActionRespawn;

        /// <summary>UnlockRegion: the MapRegion to remove from LockedRegions.</summary>
        public MapRegion ActionRegion
        {
            get { return _ActionRegion; }
            set
            {
                if (_ActionRegion == value) return;
                var oldValue = _ActionRegion;
                _ActionRegion = value;
                OnChanged(oldValue, value, "ActionRegion");
            }
        }
        private MapRegion _ActionRegion;

        /// <summary>SendMessage: broadcast text to all players in the instance.</summary>
        public string ActionMessage
        {
            get { return _ActionMessage; }
            set
            {
                if (_ActionMessage == value) return;
                var oldValue = _ActionMessage;
                _ActionMessage = value;
                OnChanged(oldValue, value, "ActionMessage");
            }
        }
        private string _ActionMessage;

        /// <summary>AwardItem: item to give to every player in the instance.</summary>
        public ItemInfo ActionItem
        {
            get { return _ActionItem; }
            set
            {
                if (_ActionItem == value) return;
                var oldValue = _ActionItem;
                _ActionItem = value;
                OnChanged(oldValue, value, "ActionItem");
            }
        }
        private ItemInfo _ActionItem;

        /// <summary>AwardItem: quantity of ActionItem to give per player.</summary>
        public int ActionItemCount
        {
            get { return _ActionItemCount; }
            set
            {
                if (_ActionItemCount == value) return;
                var oldValue = _ActionItemCount;
                _ActionItemCount = value;
                OnChanged(oldValue, value, "ActionItemCount");
            }
        }
        private int _ActionItemCount;

        protected override void OnDeleted()
        {
            Phase = null;
            ActionRespawn = null;
            ActionRegion = null;
            ActionItem = null;
            base.OnDeleted();
        }
    }
}