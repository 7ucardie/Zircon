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
        public override void OnKeyPress(KeyPressEventArgs e)
        {
            base.OnKeyPress(e);

            if (e.Handled) return;

            switch ((Keys)e.KeyChar)
            {
                case Keys.Enter:
                    ChatTextBox.ToggleVisibility(e, false);
                    break;
            }
        }

        public override void OnKeyDown(KeyEventArgs e)
        {
            base.OnKeyDown(e);

            if (e.Handled) return;

            switch (e.KeyCode)
            {
                case Keys.Escape:
                    MonsterBox.Monster = null;
                    e.Handled = true;
                    break;
            }

            foreach (KeyBindAction action in CEnvir.GetKeyAction(e.KeyCode))
            {
                switch (action)
                {
                    case KeyBindAction.MenuWindow:
                        MenuBox.Visible = !MenuBox.Visible;
                        break;
                    case KeyBindAction.HelpWindow:
                        HelpBox.Visible = !HelpBox.Visible;
                        break;
                    case KeyBindAction.ConfigWindow:
                        ConfigBox.Visible = !ConfigBox.Visible;
                        break;
                    case KeyBindAction.RankingWindow:
                        RankingBox.Visible = !RankingBox.Visible && CEnvir.Connection != null;
                        break;
                    case KeyBindAction.CharacterWindow:
                        CharacterBox.Visible = !CharacterBox.Visible;
                        break;
                    case KeyBindAction.InventoryWindow:
                        InventoryBox.Visible = !InventoryBox.Visible;
                        break;
                    case KeyBindAction.FortuneWindow:
                        FortuneCheckerBox.Visible = !FortuneCheckerBox.Visible;
                        break;
                    case KeyBindAction.CurrencyWindow:
                        CurrencyBox.Visible = !CurrencyBox.Visible;
                        break;
                    case KeyBindAction.MagicWindow:
                        MagicBox.Visible = !MagicBox.Visible;
                        break;
                    case KeyBindAction.MagicBarWindow:
                        MagicBarBox.Visible = !MagicBarBox.Visible;
                        break;
                    case KeyBindAction.GameStoreWindow:
                        if (MarketPlaceBox.StoreTab.IsVisible)
                            MarketPlaceBox.Visible = false;
                        else
                        {
                            MarketPlaceBox.Visible = true;
                            MarketPlaceBox.StoreTab.TabButton.InvokeMouseClick();
                        }
                        break;
                    case KeyBindAction.DungeonFinderWindow:
                        DungeonFinderBox.Visible = !DungeonFinderBox.Visible;
                        break;
                    case KeyBindAction.CompanionWindow:
                        CompanionBox.Visible = !CompanionBox.Visible;
                        break;
                    case KeyBindAction.FilterDropWindow:
                        FilterDropBox.Visible = !FilterDropBox.Visible;
                        break;
                    case KeyBindAction.GroupWindow:
                        GroupBox.Visible = !GroupBox.Visible;
                        break;
                    case KeyBindAction.AutoPotionWindow:
                        AutoPotionBox.Visible = !AutoPotionBox.Visible;
                        break;
                    case KeyBindAction.StorageWindow:
                        StorageBox.Visible = !StorageBox.Visible;
                        break;
                    case KeyBindAction.BlockListWindow:
                        CommunicationBox.Visible = !CommunicationBox.Visible;
                        if (CommunicationBox.Visible)
                            CommunicationBox.BlockTab.TabButton.InvokeMouseClick();
                        break;
                    case KeyBindAction.MailSendWindow:
                        CommunicationBox.Visible = !CommunicationBox.Visible;
                        if (CommunicationBox.Visible)
                            CommunicationBox.SendTab.TabButton.InvokeMouseClick();
                        break;
                    case KeyBindAction.GuildWindow:
                        GuildBox.Visible = !GuildBox.Visible;
                        break;
                    case KeyBindAction.QuestLogWindow:
                        QuestBox.Visible = !QuestBox.Visible;
                        break;
                    case KeyBindAction.QuestTrackerWindow:
                        QuestBox.CurrentTab.ShowTrackerBox.Checked = !QuestBox.CurrentTab.ShowTrackerBox.Checked;
                        break;
                    case KeyBindAction.BeltWindow:
                        BeltBox.Visible = !BeltBox.Visible;
                        break;
                    case KeyBindAction.MarketPlaceWindow:
                        if (MarketPlaceBox.ConsignTab.IsVisible || MarketPlaceBox.SearchTab.IsVisible)
                            MarketPlaceBox.Visible = false;
                        else
                        {
                            MarketPlaceBox.Visible = true;
                            MarketPlaceBox.SearchTab.TabButton.InvokeMouseClick();
                        }
                        break;
                    case KeyBindAction.MapMiniWindow:
                        if (!MiniMapBox.Visible)
                        {
                            MiniMapBox.Opacity = 1F;
                            MiniMapBox.Visible = true;
                            return;
                        }

                        if (MiniMapBox.Opacity == 1F)
                        {
                            MiniMapBox.Opacity = 0.5F;
                            return;
                        }

                        MiniMapBox.Visible = false;
                        break;
                    case KeyBindAction.MapBigWindow:
                        BigMapBox.ToggleOpen(!BigMapBox.Visible);
                        break;
                    case KeyBindAction.MailBoxWindow:
                        if (Observer) continue;
                        CommunicationBox.Visible = !CommunicationBox.Visible;
                        break;
                    case KeyBindAction.ChatOptionsWindow:
                        ChatOptionsBox.Visible = !ChatOptionsBox.Visible;
                        break;
                    case KeyBindAction.ExitGameWindow:
                        ExitBox.Visible = true;
                        ExitBox.BringToFront();
                        break;
                    case KeyBindAction.ChangeAttackMode:
                        if (Observer) continue;
                        User.AttackMode = (AttackMode)(((int)User.AttackMode + 1) % 5);
                        CEnvir.Enqueue(new C.ChangeAttackMode { Mode = User.AttackMode });
                        break;
                    case KeyBindAction.ChangePetMode:
                        if (Observer) continue;

                        User.PetMode = (PetMode)(((int)User.PetMode + 1) % 5);
                        CEnvir.Enqueue(new C.ChangePetMode { Mode = User.PetMode });
                        break;
                    case KeyBindAction.GroupAllowSwitch:
                        if (Observer) continue;

                        GroupBox.AllowGroupBox.InvokeMouseClick();
                        break;
                    case KeyBindAction.GroupTarget:
                        if (Observer) continue;

                        if (MouseObject == null || MouseObject.Race != ObjectType.Player) continue;

                        CEnvir.Enqueue(new C.GroupInvite { Name = MouseObject.Name });
                        break;
                    case KeyBindAction.TradeRequest:
                        if (Observer) continue;

                        CEnvir.Enqueue(new C.TradeRequest());
                        break;
                    case KeyBindAction.TradeAllowSwitch:
                        if (Observer) continue;

                        CEnvir.Enqueue(new C.Chat { Text = "@AllowTrade" });
                        break;
                    case KeyBindAction.ChangeChatMode:
                        ChatTextBox.ChatModeButton.InvokeMouseClick();
                        break;
                    case KeyBindAction.ItemPickUp:
                        if (Observer) continue;

                        if (CEnvir.Now > PickUpTime)
                        {
                            CEnvir.Enqueue(new C.PickUp());
                            PickUpTime = CEnvir.Now.AddMilliseconds(250);
                        }
                        break;
                    case KeyBindAction.PartnerTeleport:
                        if (Observer) continue;

                        CEnvir.Enqueue(new C.MarriageTeleport());
                        break;
                    case KeyBindAction.MountToggle:
                        if (Observer) continue;

                        if (CEnvir.Now < User.NextActionTime || User.ActionQueue.Count > 0) return;
                        if (CEnvir.Now < User.ServerTime) return; //Next Server response Time.

                        User.ServerTime = CEnvir.Now.AddSeconds(5);
                        CEnvir.Enqueue(new C.Mount());
                        break;
                    case KeyBindAction.AutoRunToggle:
                        if (Observer) continue;

                        AutoRun = !AutoRun;
                        break;
                    case KeyBindAction.UseBelt01:
                        if (Observer) continue;
                        if (e.Shift && Config.ShiftOpenChat) return;

                        if (BeltBox.Grid.Grid.Length > 0)
                        {
                            if (SelectedCell != null)
                                SelectedCell.MoveItem(BeltBox.Grid.Grid[0]);
                            else
                                BeltBox.Grid.Grid[0].UseItem();
                        }
                        break;
                    case KeyBindAction.UseBelt02:
                        if (Observer) continue;

                        if (BeltBox.Grid.Grid.Length > 1)
                        {
                            if (SelectedCell != null)
                                SelectedCell.MoveItem(BeltBox.Grid.Grid[1]);
                            else
                                BeltBox.Grid.Grid[1].UseItem();
                        }
                        break;
                    case KeyBindAction.UseBelt03:
                        if (Observer) continue;

                        if (BeltBox.Grid.Grid.Length > 2)
                        {
                            if (SelectedCell != null)
                                SelectedCell.MoveItem(BeltBox.Grid.Grid[2]);
                            else
                                BeltBox.Grid.Grid[2].UseItem();
                        }
                        break;
                    case KeyBindAction.UseBelt04:
                        if (Observer) continue;


                        if (BeltBox.Grid.Grid.Length > 3)
                        {
                            if (SelectedCell != null)
                                SelectedCell.MoveItem(BeltBox.Grid.Grid[3]);
                            else
                                BeltBox.Grid.Grid[3].UseItem();
                        }
                        break;
                    case KeyBindAction.UseBelt05:
                        if (Observer) continue;


                        if (BeltBox.Grid.Grid.Length > 4)
                        {
                            if (SelectedCell != null)
                                SelectedCell.MoveItem(BeltBox.Grid.Grid[4]);
                            else
                                BeltBox.Grid.Grid[4].UseItem();
                        }
                        break;
                    case KeyBindAction.UseBelt06:
                        if (Observer) continue;


                        if (BeltBox.Grid.Grid.Length > 5)
                        {
                            if (SelectedCell != null)
                                SelectedCell.MoveItem(BeltBox.Grid.Grid[5]);
                            else
                                BeltBox.Grid.Grid[5].UseItem();
                        }
                        break;
                    case KeyBindAction.UseBelt07:
                        if (Observer) continue;


                        if (BeltBox.Grid.Grid.Length > 6)
                        {
                            if (SelectedCell != null)
                                SelectedCell.MoveItem(BeltBox.Grid.Grid[6]);
                            else
                                BeltBox.Grid.Grid[6].UseItem();
                        }
                        break;
                    case KeyBindAction.UseBelt08:
                        if (Observer) continue;


                        if (BeltBox.Grid.Grid.Length > 7)
                        {
                            if (SelectedCell != null)
                                SelectedCell.MoveItem(BeltBox.Grid.Grid[7]);
                            else
                                BeltBox.Grid.Grid[7].UseItem();
                        }
                        break;
                    case KeyBindAction.UseBelt09:
                        if (Observer) continue;


                        if (BeltBox.Grid.Grid.Length > 8)
                        {
                            if (SelectedCell != null)
                                SelectedCell.MoveItem(BeltBox.Grid.Grid[8]);
                            else
                                BeltBox.Grid.Grid[8].UseItem();
                        }
                        break;
                    case KeyBindAction.UseBelt10:
                        if (Observer) continue;


                        if (BeltBox.Grid.Grid.Length > 9)
                        {
                            if (SelectedCell != null)
                                SelectedCell.MoveItem(BeltBox.Grid.Grid[9]);
                            else
                                BeltBox.Grid.Grid[9].UseItem();
                        }
                        break;

                    case KeyBindAction.SpellSet01:
                        MagicBarBox.SpellSet = 1;
                        break;
                    case KeyBindAction.SpellSet02:
                        MagicBarBox.SpellSet = 2;
                        break;
                    case KeyBindAction.SpellSet03:
                        MagicBarBox.SpellSet = 3;
                        break;
                    case KeyBindAction.SpellSet04:
                        MagicBarBox.SpellSet = 4;
                        break;

                    case KeyBindAction.SpellUse01:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell01);
                        break;
                    case KeyBindAction.SpellUse02:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell02);
                        break;
                    case KeyBindAction.SpellUse03:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell03);
                        break;
                    case KeyBindAction.SpellUse04:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell04);
                        break;
                    case KeyBindAction.SpellUse05:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell05);
                        break;
                    case KeyBindAction.SpellUse06:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell06);
                        break;
                    case KeyBindAction.SpellUse07:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell07);
                        break;
                    case KeyBindAction.SpellUse08:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell08);
                        break;
                    case KeyBindAction.SpellUse09:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell09);
                        break;
                    case KeyBindAction.SpellUse10:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell10);
                        break;
                    case KeyBindAction.SpellUse11:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell11);
                        break;
                    case KeyBindAction.SpellUse12:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell12);
                        break;
                    case KeyBindAction.SpellUse13:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell13);
                        break;
                    case KeyBindAction.SpellUse14:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell14);
                        break;
                    case KeyBindAction.SpellUse15:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell15);
                        break;
                    case KeyBindAction.SpellUse16:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell16);
                        break;
                    case KeyBindAction.SpellUse17:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell17);
                        break;
                    case KeyBindAction.SpellUse18:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell18);
                        break;
                    case KeyBindAction.SpellUse19:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell19);
                        break;
                    case KeyBindAction.SpellUse20:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell20);
                        break;
                    case KeyBindAction.SpellUse21:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell21);
                        break;
                    case KeyBindAction.SpellUse22:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell22);
                        break;
                    case KeyBindAction.SpellUse23:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell23);
                        break;
                    case KeyBindAction.SpellUse24:
                        if (Observer) continue;

                        UseMagic(SpellKey.Spell24);
                        break;
                    default:
                        continue;
                }

                e.Handled = true;
            }
        }

    }
}
