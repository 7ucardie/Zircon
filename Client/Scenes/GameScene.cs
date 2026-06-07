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

//Cleaned
namespace Client.Scenes
{
    public sealed partial class GameScene : DXScene
    {
        #region Properties
        public static GameScene Game;

        public DXItemCell SelectedCell
        {
            get => _SelectedCell;
            set
            {
                if (_SelectedCell == value) return;

                if (_SelectedCell != null) _SelectedCell.Selected = false;

                _SelectedCell = value;

                if (_SelectedCell != null) _SelectedCell.Selected = true;
            }
        }
        private DXItemCell _SelectedCell;

        #region User

        public UserObject User
        {
            get => _User;
            set
            {
                if (_User == value) return;

                _User = value;

                UserChanged();
            }
        }
        private UserObject _User;


        #endregion

        #region Observer

        public bool Observer
        {
            get => _Observer;
            set
            {
                if (_Observer == value) return;

                bool oldValue = _Observer;
                _Observer = value;

                OnObserverChanged(oldValue, value);
            }
        }
        private bool _Observer;
        public event EventHandler<EventArgs> ObserverChanged;
        public void OnObserverChanged(bool oValue, bool nValue)
        {
            ObserverChanged?.Invoke(this, EventArgs.Empty);
        }

        #endregion

        public ClientUserCurrency CurrencyPickedUp = null;

        public MapObject MagicObject, MouseObject, TargetObject, FocusObject;
        public DXControl ItemLabel, MagicLabel, FameLabel;

        #region MouseItem

        public ClientUserItem MouseItem
        {
            get => _MouseItem;
            set
            {
                if (_MouseItem == value) return;

                ClientUserItem oldValue = _MouseItem;
                _MouseItem = value;

                OnMouseItemChanged(oldValue, value);
            }
        }
        private ClientUserItem _MouseItem;
        public event EventHandler<EventArgs> MouseItemChanged;
        public void OnMouseItemChanged(ClientUserItem oValue, ClientUserItem nValue)
        {
            MouseItemChanged?.Invoke(this, EventArgs.Empty);

            CreateItemLabel();
        }

        #endregion

        #region MouseMagic

        public MagicInfo MouseMagic
        {
            get => _MouseMagic;
            set
            {
                if (_MouseMagic == value) return;

                MagicInfo oldValue = _MouseMagic;
                _MouseMagic = value;

                OnMouseMagicChanged(oldValue, value);
            }
        }
        private MagicInfo _MouseMagic;
        public event EventHandler<EventArgs> MouseMagicChanged;
        public void OnMouseMagicChanged(MagicInfo oValue, MagicInfo nValue)
        {
            MouseMagicChanged?.Invoke(this, EventArgs.Empty);

            if (MagicLabel != null && !MagicLabel.IsDisposed) MagicLabel.Dispose();
            MagicLabel = null;
            CreateMagicLabel();
        }

        #endregion

        #region MouseFame

        public FameInfo MouseFame
        {
            get => _MouseFame;
            set
            {
                if (_MouseFame == value) return;

                FameInfo oldValue = _MouseFame;
                _MouseFame = value;

                OnMouseFameChanged(oldValue, value);
            }
        }
        private FameInfo _MouseFame;
        public event EventHandler<EventArgs> MouseFameChanged;
        public void OnMouseFameChanged(FameInfo oValue, FameInfo nValue)
        {
            MouseFameChanged?.Invoke(this, EventArgs.Empty);

            if (FameLabel != null && !FameLabel.IsDisposed) FameLabel.Dispose();
            FameLabel = null;
            CreateFameLabel();
        }

        #endregion

        public MapControl MapControl;
        public MainPanel MainPanel;

        public MenuDialog MenuBox;
        public DXConfigWindow ConfigBox;
        public HelpDialog HelpBox;
        public CaptionDialog CaptionBox;
        public InventoryDialog InventoryBox;
        public CharacterDialog CharacterBox;
        public FilterDropDialog FilterDropBox;
        public ExitDialog ExitBox;
        public ChatTextBox ChatTextBox;
        public BeltDialog BeltBox;
        public ChatOptionsDialog ChatOptionsBox;
        public NPCDialog NPCBox;
        public NPCGoodsDialog NPCGoodsBox;
        public NPCRepairDialog NPCRepairBox;
        public NPCRefinementStoneDialog NPCRefinementStoneBox;
        public NPCRefineDialog NPCRefineBox;
        public NPCRefineRetrieveDialog NPCRefineRetrieveBox;
        public NPCQuestListDialog NPCQuestListBox;
        public NPCQuestDialog NPCQuestBox;
        public NPCAdoptCompanionDialog NPCAdoptCompanionBox;
        public NPCCompanionStorageDialog NPCCompanionStorageBox;
        public NPCWeddingRingDialog NPCWeddingRingBox;
        public NPCItemFragmentDialog NPCItemFragmentBox;
        public NPCAccessoryUpgradeDialog NPCAccessoryUpgradeBox;
        public NPCAccessoryLevelDialog NPCAccessoryLevelBox;
        public NPCAccessoryResetDialog NPCAccessoryResetBox;
        public NPCMasterRefineDialog NPCMasterRefineBox;
        public NPCRollDialog NPCRollBox;
        public MiniMapDialog MiniMapBox;
        public BigMapDialog BigMapBox;
        public MagicDialog MagicBox;
        public GroupDialog GroupBox;
        public GroupHealthDialog GroupHealthBox;
        public BuffDialog BuffBox;
        public StorageDialog StorageBox;
        public AutoPotionDialog AutoPotionBox;
        public CharacterDialog InspectBox;
        public RankingDialog RankingBox;
        public MarketPlaceDialog MarketPlaceBox;
        public DungeonFinderDialog DungeonFinderBox;
        public CommunicationDialog CommunicationBox;
        public TradeDialog TradeBox;
        public GuildDialog GuildBox;
        public GuildMemberDialog GuildMemberBox;
        public QuestDialog QuestBox;
        public QuestTrackerDialog QuestTrackerBox;
        public MilestoneAchievedDialog MilestoneAchievedBox;
        public CompanionDialog CompanionBox;
        public MonsterDialog MonsterBox;
        public MagicBarDialog MagicBarBox;
        public EditCharacterDialog EditCharacterBox;
        public FortuneCheckerDialog FortuneCheckerBox;
        public NPCWeaponCraftWindow NPCWeaponCraftBox;
        public NPCAccessoryRefineDialog NPCAccessoryRefineBox;
        public CurrencyDialog CurrencyBox;
        public TimerDialog TimerBox;
        public BundleDialog BundleBox;
        public LootBoxDialog LootBoxBox;

