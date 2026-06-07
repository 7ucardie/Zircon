using Library.SystemModels;
using Server.Models;

namespace Server.Envir.Events.Actions
{
    [EventActionType(EventActionType.RegionUnlock)]
    public class RegionUnlock : IWorldEventAction, IPlayerEventAction, IMonsterEventAction, IEventAction
    {
        public void Act(PlayerObject triggerPlayer, EventLog log, MonsterEventAction action)
        {
            UnlockRegion(action, triggerPlayer.CurrentMap.Instance, triggerPlayer.CurrentMap.InstanceSequence);
        }

        public void Act(PlayerObject triggerPlayer, EventLog log, PlayerEventAction action)
        {
            UnlockRegion(action, triggerPlayer.CurrentMap.Instance, triggerPlayer.CurrentMap.InstanceSequence);
        }

        public void Act(EventLog log, WorldEventAction action)
        {
            UnlockRegion(action, null, 0);
        }

        private static void UnlockRegion(BaseEventAction action, InstanceInfo instance, byte instanceSequence)
        {
            if (action.RegionParameter1 == null) return;
            if (action.InstanceParameter1 != instance) return;

            Map map = SEnvir.GetMap(action.RegionParameter1.Map, instance, instanceSequence);
            if (map == null) return;

            map.LockedRegions.Remove(action.RegionParameter1);
        }
    }
}
