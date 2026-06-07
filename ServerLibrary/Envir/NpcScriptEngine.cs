using Library;
using Library.SystemModels;
using MoonSharp.Interpreter;
using Server.DBModels;
using Server.Models;
using System;
using System.Collections.Generic;
using System.IO;

namespace Server.Envir
{
    /// <summary>
    /// Loads, caches, and executes Lua scripts for NPC pages.
    ///
    /// Usage from Lua:
    ///   function on_open(player, npc)   -- called before checks/actions; return false to cancel page
    ///   function my_check(player, npc)  -- NPCCheckType.Script, StringParameter1 = "my_check"
    ///   function my_action(player, npc) -- NPCActionType.Script, StringParameter1 = "my_action"
    ///
    /// Scripts live in the Scripts/NPC/ directory relative to the server executable.
    /// </summary>
    public sealed class NpcScriptEngine
    {
        public static readonly NpcScriptEngine Instance = new();

        private static readonly string ScriptRoot =
            Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "Scripts", "NPC");

        private readonly Dictionary<string, Script> _cache = new(StringComparer.OrdinalIgnoreCase);

        static NpcScriptEngine()
        {
            UserData.RegisterType<LuaPlayerProxy>();
            UserData.RegisterType<LuaNpcProxy>();
        }

        private NpcScriptEngine() { }

        /// <summary>
        /// Executes the on_open hook for a page that has a ScriptFile.
        /// Returns the page name to navigate to, or null to stay on the current page.
        /// Returns a non-null empty string to cancel the page entirely.
        /// </summary>
        public string ExecuteOnOpen(string scriptFile, PlayerObject player, NPCObject npc, NPCPage page)
        {
            Script script = GetScript(scriptFile);
            if (script == null) return null;

            DynValue fn = script.Globals.Get("on_open");
            if (fn.Type != DataType.Function) return null;

            try
            {
                var playerProxy = new LuaPlayerProxy(player);
                var npcProxy = new LuaNpcProxy(player, npc);

                script.Call(fn, playerProxy, npcProxy);
                return npcProxy.NavigateTo;
            }
            catch (ScriptRuntimeException ex)
            {
                SEnvir.SaveError($"[NpcScript] on_open in {scriptFile}: {ex.DecoratedMessage}");
                return null;
            }
        }

        /// <summary>
        /// Executes a check function. Returns true if the check passes.
        /// </summary>
        public bool ExecuteCheck(string scriptFile, string functionName, PlayerObject player, NPCObject npc, NPCPage page)
        {
            if (string.IsNullOrWhiteSpace(functionName)) return true;

            Script script = GetScript(scriptFile);
            if (script == null) return true;

            DynValue fn = script.Globals.Get(functionName);
            if (fn.Type != DataType.Function) return true;

            try
            {
                var playerProxy = new LuaPlayerProxy(player);
                var npcProxy = new LuaNpcProxy(player, npc);

                DynValue result = script.Call(fn, playerProxy, npcProxy);
                return result.Type != DataType.Boolean || result.Boolean;
            }
            catch (ScriptRuntimeException ex)
            {
                SEnvir.SaveError($"[NpcScript] {functionName} in {scriptFile}: {ex.DecoratedMessage}");
                return false;
            }
        }

        /// <summary>
        /// Executes an action function.
        /// </summary>
        public void ExecuteAction(string scriptFile, string functionName, PlayerObject player, NPCObject npc, NPCPage page)
        {
            if (string.IsNullOrWhiteSpace(functionName)) return;

            Script script = GetScript(scriptFile);
            if (script == null) return;

            DynValue fn = script.Globals.Get(functionName);
            if (fn.Type != DataType.Function) return;

            try
            {
                var playerProxy = new LuaPlayerProxy(player);
                var npcProxy = new LuaNpcProxy(player, npc);

                script.Call(fn, playerProxy, npcProxy);
            }
            catch (ScriptRuntimeException ex)
            {
                SEnvir.SaveError($"[NpcScript] {functionName} in {scriptFile}: {ex.DecoratedMessage}");
            }
        }

