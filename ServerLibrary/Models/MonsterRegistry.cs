using Library.SystemModels;
using System;
using System.Collections.Generic;

namespace Server.Models
{
    /// <summary>
    /// Maps AI identifiers to factory functions that construct the correct MonsterObject subclass.
    /// Call <see cref="MonsterRegistrations.RegisterAll"/> once at server startup, or rely on the
    /// lazy static initialiser below.  Adding a new AI type only requires one new Register() call
    /// in MonsterRegistrations — no switch case needed.
    /// </summary>
    public static class MonsterRegistry
    {
        private static readonly Dictionary<int, Func<MonsterInfo, MonsterObject>> _factories =
            new Dictionary<int, Func<MonsterInfo, MonsterObject>>();

        static MonsterRegistry()
        {
            MonsterRegistrations.RegisterAll();
        }

        internal static void Register(int aiId, Func<MonsterInfo, MonsterObject> factory)
            => _factories[aiId] = factory;

        /// <summary>
        /// Creates the correct MonsterObject for the given MonsterInfo.
        /// Falls back to a base MonsterObject when the AI id is unrecognised.
        /// </summary>
        public static MonsterObject Create(MonsterInfo info)
            => _factories.TryGetValue(info.AI, out Func<MonsterInfo, MonsterObject> factory)
                ? factory(info)
                : new MonsterObject { MonsterInfo = info };
    }
}
