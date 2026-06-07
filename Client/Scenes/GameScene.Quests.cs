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
        public bool CanAccept(QuestInfo quest)
        {
            if (QuestLog.Any(x => x.Quest == quest)) return false;

            foreach (QuestRequirement requirement in quest.Requirements)
            {
                switch (requirement.Requirement)
                {
                    case QuestRequirementType.MinLevel:
                        if (User.Level < requirement.IntParameter1) return false;
                        break;
                    case QuestRequirementType.MaxLevel:
                        if (User.Level > requirement.IntParameter1) return false;
                        break;
                    case QuestRequirementType.NotAccepted:
                        if (QuestLog.Any(x => x.Quest == requirement.QuestParameter)) return false;

                        break;
                    case QuestRequirementType.HaveCompleted:
                        if (QuestLog.Any(x => x.Quest == requirement.QuestParameter && x.Completed)) break;

                        return false;
                    case QuestRequirementType.HaveNotCompleted:
                        if (QuestLog.Any(x => x.Quest == requirement.QuestParameter && x.Completed)) return false;

                        break;
                    case QuestRequirementType.Class:
                        switch (User.Class)
                        {
                            case MirClass.Warrior:
                                if ((requirement.Class & RequiredClass.Warrior) != RequiredClass.Warrior) return false;
                                break;
                            case MirClass.Wizard:
                                if ((requirement.Class & RequiredClass.Wizard) != RequiredClass.Wizard) return false;
                                break;
                            case MirClass.Taoist:
                                if ((requirement.Class & RequiredClass.Taoist) != RequiredClass.Taoist) return false;
                                break;
                            case MirClass.Assassin:
                                if ((requirement.Class & RequiredClass.Assassin) != RequiredClass.Assassin) return false;
                                break;
                        }
                        break;
                }

            }
            return true;
        }
        public void QuestChanged(ClientUserQuest quest)
        {
            CheckNewQuests();

            QuestBox.QuestChanged(quest);
        }
        public void CheckNewQuests()
        {
            QuestBox.PopulateQuests();

            QuestTrackerBox.PopulateQuests();

            NPCQuestListBox.UpdateQuestDisplay();

            UpdateQuestIcons();
        }
        public void CancelQuest(QuestInfo quest)
        {
            QuestBox.CancelQuest(quest);
        }

        public bool HasQuest(MonsterInfo info, MapInfo map)
        {
            foreach (QuestTaskMonsterDetails detail in info.QuestDetails)
            {
                if (detail.Map != null && detail.Map != map) continue;

                QuestInfo quest = QuestBox.CurrentTab.Quests.FirstOrDefault(x => x == detail.Task.Quest);

                if (quest == null) continue;

                ClientUserQuest userQuest = QuestLog.First(x => x.Quest == quest);

                if (userQuest.IsComplete) continue;

                ClientUserQuestTask UserTask = userQuest.Tasks.FirstOrDefault(x => x.Task == detail.Task);

                if (UserTask != null && UserTask.Completed) continue;

                return true;
            }

            return false;
        }

        public string GetQuestText(QuestInfo questInfo, ClientUserQuest userQuest, bool isLog)
        {
            string text;

            if (userQuest == null)
                text = questInfo.AcceptText; //Available
            else if (userQuest.Completed)
                text = questInfo.ArchiveText; //Completed
            else if (userQuest.IsComplete && !isLog)
                text = questInfo.CompletedText; //Completed
            else
                text = questInfo.ProgressText; //Current

            text = text.Replace("[PLAYERNAME]", User.Name, StringComparison.OrdinalIgnoreCase);
            text = text.Replace("[STARTNAME]", questInfo.StartNPC.NPCName, StringComparison.OrdinalIgnoreCase);
            text = text.Replace("[FINISHNAME]", questInfo.FinishNPC.NPCName, StringComparison.OrdinalIgnoreCase);

            return text;

        }

        public string GetTaskText(QuestInfo questInfo, ClientUserQuest userQuest)
        {
            StringBuilder builder = new StringBuilder();

            foreach (QuestTask task in questInfo.Tasks)
                builder.AppendLine(GetTaskText(task, userQuest));

            return builder.ToString(); //Available
        }
        public string GetTaskText(QuestTask task, ClientUserQuest userQuest)
        {
            StringBuilder builder = new StringBuilder();

            ClientUserQuestTask userTask = userQuest?.Tasks.FirstOrDefault(x => x.Task == task);

            switch (task.Task)
            {
                case QuestTaskType.KillMonster:
                    builder.AppendFormat("Kill {0} ", task.Amount);
                    break;
                case QuestTaskType.GainItem:
                    builder.AppendFormat("Collect {0} {1} from ", task.Amount, task.ItemParameter?.ItemName);
                    break;
                case QuestTaskType.Region:
                    builder.AppendFormat("Goto {0} in {1}", task.RegionParameter?.Description, task.RegionParameter?.Map.PlayerDescription);
                    break;
            }

            if (string.IsNullOrEmpty(task.MobDescription))
            {
                bool needComma = false;
                for (int i = 0; i < task.MonsterDetails.Count; i++)
                {
                    QuestTaskMonsterDetails monster = task.MonsterDetails[i];
                    if (monster == null) continue;
                    if (i > 2)
                    {
                        builder.Append("...");
                        break;
                    }

                    if (needComma)
                        builder.Append(" or ");

                    needComma = true;

                    builder.Append(monster.Monster.MonsterName);

                    if (monster.Map != null)
                        builder.AppendFormat(" in {0}", monster.Map.PlayerDescription);
                }
            }
            else
                builder.Append(task.MobDescription);

            if (userQuest != null)
            {
                if (userTask != null && userTask.Completed)
                    builder.Append(" (Completed)");
                else
                {
                    if (task.Task != QuestTaskType.Region)
                    {
                        builder.Append($" ({userTask?.Amount ?? 0}/{task.Amount})");
                    }
                }
            }

            return builder.ToString();
        }

        public void UpdateQuestIcons()
        {
            foreach (NPCInfo info in Globals.NPCInfoList.Binding)
                info.CurrentQuest = null;

            bool completed = false;

            foreach (QuestInfo quest in QuestBox.CurrentTab.Quests)
            {
                ClientUserQuest userQuest = QuestLog.First(x => x.Quest == quest);

                if (quest.FinishNPC.CurrentQuest != null) continue;

                var current = new CurrentQuest
                {
                    Type = quest.QuestType
                };

                if (userQuest.IsComplete)
                {
                    current.Icon = QuestIcon.Complete;
                    completed = true;
                }
                else
                {
                    current.Icon = QuestIcon.Incomplete;
                }

                quest.FinishNPC.CurrentQuest = current;
            }

            foreach (QuestInfo quest in QuestBox.AvailableTab.Quests)
            {
                if (quest.StartNPC.CurrentQuest != null) continue;

                quest.StartNPC.CurrentQuest = new CurrentQuest
                {
                    Type = quest.QuestType,
                    Icon = QuestIcon.New
                };
            }

            MainPanel.AvailableQuestIcon.Visible = QuestBox.AvailableTab.Quests.Count > 0;
            MainPanel.CompletedQuestIcon.Visible = completed;

            foreach (NPCInfo info in Globals.NPCInfoList.Binding)
            {
                BigMapBox.Update(info);
                MiniMapBox.Update(info);
            }

            foreach (MapObject ob in MapControl.Objects)
                ob.UpdateQuests();

            foreach (ClientObjectData data in DataDictionary.Values)
            {
                BigMapBox.Update(data);
                MiniMapBox.Update(data);
            }

        }
        public DXControl GetNPCControl(NPCInfo npc)
        {
            int icon = 0;
            Color colour = Color.White;
            string iconString = "";

            if (npc.CurrentQuest != null)
            {
                switch (npc.CurrentQuest.Type)
                {
                    case QuestType.General:
                        icon = 16;
                        colour = Color.Yellow;
                        break;
                    case QuestType.Daily:
                        icon = 76;
                        colour = Color.Blue;
                        break;
                    case QuestType.Weekly:
                        icon = 76;
                        colour = Color.Blue;
                        break;
                    case QuestType.Repeatable:
                        icon = 16;
                        colour = Color.Yellow;
                        break;
                    case QuestType.Story:
                        icon = 56;
                        colour = Color.Green;
                        break;
                    case QuestType.Account:
                        icon = 36;
                        colour = Color.MediumPurple;
                        break;
                }

                switch (npc.CurrentQuest.Icon)
                {
                    case QuestIcon.New:
                        icon += 0;
                        iconString = "!";
                        break;
                    case QuestIcon.Incomplete:
                        icon = 2;
                        colour = Color.White;
                        iconString = "?";
                        break;
                    case QuestIcon.Complete:
                        icon += 2;
                        iconString = "?";
                        break;
                }
            }

            if (!string.IsNullOrEmpty(iconString))
            {
                DXLabel label = new DXLabel
                {
                    Text = iconString,
                    ForeColour = colour,
                    Hint = npc.NPCName,
                    Tag = npc.CurrentQuest,
                    Font = new Font(Config.FontName, CEnvir.FontSize(10F), FontStyle.Bold)
                };

                return label;
            }
            else if (icon > 0)
            {
                DXImageControl image = new DXImageControl
                {
                    LibraryFile = LibraryFile.QuestIcon,
                    Index = icon,
                    ForeColour = colour,
                    Hint = npc.NPCName,
                    Tag = npc.CurrentQuest,
                };
                image.OpacityChanged += (o, e) => image.ImageOpacity = image.Opacity;

                return image;
            }
            else if (npc.MapIcon != MapIcon.None)
            {
                DXImageControl image = new DXImageControl
                {
                    LibraryFile = LibraryFile.MiniMapIcon,
                    Opacity = Opacity,
                    Hint = npc.NPCName,
                    ImageOpacity = Opacity,
                };
                image.OpacityChanged += (o, e) => image.ImageOpacity = image.Opacity;

                GameScene.Game.UpdateMapIcon(image, npc.MapIcon);

                return image;
            }

            return new DXMapInfoControl
            {
                Size = new Size(3, 3),
                DrawTexture = true,
                Hint = npc.NPCName,
                BackColour = Color.Yellow
            };
        }

    }
}
