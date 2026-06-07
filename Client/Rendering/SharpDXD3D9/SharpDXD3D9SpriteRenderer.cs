using Vortice.D3DCompiler;
using Vortice.Direct3D9;
using Vortice.Mathematics;
using System;
using System.IO;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Color4 = Vortice.Mathematics.Color4;
using ColorBGRA = Vortice.Mathematics.ColorBgra;
using Device9 = Vortice.Direct3D9.IDirect3DDevice9;
using Matrix4x4 = System.Numerics.Matrix4x4;
using NumericsMatrix3x2 = System.Numerics.Matrix3x2;
using PixelShader = Vortice.Direct3D9.IDirect3DPixelShader9;
using StateBlock = Vortice.Direct3D9.IDirect3DStateBlock9;
using Texture = Vortice.Direct3D9.IDirect3DTexture9;
using Vector2 = System.Numerics.Vector2;
using VertexBuffer = Vortice.Direct3D9.IDirect3DVertexBuffer9;
using VertexDeclaration = Vortice.Direct3D9.IDirect3DVertexDeclaration9;
using VertexShader = Vortice.Direct3D9.IDirect3DVertexShader9;

namespace Client.Rendering.SharpDXD3D9
{
    public sealed class SharpDXD3D9SpriteRenderer : IDisposable
    {
        private readonly Device9 _device;

        private VertexShader _vertexShader;
        private VertexShader _shadowVertexShader;
        private PixelShader _outlinePixelShader;
        private PixelShader _grayscalePixelShader;
        private PixelShader _dropShadowPixelShader;
        private VertexBuffer _vertexBuffer;
        private VertexDeclaration _vertexDeclaration;

        private const string OutlineShaderFileName = "OutlineD3D9.hlsl";
        private const string GrayscaleShaderFileName = "GrayscaleD3D9.hlsl";
        private const string DropShadowShaderFileName = "DropShadowD3D9.hlsl";

        [StructLayout(LayoutKind.Sequential)]
        private struct VertexType
        {
            public Vector2 Position;
            public Vector2 TexCoord;
            public ColorBGRA Color;
        }

        public bool SupportsOutlineShader => _outlinePixelShader != null && _vertexShader != null;
        public bool SupportsGrayscaleShader => _grayscalePixelShader != null && _vertexShader != null;
        public bool SupportsDropShadowShader => _dropShadowPixelShader != null && _shadowVertexShader != null;

        public SharpDXD3D9SpriteRenderer(Device9 device)
        {
            _device = device ?? throw new ArgumentNullException(nameof(device));

            InitializeShaders();
            InitializeBuffers();
        }

        private void InitializeShaders()
        {
            InitializeOutlineShader();
            InitializeGrayscaleShader();
            InitializeDropShadowShader();
        }

        private unsafe void InitializeOutlineShader()
        {
            string shaderPath = FindShaderPath(OutlineShaderFileName);

            if (string.IsNullOrEmpty(shaderPath) || !File.Exists(shaderPath))
                return;

            Compiler.CompileFromFile(shaderPath, null, null, "VS", "vs_3_0", ShaderFlags.OptimizationLevel3, EffectFlags.None, out Blob? vsBlob, out Blob? vsErrors);
            vsErrors?.Dispose();
            if (vsBlob != null)
            {
                var vsSpan = new ReadOnlySpan<byte>((void*)vsBlob.BufferPointer, (int)(ulong)vsBlob.BufferSize);
                _vertexShader = _device.CreateVertexShader(MemoryMarshal.Cast<byte, uint>(vsSpan));
                vsBlob.Dispose();
            }

            Compiler.CompileFromFile(shaderPath, null, null, "PS_OUTLINE", "ps_3_0", ShaderFlags.OptimizationLevel3, EffectFlags.None, out Blob? psBlob, out Blob? psErrors);
            psErrors?.Dispose();
            if (psBlob != null)
            {
                var psSpan = new ReadOnlySpan<byte>((void*)psBlob.BufferPointer, (int)(ulong)psBlob.BufferSize);
                _outlinePixelShader = _device.CreatePixelShader(MemoryMarshal.Cast<byte, uint>(psSpan));
                psBlob.Dispose();
            }
        }

