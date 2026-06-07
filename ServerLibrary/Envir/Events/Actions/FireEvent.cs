using Library.SystemModels;
using Server.Models;
using System;
using System.Linq;

namespace Server.Envir.Events.Actions
{
    /// <summary>
    /// Fires a named WorldEvent unconditionally, executing all its actions.
    /// Circular chains are broken at depth 10.
    /// Set WorldEventAction.StringParameter1 to the target event's Description.
    /// </summary>
    [EventActionType(EventActionType.FireEvent)]
    public class FireEvent : IWorldEventAction, IEventAction
    {
        [ThreadStatic]
        private static int _depth;

        public void Act(EventLog log, WorldEventAction action)
        {
            if (string.IsNullOrWhiteSpace(action.StringParameter1)) return;

            if (_depth >= 10)
            {
                SEnvir.SaveError($"[FireEvent] Max chain depth (10) reached; skipping '{action.StringParameter1}'");
                return;
            }

            WorldEventInfo target = SEnvir.WorldEventInfoList?.Binding
                .FirstOrDefault(e => string.Equals(e.Description, action.StringParameter1, StringComparison.OrdinalIgnoreCase));

            if (target == null)
            {
                SEnvir.SaveError($"[FireEvent] Event not found: '{action.StringParameter1}'");
                return;
            }

            _depth++;
            try
            {
                SEnvir.EventHandler.FireWorldEvent(target);
            }
            finally
            {
                _depth--;
            }
        }
    }
}