        public FishingDialog FishingBox;
        public FishingCatchDialog FishingCatchBox;

        public ClientUserItem[] Inventory = new ClientUserItem[Globals.InventorySize];
        public ClientUserItem[] Equipment = new ClientUserItem[Globals.EquipmentSize];

        public List<ClientUserQuest> QuestLog = new List<ClientUserQuest>();

        public HashSet<string> GuildWars = new HashSet<string>();
        public HashSet<CastleInfo> ConquestWars = new HashSet<CastleInfo>();

        public SortedDictionary<uint, ClientObjectData> DataDictionary = new SortedDictionary<uint, ClientObjectData>();

        public Dictionary<ItemInfo, ClientFortuneInfo> FortuneDictionary = new Dictionary<ItemInfo, ClientFortuneInfo>();

        public Dictionary<CastleInfo, string> CastleOwners = new Dictionary<CastleInfo, string>();

        public bool MoveFrame { get; set; }
        public DateTime MoveTime, OutputTime, ItemRefreshTime;

        public bool CanRun;

        public bool AutoRun
        {
            get => _AutoRun;
            set
            {
                if (_AutoRun == value) return;
                _AutoRun = value;

                ReceiveChat(value ? CEnvir.Language.GameSceneAutoRunOn : CEnvir.Language.GameSceneAutoRunOff, MessageType.Hint);
            }
        }
        private bool _AutoRun;

        #region StorageSize

        public int StorageSize
        {
            get { return _StorageSize; }
            set
            {
                if (_StorageSize == value) return;

                int oldValue = _StorageSize;
                _StorageSize = value;

                OnStorageSizeChanged(oldValue, value);
            }
        }
        private int _StorageSize;
        public void OnStorageSizeChanged(int oValue, int nValue)
        {
            StorageBox.RefreshStorage();
        }

        #endregion

        #region NPCID

        public uint NPCID
        {
            get => _NPCID;
            set
            {
                if (_NPCID == value) return;

                uint oldValue = _NPCID;
                _NPCID = value;

                OnNPCIDChanged(oldValue, value);
            }
        }
        private uint _NPCID;
        public void OnNPCIDChanged(uint oValue, uint nValue)
        {

        }

        #endregion

        #region Companion

        public ClientUserCompanion Companion
        {
            get => _Companion;
            set
            {
                if (_Companion == value) return;

                _Companion = value;

                CompanionChanged();
            }
        }
        private ClientUserCompanion _Companion;

        #endregion

        public ClientPlayerInfo Partner
        {
            get => _Partner;
            set
            {
                if (_Partner == value) return;

                _Partner = value;

                MarriageChanged();
            }
        }
        private ClientPlayerInfo _Partner;


        public uint InspectID;
        public DateTime PickUpTime, UseItemTime, NPCTime, ToggleTime, InspectTime, ItemTime = CEnvir.Now, ReincarnationPillTime, ItemReviveTime;

        public bool StruckEnabled;

        public bool HermitEnabled
        {
            get => _HermitEnabled;
            set
            {
                if (_HermitEnabled == value) return;

                _HermitEnabled = value;
                CharacterBox.OnHermitChanged(_HermitEnabled);
            }
        }
        private bool _HermitEnabled;

        public float DayTime
        {
            get => _DayTime;
            set
            {
                if (_DayTime == value) return;

                _DayTime = value;
                MapControl.LLayer.UpdateLights();
            }
        }
        private float _DayTime;

        public TimeOfDay TimeOfDay
        {
            get => _TimeOfDay;
            set
            {
                if (_TimeOfDay == value) return;

                _TimeOfDay = value;
            }
        }
        private TimeOfDay _TimeOfDay;

