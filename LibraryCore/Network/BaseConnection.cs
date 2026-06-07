using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.IO;
using System.Net.Sockets;
using System.Reflection;
using System.Threading.Tasks;
using G = Library.Network.GeneralPackets;

namespace Library.Network
{
    public abstract class BaseConnection
    {
        public static Dictionary<string, DiagnosticValue> Diagnostics = new Dictionary<string, DiagnosticValue>();
        public static Dictionary<(Type ConnectionType, Type PacketType), MethodInfo> PacketMethods = new Dictionary<(Type ConnectionType, Type PacketType), MethodInfo>();
        private static readonly object PacketMethodsLock = new object();
        public static bool Monitor;

        public bool Connected { get; set; }
        protected bool Sending { get; set; }

        public int TotalBytesSent { get; set; }
        public int TotalBytesReceived { get; set; }
        public int TotalPacketsProcessed { get; set; }

        public bool AdditionalLogging;

        protected TcpClient Client;
        private NetworkStream _stream;

        public DateTime TimeConnected { get; set; }
        public TimeSpan Duration => Time.Now - TimeConnected;

        protected abstract TimeSpan TimeOutDelay { get; }
        public DateTime TimeOutTime { get; set; }

        private bool _disconnecting;
        public bool Disconnecting
        {
            get { return _disconnecting; }
            set
            {
                if (_disconnecting == value) return;
                _disconnecting = value;
                TimeOutTime = Time.Now.AddSeconds(2);
            }
        }

        public ConcurrentQueue<Packet> ReceiveList = new ConcurrentQueue<Packet>();
        public ConcurrentQueue<Packet> SendList = new ConcurrentQueue<Packet>();
        private byte[] _rawData = new byte[0];

        public EventHandler<Exception> OnException;

        protected BaseConnection(TcpClient client)
        {
            Client = client;
            Client.NoDelay = true;
            _stream = client.GetStream();

            Connected = true;
            TimeConnected = Time.Now;

            TotalPacketsProcessed = 0;
        }

        // Kept for subclass compatibility (CConnection calls this in its constructor).
        protected void BeginReceive() => _ = ReceiveLoopAsync();

        private async Task ReceiveLoopAsync()
        {
            byte[] buffer = new byte[8 * 1024];
            try
            {
                while (Connected)
                {
                    int dataRead = await _stream.ReadAsync(buffer, 0, buffer.Length).ConfigureAwait(false);

                    if (!Connected) return;

                    if (dataRead == 0)
                    {
                        Disconnecting = true;
                        return;
                    }

                    TotalBytesReceived += dataRead;
                    UpdateTimeOut();

                    byte[] temp = _rawData;
                    _rawData = new byte[dataRead + temp.Length];
                    Buffer.BlockCopy(temp, 0, _rawData, 0, temp.Length);
                    Buffer.BlockCopy(buffer, 0, _rawData, temp.Length, dataRead);

                    Packet p;
                    while ((p = Packet.ReceivePacket(_rawData, out _rawData)) != null)
                    {
                        ReceiveList.Enqueue(p);
                        TotalPacketsProcessed++;
                    }
                }
            }
            catch (Exception ex)
            {
                if (AdditionalLogging)
                    OnException?.Invoke(this, ex);
                Disconnecting = true;
            }
        }

        private void BeginSend(List<byte> data) => _ = SendAsync(data);

        private async Task SendAsync(List<byte> data)
        {
            if (!Connected || data.Count == 0) return;

            try
            {
                Sending = true;
                TotalBytesSent += data.Count;
                UpdateTimeOut();
                await _stream.WriteAsync(data.ToArray(), 0, data.Count).ConfigureAwait(false);
            }
            catch (Exception ex)
            {
                if (AdditionalLogging)
                    OnException?.Invoke(this, ex);
                Disconnecting = true;
            }
            finally
            {
                Sending = false;
            }
        }

        public virtual void Enqueue(Packet p)
        {
            if (!Connected || p == null) return;

            SendList.Enqueue(p);
        }

        public abstract void TryDisconnect();

        public virtual void Disconnect()
        {
            if (!Connected) return;

            Connected = false;

            SendList = null;
            ReceiveList = null;
            _rawData = null;
            _stream = null;

            Client.Client.Dispose();
            Client = null;
        }

        public abstract void TrySendDisconnect(Packet p);

        public virtual void SendDisconnect(Packet p)
        {
            if (!Connected || Disconnecting)
            {
                Disconnecting = true;
                return;
            }

            List<byte> data = new List<byte>();
            data.AddRange(p.GetPacketBytes());

            _ = SendDisconnectAsync(data);
        }

