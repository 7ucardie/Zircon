using Cronos;
using Library.SystemModels;
using System;

namespace Server.Envir.Events.Triggers
{
    /// <summary>
    /// Fires when a cron expression matches the current minute.
    /// Set WorldEventTrigger.CronExpression to a standard 5-field cron expression:
    ///   "0 20 * * 6"   — every Saturday at 8:00 pm
    ///   "0 12 * * *"   — daily at noon
    ///   "0 */4 * * *"  — every 4 hours
    /// </summary>
    [EventTriggerType("SCHEDULEDTIME")]
    public class ScheduledTime : IWorldEventTrigger, IEventTrigger
    {
        public WorldEventTriggerType[] WorldTypes => [WorldEventTriggerType.ScheduledTime];

        public bool Check(WorldEventTrigger eventTrigger)
        {
            if (string.IsNullOrWhiteSpace(eventTrigger.CronExpression)) return false;

            try
            {
                CronExpression expression = CronExpression.Parse(eventTrigger.CronExpression);
                DateTimeOffset from = new DateTimeOffset(SEnvir.Now.AddMinutes(-1), TimeZoneInfo.Local.GetUtcOffset(SEnvir.Now));
                DateTimeOffset? next = expression.GetNextOccurrence(from, TimeZoneInfo.Local);
                return next.HasValue && next.Value.DateTime <= SEnvir.Now;
            }
            catch (CronFormatException ex)
            {
                SEnvir.SaveError($"[ScheduledTime] Invalid cron expression '{eventTrigger.CronExpression}': {ex.Message}");
                return false;
            }
        }
    }
}
