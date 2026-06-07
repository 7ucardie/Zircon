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
        private void CreateItemLabel()
        {
            if (ItemLabel != null && !ItemLabel.IsDisposed) ItemLabel.Dispose();

            if (MouseItem == null) return;

            ItemRefreshTime = CEnvir.Now.AddSeconds(1);

            Stats stats = new Stats();
            stats.Add(MouseItem.Info.Stats);
            stats.Add(MouseItem.AddedStats);

            ItemLabel = new DXControl
            {
                BackColour = Color.FromArgb(200, 0, 24, 48),
                Border = true,
                BorderColour = Color.Yellow, // Color.FromArgb(144, 148, 48),
                DrawTexture = true,
                IsControl = false,
                IsVisible = true,
            };

            ItemInfo displayInfo = MouseItem.Info;

            if (MouseItem.Info.ItemEffect == ItemEffect.ItemPart)
                displayInfo = Globals.ItemInfoList.Binding.First(x => x.Index == MouseItem.AddedStats[Stat.ItemIndex]);


            DXLabel label = new DXLabel
            {
                ForeColour = Color.Yellow,
                Location = new Point(4, 4),
                Parent = ItemLabel,
                Text = displayInfo.ItemName
            };

            if (MouseItem.Info.ItemEffect == ItemEffect.ItemPart)
                label.Text += " - [Part]";
            ItemLabel.Size = new Size(label.DisplayArea.Right + 4, label.DisplayArea.Bottom);




            bool needSpacer = false;
            if (displayInfo.ItemType != ItemType.Nothing)
            {
                label = new DXLabel
                {
                    ForeColour = Color.Yellow,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = $"{displayInfo.ItemType}",
                };


                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                needSpacer = true;

            }

            if (MouseItem.Info.Weight > 0)
            {
                label = new DXLabel
                {
                    ForeColour = Color.White,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = $"Weight: {MouseItem.Info.Weight}",
                };

                switch (MouseItem.Info.ItemType)
                {
                    case ItemType.Weapon:
                    case ItemType.Shield:
                    case ItemType.Torch:
                        if (User.HandWeight - (Equipment[(int)EquipmentSlot.Weapon]?.Info.Weight ?? 0) + MouseItem.Info.Weight > User.Stats[Stat.HandWeight])
                            label.ForeColour = Color.Red;
                        break;
                    case ItemType.Armour:
                    case ItemType.Helmet:
                    case ItemType.Necklace:
                    case ItemType.Bracelet:
                    case ItemType.Ring:
                    case ItemType.Shoes:
                    case ItemType.Poison:
                    case ItemType.Amulet:
                        if (User.WearWeight - (Equipment[(int)EquipmentSlot.Armour]?.Info.Weight ?? 0) + MouseItem.Info.Weight > User.Stats[Stat.WearWeight])
                            label.ForeColour = Color.Red;
                        break;
                }

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                needSpacer = true;
            }

            if (needSpacer)
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 4);

            if (CEnvir.IsCurrencyItem(MouseItem.Info) || MouseItem.Info.ItemEffect == ItemEffect.Experience)
            {
                label = new DXLabel
                {
                    ForeColour = Color.Yellow,
                    Location = new Point(ItemLabel.DisplayArea.Right, 4),
                    Parent = ItemLabel,
                    Text = $"Amount: {MouseItem.Count:#,##0}"
                };
                ItemLabel.Size = new Size(label.DisplayArea.Right + 4, ItemLabel.Size.Height + 4);

                if (!string.IsNullOrEmpty(displayInfo.Description))
                {
                    label = new DXLabel
                    {
                        ForeColour = Color.Wheat,
                        Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                        Parent = ItemLabel,
                        Text = displayInfo.Description,
                    };

                    ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                        label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                    ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 4);
                }

                return;
            }


            if (MouseItem.Info.ItemEffect == ItemEffect.ItemPart)
            {
                label = new DXLabel
                {
                    ForeColour = Color.LightSeaGreen,
                    Location = new Point(ItemLabel.DisplayArea.Right, 4),
                    Parent = ItemLabel,
                    Text = $"Parts: {MouseItem.Count}/{displayInfo.PartCount}.",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4, ItemLabel.Size.Height);
            }
            else if (MouseItem.Info.StackSize > 1)
            {
                label = new DXLabel
                {
                    ForeColour = Color.Yellow,
                    Location = new Point(ItemLabel.DisplayArea.Right, 4),
                    Parent = ItemLabel,
                    Text = $"Count: {MouseItem.Count}/{MouseItem.Info.StackSize}"
                };
                ItemLabel.Size = new Size(label.DisplayArea.Right + 4, ItemLabel.Size.Height);
            }

            switch (displayInfo.ItemType)
            {
                case ItemType.Consumable:
                case ItemType.Scroll:
                    if (MouseItem.Info.ItemEffect == ItemEffect.StatExtractor || MouseItem.Info.ItemEffect == ItemEffect.RefineExtractor)
                        EquipmentItemInfo();
                    else
                        CreatePotionLabel();
                    break;
                case ItemType.Book:
                    if (MouseItem.Info.Durability > 0)
                    {
                        label = new DXLabel
                        {
                            ForeColour = Color.White,
                            Location = new Point(ItemLabel.DisplayArea.Right, 4),
                            Parent = ItemLabel,
                            Text = $"Pages: {MouseItem.CurrentDurability}/{MouseItem.MaxDurability}",
                        };

                        ItemLabel.Size = new Size(label.DisplayArea.Right + 4, ItemLabel.Size.Height);
                    }
                    break;
                case ItemType.Meat:
                    if (MouseItem.Info.Durability > 0)
                    {
                        label = new DXLabel
                        {
                            ForeColour = MouseItem.CurrentDurability == 0 ? Color.Red : Color.White,
                            Location = new Point(ItemLabel.DisplayArea.Right, 4),
                            Parent = ItemLabel,
                            Text = $"Quality: {Math.Round(MouseItem.CurrentDurability / 1000M)}/{Math.Round(MouseItem.MaxDurability / 1000M)}",
                        };

                        ItemLabel.Size = new Size(label.DisplayArea.Right + 4, ItemLabel.Size.Height);
                    }
                    break;
                case ItemType.Ore:
                    if (MouseItem.Info.Durability > 0)
                    {
                        label = new DXLabel
                        {
                            ForeColour = MouseItem.CurrentDurability == 0 ? Color.Red : Color.White,
                            Location = new Point(ItemLabel.DisplayArea.Right, 4),
                            Parent = ItemLabel,
                            Text = $"Purity: {Math.Round(MouseItem.CurrentDurability / 1000M)}",
                        };

                        ItemLabel.Size = new Size(label.DisplayArea.Right + 4, ItemLabel.Size.Height);
                    }
                    break;
                case ItemType.Bundle:
                    break;
                case ItemType.LootBox:

                    var remainingRerolls = MouseItem.AddedStats[Stat.Counter1];
                    var lootBoxState = MouseItem.AddedStats[Stat.Counter2];

                    if (lootBoxState > 1)
                    {
                        var openCount = 0;

                        for (int i = 0; i < LootBoxInfo.SlotSize; i++)
                        {
                            if ((MouseItem.CurrentDurability & (1 << i)) != 0)
                                openCount++;
                        }

                        label = new DXLabel
                        {
                            ForeColour = Color.Yellow,
                            Location = new Point(ItemLabel.DisplayArea.Right, 4),
                            Parent = ItemLabel,
                            Text = $"Open Count: {openCount}/{LootBoxInfo.SlotSize}",
                        };

                        ItemLabel.Size = new Size(label.DisplayArea.Right + 4, ItemLabel.Size.Height);
                    }
                    else
                    {
                        label = new DXLabel
                        {
                            ForeColour = Color.Yellow,
                            Location = new Point(ItemLabel.DisplayArea.Right, 4),
                            Parent = ItemLabel,
                            Text = $"Reroll Count: {remainingRerolls}/{Globals.LootBoxRerollCount}",
                        };

                        ItemLabel.Size = new Size(label.DisplayArea.Right + 4, ItemLabel.Size.Height);
                    }
                    break;
                default:
                    EquipmentItemInfo();
                    break;
            }

            if (displayInfo.RequiredGender != RequiredGender.None)
            {
                Color colour = Color.White;
                switch (User.Gender)
                {
                    case MirGender.Male:
                        if (!displayInfo.RequiredGender.HasFlag(RequiredGender.Male))
                            colour = Color.Red;
                        break;
                    case MirGender.Female:
                        if (!displayInfo.RequiredGender.HasFlag(RequiredGender.Female))
                            colour = Color.Red;
                        break;
                }

                label = new DXLabel
                {
                    ForeColour = colour,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = $"Required Gender: {MouseItem.Info.RequiredGender}",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
            }

            if (displayInfo.RequiredClass != RequiredClass.All)
            {
                Color colour = Color.White;
                switch (User.Class)
                {
                    case MirClass.Warrior:
                        if (!MouseItem.Info.RequiredClass.HasFlag(RequiredClass.Warrior))
                            colour = Color.Red;
                        break;
                    case MirClass.Wizard:
                        if (!MouseItem.Info.RequiredClass.HasFlag(RequiredClass.Wizard))
                            colour = Color.Red;
                        break;
                    case MirClass.Taoist:
                        if (!MouseItem.Info.RequiredClass.HasFlag(RequiredClass.Taoist))
                            colour = Color.Red;
                        break;
                    case MirClass.Assassin:
                        if (!MouseItem.Info.RequiredClass.HasFlag(RequiredClass.Assassin))
                            colour = Color.Red;
                        break;
                }

                Type type = displayInfo.RequiredClass.GetType();

                MemberInfo[] infos = type.GetMember(displayInfo.RequiredClass.ToString());

                DescriptionAttribute description = infos[0].GetCustomAttribute<DescriptionAttribute>();

                label = new DXLabel
                {
                    ForeColour = colour,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = $"Required Class: {description?.Description ?? displayInfo.RequiredClass.ToString()}",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
            }

            if (displayInfo.RequiredAmount > 0)
            {
                string text;
                Color colour = displayInfo.Rarity == Rarity.Common ? Color.White : Color.FromArgb(0, 204, 0);
                switch (displayInfo.RequiredType)
                {
                    case RequiredType.Level:
                        text = $"Required Level: {MouseItem.Info.RequiredAmount}";
                        if (User.Level < MouseItem.Info.RequiredAmount && User.Stats[Stat.Rebirth] == 0)
                            colour = Color.Red;
                        break;
                    case RequiredType.MaxLevel:
                        text = $"Max Level: {MouseItem.Info.RequiredAmount}";
                        if (User.Level > MouseItem.Info.RequiredAmount || User.Stats[Stat.Rebirth] > 0)
                            colour = Color.Red;
                        break;
                    case RequiredType.AC:
                        text = $"Required AC: {MouseItem.Info.RequiredAmount}";
                        if (User.Stats[Stat.MaxAC] < MouseItem.Info.RequiredAmount)
                            colour = Color.Red;
                        break;
                    case RequiredType.MR:
                        text = $"Required MR: {MouseItem.Info.RequiredAmount}";
                        if (User.Stats[Stat.MaxMR] < MouseItem.Info.RequiredAmount)
                            colour = Color.Red;
                        break;
                    case RequiredType.DC:
                        text = $"Required DC: {MouseItem.Info.RequiredAmount}";
                        if (User.Stats[Stat.MaxDC] < MouseItem.Info.RequiredAmount)
                            colour = Color.Red;
                        break;
                    case RequiredType.MC:
                        text = $"Required MC: {MouseItem.Info.RequiredAmount}";
                        if (User.Stats[Stat.MaxMC] < MouseItem.Info.RequiredAmount)
                            colour = Color.Red;
                        break;
                    case RequiredType.SC:
                        text = $"Required SC: {MouseItem.Info.RequiredAmount}";
                        if (User.Stats[Stat.MaxSC] < MouseItem.Info.RequiredAmount)
                            colour = Color.Red;
                        break;
                    case RequiredType.Health:
                        text = $"Required Health: {MouseItem.Info.RequiredAmount}";
                        if (User.Stats[Stat.Health] < MouseItem.Info.RequiredAmount)
                            colour = Color.Red;
                        break;
                    case RequiredType.Mana:
                        text = $"Required Mana: {MouseItem.Info.RequiredAmount}";
                        if (User.Stats[Stat.Mana] < MouseItem.Info.RequiredAmount)
                            colour = Color.Red;
                        break;
                    case RequiredType.CompanionLevel:
                        text = $"Companion Level: {MouseItem.Info.RequiredAmount}";
                        if (Companion == null || Companion.Level < MouseItem.Info.RequiredAmount)
                            colour = Color.Red;
                        break;
                    case RequiredType.MaxCompanionLevel:
                        text = $"Max Companion Level: {MouseItem.Info.RequiredAmount}";
                        if (Companion == null || Companion.Level > MouseItem.Info.RequiredAmount)
                            colour = Color.Red;
                        break;
                    case RequiredType.RebirthLevel:
                        text = $"Rebirth Level: {MouseItem.Info.RequiredAmount}";
                        if (User.Stats[Stat.Rebirth] < MouseItem.Info.RequiredAmount)
                            colour = Color.Red;
                        break;
                    case RequiredType.MaxRebirthLevel:
                        text = $"Rebirth Level: {MouseItem.Info.RequiredAmount}";
                        if (User.Stats[Stat.Rebirth] > MouseItem.Info.RequiredAmount)
                            colour = Color.Red;
                        break;
                    default:
                        text = "Unknown Type Required";
                        break;
                }

                if (displayInfo.Rarity > Rarity.Common)
                    text += $" ({displayInfo.Rarity})";


                label = new DXLabel
                {
                    ForeColour = colour,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = text,
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
            }
            else if (displayInfo.Rarity > Rarity.Common)
            {

                label = new DXLabel
                {
                    ForeColour = Color.FromArgb(0, 204, 0),
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = displayInfo.Rarity.ToString(),
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
            }

            bool spacer = false;
            long sale = MouseItem.Price(Math.Max(1, MouseItem.Count));
            if (sale > 0)
            {
                label = new DXLabel
                {
                    ForeColour = Color.LightGoldenrodYellow,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = $"Sell Value: {sale}",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
            }
            ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 4);

            if (MouseItem.Info.Durability > 0 && !MouseItem.Info.CanRepair && MouseItem.Info.StackSize == 1)
            {
                label = new DXLabel
                {
                    ForeColour = Color.Yellow,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = "Cannot be repaired.",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height);
                spacer = true;
            }

            if (!MouseItem.Info.CanSell || (MouseItem.Flags & UserItemFlags.Worthless) == UserItemFlags.Worthless)
            {
                label = new DXLabel
                {
                    ForeColour = Color.Yellow,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = "Cannot be sold.",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height);
                spacer = true;
            }

            if (!MouseItem.Info.CanStore)
            {
                label = new DXLabel
                {
                    ForeColour = Color.Yellow,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = "Cannot be stored.",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height);
                spacer = true;
            }

            if (!MouseItem.Info.CanTrade || (MouseItem.Flags & UserItemFlags.Bound) == UserItemFlags.Bound)
            {
                label = new DXLabel
                {
                    ForeColour = Color.Yellow,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = "Cannot be traded.",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height);
                spacer = true;
            }

            if (!MouseItem.Info.CanDrop)
            {
                label = new DXLabel
                {
                    ForeColour = Color.Yellow,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = "Cannot be dropped.",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height);
                spacer = true;
            }

            if (!MouseItem.Info.CanDeathDrop || (MouseItem.Flags & UserItemFlags.Worthless) == UserItemFlags.Worthless || (MouseItem.Flags & UserItemFlags.Bound) == UserItemFlags.Bound)
            {
                label = new DXLabel
                {
                    ForeColour = Color.Yellow,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = "Cannot be dropped on death.",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height);
                spacer = true;
            }

            if ((MouseItem.Flags & UserItemFlags.Bound) == UserItemFlags.Bound)
            {
                label = new DXLabel
                {
                    ForeColour = Color.Yellow,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = "Bound Item.",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height);
                spacer = true;
            }

            if ((MouseItem.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable)
            {
                label = new DXLabel
                {
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                };

                switch (MouseItem.Info.ItemType)
                {
                    case ItemType.Book:
                        label.ForeColour = Color.Red;
                        label.Text = "Does not contain Level 4 Pages.";
                        break;
                    default:
                        label.ForeColour = Color.Yellow;
                        label.Text = "Cannot be Refined or Upgraded.";
                        break;
                }

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height);
                spacer = true;
            }
            else if (MouseItem.Info.ItemType == ItemType.Book)
            {
                label = new DXLabel
                {
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    ForeColour = Color.Green,
                    Text = "Contains high level Pages.",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height);
                spacer = true;
            }

            if (!string.IsNullOrEmpty(displayInfo.Description))
            {
                label = new DXLabel
                {
                    ForeColour = Color.Wheat,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = displayInfo.Description,
                };

                if (displayInfo.ItemEffect == ItemEffect.FootBallWhistle)
                    label.ForeColour = Color.Red;

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height);
                spacer = true;
            }

            if (spacer)
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 4);

            if (MouseItem.Info.Durability > 0 && MouseItem.Info.CanRepair && MouseItem.Info.StackSize == 1 && MouseItem.Info.ItemType != ItemType.Book)
            {
                label = new DXLabel
                {
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                };

                if (CEnvir.Now >= MouseItem.NextSpecialRepair)
                {
                    label.Text = "Can Special Repair";
                    label.ForeColour = Color.LimeGreen;
                }
                else
                {
                    label.Text = $"Special Repair in {Functions.ToString(MouseItem.NextSpecialRepair - CEnvir.Now, true)}";
                    label.ForeColour = Color.Red;
                }


                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);

                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 4);
            }

            if ((MouseItem.Flags & UserItemFlags.Expirable) == UserItemFlags.Expirable)
            {
                label = new DXLabel
                {
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = $"Expires in {Functions.ToString(MouseItem.ExpireTime, true)}",
                    ForeColour = Color.Chocolate,
                };


                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);

                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 4);
            }

            if (stats[Stat.ItemReviveTime] > 0)
            {
                label = new DXLabel
                {
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                };

                DateTime value = MouseItem.Info.ItemEffect == ItemEffect.PillOfReincarnation ? ReincarnationPillTime : ItemReviveTime;

                if (CEnvir.Now >= value)
                {
                    label.Text = "Revival ready";
                    label.ForeColour = Color.LimeGreen;
                }
                else
                {
                    label.Text = $"Revival ready in {Functions.ToString(value - CEnvir.Now, true)}";
                    label.ForeColour = Color.Red;
                }


                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);

                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 4);
            }

            if (MouseItem.Info.Set != null)
                SetItemInfo(MouseItem.Info.Set);

            if ((MouseItem.Flags & UserItemFlags.Marriage) == UserItemFlags.Marriage)
            {
                label = new DXLabel
                {
                    ForeColour = Color.MediumOrchid,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = "Wedding Ring.",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height);

                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 4);
            }

            if ((MouseItem.Flags & UserItemFlags.GameMaster) == UserItemFlags.GameMaster)
            {
                label = new DXLabel
                {
                    ForeColour = Color.LightSeaGreen,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = "Created by a Game Master.",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height);

                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 4);
            }

            if (NPCItemFragmentBox.IsVisible && MouseItem.CanFragment())
            {
                label = new DXLabel
                {
                    ForeColour = Color.MediumAquamarine,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = $"Fragment Cost: {MouseItem.FragmentCost():#,##0}",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);

                label = new DXLabel
                {
                    ForeColour = Color.MediumAquamarine,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = $"Fragments: {(MouseItem.Info.Rarity == Rarity.Common ? "Fragment" : "Framgent (II)")} x{MouseItem.FragmentCount():#,##0}",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);

                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 4);
            }

            if (CEnvir.Now < MouseItem.NextReset)
            {
                label = new DXLabel
                {
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = $"Reset Available in {Functions.ToString(MouseItem.NextReset - CEnvir.Now, true)}",
                    ForeColour = Color.Red,
                };


                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);

                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 4);
            }

            if ((MouseItem.Flags & UserItemFlags.Locked) == UserItemFlags.Locked)
            {
                label = new DXLabel
                {
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = $"Locked: Prevents accidentally selling or throwing away\n" +
                           $"[Middle Mouse Button] or [Scroll Lock] to Unlock.",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height);

                ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 4);
            }
        }

        private void EquipmentItemInfo()
        {
            Stats stats = new Stats();

            ItemInfo displayInfo = MouseItem.Info;

            if (MouseItem.Info.ItemEffect == ItemEffect.ItemPart)
                displayInfo = Globals.ItemInfoList.Binding.First(x => x.Index == MouseItem.AddedStats[Stat.ItemIndex]);

            stats.Add(displayInfo.Stats, displayInfo.ItemType != ItemType.Weapon);
            stats.Add(MouseItem.AddedStats, MouseItem.Info.ItemType != ItemType.Weapon);

            if (displayInfo.ItemType == ItemType.Weapon)
            {
                Stat ele = MouseItem.AddedStats.GetWeaponElement();

                if (ele == Stat.None)
                    ele = displayInfo.Stats.GetWeaponElement();

                if (ele != Stat.None)
                    stats[ele] += MouseItem.AddedStats.GetWeaponElementValue() + displayInfo.Stats.GetWeaponElementValue();
            }

            DXLabel label;
            if (MouseItem.Info.Durability > 0)
            {
                label = new DXLabel
                {
                    ForeColour = MouseItem.CurrentDurability == 0 ? Color.Red : Color.FromArgb(132, 255, 255),
                    Location = new Point(ItemLabel.DisplayArea.Right, 4),
                    Parent = ItemLabel,
                    Text = $"Durability: {Math.Round(MouseItem.CurrentDurability / 1000M)}/{Math.Round(MouseItem.MaxDurability / 1000M)}",
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4, ItemLabel.Size.Height);
            }

            ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 5);

            bool firstele = stats.HasElementalWeakness();
            foreach (KeyValuePair<Stat, int> pair in stats.Values)
            {
                string text = stats.GetDisplay(pair.Key);

                if (text == null) continue;

                string added = MouseItem.AddedStats.GetFormat(pair.Key);

                label = new DXLabel
                {
                    ForeColour = Color.White,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = text
                };

                switch (pair.Key)
                {
                    case Stat.Luck:
                        label.ForeColour = Color.Yellow;
                        break;
                    case Stat.Strength:
                        label.ForeColour = Color.FromArgb(148, 255, 206);
                        break;
                    case Stat.DropRate:
                    case Stat.ExperienceRate:
                    case Stat.SkillRate:
                    case Stat.GoldRate:
                        label.ForeColour = Color.Yellow;

                        if (added == null) break;
                        label.Text += $" ({added})";
                        break;
                    case Stat.FireAttack:
                    case Stat.IceAttack:
                    case Stat.LightningAttack:
                    case Stat.WindAttack:
                    case Stat.HolyAttack:
                    case Stat.DarkAttack:
                    case Stat.PhantomAttack:
                        label.ForeColour = Color.DeepSkyBlue;
                        break;
                    case Stat.FireResistance:
                    case Stat.IceResistance:
                    case Stat.LightningResistance:
                    case Stat.WindResistance:
                    case Stat.HolyResistance:
                    case Stat.DarkResistance:
                    case Stat.PhantomResistance:
                    case Stat.PhysicalResistance:
                        label.ForeColour = !firstele ? Color.Lime : Color.IndianRed;
                        firstele = true;
                        break;
                    default:
                        if (MouseItem.AddedStats[pair.Key] == 0) break;
                        label.Text += $"   ({added})";
                        label.ForeColour = Color.FromArgb(148, 255, 206);
                        break;
                }

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
            }
            ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 5);

            switch (displayInfo.ItemType)
            {
                case ItemType.Weapon:
                    if ((MouseItem.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) break;

                    label = new DXLabel
                    {
                        ForeColour = Color.White,
                        Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                        Parent = ItemLabel,
                        Text = $"{displayInfo.ItemType} Level: " + (MouseItem.Level < Globals.WeaponExperienceList.Count ? MouseItem.Level.ToString() : "Max")
                    };

                    ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                        label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);

                    if (MouseItem.Level < Globals.WeaponExperienceList.Count)
                    {
                        label = new DXLabel
                        {
                            Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                            Parent = ItemLabel,
                        };

                        if ((MouseItem.Flags & UserItemFlags.Refinable) == UserItemFlags.Refinable)
                        {
                            label.Text = "Ready for Refine";
                            label.ForeColour = Color.LightGreen;
                        }
                        else
                        {
                            label.Text = $"{displayInfo.ItemType} Training Points: {MouseItem.Experience / Globals.WeaponExperienceList[MouseItem.Level]:0.##%}";
                            label.ForeColour = Color.White;
                        }



                        ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                            label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                    }
                    ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 5);
                    break;
                case ItemType.Necklace:
                case ItemType.Bracelet:
                case ItemType.Ring:

                    if ((MouseItem.Flags & UserItemFlags.NonRefinable) == UserItemFlags.NonRefinable) break;

                    label = new DXLabel
                    {
                        ForeColour = Color.White,
                        Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                        Parent = ItemLabel,
                        Text = $"{displayInfo.ItemType} Level: " + (MouseItem.Level < Globals.AccessoryExperienceList.Count ? MouseItem.Level.ToString() : "Max")
                    };

                    ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                        label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);

                    if (MouseItem.Level < Globals.AccessoryExperienceList.Count)
                    {
                        label = new DXLabel
                        {
                            Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                            Parent = ItemLabel,
                        };

                        if ((MouseItem.Flags & UserItemFlags.Refinable) == UserItemFlags.Refinable)
                        {
                            label.Text = "Ready for Refine";
                            label.ForeColour = Color.LightGreen;
                        }
                        else
                        {
                            label.Text = $"{displayInfo.ItemType} Training Points: {MouseItem.Experience / Globals.AccessoryExperienceList[MouseItem.Level]:0.##%}";
                            label.ForeColour = Color.White;
                        }



                        ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                            label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
                    }
                    ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 5);
                    break;

            }
        }

        private void CreatePotionLabel()
        {
            if (MouseItem == null) return;

            Stats stats = new Stats();

            stats.Add(MouseItem.Info.Stats);

            DXLabel label;
            foreach (KeyValuePair<Stat, int> pair in stats.Values)
            {
                string text = stats.GetDisplay(pair.Key);

                if (text == null) continue;

                label = new DXLabel
                {
                    ForeColour = Color.White,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = text
                };

                switch (pair.Key)
                {
                    case Stat.Luck:
                    case Stat.DropRate:
                    case Stat.ExperienceRate:
                    case Stat.SkillRate:
                    case Stat.GoldRate:
                        label.ForeColour = Color.Yellow;
                        break;
                    case Stat.DeathDrops:
                        label.ForeColour = Color.OrangeRed;
                        break;
                }

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
            }
            ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 5);

            if (MouseItem.Info.Durability > 0)
            {
                label = new DXLabel
                {
                    ForeColour = Color.White,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = $"Cooldown: {Functions.ToString(TimeSpan.FromMilliseconds(MouseItem.Info.Durability), true)}"
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
            }
        }

        private void CreateFameLabel()
        {
            if (MouseFame == null) return;

            FameLabel = new DXControl
            {
                BackColour = Color.FromArgb(200, 0, 24, 48),
                Border = true,
                BorderColour = Color.Yellow, // Color.FromArgb(144, 148, 48),
                DrawTexture = true,
                IsControl = false,
                IsVisible = true,
            };

            DXLabel label = new DXLabel
            {
                ForeColour = Color.Yellow,
                Location = new Point(4, 4),
                Parent = FameLabel,
                Text = MouseFame.Name
            };
            FameLabel.Size = new Size(label.DisplayArea.Right + 4, label.DisplayArea.Bottom + 4);

            if (!string.IsNullOrEmpty(MouseFame.Description))
            {
                label = new DXLabel
                {
                    ForeColour = Color.Wheat,
                    Location = new Point(4, FameLabel.DisplayArea.Bottom),
                    Parent = FameLabel,
                    Text = Functions.BreakStringIntoLines(MouseFame.Description, 45),
                };

                FameLabel.Size = new Size(label.DisplayArea.Right + 4 > FameLabel.Size.Width ? label.DisplayArea.Right + 4 : FameLabel.Size.Width,
                    label.DisplayArea.Bottom > FameLabel.Size.Height ? label.DisplayArea.Bottom + 4 : FameLabel.Size.Height);
            }

            if (MouseFame.BuffStats.Count > 0)
            {
                Stats stats = new Stats();
                foreach (FameInfoStat stat in MouseFame.BuffStats)
                    stats[stat.Stat] = stat.Amount;

                string statsText = string.Empty;
                foreach (KeyValuePair<Stat, int> pair in stats.Values)
                {
                    if (pair.Key == Stat.Duration) continue;

                    string temp = stats.GetDisplay(pair.Key);

                    if (temp == null) continue;
                    statsText += $"\n{temp}";
                }

                if (!string.IsNullOrEmpty(statsText))
                {
                    label = new DXLabel
                    {
                        ForeColour = Color.White,
                        Location = new Point(4, FameLabel.DisplayArea.Bottom),
                        Parent = FameLabel,
                        Text = statsText.Trim(),
                    };

                    FameLabel.Size = new Size(label.DisplayArea.Right + 4 > FameLabel.Size.Width ? label.DisplayArea.Right + 4 : FameLabel.Size.Width,
                        label.DisplayArea.Bottom > FameLabel.Size.Height ? label.DisplayArea.Bottom + 4 : FameLabel.Size.Height);
                }
            }
        }

        private void CreateMagicLabel()
        {
            if (MouseMagic == null) return;

            MagicLabel = new DXControl
            {
                BackColour = Color.FromArgb(200, 0, 24, 48),
                Border = true,
                BorderColour = Color.Yellow, // Color.FromArgb(144, 148, 48),
                DrawTexture = true,
                IsControl = false,
                IsVisible = true,
            };

            DXLabel label = new DXLabel
            {
                ForeColour = Color.Yellow,
                Location = new Point(4, 4),
                Parent = MagicLabel,
                Text = MouseMagic.Name
            };
            MagicLabel.Size = new Size(label.DisplayArea.Right + 4, label.DisplayArea.Bottom + 4);

            label = new DXLabel
            {
                ForeColour = Color.Yellow,
                Location = new Point(4, MagicLabel.DisplayArea.Bottom),
                Parent = MagicLabel,
                Text = $"<{MouseMagic.Property}>"
            };
            MagicLabel.Size = new Size(label.DisplayArea.Right + 4, label.DisplayArea.Bottom + 4);

            ClientUserMagic magic;

            int width;
            bool disciplineSkill = false;

            if (User.Magics.TryGetValue(MouseMagic, out magic))
            {
                int level = magic.Level;
                disciplineSkill = magic.Info.School == MagicSchool.Discipline;

                if (disciplineSkill)
                {
                    MagicLabel.BorderColour = Color.LimeGreen;
                }

                label = new DXLabel
                {
                    ForeColour = Color.LimeGreen,
                    Location = new Point(4, MagicLabel.DisplayArea.Bottom),
                    Parent = MagicLabel,
                    Text = $"Current Level: {level}",
                };

                string text;
                if (magic.Level < Globals.MagicMaxLevel)
                {
                    text = magic.Level switch
                    {
                        0 => $"{magic.Experience}/{magic.Info.Experience1}",
                        1 => $"{magic.Experience}/{magic.Info.Experience2}",
                        2 => $"{magic.Experience}/{magic.Info.Experience3}",
                        _ => $"{magic.Experience}/{(magic.Level - 2) * 500}",
                    };
                }
                else
                {
                    text = $"Max Level";
                }

                width = label.DisplayArea.Right;
                label = new DXLabel
                {
                    ForeColour = Color.LimeGreen,
                    Location = new Point(width + 4, MagicLabel.DisplayArea.Bottom),
                    Parent = MagicLabel,
                    Text = $"Experience: {text}",
                };
            }
            else
            {
                label = new DXLabel
                {
                    ForeColour = Color.Red,
                    Location = new Point(4, MagicLabel.DisplayArea.Bottom),
                    Parent = MagicLabel,
                    Text = $"Not learned",
                };
            }
            MagicLabel.Size = new Size(label.DisplayArea.Right + 4 > MagicLabel.Size.Width ? label.DisplayArea.Right + 4 : MagicLabel.Size.Width, label.DisplayArea.Bottom);

            label = new DXLabel
            {
                ForeColour = User.Level < MouseMagic.NeedLevel1 ? Color.Red : Color.White,
                Location = new Point(4, MagicLabel.DisplayArea.Bottom),
                Parent = MagicLabel,
                Text = $"Rank 1 Requirement: Level {MouseMagic.NeedLevel1}",
            };
            width = label.DisplayArea.Right + 10;
            label = new DXLabel
            {
                ForeColour = Color.White,
                Location = new Point(width, MagicLabel.DisplayArea.Bottom),
                Parent = MagicLabel,
                Text = $"Experience: {MouseMagic.Experience1:#,##0}",
            };

            MagicLabel.Size = new Size(label.DisplayArea.Right + 4 > MagicLabel.Size.Width ? label.DisplayArea.Right + 4 : MagicLabel.Size.Width, label.DisplayArea.Bottom);

            new DXLabel
            {
                ForeColour = User.Level < MouseMagic.NeedLevel2 ? Color.Red : Color.White,
                Location = new Point(4, MagicLabel.DisplayArea.Bottom),
                Parent = MagicLabel,
                Text = $"Rank 2 Requirement: Level {MouseMagic.NeedLevel2}",
            };

            label = new DXLabel
            {
                ForeColour = Color.White,
                Location = new Point(width, MagicLabel.DisplayArea.Bottom),
                Parent = MagicLabel,
                Text = $"Experience: {MouseMagic.Experience2:#,##0}",
            };

            MagicLabel.Size = new Size(label.DisplayArea.Right + 4 > MagicLabel.Size.Width ? label.DisplayArea.Right + 4 : MagicLabel.Size.Width, label.DisplayArea.Bottom);

            new DXLabel
            {
                ForeColour = User.Level < MouseMagic.NeedLevel3 ? Color.Red : Color.White,
                Location = new Point(4, MagicLabel.DisplayArea.Bottom),
                Parent = MagicLabel,
                Text = $"Rank 3 Requirement: Level {MouseMagic.NeedLevel3}",
            };

            label = new DXLabel
            {
                ForeColour = Color.White,
                Location = new Point(width, MagicLabel.DisplayArea.Bottom),
                Parent = MagicLabel,
                Text = $"Experience: {MouseMagic.Experience3:#,##0}",
            };
            MagicLabel.Size = new Size(label.DisplayArea.Right + 4 > MagicLabel.Size.Width ? label.DisplayArea.Right + 4 : MagicLabel.Size.Width, label.DisplayArea.Bottom);

            if (!disciplineSkill)
            {
                label = new DXLabel
                {
                    ForeColour = magic?.Level < 3 ? Color.Red : Color.White,
                    Location = new Point(4, MagicLabel.DisplayArea.Bottom),
                    Parent = MagicLabel,
                    Text = $"Rank 4+ Requirement: Books",
                };
                MagicLabel.Size = new Size(label.DisplayArea.Right + 4 > MagicLabel.Size.Width ? label.DisplayArea.Right + 4 : MagicLabel.Size.Width, label.DisplayArea.Bottom);
            }

            label = new DXLabel
            {
                AutoSize = false,
                ForeColour = Color.Wheat,
                Location = new Point(4, MagicLabel.DisplayArea.Bottom),
                Parent = MagicLabel,
                Text = MouseMagic.Description,
            };
            label.Size = DXLabel.GetHeight(label, MagicLabel.Size.Width);

            MagicLabel.Size = new Size(label.DisplayArea.Right + 4 > MagicLabel.Size.Width ? label.DisplayArea.Right + 4 : MagicLabel.Size.Width, label.DisplayArea.Bottom + 4);
        }

        private void SetItemInfo(SetInfo set)
        {

            DXLabel label = new DXLabel
            {
                ForeColour = Color.LimeGreen,
                Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                Parent = ItemLabel,
                Text = $"Item Set:"
            };

            ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);

            label = new DXLabel
            {
                ForeColour = Color.LimeGreen,
                Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                Parent = ItemLabel,
                Text = $"    {set.SetName}"
            };

            ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);

            label = new DXLabel
            {
                ForeColour = Color.LimeGreen,
                Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                Parent = ItemLabel,
                Text = "Parts:"
            };

            ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);

            bool hasFullSet = true;
            List<int> counted = new List<int>();

            Stats setBonus = new Stats();

            int l;
            MirClass c;
            ClientUserItem[] equip;

            DXItemCell cell = MouseControl as DXItemCell;
            if (cell?.GridType == GridType.Inspect)
            {
                l = InspectBox.Level;
                c = InspectBox.Class;
                equip = InspectBox.Equipment;
            }
            else
            {
                l = User.Level;
                c = User.Class;
                equip = Equipment;
            }

            foreach (ItemInfo info in set.Items)
            {
                bool hasPart = false;
                for (int j = 0; j < equip.Length; j++)
                {
                    if (counted.Contains(j)) continue;
                    if (equip[j] == null) continue;
                    if (equip[j].Info != info) continue;
                    if (equip[j].CurrentDurability == 0 && equip[j].Info.Durability > 0) continue;

                    counted.Add(j);

                    hasPart = true;
                    break;
                }

                if (!hasPart)
                    hasFullSet = false;

                label = new DXLabel
                {
                    ForeColour = hasPart ? Color.LimeGreen : Color.Gray,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = "    " + info.ItemName
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                    label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);
            }
            label = new DXLabel
            {
                ForeColour = Color.LimeGreen,
                Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                Parent = ItemLabel,
                Text = $"Set Bonus:"
            };

            ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);


            foreach (SetInfoStat stat in set.SetStats)
            {
                if (l < stat.Level) continue;

                switch (c)
                {
                    case MirClass.Warrior:
                        if ((stat.Class & RequiredClass.Warrior) != RequiredClass.Warrior) continue;
                        break;
                    case MirClass.Wizard:
                        if ((stat.Class & RequiredClass.Wizard) != RequiredClass.Wizard) continue;
                        break;
                    case MirClass.Taoist:
                        if ((stat.Class & RequiredClass.Taoist) != RequiredClass.Taoist) continue;
                        break;
                    case MirClass.Assassin:
                        if ((stat.Class & RequiredClass.Assassin) != RequiredClass.Assassin) continue;
                        break;
                }

                setBonus[stat.Stat] += stat.Amount;
            }



            foreach (KeyValuePair<Stat, int> pair in setBonus.Values)
            {
                string text = setBonus.GetDisplay(pair.Key);

                if (text == null) continue;

                label = new DXLabel
                {
                    ForeColour = hasFullSet ? Color.LimeGreen : Color.Gray,
                    Location = new Point(4, ItemLabel.DisplayArea.Bottom),
                    Parent = ItemLabel,
                    Text = "    " + text
                };

                ItemLabel.Size = new Size(label.DisplayArea.Right + 4 > ItemLabel.Size.Width ? label.DisplayArea.Right + 4 : ItemLabel.Size.Width,
                                          label.DisplayArea.Bottom > ItemLabel.Size.Height ? label.DisplayArea.Bottom : ItemLabel.Size.Height);

            }


            ItemLabel.Size = new Size(ItemLabel.Size.Width, ItemLabel.Size.Height + 4);
        }

    }
}