        public string TimeOfDayLabel
        {
            get => _TimeOfDayLabel;
            set
            {
                if (_TimeOfDayLabel == value) return;

                _TimeOfDayLabel = value;
            }
        }
        private string _TimeOfDayLabel = string.Empty;

        public override void OnSizeChanged(Size oValue, Size nValue)
        {
            base.OnSizeChanged(oValue, nValue);

            SetDefaultLocations();

            foreach (DXWindow window in DXWindow.Windows)
                window.LoadSettings();

            CharacterBox?.LoadSettings();
            InventoryBox?.LoadSettings();
            MagicBox?.LoadSettings();
            StorageBox?.LoadSettings();
            TradeBox?.LoadSettings();
            CompanionBox?.LoadSettings();
            CommunicationBox?.LoadSettings();
            RankingBox?.LoadSettings();
            QuestBox?.LoadSettings();
            FishingBox?.LoadSettings();
            GroupBox?.LoadSettings();
            GuildBox?.LoadSettings();
            ConfigBox?.LoadSettings();
            MenuBox?.LoadSettings();          
            HelpBox?.LoadSettings();

            LoadChatTabs();
        }

        #endregion

        public GameScene(Size size) : base(size)
        {
            DrawTexture = false;
            Game = this;

            foreach (NPCInfo info in Globals.NPCInfoList.Binding)
                info.CurrentQuest = null;

            MapControl = new MapControl
            {
                Parent = this,
                Size = Size,
            };
            MapControl.MouseWheel += (o, e) =>
            {
                foreach (ChatTab tab in ChatTab.Tabs)
                {
                    if (!tab.DisplayArea.Contains(e.Location) || !tab.Visible) continue;

                    tab.ScrollBar.DoMouseWheel(tab.ScrollBar, e);
                }
            };

            MainPanel = new MainPanel { Parent = this };

            MenuBox = new MenuDialog
            {
                Parent = this,
                Visible = false
            };

            ConfigBox = new DXConfigWindow
            {
                Parent = this,
                Visible = false,
                NetworkTab = { Enabled = false, TabButton = { Visible = false } },
                UITab = { TabButton = { Visible = true } },
            };

            HelpBox = new HelpDialog
            {
                Parent = this,
                Visible = false
            };

            ExitBox = new ExitDialog
            {
                Parent = this,
                Visible = false,
            };

            CaptionBox = new CaptionDialog
            {
                Parent = this,
                Visible = false,
            };

            InventoryBox = new InventoryDialog
            {
                Parent = this,
                Visible = false,
            };

            CharacterBox = new CharacterDialog(false)
            {
                Parent = this,
                Visible = false,
            };

            FilterDropBox = new FilterDropDialog
            {
                Parent = this,
                Visible = false,
            };

            ChatTextBox = new ChatTextBox
            {
                Parent = this,
                Visible = false
            };
            ChatOptionsBox = new ChatOptionsDialog
            {
                Parent = this,
                Visible = false,
            };
            BeltBox = new BeltDialog
            {
                Parent = this,
            };
            NPCBox = new NPCDialog
            {
                Parent = this,
                Visible = false
            };
            NPCGoodsBox = new NPCGoodsDialog
            {
                Parent = this,
                Visible = false
            };

            NPCRepairBox = new NPCRepairDialog
            {
                Parent = this,
                Visible = false,
            };
            NPCQuestListBox = new NPCQuestListDialog
            {
                Parent = this,
                Visible = false,
            };
            NPCQuestBox = new NPCQuestDialog
            {
                Parent = this,
                Visible = false,
            };
            NPCAdoptCompanionBox = new NPCAdoptCompanionDialog
            {
                Parent = this,
                Visible = false,
            };
            NPCCompanionStorageBox = new NPCCompanionStorageDialog
            {
                Parent = this,
                Visible = false,
            };
            NPCWeddingRingBox = new NPCWeddingRingDialog
            {
                Parent = this,
                Visible = false,
            };

            MiniMapBox = new MiniMapDialog
            {
                Parent = this,
            };
            MagicBox = new MagicDialog()
            {
                Parent = this,
                Visible = false,
            };
            GroupBox = new GroupDialog()
            {
                Parent = this,
                Visible = false,
            };
            GroupHealthBox = new GroupHealthDialog()
            {
                Parent = this,
                Visible = true,
            };

            BigMapBox = new BigMapDialog
            {
                Parent = this,
                Visible = false,
            };
            BuffBox = new BuffDialog
            {
                Parent = this,
            };
            StorageBox = new StorageDialog
            {
                Parent = this,
                Visible = false
            };
            AutoPotionBox = new AutoPotionDialog
            {
                Parent = this,
                Visible = false
            };
            NPCRefinementStoneBox = new NPCRefinementStoneDialog
            {
                Parent = this,
                Visible = false,
            };
            NPCItemFragmentBox = new NPCItemFragmentDialog()
            {
                Parent = this,
                Visible = false,
            };
            NPCAccessoryUpgradeBox = new NPCAccessoryUpgradeDialog
            {
                Parent = this,
                Visible = false,
            };
            NPCAccessoryLevelBox = new NPCAccessoryLevelDialog
            {
                Parent = this,
                Visible = false,
            };
            NPCAccessoryResetBox = new NPCAccessoryResetDialog
            {
                Parent = this,
                Visible = false,
            };
            NPCRefineBox = new NPCRefineDialog
            {
                Parent = this,
                Visible = false
            };
            NPCRefineRetrieveBox = new NPCRefineRetrieveDialog
            {
                Parent = this,
                Visible = false
            };
            NPCMasterRefineBox = new NPCMasterRefineDialog
            {
                Parent = this,
                Visible = false
            };
            NPCRollBox = new NPCRollDialog
            {
                Parent = this,
                Visible = false
            };

            InspectBox = new CharacterDialog(true)
            {
                Parent = this,
                Visible = false
            };
            RankingBox = new RankingDialog(true)
            {
                Parent = this,
                Visible = false
            };
            MarketPlaceBox = new MarketPlaceDialog
            {
                Parent = this,
                Visible = false,
            };
            DungeonFinderBox = new DungeonFinderDialog
            {
                Parent = this,
                Visible = false,
            };
            EditCharacterBox = new EditCharacterDialog
            {
                Parent = this,
                Visible = false
            };
            CommunicationBox = new CommunicationDialog
            {
                Parent = this,
                Visible = false
            };
            TradeBox = new TradeDialog
            {
                Parent = this,
                Visible = false
            };
            GuildBox = new GuildDialog
            {
                Parent = this,
                Visible = false
            };
            GuildMemberBox = new GuildMemberDialog
            {
                Parent = this,
                Visible = false
            };

            QuestBox = new QuestDialog
            {
                Parent = this,
                Visible = false
            };

            QuestTrackerBox = new QuestTrackerDialog
            {
                Parent = this,
                Visible = false
            };

            MilestoneAchievedBox = new MilestoneAchievedDialog
            {
                Parent = this,
                Visible = false,
            };

            CompanionBox = new CompanionDialog
            {
                Parent = this,
                Visible = false,
            };

            MonsterBox = new MonsterDialog
            {
                Parent = this,
                Visible = false,
            };
            MagicBarBox = new MagicBarDialog
            {
                Parent = this,
                Visible = false,
            };

            FortuneCheckerBox = new FortuneCheckerDialog
            {
                Parent = this,
                Visible = false,
            };

            CurrencyBox = new CurrencyDialog
            {
                Parent = this,
                Visible = false,
            };

            TimerBox = new TimerDialog
            {
                Parent = this,
                Visible = true,
            };

            NPCWeaponCraftBox = new NPCWeaponCraftWindow
            {
                Visible = false,
                Parent = this,
            };
            NPCAccessoryRefineBox = new NPCAccessoryRefineDialog
            {
                Parent = this,
                Visible = false,
            };

            FishingBox = new FishingDialog(CharacterBox)
            {
                Parent = this,
                Visible = false,
            };

            FishingCatchBox = new FishingCatchDialog
            {
                Parent = this,
                Visible = false,
            };

            BundleBox = new BundleDialog
            {
                Parent = this,
                Visible = false
            };

            LootBoxBox = new LootBoxDialog
            {
                Parent = this,
                Visible = false
            };

            SetDefaultLocations();

            LoadChatTabs();

            foreach (DXWindow window in DXWindow.Windows)
                window.LoadSettings();

            CharacterBox.LoadSettings();
            MagicBox.LoadSettings();
            InventoryBox.LoadSettings();
            StorageBox.LoadSettings();
            TradeBox.LoadSettings();
            CompanionBox.LoadSettings();
            CommunicationBox.LoadSettings();
            RankingBox.LoadSettings();
            QuestBox.LoadSettings();
            FishingBox.LoadSettings();
            GroupBox.LoadSettings();
            GuildBox.LoadSettings();
            MenuBox.LoadSettings();
            HelpBox.LoadSettings();
        }

