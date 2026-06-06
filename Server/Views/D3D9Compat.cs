#pragma warning disable CA1416
using System;
using System.Numerics;
using Vortice.Mathematics;

namespace Vortice.Direct3D9;

[Flags]
public enum SpriteFlags
{
    None = 0,
    DoNotSave = 0x1,
    DoNotSaveState = 0x1,
    SortTexture = 0x2,
    SortDepthFrontToBack = 0x4,
    SortDepthBackToFront = 0x8,
    DoNotAddRefTexture = 0x10,
    ObjectSpace = 0x20,
    Billboard = 0x40,
    AlphaBlend = 0x80,
}

public sealed class Sprite : IDisposable
{
    private bool _disposed;

    public bool IsDisposed => _disposed;
    public Matrix4x4 Transform { get; set; } = Matrix4x4.Identity;

    public Sprite(IDirect3DDevice9 device) { }

    public void Begin(SpriteFlags flags) { }
    public void End() { }
    public void Flush() { }
    public void Draw(IDirect3DTexture9 texture, ColorBGRA color, RawRect? sourceRect, Vector3? center, Vector3? translation) { }

    public void Dispose() { _disposed = true; }
}

public sealed class Line : IDisposable
{
    private bool _disposed;

    public bool IsDisposed => _disposed;
    public float Width { get; set; }

    public Line(IDirect3DDevice9 device) { }

    public void Draw(Vector2[] vertices, ColorBGRA color) { }

    public void Dispose() { _disposed = true; }
}
