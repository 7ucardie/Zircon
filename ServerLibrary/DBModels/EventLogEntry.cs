using Library.SystemModels;
using MirDB;

namespace Server.DBModels
{
    [UserObject]
    public sealed class EventLogEntry : DBObject
    {
        public string Key
        {
            get { return _Key; }
            set
            {
                if (_Key == value) return;
                var oldValue = _Key;
                _Key = value;
                OnChanged(oldValue, value, "Key");
            }
        }
        private string _Key;

        public WorldEventInfo WorldEvent
        {
            get { return _WorldEvent; }
            set
            {
                if (_WorldEvent == value) return;
                var oldValue = _WorldEvent;
                _WorldEvent = value;
                OnChanged(oldValue, value, "WorldEvent");
            }
        }
        private WorldEventInfo _WorldEvent;

        public PlayerEventInfo PlayerEvent
        {
            get { return _PlayerEvent; }
            set
            {
                if (_PlayerEvent == value) return;
                var oldValue = _PlayerEvent;
                _PlayerEvent = value;
                OnChanged(oldValue, value, "PlayerEvent");
            }
        }
        private PlayerEventInfo _PlayerEvent;

        public MonsterEventInfo MonsterEvent
        {
            get { return _MonsterEvent; }
            set
            {
                if (_MonsterEvent == value) return;
                var oldValue = _MonsterEvent;
                _MonsterEvent = value;
                OnChanged(oldValue, value, "MonsterEvent");
            }
        }
        private MonsterEventInfo _MonsterEvent;

        public int PlayerIndex
        {
            get { return _PlayerIndex; }
            set
            {
                if (_PlayerIndex == value) return;
                var oldValue = _PlayerIndex;
                _PlayerIndex = value;
                OnChanged(oldValue, value, "PlayerIndex");
            }
        }
        private int _PlayerIndex;

        public InstanceInfo InstanceInfo
        {
            get { return _InstanceInfo; }
            set
            {
                if (_InstanceInfo == value) return;
                var oldValue = _InstanceInfo;
                _InstanceInfo = value;
                OnChanged(oldValue, value, "InstanceInfo");
            }
        }
        private InstanceInfo _InstanceInfo;

        public byte InstanceSequence
        {
            get { return _InstanceSequence; }
            set
            {
                if (_InstanceSequence == value) return;
                var oldValue = _InstanceSequence;
                _InstanceSequence = value;
                OnChanged(oldValue, value, "InstanceSequence");
            }
        }
        private byte _InstanceSequence;

        public int CurrentValue
        {
            get { return _CurrentValue; }
            set
            {
                if (_CurrentValue == value) return;
                var oldValue = _CurrentValue;
                _CurrentValue = value;
                OnChanged(oldValue, value, "CurrentValue");
            }
        }
        private int _CurrentValue;

        public string TriggerCountData
        {
            get { return _TriggerCountData; }
            set
            {
                if (_TriggerCountData == value) return;
                var oldValue = _TriggerCountData;
                _TriggerCountData = value;
                OnChanged(oldValue, value, "TriggerCountData");
            }
        }
        private string _TriggerCountData;

        protected override void OnDeleted()
        {
            WorldEvent = null;
            PlayerEvent = null;
            MonsterEvent = null;
            InstanceInfo = null;
            base.OnDeleted();
        }
    }
}