        private async Task SendDisconnectAsync(List<byte> data)
        {
            if (!Connected || data.Count == 0 || Disconnecting) return;

            try
            {
                Disconnecting = true;
                TotalBytesSent += data.Count;
                await _stream.WriteAsync(data.ToArray(), 0, data.Count).ConfigureAwait(false);
            }
            catch (Exception ex)
            {
                if (AdditionalLogging)
                    OnException?.Invoke(this, ex);
            }
        }

        public virtual void Process()
        {
            if (Client == null || !Client.Connected)
            {
                TryDisconnect();
                return;
            }

            while (!ReceiveList.IsEmpty && !Disconnecting)
            {
                try
                {
                    Packet p;
                    if (!ReceiveList.TryDequeue(out p)) continue;

                    ProcessPacket(p);
                }
                catch (NotImplementedException ex)
                {
                    OnException(this, ex);
                    Disconnecting = true;
                }
                catch (Exception ex)
                {
                    OnException(this, ex);
                    throw;
                }
            }

            if (Time.Now >= TimeOutTime)
            {
                if (!Disconnecting)
                    TrySendDisconnect(new G.Disconnect { Reason = DisconnectReason.TimedOut });
                else
                    TryDisconnect();

                return;
            }

            if (!Disconnecting && Sending)
                UpdateTimeOut();

            if (SendList.IsEmpty || Sending) return;

            List<byte> data = new List<byte>();
            while (!SendList.IsEmpty)
            {
                Packet p;

                if (!SendList.TryDequeue(out p)) continue;

                if (p == null) continue;

                try
                {
                    byte[] bytes = p.GetPacketBytes();
                    data.AddRange(bytes);
                }
                catch (Exception ex)
                {
                    OnException?.Invoke(this, ex);
                    Disconnecting = true;
                    return;
                }

                if (!Monitor) continue;

                DiagnosticValue value;
                Type type = p.GetType();

                if (!Diagnostics.TryGetValue(type.FullName, out value))
                    Diagnostics[type.FullName] = value = new DiagnosticValue { Name = type.FullName };

                value.Count++;
                value.TotalSize += p.Length;

                if (p.Length > value.LargestSize)
                    value.LargestSize = p.Length;
            }

            BeginSend(data);
        }

        private void ProcessPacket(Packet p)
        {
            if (p == null) return;

            DateTime start = Time.Now;

            Type connectionType = GetType();
            (Type ConnectionType, Type PacketType) key = (connectionType, p.PacketType);

            MethodInfo info;
            lock (PacketMethodsLock)
            {
                if (!PacketMethods.TryGetValue(key, out info))
                {
                    info = connectionType.GetMethod("Process", new[] { p.PacketType });
                    if (info != null)
                        PacketMethods[key] = info;
                }
            }

            if (info == null)
            {
                ProcessUnhandledPacket(p);
                return;
            }

            info.Invoke(this, new object[] { p });

            if (!Monitor) return;

            TimeSpan execution = Time.Now - start;
            DiagnosticValue value;

            if (!Diagnostics.TryGetValue(p.PacketType.FullName, out value))
                Diagnostics[p.PacketType.FullName] = value = new DiagnosticValue { Name = p.PacketType.FullName };

            value.Count++;
            value.TotalTime += execution;
            value.TotalSize += p.Length;

            if (execution > value.LargestTime)
                value.LargestTime = execution;

            if (p.Length > value.LargestSize)
                value.LargestSize = p.Length;
        }

        protected virtual void ProcessUnhandledPacket(Packet p)
        {
            throw new NotImplementedException($"Not Implemented Exception: Method Process({p.PacketType}).");
        }

        public void UpdateTimeOut()
        {
            if (Disconnecting) return;

            TimeOutTime = Time.Now + TimeOutDelay;
        }
    }


    public class DiagnosticValue
    {
        public string Name { get; set; }
        public TimeSpan TotalTime { get; set; }
        public TimeSpan LargestTime { get; set; }
        public int Count { get; set; }
        public long TotalSize { get; set; }
        public long LargestSize { get; set; }

        public long TotalTicks => TotalTime.Ticks;
        public long TotalMilliseconds => TotalTicks / TimeSpan.TicksPerMillisecond;

        public long LargestTicks => LargestTime.Ticks;
        public long LargestMilliseconds => LargestTicks / TimeSpan.TicksPerMillisecond;
    }
}
