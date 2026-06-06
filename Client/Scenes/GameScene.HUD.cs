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
        public void UpdateMapIcon(DXImageControl control, MapIcon icon)
        {
            switch (icon)
            {
                case MapIcon.Cave:
                    control.Index = 1;
                    control.ForeColour = Color.Red;
                    break;
                case MapIcon.Exit:
                    control.Index = 1;
                    control.ForeColour = Color.Green;
                    break;
                case MapIcon.Down:
                    control.Index = 1;
                    control.ForeColour = Color.MediumVioletRed;
                    break;
                case MapIcon.Up:
                    control.Index = 1;
                    control.ForeColour = Color.DeepSkyBlue;
                    break;
                case MapIcon.Province:
                    control.Index = 7;
                    break;
                case MapIcon.Building:
                    control.Index = 6;
                    break;
                default:
                    control.Index = (int)icon;
                    break;
            }
        }

        public bool IsAlly(uint objectID)
        {
            if (User.ObjectID == objectID) return true;

            if (Partner != null && Partner.ObjectID == objectID) return true;

            foreach (ClientPlayerInfo member in GroupBox.Members)
                if (member.ObjectID == objectID) return true;

            if (GuildBox.GuildInfo != null)
                foreach (ClientGuildMemberInfo member in GuildBox.GuildInfo.Members)
                    if (member.ObjectID == objectID) return true;

            return false;
        }


    }
}