        #region Methods
        private void SetDefaultLocations()
        {
            if (ConfigBox == null) return;

            MenuBox.Location = new Point(Size.Width - MenuBox.Size.Width, Size.Height - MenuBox.Size.Height - MainPanel.Size.Height);

            ConfigBox.Location = new Point((Size.Width - ConfigBox.Size.Width) / 2, (Size.Height - ConfigBox.Size.Height) / 2);

            CaptionBox.Location = Point.Empty;

            ChatOptionsBox.Location = new Point((Size.Width - ChatOptionsBox.Size.Width) / 2, (Size.Height - ChatOptionsBox.Size.Height) / 2);

            ExitBox.Location = new Point((Size.Width - ExitBox.Size.Width) / 2, (Size.Height - ExitBox.Size.Height) / 2);

            TradeBox.Location = new Point((Size.Width - TradeBox.Size.Width) / 2, (Size.Height - TradeBox.Size.Height) / 2);

            GuildBox.Location = new Point((Size.Width - GuildBox.Size.Width) / 2, (Size.Height - GuildBox.Size.Height) / 2);

            GuildMemberBox.Location = new Point((Size.Width - GuildMemberBox.Size.Width) / 2, (Size.Height - GuildMemberBox.Size.Height) / 2);

            InventoryBox.Location = new Point(Size.Width - InventoryBox.Size.Width, MiniMapBox.Size.Height);

            CharacterBox.Location = Point.Empty;

            MapControl.Size = Size;

            MainPanel.Location = new Point((Size.Width - MainPanel.Size.Width) / 2, Size.Height - MainPanel.Size.Height);

            ChatTextBox.Location = new Point((Size.Width - ChatTextBox.Size.Width) / 2, (Size.Height - ChatTextBox.Size.Height) / 2);

            BeltBox.Location = new Point(MainPanel.Location.X + MainPanel.Size.Width - BeltBox.Size.Width, MainPanel.Location.Y - BeltBox.Size.Height);

            NPCBox.Location = Point.Empty;

            NPCGoodsBox.Location = new Point(0, NPCBox.Size.Height);

            NPCRollBox.Location = new Point((Size.Width - NPCRollBox.Size.Width) / 2, (Size.Height - NPCRollBox.Size.Height) / 2);

            NPCRepairBox.Location = new Point(0, NPCBox.Size.Height);

            MiniMapBox.Location = new Point(Size.Width - MiniMapBox.Size.Width, 0);

            QuestTrackerBox.Location = new Point(Size.Width - QuestTrackerBox.Size.Width, MiniMapBox.Size.Height + 5);

            MilestoneAchievedBox.Location = new Point((Size.Width - MilestoneAchievedBox.Size.Width) / 2, ((Size.Height - MilestoneAchievedBox.Size.Height) / 2) + 100);

            BuffBox.Location = new Point(Size.Width - MiniMapBox.Size.Width - BuffBox.Size.Width - 5, 0);

            MagicBox.Location = new Point(Size.Width - MagicBox.Size.Width, 0);

            GroupBox.Location = new Point((Size.Width - GroupBox.Size.Width) / 2, (Size.Height - GroupBox.Size.Height) / 2);

            StorageBox.Location = new Point(Size.Width - StorageBox.Size.Width - InventoryBox.Size.Width, 0);

            AutoPotionBox.Location = new Point((Size.Width - AutoPotionBox.Size.Width) / 2, (Size.Height - AutoPotionBox.Size.Height) / 2);

            InspectBox.Location = new Point(CharacterBox.Size.Width, 0);

            RankingBox.Location = new Point((Size.Width - RankingBox.Size.Width) / 2, (Size.Height - RankingBox.Size.Height) / 2);

            MarketPlaceBox.Location = new Point((Size.Width - MarketPlaceBox.Size.Width) / 2, (Size.Height - MarketPlaceBox.Size.Height) / 2);

            CommunicationBox.Location = new Point((Size.Width - CommunicationBox.Size.Width) / 2, (Size.Height - CommunicationBox.Size.Height) / 2);

            CompanionBox.Location = new Point((Size.Width - CompanionBox.Size.Width) / 2, (Size.Height - CompanionBox.Size.Height) / 2);

            MonsterBox.Location = new Point((Size.Width - MonsterBox.Size.Width) / 2, 50);

            EditCharacterBox.Location = new Point((Size.Width - EditCharacterBox.Size.Width) / 2, (Size.Height - EditCharacterBox.Size.Height) / 2);

            FortuneCheckerBox.Location = new Point((Size.Width - FortuneCheckerBox.Size.Width) / 2, (Size.Height - FortuneCheckerBox.Size.Height) / 2);

            NPCWeaponCraftBox.Location = new Point((Size.Width - NPCWeaponCraftBox.Size.Width) / 2, (Size.Height - NPCWeaponCraftBox.Size.Height) / 2);

            CurrencyBox.Location = new Point((Size.Width - CurrencyBox.Size.Width) / 2, (Size.Height - CurrencyBox.Size.Height) / 2);

            FishingBox.Location = new Point(CharacterBox.Location.X + CharacterBox.Size.Width, CharacterBox.Location.Y);

            FishingCatchBox.Location = new Point(((Size.Width - FishingCatchBox.Size.Width) / 2), ((Size.Height - FishingCatchBox.Size.Height) / 2) + 200);

            TimerBox.Location = new Point(MainPanel.DisplayArea.Right - 115, Size.Height - 170);

            BundleBox.Location = new Point((Size.Width - BundleBox.Size.Width) / 2, (Size.Height - BundleBox.Size.Height) / 2);

            LootBoxBox.Location = new Point((Size.Width - LootBoxBox.Size.Width) / 2, (Size.Height - LootBoxBox.Size.Height) / 2);
        }

