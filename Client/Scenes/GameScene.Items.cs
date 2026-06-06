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
        public void FillItems(List<ClientUserItem> items)
        {
            foreach (ClientUserItem item in items)
            {
                if (item.Slot >= Globals.EquipmentOffSet)
                {
                    CharacterBox.Grid[item.Slot - Globals.EquipmentOffSet].Item = item;
                    continue;
                }

                InventoryBox.Grid.Grid[item.Slot].Item = item;
            }
        }
        public void AddItems(List<ClientUserItem> items)
        {
            foreach (ClientUserItem item in items)
            {
                if (item.Info.ItemEffect == ItemEffect.Experience) continue;
                if ((item.Flags & UserItemFlags.QuestItem) == UserItemFlags.QuestItem) continue;

                var currency = User.GetCurrency(item.Info);
                if (currency != null)
                {
                    currency.Amount += item.Count;

                    GameScene.Game.CurrencyChanged();

                    if (currency.Info.Type == CurrencyType.Gold)
                        DXSoundManager.Play(SoundIndex.GoldGained);

                    continue;
                }

                bool handled = false;
                if (item.Info.StackSize > 1 && (item.Flags & UserItemFlags.Expirable) != UserItemFlags.Expirable)
                {
                    foreach (DXItemCell cell in InventoryBox.Grid.Grid)
                    {
                        if (cell.Item == null || cell.Item.Info != item.Info || cell.Item.Count >= cell.Item.Info.StackSize) continue;

                        if ((cell.Item.Flags & UserItemFlags.Expirable) == UserItemFlags.Expirable) continue;
                        if ((cell.Item.Flags & UserItemFlags.Bound) != (item.Flags & UserItemFlags.Bound)) continue;
                        if ((cell.Item.Flags & UserItemFlags.Worthless) != (item.Flags & UserItemFlags.Worthless)) continue;
                        if ((cell.Item.Flags & UserItemFlags.NonRefinable) != (item.Flags & UserItemFlags.NonRefinable)) continue;
                        if (!cell.Item.AddedStats.Compare(item.AddedStats)) continue;

                        if (cell.Item.Count + item.Count <= item.Info.StackSize)
                        {
                            cell.Item.Count += item.Count;
                            cell.RefreshItem();
                            handled = true;
                            break;
                        }

                        item.Count -= item.Info.StackSize - cell.Item.Count;
                        cell.Item.Count = item.Info.StackSize;
                        cell.RefreshItem();
                    }
                    if (handled) continue;
                }

                for (int i = 0; i < InventoryBox.Grid.Grid.Length; i++)
                {
                    if (InventoryBox.Grid.Grid[i].Item != null) continue;

                    InventoryBox.Grid.Grid[i].Item = item;
                    item.Slot = i;
                    break;
                }
            }
        }
        public void AddCompanionItems(List<ClientUserItem> items)
        {
            foreach (ClientUserItem item in items)
            {
                if (item.Info.ItemEffect == ItemEffect.Experience) continue;
                if ((item.Flags & UserItemFlags.QuestItem) == UserItemFlags.QuestItem) continue;

                var currency = User.GetCurrency(item.Info);
                if (currency != null)
                {
                    currency.Amount += item.Count;

                    GameScene.Game.CurrencyChanged();

                    if (currency.Info.Type == CurrencyType.Gold)
                        DXSoundManager.Play(SoundIndex.GoldGained);

                    continue;
                }

                bool handled = false;
                if (item.Info.StackSize > 1 && (item.Flags & UserItemFlags.Expirable) != UserItemFlags.Expirable)
                {
                    foreach (DXItemCell cell in CompanionBox.InventoryGrid.Grid)
                    {
                        if (cell.Item == null || cell.Item.Info != item.Info || cell.Item.Count >= cell.Item.Info.StackSize) continue;

                        if ((cell.Item.Flags & UserItemFlags.Expirable) == UserItemFlags.Expirable) continue;
                        if ((cell.Item.Flags & UserItemFlags.Bound) != (item.Flags & UserItemFlags.Bound)) continue;
                        if ((cell.Item.Flags & UserItemFlags.Worthless) != (item.Flags & UserItemFlags.Worthless)) continue;
                        if ((cell.Item.Flags & UserItemFlags.NonRefinable) != (item.Flags & UserItemFlags.NonRefinable)) continue;
                        if (!cell.Item.AddedStats.Compare(item.AddedStats)) continue;

                        if (cell.Item.Count + item.Count <= item.Info.StackSize)
                        {
                            cell.Item.Count += item.Count;
                            cell.RefreshItem();
                            handled = true;
                            break;
                        }

                        item.Count -= item.Info.StackSize - cell.Item.Count;
                        cell.Item.Count = item.Info.StackSize;
                        cell.RefreshItem();
                    }
                    if (handled) continue;
                }

                for (int i = 0; i < CompanionBox.InventoryGrid.Grid.Length; i++)
                {
                    if (CompanionBox.InventoryGrid.Grid[i].Item != null) continue;

                    CompanionBox.InventoryGrid.Grid[i].Item = item;
                    item.Slot = i;
                    break;
                }
            }
        }
        public bool CanUseItem(ClientUserItem item)
        {
            switch (User.Gender)
            {
                case MirGender.Male:
                    if (!item.Info.RequiredGender.HasFlag(RequiredGender.Male))
                        return false;
                    break;
                case MirGender.Female:
                    if (!item.Info.RequiredGender.HasFlag(RequiredGender.Female))
                        return false;
                    break;
            }

            switch (User.Class)
            {
                case MirClass.Warrior:
                    if (!item.Info.RequiredClass.HasFlag(RequiredClass.Warrior))
                        return false;
                    break;
                case MirClass.Wizard:
                    if (!item.Info.RequiredClass.HasFlag(RequiredClass.Wizard))
                        return false;
                    break;
                case MirClass.Taoist:
                    if (!item.Info.RequiredClass.HasFlag(RequiredClass.Taoist))
                        return false;
                    break;
                case MirClass.Assassin:
                    if (!item.Info.RequiredClass.HasFlag(RequiredClass.Assassin))
                        return false;
                    break;
            }
            switch (item.Info.RequiredType)
            {
                case RequiredType.Level:
                    if (User.Level < item.Info.RequiredAmount && User.Stats[Stat.Rebirth] == 0) return false;
                    break;
                case RequiredType.MaxLevel:
                    if (User.Level > item.Info.RequiredAmount || User.Stats[Stat.Rebirth] > 0) return false;
                    break;
                case RequiredType.AC:
                    if (User.Stats[Stat.MaxAC] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.MR:
                    if (User.Stats[Stat.MaxMR] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.DC:
                    if (User.Stats[Stat.MaxDC] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.MC:
                    if (User.Stats[Stat.MaxMC] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.SC:
                    if (User.Stats[Stat.MaxSC] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.Health:
                    if (User.Stats[Stat.Health] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.Mana:
                    if (User.Stats[Stat.Mana] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.Accuracy:
                    if (User.Stats[Stat.Accuracy] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.Agility:
                    if (User.Stats[Stat.Agility] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.CompanionLevel:
                    if (Companion == null || Companion.Level < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.MaxCompanionLevel:
                    if (Companion == null || Companion.Level > item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.RebirthLevel:
                    if (User.Stats[Stat.Rebirth] < item.Info.RequiredAmount) return false;
                    break;
                case RequiredType.MaxRebirthLevel:
                    if (User.Stats[Stat.Rebirth] > item.Info.RequiredAmount) return false;
                    break;
            }

            switch (item.Info.ItemType)
            {
                case ItemType.Book:
                    MagicInfo magic = Globals.MagicInfoList.Binding.FirstOrDefault(x => x.Index == item.Info.Shape);
                    if (magic == null) return false;
                    if (magic.School == MagicSchool.None) return false;
                    if (User.Magics.TryGetValue(magic, out ClientUserMagic value) && (value.Level < 3 || (item.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable)) return false;
                    break;
                case ItemType.Consumable:
                    switch (item.Info.Shape)
                    {
                        case 1: //Item Buffs

                            ClientBuffInfo buff = User.Buffs.FirstOrDefault(x => x.Type == BuffType.ItemBuff && x.ItemIndex == item.Info.Index);

                            if (buff != null && buff.RemainingTime == TimeSpan.MaxValue) return false;
                            break;
                    }
                    break;
            }

            return true;
        }

        public bool CanWearItem(ClientUserItem item, EquipmentSlot slot)
        {
            if (!CanUseItem(item)) return false;

            switch (slot)
            {
                case EquipmentSlot.Weapon:
                case EquipmentSlot.Torch:
                case EquipmentSlot.Shield:
                    if (User.HandWeight - (Equipment[(int)slot]?.Info.Weight ?? 0) + item.Weight > User.Stats[Stat.HandWeight])
                    {
                        ReceiveChat(string.Format(CEnvir.Language.GameSceneHoldTooHeavy, item.Info.ItemName), MessageType.System);
                        return false;
                    }
                    break;
                case EquipmentSlot.Hook:
                case EquipmentSlot.Float:
                case EquipmentSlot.Bait:
                case EquipmentSlot.Finder:
                case EquipmentSlot.Reel:
                    if (Equipment[(int)EquipmentSlot.Weapon]?.Info.ItemEffect != ItemEffect.FishingRod)
                    {
                        ReceiveChat(string.Format(CEnvir.Language.GameSceneNeedFishingRod, item.Info.ItemName), MessageType.System);
                        return false;
                    }
                    break;
                default:
                    if (User.WearWeight - (Equipment[(int)slot]?.Info.Weight ?? 0) + item.Weight > User.Stats[Stat.WearWeight])
                    {
                        ReceiveChat(string.Format(CEnvir.Language.GameSceneWearTooHeavy, item.Info.ItemName), MessageType.System);
                        return false;
                    }
                    break;
            }

            return true;
        }

        public bool CanCompanionWearItem(ClientUserItem item, CompanionSlot slot)
        {
            if (Companion == null) return false;

            if (!CanCompanionUseItem(item.Info)) return false;

            return true;
        }
        public bool CanCompanionUseItem(ItemInfo info)
        {
            switch (info.RequiredType)
            {
                case RequiredType.CompanionLevel:
                    if (Companion == null || Companion.Level < info.RequiredAmount) return false;
                    break;
                case RequiredType.MaxCompanionLevel:
                    if (Companion == null || Companion.Level > info.RequiredAmount) return false;
                    break;
            }


            return true;
        }

    }
}