        private unsafe void InitializeGrayscaleShader()
        {
            string shaderPath = FindShaderPath(GrayscaleShaderFileName);

            if (string.IsNullOrEmpty(shaderPath) || !File.Exists(shaderPath))
                return;

            Compiler.CompileFromFile(shaderPath, null, null, "PS_GRAY", "ps_3_0", ShaderFlags.OptimizationLevel3, EffectFlags.None, out Blob? psBlob, out Blob? psErrors);
            psErrors?.Dispose();
            if (psBlob != null)
            {
                var psSpan = new ReadOnlySpan<byte>((void*)psBlob.BufferPointer, (int)(ulong)psBlob.BufferSize);
                _grayscalePixelShader = _device.CreatePixelShader(MemoryMarshal.Cast<byte, uint>(psSpan));
                psBlob.Dispose();
            }
        }

        private unsafe void InitializeDropShadowShader()
        {
            string shaderPath = FindShaderPath(DropShadowShaderFileName);

            if (string.IsNullOrEmpty(shaderPath) || !File.Exists(shaderPath))
                return;

            Compiler.CompileFromFile(shaderPath, null, null, "VS", "vs_3_0", ShaderFlags.OptimizationLevel3, EffectFlags.None, out Blob? vsBlob, out Blob? vsErrors);
            vsErrors?.Dispose();
            if (vsBlob != null)
            {
                var vsSpan = new ReadOnlySpan<byte>((void*)vsBlob.BufferPointer, (int)(ulong)vsBlob.BufferSize);
                _shadowVertexShader = _device.CreateVertexShader(MemoryMarshal.Cast<byte, uint>(vsSpan));
                vsBlob.Dispose();
            }

            Compiler.CompileFromFile(shaderPath, null, null, "PS_SHADOW", "ps_3_0", ShaderFlags.OptimizationLevel3, EffectFlags.None, out Blob? psBlob, out Blob? psErrors);
            psErrors?.Dispose();
            if (psBlob != null)
            {
                var psSpan = new ReadOnlySpan<byte>((void*)psBlob.BufferPointer, (int)(ulong)psBlob.BufferSize);
                _dropShadowPixelShader = _device.CreatePixelShader(MemoryMarshal.Cast<byte, uint>(psSpan));
                psBlob.Dispose();
            }
        }

        private static string FindShaderPath(string filename)
        {
            string baseDirectory = AppDomain.CurrentDomain.BaseDirectory ?? string.Empty;

            string[] candidates = new[]
            {
                Path.Combine(baseDirectory, "Rendering", "SharpDXD3D9", "Shaders", filename)
            };

            foreach (string candidate in candidates)
            {
                if (File.Exists(candidate))
                    return candidate;
            }

            return null;
        }

        private void InitializeBuffers()
        {
            _vertexBuffer = _device.CreateVertexBuffer((uint)(Marshal.SizeOf<VertexType>() * 4), Usage.WriteOnly | Usage.Dynamic, VertexFormat.None, Pool.Default);

            _vertexDeclaration = _device.CreateVertexDeclaration(new[]
            {
                new VertexElement(0, 0, DeclarationType.Float2, DeclarationMethod.Default, DeclarationUsage.Position, 0),
                new VertexElement(0, 8, DeclarationType.Float2, DeclarationMethod.Default, DeclarationUsage.TextureCoordinate, 0),
                new VertexElement(0, 16, DeclarationType.Color, DeclarationMethod.Default, DeclarationUsage.Color, 0),
                VertexElement.VertexDeclarationEnd
            });
        }