        public void SaveChatTabs()
        {
            DBCollection<ChatTabControlSetting> controlSettings = CEnvir.Session.GetCollection<ChatTabControlSetting>();
            DBCollection<ChatTabPageSetting> pageSettings = CEnvir.Session.GetCollection<ChatTabPageSetting>();

            for (int i = controlSettings.Binding.Count - 1; i >= 0; i--)
                controlSettings.Binding[i].Delete();

            foreach (DXControl temp1 in Controls)
            {
                DXTabControl tabControl = temp1 as DXTabControl;

                if (tabControl == null) continue;

                ChatTabControlSetting cSetting = controlSettings.CreateNewObject();

                cSetting.Resolution = Config.GameSize;
                cSetting.Location = tabControl.Location;
                cSetting.Size = tabControl.Size;

                foreach (DXControl tempC in tabControl.Controls)
                {
                    ChatTab tab = tempC as ChatTab;
                    if (tab == null) continue;

                    ChatTabPageSetting pSetting = pageSettings.CreateNewObject();

                    pSetting.Parent = cSetting;

                    if (tabControl.SelectedTab == tab)
                        cSetting.SelectedPage = pSetting;

                    pSetting.Name = tab.Panel.NameTextBox.TextBox.Text;
                    pSetting.Transparent = tab.Panel.TransparentCheckBox.Checked;
                    pSetting.Alert = tab.Panel.AlertCheckBox.Checked;
                    pSetting.HideTab = tab.Panel.HideTabCheckBox.Checked;
                    pSetting.ReverseList = tab.Panel.ReverseListCheckBox.Checked;
                    pSetting.CleanUp = tab.Panel.CleanUpCheckBox.Checked;
                    pSetting.FadeOut = tab.Panel.FadeOutCheckBox.Checked;

                    pSetting.LocalChat = tab.Panel.LocalCheckBox.Checked;
                    pSetting.WhisperChat = tab.Panel.WhisperCheckBox.Checked;
                    pSetting.GroupChat = tab.Panel.GroupCheckBox.Checked;
                    pSetting.GuildChat = tab.Panel.GuildCheckBox.Checked;
                    pSetting.ShoutChat = tab.Panel.ShoutCheckBox.Checked;
                    pSetting.GlobalChat = tab.Panel.GlobalCheckBox.Checked;
                    pSetting.ObserverChat = tab.Panel.ObserverCheckBox.Checked;
                    pSetting.HintChat = tab.Panel.HintCheckBox.Checked;
                    pSetting.SystemChat = tab.Panel.SystemCheckBox.Checked;
                    pSetting.GainsChat = tab.Panel.GainsCheckBox.Checked;
                }
            }
        }