        private Script GetScript(string fileName)
        {
            if (string.IsNullOrWhiteSpace(fileName)) return null;

            if (_cache.TryGetValue(fileName, out Script cached)) return cached;

            string path = Path.Combine(ScriptRoot, fileName);
            if (!File.Exists(path))
            {
                SEnvir.SaveError($"[NpcScript] Script file not found: {path}");
                _cache[fileName] = null;
                return null;
            }

            try
            {
                Script script = new Script(CoreModules.Preset_SoftSandbox);
                script.DoFile(path);
                _cache[fileName] = script;
                return script;
            }
            catch (ScriptRuntimeException ex)
            {
                SEnvir.SaveError($"[NpcScript] Failed to load {fileName}: {ex.DecoratedMessage}");
                _cache[fileName] = null;
                return null;
            }
        }

        /// <summary>
        /// Evicts all cached scripts so they are reloaded from disk on next use.
        /// </summary>
        public void InvalidateCache()
        {
            _cache.Clear();
        }
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Lua-visible proxy objects
    // ──────────────────────────────────────────────────────────────────────────

    [MoonSharpUserData]
    public sealed class LuaPlayerProxy
    {
        private readonly PlayerObject _player;

        internal LuaPlayerProxy(PlayerObject player)
        {
            _player = player;
        }

        public string name => _player.Name;
        public int level => _player.Level;
        public long gold => _player.Gold?.Amount ?? 0L;

        public bool has_item(string itemName, int count = 1)
        {
            if (count <= 0) return true;
            int found = 0;
            foreach (UserItem item in _player.Inventory)
            {
                if (item == null) continue;
                if (!string.Equals(item.Info?.ItemName, itemName, StringComparison.OrdinalIgnoreCase)) continue;
                found += (int)item.Count;
                if (found >= count) return true;
            }
            return false;
        }
    }

    [MoonSharpUserData]
    public sealed class LuaNpcProxy
    {
        private readonly PlayerObject _player;
        private readonly NPCObject _npc;

        internal string NavigateTo { get; private set; }

        internal LuaNpcProxy(PlayerObject player, NPCObject npc)
        {
            _player = player;
            _npc = npc;
        }

        public void navigate(string pageName)
        {
            NavigateTo = pageName ?? string.Empty;
        }

        public void give_gold(long amount)
        {
            if (amount <= 0) return;
            _player.Gold.Amount += amount;
            _player.GoldChanged();
        }

        public void take_gold(long amount)
        {
            if (amount <= 0 || _player.Gold.Amount < amount) return;
            _player.Gold.Amount -= amount;
            _player.GoldChanged();
        }

        public void give_item(string itemName, int count = 1)
        {
            if (count <= 0 || string.IsNullOrWhiteSpace(itemName)) return;

            ItemInfo info = SEnvir.ItemInfoList.Binding
                .Find(x => string.Equals(x.ItemName, itemName, StringComparison.OrdinalIgnoreCase));
            if (info == null) return;

            ItemCheck check = new ItemCheck(info, count, UserItemFlags.None, TimeSpan.Zero);
            if (!_player.CanGainItems(false, check)) return;

            while (check.Count > 0)
                _player.GainItem(SEnvir.CreateFreshItem(check));
        }

        public void take_item(string itemName, int count = 1)
        {
            if (count <= 0 || string.IsNullOrWhiteSpace(itemName)) return;

            ItemInfo info = SEnvir.ItemInfoList.Binding
                .Find(x => string.Equals(x.ItemName, itemName, StringComparison.OrdinalIgnoreCase));
            if (info == null) return;

            _player.TakeItem(info, count);
        }

        public void message(string text)
        {
            if (string.IsNullOrEmpty(text)) return;
            _player.Connection?.ReceiveChat(text, MessageType.System);
        }
    }
}