        public void DrawOutlined(Texture texture, System.Drawing.RectangleF destination, System.Drawing.Rectangle? source, System.Drawing.Color color, NumericsMatrix3x2 transform, Color4 outlineColor, float outlineThickness)
        {
            if (!SupportsOutlineShader || texture == null || texture.NativePointer == IntPtr.Zero)
                return;

            float effectiveThickness = outlineThickness > 0 ? 1.0f : 0.0f;

            using var stateBlock = _device.CreateStateBlock(StateBlockType.All);
            stateBlock.Capture();

            var desc = texture.GetLevelDescription(0);

            float left = destination.Left;
            float right = destination.Right;
            float top = destination.Top;
            float bottom = destination.Bottom;

            float u1 = 0, v1 = 0, u2 = 1, v2 = 1;
            if (source.HasValue)
            {
                u1 = source.Value.Left / (float)desc.Width;
                v1 = source.Value.Top / (float)desc.Height;
                u2 = source.Value.Right / (float)desc.Width;
                v2 = source.Value.Bottom / (float)desc.Height;
            }

            if (effectiveThickness > 0)
            {
                left -= effectiveThickness;
                right += effectiveThickness;
                top -= effectiveThickness;
                bottom += effectiveThickness;

                float uPad = effectiveThickness / desc.Width;
                float vPad = effectiveThickness / desc.Height;
                u1 -= uPad;
                v1 -= vPad;
                u2 += uPad;
                v2 += vPad;
            }

            var vertexColor = new ColorBGRA(color.R, color.G, color.B, color.A);

            var viewport = _device.Viewport;

            UpdateMatrix(transform, viewport.Width, viewport.Height);
            UpdateOutlineConstants(desc.Width, desc.Height, outlineColor, effectiveThickness, u1, v1, u2, v2);

            unsafe
            {
                nint dataPtr = _vertexBuffer.Lock(0, 0, LockFlags.Discard);
                VertexType* ptr = (VertexType*)dataPtr;
                ptr[0] = new VertexType { Position = new Vector2(left, top), TexCoord = new Vector2(u1, v1), Color = vertexColor };
                ptr[1] = new VertexType { Position = new Vector2(right, top), TexCoord = new Vector2(u2, v1), Color = vertexColor };
                ptr[2] = new VertexType { Position = new Vector2(left, bottom), TexCoord = new Vector2(u1, v2), Color = vertexColor };
                ptr[3] = new VertexType { Position = new Vector2(right, bottom), TexCoord = new Vector2(u2, v2), Color = vertexColor };
                _vertexBuffer.Unlock();
            }

            _device.SetRenderState(RenderState.AlphaBlendEnable, true);
            _device.SetRenderState(RenderState.SourceBlend, Blend.SourceAlpha);
            _device.SetRenderState(RenderState.DestinationBlend, Blend.InverseSourceAlpha);
            _device.SetRenderState(RenderState.CullMode, Cull.None);

            _device.SetTexture(0, texture);
            _device.SetSamplerState(0, SamplerState.AddressU, (int)TextureAddress.Clamp);
            _device.SetSamplerState(0, SamplerState.AddressV, (int)TextureAddress.Clamp);
            _device.SetSamplerState(0, SamplerState.MinFilter, (int)TextureFilter.Point);
            _device.SetSamplerState(0, SamplerState.MagFilter, (int)TextureFilter.Point);
            _device.SetSamplerState(0, SamplerState.MipFilter, (int)TextureFilter.Point);

            _device.VertexDeclaration = _vertexDeclaration;
            _device.SetStreamSource(0, _vertexBuffer, 0, Marshal.SizeOf<VertexType>());
            _device.VertexShader = _vertexShader;
            _device.PixelShader = _outlinePixelShader;

            _device.DrawPrimitives(PrimitiveType.TriangleStrip, 0, 2);

            stateBlock.Apply();
        }

