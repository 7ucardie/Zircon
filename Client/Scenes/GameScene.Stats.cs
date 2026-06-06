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
        public void UserChanged()
        {
            LevelChanged();
            ClassChanged();
            StatsChanged();
            ExperienceChanged();
            HealthChanged();
            ManaChanged();
            FocusChanged();
            CurrencyChanged();
            SafeZoneChanged();
            AttackModeChanged();
            PetModeChanged();
            MagicBarBox.UpdateIcons();
            MarketPlaceBox.ConsignTab.TabButton.Visible = !Observer;
            TradeBox.CloseButton.Enabled = !Observer;
            TradeBox.ConfirmButton.Visible = !Observer;

            NPCBox.CloseButton.Enabled = !Observer;
            NPCGoodsBox.CloseButton.Enabled = !Observer;
            NPCRefineBox.CloseButton.Enabled = !Observer;
            NPCRepairBox.CloseButton.Enabled = !Observer;
            NPCRefineRetrieveBox.CloseButton.Enabled = !Observer;
            NPCQuestBox.CloseButton.Enabled = !Observer;
        }
        public void LevelChanged()
        {
            if (User == null) return;

            //User.MaxExperience = User.Level < Globals.ExperienceList.Count ? Globals.ExperienceList[User.Level] : 0;
            MainPanel.LevelLabel.Text = User.Level.ToString();

            foreach (NPCGoodsCell cell in NPCGoodsBox.Cells)
                cell.UpdateColours();

            foreach (KeyValuePair<MagicInfo, MagicCell> pair in MagicBox.Magics)
                pair.Value.Refresh();

            CheckNewQuests();
        }
        public void ClassChanged()
        {
            if (User == null) return;

            MainPanel.ClassLabel.Text = User.Class.ToString();

            foreach (NPCGoodsCell cell in NPCGoodsBox.Cells)
                cell.UpdateColours();

            MainPanel.MCLabel.Visible = User.Class == MirClass.Wizard || User.Class == MirClass.Warrior;
            MainPanel.SCLabel.Visible = User.Class == MirClass.Taoist || User.Class == MirClass.Assassin;

            MagicBox?.CreateTabs();
        }
        public void StatsChanged()
        {
            if (User.Stats == null) return;

            User.Light = Math.Max(3, User.Stats[Stat.Light]);

            if (User.Stats[Stat.Light] == 0)
            {
                User.LightColour = Globals.PlayerLightColour;
            }
            else
            {
                User.LightColour = Globals.NoneColour;
            }

            MainPanel.ACLabel.Text = User.Stats.GetFormat(Stat.MaxAC);
            MainPanel.MACLabel.Text = User.Stats.GetFormat(Stat.MaxMR);

            MainPanel.DCLabel.Text = User.Stats.GetFormat(Stat.MaxDC);
            MainPanel.SCLabel.Text = User.Stats.GetFormat(Stat.MaxSC);
            MainPanel.MCLabel.Text = User.Stats.GetFormat(Stat.MaxMC);

            HealthChanged();
            ManaChanged();
            FocusChanged();

            foreach (NPCGoodsCell cell in NPCGoodsBox.Cells)
                cell.UpdateColours();

            CharacterBox.UpdateStats();

            FilterDropBox.UpdateDropFilters();
        }
        public void ExperienceChanged()
        {
            if (User == null) return;

            MainPanel.ExperienceBar.Hint = User.MaxExperience > 0 ? $"(Experience) {User.Experience / User.MaxExperience:#,##0.00%}" : "(Experience) Max";
        }
        public void HealthChanged()
        {
            if (User == null) return;

            MainPanel.HealthLabel.Text = $"{User.CurrentHP}/{User.Stats[Stat.Health]}";

        }
        public void ManaChanged()
        {
            if (User == null) return;

            MainPanel.ManaLabel.Text = $"{User.CurrentMP}/{User.Stats[Stat.Mana]}";
        }
        public void FocusChanged()
        {
            if (User == null) return;

            MainPanel.FocusLabel.Visible = User.Stats[Stat.Focus] > 0;
            MainPanel.FocusLabel.Text = $"{User.CurrentFP}/{User.Stats[Stat.Focus]}";
        }

        public void AttackModeChanged()
        {
            if (User == null) return;

            Type type = typeof(AttackMode);

            MemberInfo[] infos = type.GetMember(User.AttackMode.ToString());

            DescriptionAttribute description = infos[0].GetCustomAttribute<DescriptionAttribute>();

            MainPanel.AttackModeLabel.Text = description?.Description ?? User.AttackMode.ToString();
        }
        public void PetModeChanged()
        {
            if (User == null) return;

            Type type = typeof(PetMode);

            MemberInfo[] infos = type.GetMember(User.PetMode.ToString());

            DescriptionAttribute description = infos[0].GetCustomAttribute<DescriptionAttribute>();

            MainPanel.PetModeLabel.Text = description?.Description ?? User.PetMode.ToString();
        }
        public void CurrencyChanged()
        {
            if (User == null) return;

            InventoryBox.RefreshCurrency();

            MainPanel.FPLabel.Text = User.GetCurrency(CurrencyType.FP)?.Amount.ToString() ?? "0";
            MainPanel.CPLabel.Text = User.GetCurrency(CurrencyType.CP)?.Amount.ToString() ?? "0";

            MarketPlaceBox.GameGoldBox.Value = User.GameGold.Amount;
            MarketPlaceBox.HuntGoldBox.Value = User.HuntGold.Amount;
            NPCAdoptCompanionBox.RefreshUnlockButton();

            foreach (NPCGoodsCell cell in NPCGoodsBox.Cells)
            {
                cell.UpdateCosts();
                cell.UpdateColours();
            }

            //foreach (CurrencyCell cell in CurrencyBox.Cells)
            //{
            //    cell.UpdateAmount();
            //}
        }
        public void SafeZoneChanged()
        {

        }
        public void WeightChanged()
        {
            if (User == null) return;

            InventoryBox.WeightLabel.Text = $"{User.BagWeight} of {User.Stats[Stat.BagWeight]}";

            InventoryBox.WeightLabel.ForeColour = User.BagWeight > User.Stats[Stat.BagWeight] ? Color.Red : Color.White;

            CharacterBox.WearWeightLabel.Text = $"{User.WearWeight}/{User.Stats[Stat.WearWeight]}";
            CharacterBox.HandWeightLabel.Text = $"{User.HandWeight}/{User.Stats[Stat.HandWeight]}";

            CharacterBox.WearWeightLabel.ForeColour = User.WearWeight > User.Stats[Stat.WearWeight] ? Color.Red : Color.White;
            CharacterBox.HandWeightLabel.ForeColour = User.HandWeight > User.Stats[Stat.HandWeight] ? Color.Red : Color.White;
        }
        public void CompanionChanged()
        {
            NPCCompanionStorageBox.Refresh();

            CompanionBox.CompanionChanged();
        }
        public void MarriageChanged()
        {
            CharacterBox.MarriageIcon.Visible = !string.IsNullOrEmpty(Partner?.Name);
            CharacterBox.MarriageLabel.Visible = !string.IsNullOrEmpty(Partner?.Name);
            CharacterBox.MarriageLabel.Text = Partner?.Name;
        }

    }
}
