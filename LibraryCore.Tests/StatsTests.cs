using Library;
using System.IO;
using Xunit;

namespace LibraryCore.Tests;

public class StatsTests
{
    [Fact]
    public void DefaultValue_IsZero()
    {
        var stats = new Stats();
        Assert.Equal(0, stats[Stat.BaseHealth]);
    }

    [Fact]
    public void SetAndGet_Value()
    {
        var stats = new Stats();
        stats[Stat.BaseHealth] = 100;
        Assert.Equal(100, stats[Stat.BaseHealth]);
    }

    [Fact]
    public void SetZero_RemovesKey()
    {
        var stats = new Stats();
        stats[Stat.BaseHealth] = 50;
        stats[Stat.BaseHealth] = 0;
        Assert.False(stats.Values.ContainsKey(Stat.BaseHealth));
    }

    [Fact]
    public void Count_IsSumOfAbsoluteValues()
    {
        var stats = new Stats();
        stats[Stat.BaseHealth] = 10;
        stats[Stat.BaseMana] = -5;
        Assert.Equal(15, stats.Count);
    }

    [Fact]
    public void NegativeValue_IsReturned()
    {
        var stats = new Stats();
        stats[Stat.BaseHealth] = -30;
        Assert.Equal(-30, stats[Stat.BaseHealth]);
    }

    [Fact]
    public void CopyConstructor_CopiesAllValues()
    {
        var original = new Stats();
        original[Stat.BaseHealth] = 100;
        original[Stat.BaseMana] = 50;

        var copy = new Stats(original);
        Assert.Equal(100, copy[Stat.BaseHealth]);
        Assert.Equal(50, copy[Stat.BaseMana]);
    }

    [Fact]
    public void CopyConstructor_IsIndependent()
    {
        var original = new Stats();
        original[Stat.BaseHealth] = 100;

        var copy = new Stats(original);
        copy[Stat.BaseHealth] = 999;

        Assert.Equal(100, original[Stat.BaseHealth]);
    }

    [Fact]
    public void Serialization_RoundTrip_PreservesValues()
    {
        var original = new Stats();
        original[Stat.BaseHealth] = 500;
        original[Stat.BaseMana] = 200;

        using var ms = new MemoryStream();
        using (var writer = new BinaryWriter(ms, System.Text.Encoding.UTF8, leaveOpen: true))
            original.Write(writer);

        ms.Position = 0;
        using var reader = new BinaryReader(ms);
        var restored = new Stats(reader);

        Assert.Equal(500, restored[Stat.BaseHealth]);
        Assert.Equal(200, restored[Stat.BaseMana]);
        Assert.Equal(2, restored.Values.Count);
    }

    [Fact]
    public void Serialization_EmptyStats_RoundTrip()
    {
        var original = new Stats();

        using var ms = new MemoryStream();
        using (var writer = new BinaryWriter(ms, System.Text.Encoding.UTF8, leaveOpen: true))
            original.Write(writer);

        ms.Position = 0;
        using var reader = new BinaryReader(ms);
        var restored = new Stats(reader);

        Assert.Equal(0, restored.Values.Count);
    }
}