        public void DrawGrayscale(Texture texture, System.Drawing.RectangleF destination, System.Drawing.Rectangle? source, System.Drawing.Color color, NumericsMatrix3x2 transform)
        {
            if (!SupportsGrayscaleShader || texture == null || texture.NativePointer == IntPtr.Zero)
                return;

            using var stateBlock = _device.CreateStateBlock(StateBlockType.All);
            stateBlock.Capture();

            var desc = texture.GetLevelDescription(0);

            float left = destination.Left;
            float right = destination.Right;
            float top = destination.Top;
            float bottom = destination.Bottom;

            float u1 = 0, v1 = 0, u2 = 1, v2 = 1;
            if (source.HasValue)
            {
                u1 = source.Value.Left / (float)desc.Width;
                v1 = source.Value.Top / (float)desc.Height;
                u2 = source.Value.Right / (float)desc.Width;
                v2 = source.Value.Bottom / (float)desc.Height;
            }

            var vertexColor = new ColorBGRA(color.R, color.G, color.B, color.A);

            var viewport = _device.Viewport;

            UpdateMatrix(transform, viewport.Width, viewport.Height);

            unsafe
            {
                nint dataPtr = _vertexBuffer.Lock(0, 0, LockFlags.Discard);
                VertexType* ptr = (VertexType*)dataPtr;
                ptr[0] = new VertexType { Position = new Vector2(left, top), TexCoord = new Vector2(u1, v1), Color = vertexColor };
                ptr[1] = new VertexType { Position = new Vector2(right, top), TexCoord = new Vector2(u2, v1), Color = vertexColor };
                ptr[2] = new VertexType { Position = new Vector2(left, bottom), TexCoord = new Vector2(u1, v2), Color = vertexColor };
                ptr[3] = new VertexType { Position = new Vector2(right, bottom), TexCoord = new Vector2(u2, v2), Color = vertexColor };
                _vertexBuffer.Unlock();
            }

            _device.SetRenderState(RenderState.AlphaBlendEnable, SharpDXD3D9Manager.Blending);
            _device.SetRenderState(RenderState.SourceBlend, Blend.InverseDestinationColor);
            _device.SetRenderState(RenderState.DestinationBlend, Blend.One);
            _device.SetRenderState(RenderState.CullMode, Cull.None);

            _device.SetTexture(0, texture);
            _device.SetSamplerState(0, SamplerState.AddressU, (int)TextureAddress.Clamp);
            _device.SetSamplerState(0, SamplerState.AddressV, (int)TextureAddress.Clamp);
            _device.SetSamplerState(0, SamplerState.MinFilter, (int)TextureFilter.Point);
            _device.SetSamplerState(0, SamplerState.MagFilter, (int)TextureFilter.Point);
            _device.SetSamplerState(0, SamplerState.MipFilter, (int)TextureFilter.Point);

            _device.VertexDeclaration = _vertexDeclaration;
            _device.SetStreamSource(0, _vertexBuffer, 0, Marshal.SizeOf<VertexType>());
            _device.VertexShader = _vertexShader;
            _device.PixelShader = _grayscalePixelShader;

            _device.DrawPrimitives(PrimitiveType.TriangleStrip, 0, 2);

            stateBlock.Apply();
        }

        public void DrawDropShadow(Texture texture, System.Drawing.RectangleF destination, System.Drawing.RectangleF shadowBounds, System.Drawing.Rectangle? source, System.Drawing.Color color, NumericsMatrix3x2 transform, Color4 shadowColor, float shadowWidth, float shadowMaxOpacity)
        {
            if (!SupportsDropShadowShader || texture == null || texture.NativePointer == IntPtr.Zero)
                return;

            using var stateBlock = _device.CreateStateBlock(StateBlockType.All);
            stateBlock.Capture();

            var desc = texture.GetLevelDescription(0);

            float imageLeft = shadowBounds.Left;
            float imageRight = shadowBounds.Right;
            float imageTop = shadowBounds.Top;
            float imageBottom = shadowBounds.Bottom;

            float left = destination.Left;
            float right = destination.Right;
            float top = destination.Top;
            float bottom = destination.Bottom;

            float u1 = 0, v1 = 0, u2 = 1, v2 = 1;
            if (source.HasValue)
            {
                u1 = source.Value.Left / (float)desc.Width;
                v1 = source.Value.Top / (float)desc.Height;
                u2 = source.Value.Right / (float)desc.Width;
                v2 = source.Value.Bottom / (float)desc.Height;
            }

            float effectiveWidth = Math.Max(0f, shadowWidth);

            if (effectiveWidth > 0)
            {
                left -= effectiveWidth;
                right += effectiveWidth;
                top -= effectiveWidth;
                bottom += effectiveWidth;

                float uPad = effectiveWidth / desc.Width;
                float vPad = effectiveWidth / desc.Height;
                u1 -= uPad;
                v1 -= vPad;
                u2 += uPad;
                v2 += vPad;
            }

            var vertexColor = new ColorBGRA(color.R, color.G, color.B, color.A);

            var viewport = _device.Viewport;

            UpdateMatrix(transform, viewport.Width, viewport.Height);
            UpdateShadowConstants(imageLeft, imageTop, imageRight, imageBottom, effectiveWidth, shadowMaxOpacity);

            unsafe
            {
                nint dataPtr = _vertexBuffer.Lock(0, 0, LockFlags.Discard);
                VertexType* ptr = (VertexType*)dataPtr;
                ptr[0] = new VertexType { Position = new Vector2(left, top), TexCoord = new Vector2(u1, v1), Color = vertexColor };
                ptr[1] = new VertexType { Position = new Vector2(right, top), TexCoord = new Vector2(u2, v1), Color = vertexColor };
                ptr[2] = new VertexType { Position = new Vector2(left, bottom), TexCoord = new Vector2(u1, v2), Color = vertexColor };
                ptr[3] = new VertexType { Position = new Vector2(right, bottom), TexCoord = new Vector2(u2, v2), Color = vertexColor };
                _vertexBuffer.Unlock();
            }

            _device.SetRenderState(RenderState.AlphaBlendEnable, true);
            _device.SetRenderState(RenderState.SourceBlend, Blend.SourceAlpha);
            _device.SetRenderState(RenderState.DestinationBlend, Blend.InverseSourceAlpha);
            _device.SetRenderState(RenderState.CullMode, Cull.None);

            _device.SetTexture(0, texture);
            _device.SetSamplerState(0, SamplerState.AddressU, (int)TextureAddress.Clamp);
            _device.SetSamplerState(0, SamplerState.AddressV, (int)TextureAddress.Clamp);
            _device.SetSamplerState(0, SamplerState.MinFilter, (int)TextureFilter.Point);
            _device.SetSamplerState(0, SamplerState.MagFilter, (int)TextureFilter.Point);
            _device.SetSamplerState(0, SamplerState.MipFilter, (int)TextureFilter.Point);

            _device.VertexDeclaration = _vertexDeclaration;
            _device.SetStreamSource(0, _vertexBuffer, 0, Marshal.SizeOf<VertexType>());
            _device.VertexShader = _shadowVertexShader;
            _device.PixelShader = _dropShadowPixelShader;

            _device.DrawPrimitives(PrimitiveType.TriangleStrip, 0, 2);

            stateBlock.Apply();
        }

