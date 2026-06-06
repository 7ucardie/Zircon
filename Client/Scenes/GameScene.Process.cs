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
        public override void Process()
        {
            base.Process();

            if (CEnvir.Now >= MoveTime)
            {
                MoveTime = CEnvir.Now.AddMilliseconds(100);
                MapControl.Animation++;
                MoveFrame = true;
            }
            else if (!Config.SmoothMove)
            {
                MoveFrame = false;
            }

            if (MouseControl == MapControl)
                MapControl.CheckCursor();

            if (MouseControl == MapControl)
            {
                if (CEnvir.Ctrl && MapObject.MouseObject?.Race == ObjectType.Item)
                    MouseItem = ((ItemObject)MapObject.MouseObject).Item;
                else
                    MouseItem = null;
            }

            TimeSpan ticks = CEnvir.Now - ItemTime;
            ItemTime = CEnvir.Now;

            if (!User.InSafeZone)
            {
                foreach (ClientUserItem item in Equipment)
                {
                    if ((item?.Flags & UserItemFlags.Expirable) != UserItemFlags.Expirable) continue;

                    item.ExpireTime -= ticks;
                }

                foreach (ClientUserItem item in Inventory)
                {
                    if ((item?.Flags & UserItemFlags.Expirable) != UserItemFlags.Expirable) continue;

                    item.ExpireTime -= ticks;
                }

                if (Companion != null)
                {
                    foreach (ClientUserItem item in Companion.InventoryArray)
                    {
                        if ((item?.Flags & UserItemFlags.Expirable) != UserItemFlags.Expirable) continue;

                        item.ExpireTime -= ticks;
                    }
                    foreach (ClientUserItem item in Companion.EquipmentArray)
                    {
                        if ((item?.Flags & UserItemFlags.Expirable) != UserItemFlags.Expirable) continue;

                        item.ExpireTime -= ticks;
                    }
                }
            }

            if (MouseItem != null && CEnvir.Now > ItemRefreshTime)
            {
                CreateItemLabel();
            }

            MapControl.ProcessInput();

            foreach (MapObject ob in MapControl.Objects)
                ob.Process();

            for (int i = MapControl.Effects.Count - 1; i >= 0; i--)
                MapControl.Effects[i].Process();

            for (int i = MapControl.ParticleEffects.Count - 1; i >= 0; i--)
                MapControl.ParticleEffects[i].Process();

            if (ItemLabel != null && !ItemLabel.IsDisposed)
            {
                int x = CEnvir.MouseLocation.X + 15, y = CEnvir.MouseLocation.Y;

                if (x + ItemLabel.Size.Width > Size.Width + Location.X)
                    x = Size.Width - ItemLabel.Size.Width + Location.X;

                if (y + ItemLabel.Size.Height > Size.Height + Location.Y)
                    y = Size.Height - ItemLabel.Size.Height + Location.Y;

                if (x < Location.X)
                    x = Location.X;

                if (y <= Location.Y)
                    y = Location.Y;

                ItemLabel.Location = new Point(x, y);
            }

            if (MagicLabel != null && !MagicLabel.IsDisposed)
            {
                int x = CEnvir.MouseLocation.X + 15, y = CEnvir.MouseLocation.Y;

                if (x + MagicLabel.Size.Width > Size.Width + Location.X)
                    x = Size.Width - MagicLabel.Size.Width + Location.X;

                if (y + MagicLabel.Size.Height > Size.Height + Location.Y)
                    y = Size.Height - MagicLabel.Size.Height + Location.Y;

                if (x < Location.X)
                    x = Location.X;

                if (y <= Location.Y)
                    y = Location.Y;

                MagicLabel.Location = new Point(x, y);
            }

            if (FameLabel != null && !FameLabel.IsDisposed)
            {
                int x = CEnvir.MouseLocation.X + 15, y = CEnvir.MouseLocation.Y;

                if (x + FameLabel.Size.Width > Size.Width + Location.X)
                    x = Size.Width - FameLabel.Size.Width + Location.X;

                if (y + FameLabel.Size.Height > Size.Height + Location.Y)
                    y = Size.Height - FameLabel.Size.Height + Location.Y;

                if (x < Location.X)
                    x = Location.X;

                if (y <= Location.Y)
                    y = Location.Y;

                FameLabel.Location = new Point(x, y);
            }

            MonsterObject mob = MouseObject as MonsterObject;

            if (mob != null && mob.CompanionObject == null)
                MonsterBox.Monster = mob;
            else
            {
                mob = FocusObject as MonsterObject;
                if (mob != null && mob.CompanionObject == null && !FocusObject.Dead)
                    MonsterBox.Monster = mob;
            }
        }

    }
}
