using Library;
using Library.Network;
using System.Linq;
using Xunit;
using G = Library.Network.GeneralPackets;

namespace LibraryCore.Tests;

public class PacketSerializationTests
{
    [Fact]
    public void Disconnect_SerializeDeserialize_PreservesReason()
    {
        var original = new G.Disconnect { Reason = DisconnectReason.TimedOut };
        byte[] bytes = original.GetPacketBytes();

        Packet? p = Packet.ReceivePacket(bytes, out byte[] extra);

        Assert.NotNull(p);
        Assert.IsType<G.Disconnect>(p);
        Assert.Empty(extra);
        Assert.Equal(DisconnectReason.TimedOut, ((G.Disconnect)p).Reason);
    }

    [Fact]
    public void PingResponse_SerializeDeserialize_PreservesPing()
    {
        var original = new G.PingResponse { Ping = 42 };
        byte[] bytes = original.GetPacketBytes();

        Packet? p = Packet.ReceivePacket(bytes, out byte[] extra);

        Assert.NotNull(p);
        Assert.IsType<G.PingResponse>(p);
        Assert.Empty(extra);
        Assert.Equal(42, ((G.PingResponse)p).Ping);
    }

    [Fact]
    public void GetPacketBytes_LengthPrefixMatchesTotalSize()
    {
        var packet = new G.Disconnect { Reason = DisconnectReason.Kicked };
        byte[] bytes = packet.GetPacketBytes();
        int declaredLength = bytes[3] << 24 | bytes[2] << 16 | bytes[1] << 8 | bytes[0];
        Assert.Equal(bytes.Length, declaredLength);
    }

    [Fact]
    public void ReceivePacket_InsufficientData_ReturnsNull()
    {
        byte[] tooShort = new byte[3];
        Packet? p = Packet.ReceivePacket(tooShort, out byte[] extra);
        Assert.Null(p);
        Assert.Same(tooShort, extra);
    }

    [Fact]
    public void MultiplePackets_ConsumesOneAtATime()
    {
        byte[] p1Bytes = new G.Disconnect { Reason = DisconnectReason.Kicked }.GetPacketBytes();
        byte[] p2Bytes = new G.PingResponse { Ping = 10 }.GetPacketBytes();
        byte[] combined = p1Bytes.Concat(p2Bytes).ToArray();

        Packet? first = Packet.ReceivePacket(combined, out byte[] remaining);

        Assert.NotNull(first);
        Assert.IsType<G.Disconnect>(first);
        Assert.Equal(p2Bytes.Length, remaining.Length);

        Packet? second = Packet.ReceivePacket(remaining, out byte[] noMore);
        Assert.NotNull(second);
        Assert.IsType<G.PingResponse>(second);
        Assert.Empty(noMore);
    }

    [Fact]
    public void Disconnect_DifferentReasons_SerializeDistinctly()
    {
        byte[] timedOut = new G.Disconnect { Reason = DisconnectReason.TimedOut }.GetPacketBytes();
        byte[] byPlayer = new G.Disconnect { Reason = DisconnectReason.Kicked }.GetPacketBytes();
        Assert.False(timedOut.SequenceEqual(byPlayer));
    }
}