        private void UpdateMatrix(NumericsMatrix3x2 transform, int backBufferWidth, int backBufferHeight)
        {
            Matrix4x4 projection = Matrix4x4.Identity;
            projection.M11 = 2f / backBufferWidth;
            projection.M22 = -2f / backBufferHeight;

            // Half-pixel offset to align texels to pixel centers in D3D9 point sampling.
            float halfPixelX = 1f / backBufferWidth;
            float halfPixelY = 1f / backBufferHeight;
            projection.M41 = -1f - halfPixelX;
            projection.M42 = 1f + halfPixelY;

            Matrix4x4 world = Matrix4x4.Identity;
            world.M11 = transform.M11;
            world.M12 = transform.M12;
            world.M21 = transform.M21;
            world.M22 = transform.M22;
            world.M41 = transform.M31;
            world.M42 = transform.M32;

            Matrix4x4 final = Matrix4x4.Transpose(world * projection);

            ReadOnlySpan<float> matFloats = MemoryMarshal.Cast<Matrix4x4, float>(MemoryMarshal.CreateSpan(ref final, 1));
            _device.SetVertexShaderConstantF(0, matFloats, 4);
        }

        private void UpdateOutlineConstants(int texWidth, int texHeight, Color4 outlineColor, float outlineThickness, float u1, float v1, float u2, float v2)
        {
            _device.SetPixelShaderConstantF(4, new[] { outlineColor.R, outlineColor.G, outlineColor.B, outlineColor.A }, 1);
            _device.SetPixelShaderConstantF(5, new float[] { texWidth, texHeight, outlineThickness, 0f }, 1);
            _device.SetPixelShaderConstantF(6, new[] { u1, v1, u2, v2 }, 1);
        }

        private void UpdateShadowConstants(float imageLeft, float imageTop, float imageRight, float imageBottom, float shadowWidth, float shadowMaxOpacity)
        {
            _device.SetPixelShaderConstantF(4, new[] { imageLeft, imageTop, imageRight, imageBottom }, 1);
            _device.SetPixelShaderConstantF(5, new[] { shadowWidth, shadowMaxOpacity, 0f, 0f }, 1);
        }

        public void Dispose()
        {
            _vertexBuffer?.Dispose();
            _vertexDeclaration?.Dispose();
            _vertexShader?.Dispose();
            _outlinePixelShader?.Dispose();
            _grayscalePixelShader?.Dispose();
            _dropShadowPixelShader?.Dispose();
            _shadowVertexShader?.Dispose();
        }
    }
}
