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
        public void ReceiveChat(string message, MessageType type, List<ClientUserItem> linkedItems = null)
        {
            if (Config.LogChat)
                CEnvir.ChatLog.Enqueue($"[{Time.Now:F}]: {message}");

            foreach (ChatTab tab in ChatTab.Tabs)
                tab.ReceiveChat(message, type, linkedItems);
        }
        public void ReceiveChat(MessageAction action, params object[] args)
        {
            foreach (ChatTab tab in ChatTab.Tabs)
                tab.ReceiveChat(action, args);
        }

    }
}
