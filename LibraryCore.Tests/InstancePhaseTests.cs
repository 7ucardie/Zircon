using Library;
using Xunit;

namespace LibraryCore.Tests
{
    public class InstancePhaseTests
    {
        [Fact]
        public void ConditionType_MonsterClear_IsZero()
        {
            Assert.Equal(0, (int)InstancePhaseConditionType.MonsterClear);
        }

        [Fact]
        public void ConditionType_Timer_IsOne()
        {
            Assert.Equal(1, (int)InstancePhaseConditionType.Timer);
        }

        [Fact]
        public void ConditionType_ItemUsed_IsTwo()
        {
            Assert.Equal(2, (int)InstancePhaseConditionType.ItemUsed);
        }

        [Fact]
        public void ActionType_SpawnGroup_IsZero()
        {
            Assert.Equal(0, (int)InstancePhaseActionType.SpawnGroup);
        }

        [Fact]
        public void ActionType_UnlockRegion_IsOne()
        {
            Assert.Equal(1, (int)InstancePhaseActionType.UnlockRegion);
        }

        [Fact]
        public void ActionType_SendMessage_IsTwo()
        {
            Assert.Equal(2, (int)InstancePhaseActionType.SendMessage);
        }

        [Fact]
        public void ActionType_AwardItem_IsThree()
        {
            Assert.Equal(3, (int)InstancePhaseActionType.AwardItem);
        }

        [Theory]
        [InlineData(InstancePhaseConditionType.MonsterClear)]
        [InlineData(InstancePhaseConditionType.Timer)]
        [InlineData(InstancePhaseConditionType.ItemUsed)]
        public void ConditionType_AllValues_AreDefined(InstancePhaseConditionType value)
        {
            Assert.True(System.Enum.IsDefined(typeof(InstancePhaseConditionType), value));
        }

        [Theory]
        [InlineData(InstancePhaseActionType.SpawnGroup)]
        [InlineData(InstancePhaseActionType.UnlockRegion)]
        [InlineData(InstancePhaseActionType.SendMessage)]
        [InlineData(InstancePhaseActionType.AwardItem)]
        public void ActionType_AllValues_AreDefined(InstancePhaseActionType value)
        {
            Assert.True(System.Enum.IsDefined(typeof(InstancePhaseActionType), value));
        }
    }
}