        public void LoadChatTabs()
        {
            if (ConfigBox == null) return;

            for (int i = ChatTab.Tabs.Count - 1; i >= 0; i--)
                ChatTab.Tabs[i].Panel.RemoveButton.InvokeMouseClick();

            DBCollection<ChatTabControlSetting> controlSettings = CEnvir.Session.GetCollection<ChatTabControlSetting>();

            bool result = false;
            foreach (ChatTabControlSetting cSetting in controlSettings.Binding)
            {
                if (cSetting.Resolution != Config.GameSize) continue;

                result = true;

                DXTabControl tabControl = new DXTabControl
                {
                    Location = cSetting.Location,
                    Size = cSetting.Size,
                    Parent = this,
                };

                ChatTab selected = null;
                foreach (ChatTabPageSetting pSetting in cSetting.Controls)
                {
                    ChatTab tab = ChatOptionsBox.AddNewTab(pSetting);

                    tab.Parent = tabControl;

                    tab.Panel.NameTextBox.TextBox.Text = tab.Settings.Name;
                    tab.Panel.AlertCheckBox.Checked = tab.Settings.Alert;
                    tab.Panel.HideTabCheckBox.Checked = tab.Settings.HideTab;
                    tab.Panel.ReverseListCheckBox.Checked = tab.Settings.ReverseList;
                    tab.Panel.CleanUpCheckBox.Checked = tab.Settings.CleanUp;
                    tab.Panel.FadeOutCheckBox.Checked = tab.Settings.FadeOut;

                    tab.Panel.LocalCheckBox.Checked = tab.Settings.LocalChat;
                    tab.Panel.WhisperCheckBox.Checked = tab.Settings.WhisperChat;
                    tab.Panel.GroupCheckBox.Checked = tab.Settings.GroupChat;
                    tab.Panel.GuildCheckBox.Checked = tab.Settings.GuildChat;
                    tab.Panel.ShoutCheckBox.Checked = tab.Settings.ShoutChat;
                    tab.Panel.GlobalCheckBox.Checked = tab.Settings.GlobalChat;
                    tab.Panel.ObserverCheckBox.Checked = tab.Settings.ObserverChat;

                    tab.Panel.HintCheckBox.Checked = tab.Settings.HintChat;
                    tab.Panel.SystemCheckBox.Checked = tab.Settings.SystemChat;
                    tab.Panel.GainsCheckBox.Checked = tab.Settings.GainsChat;
                }

                foreach (ChatTab tab in ChatTab.Tabs)
                {
                    tab.Panel.TransparentCheckBox.Checked = tab.Settings.Transparent;

                    if (tab.Settings == cSetting.SelectedPage)
                        selected = tab;
                }

                tabControl.SelectedTab = selected;
            }

            if (result)
                Game.ReceiveChat(CEnvir.Language.ChatLayoutLoaded, MessageType.Announcement);
            else
                ChatOptionsBox.CreateDefaultWindows();
        }

        #endregion

        #region IDisposable

