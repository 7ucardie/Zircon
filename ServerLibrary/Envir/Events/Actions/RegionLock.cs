using Library.SystemModels;
using Server.Models;

namespace Server.Envir.Events.Actions
{
    [EventActionType(EventActionType.RegionLock)]
    public class RegionLock : IWorldEventAction, IPlayerEventAction, IMonsterEventAction, IEventAction
    {
        public void Act(PlayerObject triggerPlayer, EventLog log, MonsterEventAction action)
        {
            LockRegion(action, triggerPlayer.CurrentMap.Instance, triggerPlayer.CurrentMap.InstanceSequence);
        }

        public void Act(PlayerObject triggerPlayer, EventLog log, PlayerEventAction action)
        {
            LockRegion(action, triggerPlayer.CurrentMap.Instance, triggerPlayer.CurrentMap.InstanceSequence);
        }

        public void Act(EventLog log, WorldEventAction action)
        {
            LockRegion(action, null, 0);
        }

        private static void LockRegion(BaseEventAction action, InstanceInfo instance, byte instanceSequence)
        {
            if (action.RegionParameter1 == null) return;
            if (action.InstanceParameter1 != instance) return;

            Map map = SEnvir.GetMap(action.RegionParameter1.Map, instance, instanceSequence);
            if (map == null) return;

            map.LockedRegions.Add(action.RegionParameter1);
        }
    }
}
