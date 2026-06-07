using Library;
using System;
using System.Drawing;
using Xunit;

namespace LibraryCore.Tests;

public class FunctionsTests
{
    [Fact]
    public void Max_ReturnsLarger()
    {
        var a = TimeSpan.FromSeconds(5);
        var b = TimeSpan.FromSeconds(3);
        Assert.Equal(a, Functions.Max(a, b));
        Assert.Equal(a, Functions.Max(b, a));
    }

    [Fact]
    public void Min_ReturnsSmaller()
    {
        var a = TimeSpan.FromSeconds(5);
        var b = TimeSpan.FromSeconds(3);
        Assert.Equal(b, Functions.Min(a, b));
        Assert.Equal(b, Functions.Min(b, a));
    }

    [Theory]
    [InlineData(0, 0, 0, 0, 0)]
    [InlineData(0, 0, 3, 0, 3)]
    [InlineData(0, 0, 0, 3, 3)]
    [InlineData(0, 0, 3, 3, 3)]
    [InlineData(0, 0, 4, 3, 4)]
    public void Distance_IsChebysevMax(int x1, int y1, int x2, int y2, int expected)
    {
        Assert.Equal(expected, Functions.Distance(new Point(x1, y1), new Point(x2, y2)));
    }

    [Theory]
    [InlineData(0, 0, 3, 3, 6)]
    [InlineData(0, 0, 4, 0, 4)]
    [InlineData(0, 0, 0, 0, 0)]
    public void Distance4Directions_IsManhattanSum(int x1, int y1, int x2, int y2, int expected)
    {
        Assert.Equal(expected, Functions.Distance4Directions(new Point(x1, y1), new Point(x2, y2)));
    }

    [Theory]
    [InlineData(5, 5, 5, 5, 0, true)]
    [InlineData(5, 5, 8, 8, 3, true)]
    [InlineData(5, 5, 9, 5, 3, false)]
    [InlineData(5, 5, 5, 9, 3, false)]
    public void InRange_ChecksChebysevBounds(int ax, int ay, int bx, int by, int range, bool expected)
    {
        Assert.Equal(expected, Functions.InRange(new Point(ax, ay), new Point(bx, by), range));
    }

    [Theory]
    [InlineData(5, 5, 5, 3, MirDirection.Up)]
    [InlineData(5, 5, 7, 3, MirDirection.UpRight)]
    [InlineData(5, 5, 7, 5, MirDirection.Right)]
    [InlineData(5, 5, 7, 7, MirDirection.DownRight)]
    [InlineData(5, 5, 5, 7, MirDirection.Down)]
    [InlineData(5, 5, 3, 7, MirDirection.DownLeft)]
    [InlineData(5, 5, 3, 5, MirDirection.Left)]
    [InlineData(5, 5, 3, 3, MirDirection.UpLeft)]
    public void DirectionFromPoint_AllEightDirections(int sx, int sy, int dx, int dy, MirDirection expected)
    {
        Assert.Equal(expected, Functions.DirectionFromPoint(new Point(sx, sy), new Point(dx, dy)));
    }

    [Theory]
    [InlineData(5, 5, MirDirection.Up,        5, 4)]
    [InlineData(5, 5, MirDirection.UpRight,   6, 4)]
    [InlineData(5, 5, MirDirection.Right,     6, 5)]
    [InlineData(5, 5, MirDirection.DownRight, 6, 6)]
    [InlineData(5, 5, MirDirection.Down,      5, 6)]
    [InlineData(5, 5, MirDirection.DownLeft,  4, 6)]
    [InlineData(5, 5, MirDirection.Left,      4, 5)]
    [InlineData(5, 5, MirDirection.UpLeft,    4, 4)]
    public void Move_AllDirections_SingleStep(int sx, int sy, MirDirection dir, int ex, int ey)
    {
        Assert.Equal(new Point(ex, ey), Functions.Move(new Point(sx, sy), dir));
    }

    [Fact]
    public void Move_MultiStep_ScalesCorrectly()
    {
        Assert.Equal(new Point(5, 2), Functions.Move(new Point(5, 5), MirDirection.Up, 3));
        Assert.Equal(new Point(8, 5), Functions.Move(new Point(5, 5), MirDirection.Right, 3));
    }

    [Fact]
    public void ShiftDirection_WrapsAroundCorrectly()
    {
        // Up(0) shifted -1 = UpLeft(7)
        Assert.Equal(MirDirection.UpLeft, Functions.ShiftDirection(MirDirection.Up, -1));
        // UpLeft(7) shifted +1 = Up(0)
        Assert.Equal(MirDirection.Up, Functions.ShiftDirection(MirDirection.UpLeft, 1));
        // Right(2) shifted +4 = Left(6)
        Assert.Equal(MirDirection.Left, Functions.ShiftDirection(MirDirection.Right, 4));
    }

    [Fact]
    public void IsStraightEightDirection_DiagonalAndCardinal_ReturnTrue()
    {
        Assert.True(Functions.IsStraightEightDirection(new Point(0, 0), new Point(1, 0)));
        Assert.True(Functions.IsStraightEightDirection(new Point(0, 0), new Point(0, 1)));
        Assert.True(Functions.IsStraightEightDirection(new Point(0, 0), new Point(3, 3)));
    }

    [Fact]
    public void IsStraightEightDirection_OffAxis_ReturnFalse()
    {
        Assert.False(Functions.IsStraightEightDirection(new Point(0, 0), new Point(2, 3)));
        Assert.False(Functions.IsStraightEightDirection(new Point(0, 0), new Point(0, 0)));
    }
}