        protected override void Dispose(bool disposing)
        {
            base.Dispose(disposing);

            if (disposing)
            {
                if (Game == this) Game = null;

                _SelectedCell = null;

                _User = null;
                _MouseItem = null;
                _MouseMagic = null;
                _MouseFame = null;

                CurrencyPickedUp = null;

                MagicObject = null;
                MouseObject = null;
                TargetObject = null;
                FocusObject = null;

                if (ItemLabel != null)
                {
                    if (!ItemLabel.IsDisposed)
                        ItemLabel.Dispose();

                    ItemLabel = null;
                }

                if (MagicLabel != null)
                {
                    if (!MagicLabel.IsDisposed)
                        MagicLabel.Dispose();

                    MagicLabel = null;
                }

                if (FameLabel != null)
                {
                    if (!FameLabel.IsDisposed)
                        FameLabel.Dispose();

                    FameLabel = null;
                }

                if (MapControl != null)
                {
                    if (!MapControl.IsDisposed)
                        MapControl.Dispose();

                    MapControl = null;
                }

                if (MainPanel != null)
                {
                    if (!MainPanel.IsDisposed)
                        MainPanel.Dispose();

                    MainPanel = null;
                }

                if (MenuBox != null)
                {
                    if (!MenuBox.IsDisposed)
                        MenuBox.Dispose();

                    MenuBox = null;
                }

                if (ConfigBox != null)
                {
                    if (!ConfigBox.IsDisposed)
                        ConfigBox.Dispose();

                    ConfigBox = null;
                }

                if (InventoryBox != null)
                {
                    if (!InventoryBox.IsDisposed)
                        InventoryBox.Dispose();

                    InventoryBox = null;
                }

                if (CharacterBox != null)
                {
                    if (!CharacterBox.IsDisposed)
                        CharacterBox.Dispose();

                    CharacterBox = null;
                }

                if (ExitBox != null)
                {
                    if (!ExitBox.IsDisposed)
                        ExitBox.Dispose();

                    ExitBox = null;
                }

                if (HelpBox != null)
                {
                    if (!HelpBox.IsDisposed)
                        HelpBox.Dispose();

                    HelpBox = null;
                }

                if (ChatTextBox != null)
                {
                    if (!ChatTextBox.IsDisposed)
                        ChatTextBox.Dispose();

                    ChatTextBox = null;
                }

                if (BeltBox != null)
                {
                    if (!BeltBox.IsDisposed)
                        BeltBox.Dispose();

                    BeltBox = null;
                }

                if (ChatOptionsBox != null)
                {
                    if (!ChatOptionsBox.IsDisposed)
                        ChatOptionsBox.Dispose();

                    ChatOptionsBox = null;
                }

                if (NPCBox != null)
                {
                    if (!NPCBox.IsDisposed)
                        NPCBox.Dispose();

                    NPCBox = null;
                }

                if (NPCGoodsBox != null)
                {
                    if (!NPCGoodsBox.IsDisposed)
                        NPCGoodsBox.Dispose();

                    NPCGoodsBox = null;
                }

                if (NPCRefinementStoneBox != null)
                {
                    if (!NPCRefinementStoneBox.IsDisposed)
                        NPCRefinementStoneBox.Dispose();

                    NPCRefinementStoneBox = null;
                }

                if (NPCRepairBox != null)
                {
                    if (!NPCRepairBox.IsDisposed)
                        NPCRepairBox.Dispose();

                    NPCRepairBox = null;
                }

                if (NPCRefineBox != null)
                {
                    if (!NPCRefineBox.IsDisposed)
                        NPCRefineBox.Dispose();

                    NPCRefineBox = null;
                }

                if (NPCRefineRetrieveBox != null)
                {
                    if (!NPCRefineRetrieveBox.IsDisposed)
                        NPCRefineRetrieveBox.Dispose();

                    NPCRefineRetrieveBox = null;
                }

                if (NPCMasterRefineBox != null)
                {
                    if (!NPCMasterRefineBox.IsDisposed)
                        NPCMasterRefineBox.Dispose();

                    NPCMasterRefineBox = null;
                }

                if (NPCRollBox != null)
                {
                    if (!NPCRollBox.IsDisposed)
                        NPCRollBox.Dispose();

                    NPCRollBox = null;
                }

                if (NPCQuestListBox != null)
                {
                    if (!NPCQuestListBox.IsDisposed)
                        NPCQuestListBox.Dispose();

                    NPCQuestListBox = null;
                }

                if (NPCQuestBox != null)
                {
                    if (!NPCQuestBox.IsDisposed)
                        NPCQuestBox.Dispose();

                    NPCQuestBox = null;
                }

                if (NPCAdoptCompanionBox != null)
                {
                    if (!NPCAdoptCompanionBox.IsDisposed)
                        NPCAdoptCompanionBox.Dispose();

                    NPCAdoptCompanionBox = null;
                }

                if (NPCCompanionStorageBox != null)
                {
                    if (!NPCCompanionStorageBox.IsDisposed)
                        NPCCompanionStorageBox.Dispose();

                    NPCCompanionStorageBox = null;
                }

                if (NPCWeddingRingBox != null)
                {
                    if (!NPCWeddingRingBox.IsDisposed)
                        NPCWeddingRingBox.Dispose();

                    NPCWeddingRingBox = null;
                }

                if (NPCItemFragmentBox != null)
                {
                    if (!NPCItemFragmentBox.IsDisposed)
                        NPCItemFragmentBox.Dispose();

                    NPCItemFragmentBox = null;
                }

                if (NPCAccessoryUpgradeBox != null)
                {
                    if (!NPCAccessoryUpgradeBox.IsDisposed)
                        NPCAccessoryUpgradeBox.Dispose();

                    NPCAccessoryUpgradeBox = null;
                }

                if (NPCAccessoryLevelBox != null)
                {
                    if (!NPCAccessoryLevelBox.IsDisposed)
                        NPCAccessoryLevelBox.Dispose();

                    NPCAccessoryLevelBox = null;
                }

                if (NPCAccessoryResetBox != null)
                {
                    if (!NPCAccessoryResetBox.IsDisposed)
                        NPCAccessoryResetBox.Dispose();

                    NPCAccessoryResetBox = null;
                }

                if (MiniMapBox != null)
                {
                    if (!MiniMapBox.IsDisposed)
                        MiniMapBox.Dispose();

                    MiniMapBox = null;
                }

                if (BigMapBox != null)
                {
                    if (!BigMapBox.IsDisposed)
                        BigMapBox.Dispose();

                    BigMapBox = null;
                }

                if (MagicBox != null)
                {
                    if (!MagicBox.IsDisposed)
                        MagicBox.Dispose();

                    MagicBox = null;
                }

                if (GroupBox != null)
                {
                    if (!GroupBox.IsDisposed)
                        GroupBox.Dispose();

                    GroupBox = null;
                }

                if (GroupHealthBox != null)
                {
                    if (!GroupHealthBox.IsDisposed)
                        GroupHealthBox.Dispose();

                    GroupHealthBox = null;
                }

                if (BuffBox != null)
                {
                    if (!BuffBox.IsDisposed)
                        BuffBox.Dispose();

                    BuffBox = null;
                }

                if (StorageBox != null)
                {
                    if (!StorageBox.IsDisposed)
                        StorageBox.Dispose();

                    StorageBox = null;
                }

                if (AutoPotionBox != null)
                {
                    if (!AutoPotionBox.IsDisposed)
                        AutoPotionBox.Dispose();

                    AutoPotionBox = null;
                }

                if (InspectBox != null)
                {
                    if (!InspectBox.IsDisposed)
                        InspectBox.Dispose();

                    InspectBox = null;
                }

                if (RankingBox != null)
                {
                    if (!RankingBox.IsDisposed)
                        RankingBox.Dispose();

                    RankingBox = null;
                }

                if (MarketPlaceBox != null)
                {
                    if (!MarketPlaceBox.IsDisposed)
                        MarketPlaceBox.Dispose();

                    MarketPlaceBox = null;
                }

                if (CommunicationBox != null)
                {
                    if (!CommunicationBox.IsDisposed)
                        CommunicationBox.Dispose();

                    CommunicationBox = null;
                }

                if (TradeBox != null)
                {
                    if (!TradeBox.IsDisposed)
                        TradeBox.Dispose();

                    TradeBox = null;
                }

                if (GuildBox != null)
                {
                    if (!GuildBox.IsDisposed)
                        GuildBox.Dispose();

                    GuildBox = null;
                }

                if (GuildMemberBox != null)
                {
                    if (!GuildMemberBox.IsDisposed)
                        GuildMemberBox.Dispose();

                    GuildMemberBox = null;
                }

                if (QuestBox != null)
                {
                    if (!QuestBox.IsDisposed)
                        QuestBox.Dispose();

                    QuestBox = null;
                }

                if (QuestTrackerBox != null)
                {
                    if (!QuestTrackerBox.IsDisposed)
                        QuestTrackerBox.Dispose();

                    QuestTrackerBox = null;
                }

                if (CompanionBox != null)
                {
                    if (!CompanionBox.IsDisposed)
                        CompanionBox.Dispose();

                    CompanionBox = null;
                }

                if (MonsterBox != null)
                {
                    if (!MonsterBox.IsDisposed)
                        MonsterBox.Dispose();

                    MonsterBox = null;
                }

                if (MagicBarBox != null)
                {
                    if (!MagicBarBox.IsDisposed)
                        MagicBarBox.Dispose();

                    MagicBarBox = null;
                }

                if (NPCAccessoryRefineBox != null)
                {
                    if (!NPCAccessoryRefineBox.IsDisposed)
                        NPCAccessoryRefineBox.Dispose();

                    NPCAccessoryRefineBox = null;
                }

                if (FishingBox != null)
                {
                    if (!FishingBox.IsDisposed)
                        FishingBox.Dispose();

                    FishingBox = null;
                }

                if (FishingCatchBox != null)
                {
                    if (!FishingCatchBox.IsDisposed)
                        FishingCatchBox.Dispose();

                    FishingCatchBox = null;
                }

                Inventory = null;
                Equipment = null;
                QuestLog = null;

                DataDictionary.Clear();
                DataDictionary = null;

                MoveFrame = false;
                MoveTime = DateTime.MinValue;
                OutputTime = DateTime.MinValue;
                ItemRefreshTime = DateTime.MinValue;

                CanRun = false;
                AutoRun = false;
                _NPCID = 0;
                _Companion = null;
                _Partner = null;

                PickUpTime = DateTime.MinValue;
                UseItemTime = DateTime.MinValue;
                NPCTime = DateTime.MinValue;
                ToggleTime = DateTime.MinValue;
                InspectTime = DateTime.MinValue;
                ItemTime = DateTime.MinValue;
                ItemReviveTime = DateTime.MinValue;

                _DayTime = 0f;
            }
        }

        #endregion

    }
}
