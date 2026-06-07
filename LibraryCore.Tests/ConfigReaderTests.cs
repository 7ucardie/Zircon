using Library;
using System;
using System.Drawing;
using Xunit;

namespace LibraryCore.Tests;

public class ConfigReaderTests
{
    // Each test uses a unique section/key to avoid cross-test contamination
    // in the static ConfigContents dictionary.

    [Fact]
    public void ReadInt32_ReturnsDefaultWhenNotPreviouslySet()
    {
        int result = ConfigReader.Read(typeof(ConfigReaderTests), "S_Int32_Default", "K1", 42);
        Assert.Equal(42, result);
    }

    [Fact]
    public void ReadInt32_ReturnsParsedValueOnSubsequentCall()
    {
        ConfigReader.Read(typeof(ConfigReaderTests), "S_Int32_Parse", "K2", 100);
        int result = ConfigReader.Read(typeof(ConfigReaderTests), "S_Int32_Parse", "K2", 999);
        Assert.Equal(100, result);
    }

    [Fact]
    public void ReadString_RoundTrips()
    {
        ConfigReader.Read(typeof(ConfigReaderTests), "S_String", "K3", "hello");
        string result = ConfigReader.Read(typeof(ConfigReaderTests), "S_String", "K3", "world");
        Assert.Equal("hello", result);
    }

    [Fact]
    public void ReadBool_TrueRoundTrips()
    {
        ConfigReader.Read(typeof(ConfigReaderTests), "S_Bool", "K4", true);
        bool result = ConfigReader.Read(typeof(ConfigReaderTests), "S_Bool", "K4", false);
        Assert.True(result);
    }

    [Fact]
    public void ReadBool_FalseRoundTrips()
    {
        ConfigReader.Read(typeof(ConfigReaderTests), "S_BoolFalse", "K5", false);
        bool result = ConfigReader.Read(typeof(ConfigReaderTests), "S_BoolFalse", "K5", true);
        Assert.False(result);
    }

    [Fact]
    public void ReadTimeSpan_RoundTrips()
    {
        var span = TimeSpan.FromMinutes(5);
        ConfigReader.Read(typeof(ConfigReaderTests), "S_TimeSpan", "K6", span);
        TimeSpan result = ConfigReader.Read(typeof(ConfigReaderTests), "S_TimeSpan", "K6", TimeSpan.Zero);
        Assert.Equal(span, result);
    }

    [Fact]
    public void ReadDouble_RoundTrips()
    {
        ConfigReader.Read(typeof(ConfigReaderTests), "S_Double", "K7", 3.14);
        double result = ConfigReader.Read(typeof(ConfigReaderTests), "S_Double", "K7", 0.0);
        Assert.Equal(3.14, result, precision: 10);
    }

    [Fact]
    public void ReadPoint_RoundTrips()
    {
        var point = new Point(10, 20);
        ConfigReader.Read(typeof(ConfigReaderTests), "S_Point", "K8", point);
        Point result = ConfigReader.Read(typeof(ConfigReaderTests), "S_Point", "K8", Point.Empty);
        Assert.Equal(point, result);
    }
}
