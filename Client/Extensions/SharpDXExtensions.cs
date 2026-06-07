using Vortice.Direct3D9;
using Vortice.Mathematics;
using System;
using System.Drawing;
using System.Linq;
using System.Numerics;
using Color = System.Drawing.Color;
using ColorBGRA = Vortice.Mathematics.Color;
using Device9 = Vortice.Direct3D9.IDirect3DDevice9;
using RawRect = Vortice.Mathematics.RectI;
using Texture = Vortice.Direct3D9.IDirect3DTexture9;

namespace Client.Extensions;

public static class SharpDXExtensions
{
    public static void Draw(this Sprite sprite, Texture texture, Rectangle? sourceRectangle, Vector3? center, Vector3? position, Color color)
    {
        sprite.DrawInternal(texture, sourceRectangle, center, position, color.ToColorBGRA());
    }

    public static void Draw(this Sprite sprite, Texture texture, Rectangle? sourceRectangle, Vector3? center, Vector3? position, Color4 color)
    {
        sprite.DrawInternal(texture, sourceRectangle, center, position, color.ToColorBGRA());
    }

    public static void Draw(this Sprite sprite, Texture texture, Vector3? center, Vector3? position, Color color)
    {
        sprite.Draw(texture, null, center, position, color);
    }

    public static void Draw(this Sprite sprite, Texture texture, Vector3? center, Vector3? position, Color4 color)
    {
        sprite.Draw(texture, null, center, position, color);
    }

    public static void Draw(this Sprite sprite, Texture texture, Color color)
    {
        sprite.Draw(texture, null, null, null, color);
    }

    public static void Draw(this Sprite sprite, Texture texture, Color4 color)
    {
        sprite.Draw(texture, null, null, null, color);
    }

    public static void Draw(this Line line, Vector2[] vertexList, Color color)
    {
        line.DrawInternal(vertexList, color.ToColorBGRA());
    }

    public static void Draw(this Line line, Vector2[] vertexList, Color4 color)
    {
        line.DrawInternal(vertexList, color.ToColorBGRA());
    }

    public static void Clear(this Device9 device, ClearFlags flags, Color color, float z, int stencil)
    {
        ArgumentNullException.ThrowIfNull(device);

        device.Clear(flags, color.ToColorBGRA(), z, stencil);
    }

    public static void Clear(this Device9 device, ClearFlags flags, int color, float z, int stencil)
    {
        ArgumentNullException.ThrowIfNull(device);

        device.Clear(flags, Color.FromArgb(color).ToColorBGRA(), z, stencil);
    }

    public static void Clear(this Device9 device, ClearFlags flags, Color color, float z, int stencil, Rectangle[] rectangles)
    {
        ArgumentNullException.ThrowIfNull(device);

        RawRect[] rawRectangles = rectangles?.Select(rectangle => new RawRect(rectangle)).ToArray();

        device.Clear(flags, color.ToColorBGRA(), z, stencil, rawRectangles);
    }

    private static void DrawInternal(this Sprite sprite, Texture texture, Rectangle? sourceRectangle, Vector3? center, Vector3? position, ColorBGRA color)
    {
        ArgumentNullException.ThrowIfNull(sprite);
        ArgumentNullException.ThrowIfNull(texture);

        RawRect? rawRectangle = sourceRectangle.HasValue ? ToVorticeRect(sourceRectangle.Value) : null;
        Vector3? rawCenter = center;
        Vector3? rawPosition = position;

        sprite.Draw(texture, color, rawRectangle, rawCenter, rawPosition);
    }

    private static void DrawInternal(this Line line, Vector2[] vertexList, ColorBGRA color)
    {
        ArgumentNullException.ThrowIfNull(line);
        ArgumentNullException.ThrowIfNull(vertexList);

        line.Draw(vertexList, color);
    }

    private static RawRect ToVorticeRect(Rectangle rectangle) => new(rectangle);
}

public static class SharpDXColorExtensions
{
    public static Color4 ToColor4(this Color color)
    {
        return new Color4(
            color.R / 255f,
            color.G / 255f,
            color.B / 255f,
            color.A / 255f);
    }

    public static Color ToColor(this Color4 color)
    {
        return Color.FromArgb(
            ToByte(color.A),
            ToByte(color.R),
            ToByte(color.G),
            ToByte(color.B));
    }

    public static ColorBGRA ToColorBGRA(this Color color)
    {
        return new ColorBGRA(color.R, color.G, color.B, color.A);
    }

    public static ColorBGRA ToColorBGRA(this Color4 color)
    {
        return new ColorBGRA(
            ToByte(color.R),
            ToByte(color.G),
            ToByte(color.B),
            ToByte(color.A));
    }

    private static byte ToByte(float value)
    {
        if (value <= 0f) return 0;
        if (value >= 1f) return 255;

        return (byte)Math.Round(value * 255f);
    }
}
