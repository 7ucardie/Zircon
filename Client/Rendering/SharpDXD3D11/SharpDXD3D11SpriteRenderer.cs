using Vortice.D3DCompiler;
using Vortice.Direct3D;
using Vortice.Direct3D11;
using Vortice.DXGI;
using Vortice.Mathematics;
using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Buffer = Vortice.Direct3D11.ID3D11Buffer;
using Color = System.Drawing.Color;
using Color4 = Vortice.Mathematics.Color4;
using DepthStencilView = Vortice.Direct3D11.ID3D11DepthStencilView;
using Device = Vortice.Direct3D11.ID3D11Device;
using DeviceContext = Vortice.Direct3D11.ID3D11DeviceContext;
using MapFlags = Vortice.Direct3D11.MapFlags;
using Matrix3x2 = System.Numerics.Matrix3x2;
using Matrix4x4 = System.Numerics.Matrix4x4;
using RectangleF = System.Drawing.RectangleF;
using RenderTargetView = Vortice.Direct3D11.ID3D11RenderTargetView;
using Vector2 = System.Numerics.Vector2;
using Vector4 = System.Numerics.Vector4;
using VertexShader = Vortice.Direct3D11.ID3D11VertexShader;
using PixelShader = Vortice.Direct3D11.ID3D11PixelShader;
using InputLayout = Vortice.Direct3D11.ID3D11InputLayout;
using BlendOption = Vortice.Direct3D11.Blend;
using BlendState = Vortice.Direct3D11.ID3D11BlendState;
using SamplerState = Vortice.Direct3D11.ID3D11SamplerState;
using Texture2D = Vortice.Direct3D11.ID3D11Texture2D;
using ShaderResourceView = Vortice.Direct3D11.ID3D11ShaderResourceView;

namespace Client.Rendering.SharpDXD3D11
{
    public sealed class SharpDXD3D11SpriteRenderer : IDisposable
    {
        private readonly Device _device;
        private readonly DeviceContext _context;
        private VertexShader _vertexShader;
        private PixelShader _pixelShader;
        private PixelShader _grayscalePixelShader;
        private PixelShader _outlinePixelShader;
        private PixelShader _dropShadowPixelShader;
        private InputLayout _inputLayout;
        private Buffer _vertexBuffer;
        private Buffer _matrixBuffer;
        private Buffer _outlineBuffer;
        private Buffer _dropShadowBuffer;
        private SamplerState _samplerState;

        private readonly Dictionary<BlendMode, BlendState> _blendStates = new Dictionary<BlendMode, BlendState>();
        private readonly Dictionary<Texture2D, ShaderResourceView> _srvCache = new Dictionary<Texture2D, ShaderResourceView>();

        private const string ShaderFileName = "SpriteD3D11.hlsl";
        private const string OutlineShaderFileName = "OutlineD3D11.hlsl";
        private const string GrayscaleFileName = "GrayscaleD3D11.hlsl";
        private const string DropShadowFileName = "DropShadowD3D11.hlsl";

        [StructLayout(LayoutKind.Sequential)]
        private struct VertexType
        {
            public Vector2 position;
            public Vector2 texture;
            public Color4 color;

            public VertexType(Vector2 pos, Vector2 tex, Color4 col)
            {
                position = pos;
                texture = tex;
                color = col;
            }
        }

        private readonly struct SpriteEffect
        {
            public PixelShader Shader { get; }
            public Buffer ConstantBuffer { get; }
            public int ConstantBufferSizeInBytes { get; }
            public float GeometryExpand { get; }
            public bool ExpandUvs { get; }
            public Action<nint> WriteConstants { get; }

            public bool IsValid => Shader != null;

            public SpriteEffect(PixelShader shader, Buffer constantBuffer, int constantBufferSizeInBytes, float geometryExpand, bool expandUvs, Action<nint> writeConstants)
            {
                Shader = shader;
                ConstantBuffer = constantBuffer;
                ConstantBufferSizeInBytes = constantBufferSizeInBytes;
                GeometryExpand = geometryExpand;
                ExpandUvs = expandUvs;
                WriteConstants = writeConstants;
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct OutlineBufferType
        {
            public Color4 OutlineColor;
            public Vector2 TextureSize;
            public float OutlineThickness;
            public float Padding;
            public Vector4 SourceUV;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct DropShadowBufferType
        {
            public Vector2 ImgMin;
            public Vector2 ImgMax;
            public float ShadowSize;
            public float MaxAlpha;
            public Vector2 Padding;
        }

        public SharpDXD3D11SpriteRenderer(Device device)
        {
            _device = device ?? throw new ArgumentNullException(nameof(device));
            _context = _device.ImmediateContext;

            InitializeShaders();
            InitializeBuffers();
            InitializeBlendStates();
            InitializeSampler();
        }

        private void InitializeShaders()
        {
            InitializeShader();
            InitializeOutlineShader();
            InitializeGrayscaleShader();
            InitializeDropShadowShader();
        }

        private unsafe void InitializeShader()
        {
            string shaderPath = FindShaderPath(ShaderFileName);

            if (string.IsNullOrEmpty(shaderPath) || !File.Exists(shaderPath))
                return;

            Compiler.CompileFromFile(shaderPath, null, null, "VS", "vs_5_0", ShaderFlags.None, EffectFlags.None, out Blob? vsBytecode, out Blob? vsErrors);
            vsErrors?.Dispose();
            if (vsBytecode != null)
            {
                var vsBytes = new ReadOnlySpan<byte>((void*)vsBytecode.BufferPointer, (int)(ulong)vsBytecode.BufferSize);
                _vertexShader = _device.CreateVertexShader(vsBytes);
                _inputLayout = _device.CreateInputLayout(new[]
                {
                    new InputElementDescription("POSITION", 0, Format.R32G32_Float, 0, 0),
                    new InputElementDescription("TEXCOORD", 0, Format.R32G32_Float, 8, 0),
                    new InputElementDescription("COLOR", 0, Format.R32G32B32A32_Float, 16, 0)
                }, vsBytes);
                vsBytecode.Dispose();
            }

            Compiler.CompileFromFile(shaderPath, null, null, "PS", "ps_5_0", ShaderFlags.None, EffectFlags.None, out Blob? psBytecode, out Blob? psErrors);
            psErrors?.Dispose();
            if (psBytecode != null)
            {
                var psBytes = new ReadOnlySpan<byte>((void*)psBytecode.BufferPointer, (int)(ulong)psBytecode.BufferSize);
                _pixelShader = _device.CreatePixelShader(psBytes);
                psBytecode.Dispose();
            }
        }

        private unsafe void InitializeGrayscaleShader()
        {
            string shaderPath = FindShaderPath(GrayscaleFileName);

            if (string.IsNullOrEmpty(shaderPath) || !File.Exists(shaderPath))
                return;

            Compiler.CompileFromFile(shaderPath, null, null, "PS_GRAY", "ps_5_0", ShaderFlags.None, EffectFlags.None, out Blob? bytecode, out Blob? errors);
            errors?.Dispose();
            if (bytecode != null)
            {
                var bytes = new ReadOnlySpan<byte>((void*)bytecode.BufferPointer, (int)(ulong)bytecode.BufferSize);
                _grayscalePixelShader = _device.CreatePixelShader(bytes);
                bytecode.Dispose();
            }
        }

        private unsafe void InitializeOutlineShader()
        {
            string shaderPath = FindShaderPath(OutlineShaderFileName);

            if (string.IsNullOrEmpty(shaderPath) || !File.Exists(shaderPath))
                return;

            Compiler.CompileFromFile(shaderPath, null, null, "PS_OUTLINE", "ps_5_0", ShaderFlags.None, EffectFlags.None, out Blob? bytecode, out Blob? errors);
            errors?.Dispose();
            if (bytecode != null)
            {
                var bytes = new ReadOnlySpan<byte>((void*)bytecode.BufferPointer, (int)(ulong)bytecode.BufferSize);
                _outlinePixelShader = _device.CreatePixelShader(bytes);
                bytecode.Dispose();
            }

            _outlineBuffer = _device.CreateBuffer(new BufferDescription
            {
                Usage = ResourceUsage.Dynamic,
                SizeInBytes = Unsafe.SizeOf<OutlineBufferType>(),
                BindFlags = BindFlags.ConstantBuffer,
                CpuAccessFlags = CpuAccessFlags.Write,
                OptionFlags = ResourceOptionFlags.None,
                StructureByteStride = 0
            });
        }

        private unsafe void InitializeDropShadowShader()
        {
            string shaderPath = FindShaderPath(DropShadowFileName);

            if (string.IsNullOrEmpty(shaderPath) || !File.Exists(shaderPath))
                return;

            Compiler.CompileFromFile(shaderPath, null, null, "PS_SHADOW", "ps_5_0", ShaderFlags.None, EffectFlags.None, out Blob? bytecode, out Blob? errors);
            errors?.Dispose();
            if (bytecode != null)
            {
                var bytes = new ReadOnlySpan<byte>((void*)bytecode.BufferPointer, (int)(ulong)bytecode.BufferSize);
                _dropShadowPixelShader = _device.CreatePixelShader(bytes);
                bytecode.Dispose();
            }

            _dropShadowBuffer = _device.CreateBuffer(new BufferDescription
            {
                Usage = ResourceUsage.Dynamic,
                SizeInBytes = Unsafe.SizeOf<DropShadowBufferType>(),
                BindFlags = BindFlags.ConstantBuffer,
                CpuAccessFlags = CpuAccessFlags.Write,
                OptionFlags = ResourceOptionFlags.None,
                StructureByteStride = 0
            });
        }

        private static string FindShaderPath(string filename)
        {
            string baseDirectory = AppDomain.CurrentDomain.BaseDirectory ?? string.Empty;

            string[] candidates = new[]
            {
                Path.Combine(baseDirectory, "Rendering", "SharpDXD3D11", "Shaders", filename)
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
            _vertexBuffer = _device.CreateBuffer(new BufferDescription
            {
                Usage = ResourceUsage.Dynamic,
                SizeInBytes = Unsafe.SizeOf<VertexType>() * 4,
                BindFlags = BindFlags.VertexBuffer,
                CpuAccessFlags = CpuAccessFlags.Write,
                OptionFlags = ResourceOptionFlags.None,
                StructureByteStride = 0
            });

            _matrixBuffer = _device.CreateBuffer(new BufferDescription
            {
                Usage = ResourceUsage.Dynamic,
                SizeInBytes = Unsafe.SizeOf<Matrix4x4>(),
                BindFlags = BindFlags.ConstantBuffer,
                CpuAccessFlags = CpuAccessFlags.Write,
                OptionFlags = ResourceOptionFlags.None,
                StructureByteStride = 0
            });
        }

        private void InitializeSampler()
        {
            _samplerState = _device.CreateSamplerState(new SamplerStateDescription
            {
                Filter = Filter.MinMagMipLinear,
                AddressU = TextureAddressMode.Clamp,
                AddressV = TextureAddressMode.Clamp,
                AddressW = TextureAddressMode.Clamp,
                ComparisonFunction = ComparisonFunction.Never,
                MinLod = 0,
                MaxLod = float.MaxValue
            });
        }

        private void InitializeBlendStates()
        {
            // Replicate DX9 default behavior (Screen Blend for both Color and Alpha) for falling-through modes
            // DX9: SourceBlend = InverseDestinationColor, DestinationBlend = One
            // Applied to both Color and Alpha channels.

            CreateBlendState(BlendMode.NORMAL, BlendOption.InverseDestinationColor, BlendOption.One, BlendOption.InverseDestinationAlpha, BlendOption.One);
            CreateBlendState(BlendMode.LIGHT, BlendOption.InverseDestinationColor, BlendOption.One, BlendOption.InverseDestinationAlpha, BlendOption.One);
            CreateBlendState(BlendMode.LIGHTINV, BlendOption.InverseDestinationColor, BlendOption.One, BlendOption.InverseDestinationAlpha, BlendOption.One);
            CreateBlendState(BlendMode.INVNORMAL, BlendOption.InverseDestinationColor, BlendOption.One, BlendOption.InverseDestinationAlpha, BlendOption.One);
            CreateBlendState(BlendMode.INVLIGHTINV, BlendOption.InverseDestinationColor, BlendOption.One, BlendOption.InverseDestinationAlpha, BlendOption.One);
            CreateBlendState(BlendMode.INVCOLOR, BlendOption.InverseDestinationColor, BlendOption.One, BlendOption.InverseDestinationAlpha, BlendOption.One);
            CreateBlendState(BlendMode.INVBACKGROUND, BlendOption.InverseDestinationColor, BlendOption.One, BlendOption.InverseDestinationAlpha, BlendOption.One);

            // INVLIGHT: Source = BlendFactor, Destination = InverseSourceColor
            // Alpha: BlendFactor, InverseSourceAlpha
            CreateBlendState(BlendMode.INVLIGHT, BlendOption.BlendFactor, BlendOption.InverseSourceColor, BlendOption.BlendFactor, BlendOption.InverseSourceAlpha);

            // COLORFY: Source = SourceAlpha, Destination = One
            // Alpha: SourceAlpha, One
            CreateBlendState(BlendMode.COLORFY, BlendOption.SourceAlpha, BlendOption.One, BlendOption.SourceAlpha, BlendOption.One);

            // MASK: Source = Zero, Destination = InverseSourceAlpha
            // Alpha: Zero, InverseSourceAlpha
            CreateBlendState(BlendMode.MASK, BlendOption.Zero, BlendOption.InverseSourceAlpha, BlendOption.Zero, BlendOption.InverseSourceAlpha);

            // EFFECTMASK: Source = DestinationAlpha, Destination = One
            // Alpha: DestinationAlpha, One
            CreateBlendState(BlendMode.EFFECTMASK, BlendOption.DestinationAlpha, BlendOption.One, BlendOption.DestinationAlpha, BlendOption.One);

            // HIGHLIGHT: Source = BlendFactor, Destination = One
            // Alpha: BlendFactor, One
            CreateBlendState(BlendMode.HIGHLIGHT, BlendOption.BlendFactor, BlendOption.One, BlendOption.BlendFactor, BlendOption.One);

            // LIGHTMAP: Source = Zero, Destination = SourceColor
            // Alpha: Zero, SourceAlpha (SourceColor is invalid for Alpha, maps to SourceAlpha)
            CreateBlendState(BlendMode.LIGHTMAP, BlendOption.Zero, BlendOption.SourceColor, BlendOption.Zero, BlendOption.SourceAlpha);

            // NONE: Standard alpha blend as safe fallback.
            CreateBlendState(BlendMode.NONE, BlendOption.SourceAlpha, BlendOption.InverseSourceAlpha, BlendOption.SourceAlpha, BlendOption.InverseSourceAlpha);
        }

        private void CreateBlendState(BlendMode mode, BlendOption src, BlendOption dest, BlendOption srcAlpha, BlendOption destAlpha)
        {
            var desc = new BlendStateDescription();
            desc.RenderTarget[0].IsBlendEnabled = true;

            desc.RenderTarget[0].SourceBlend = src;
            desc.RenderTarget[0].DestinationBlend = dest;
            desc.RenderTarget[0].BlendOperation = BlendOperation.Add;

            desc.RenderTarget[0].SourceAlphaBlend = srcAlpha;
            desc.RenderTarget[0].DestinationAlphaBlend = destAlpha;
            desc.RenderTarget[0].AlphaBlendOperation = BlendOperation.Add;

            desc.RenderTarget[0].RenderTargetWriteMask = ColorWriteMaskFlags.All;

            _blendStates[mode] = _device.CreateBlendState(desc);
        }

        public bool SupportsOutlineShader => _outlinePixelShader != null && _outlineBuffer != null;

        public void Draw(Texture2D texture, RectangleF destination, RectangleF? source, Color color, Matrix3x2 transform, BlendMode blendMode, float opacity, float blendRate)
        {
            DrawInternal(texture, destination, source, color, transform, blendMode, opacity, blendRate, _pixelShader, null);
        }

        public void DrawOutlined(Texture2D texture, RectangleF destination, RectangleF? source, Color color, Matrix3x2 transform, BlendMode blendMode, float opacity, float blendRate, Color4 outlineColor, float outlineThickness)
        {
            var outlineEffect = CreateOutlineEffect(texture, source, outlineColor, outlineThickness);
            DrawInternal(texture, destination, source, color, transform, blendMode, opacity, blendRate, _outlinePixelShader ?? _pixelShader, outlineEffect);
        }

        public void DrawGrayscale(Texture2D texture, RectangleF destination, RectangleF? source, Color color, Matrix3x2 transform, BlendMode blendMode, float opacity, float blendRate)
        {
            var grayscaleEffect = CreateGrayscaleEffect();
            DrawInternal(texture, destination, source, color, transform, blendMode, opacity, blendRate, _grayscalePixelShader ?? _pixelShader, grayscaleEffect);
        }

        public void DrawDropShadow(Texture2D texture, RectangleF destination, RectangleF shadowBounds, RectangleF? source, Color color, Matrix3x2 transform, BlendMode blendMode, float opacity, float blendRate, Color4 shadowColor, float shadowWidth, float shadowMaxOpacity)
        {
            var dropShadowEffect = CreateDropShadowEffect(texture, shadowBounds, shadowWidth, shadowMaxOpacity);
            DrawInternal(texture, destination, source, color, transform, blendMode, opacity, blendRate, _dropShadowPixelShader ?? _pixelShader, dropShadowEffect);
        }

        private SpriteEffect? CreateOutlineEffect(Texture2D texture, RectangleF? source, Color4 outlineColor, float outlineThickness)
        {
            if (!SupportsOutlineShader || _outlinePixelShader == null)
                return null;

            var texDesc = texture.Description;
            var texWidth = texDesc.Width;
            var texHeight = texDesc.Height;

            float u1 = 0, v1 = 0, u2 = 1, v2 = 1;
            if (source.HasValue)
            {
                u1 = source.Value.Left / texWidth;
                v1 = source.Value.Top / texHeight;
                u2 = source.Value.Right / texWidth;
                v2 = source.Value.Bottom / texHeight;
            }

            var outlineBuffer = new OutlineBufferType
            {
                OutlineColor = outlineColor,
                TextureSize = new Vector2(texWidth, texHeight),
                OutlineThickness = outlineThickness,
                Padding = 0f,
                SourceUV = new Vector4(u1, v1, u2, v2)
            };

            return new SpriteEffect(
                _outlinePixelShader,
                _outlineBuffer,
                Unsafe.SizeOf<OutlineBufferType>(),
                outlineThickness,
                true,
                ptr =>
                {
                    unsafe { *(OutlineBufferType*)ptr = outlineBuffer; }
                });
        }

        private SpriteEffect? CreateGrayscaleEffect()
        {
            if (_grayscalePixelShader == null)
                return null;

            return new SpriteEffect(_grayscalePixelShader, null, 0, 0f, false, null);
        }

        private SpriteEffect? CreateDropShadowEffect(Texture2D texture, RectangleF shadowBounds, float shadowWidth, float shadowMaxOpacity)
        {
            if (_dropShadowPixelShader == null || _dropShadowBuffer == null)
                return null;

            var shadowBuffer = new DropShadowBufferType
            {
                ImgMin = new Vector2(shadowBounds.Left, shadowBounds.Top),
                ImgMax = new Vector2(shadowBounds.Right, shadowBounds.Bottom),
                ShadowSize = shadowWidth,
                MaxAlpha = shadowMaxOpacity,
                Padding = Vector2.Zero
            };

            return new SpriteEffect(
                _dropShadowPixelShader,
                _dropShadowBuffer,
                Unsafe.SizeOf<DropShadowBufferType>(),
                shadowWidth,
                false,
                ptr =>
                {
                    unsafe { *(DropShadowBufferType*)ptr = shadowBuffer; }
                });
        }

        private void DrawInternal(Texture2D texture, RectangleF destination, RectangleF? source, Color color, Matrix3x2 transform, BlendMode blendMode, float opacity, float blendRate, PixelShader pixelShader, SpriteEffect? effect)
        {
            if (texture == null) return;

            var activePixelShader = effect?.Shader ?? pixelShader ?? _pixelShader;
            if (activePixelShader == null) return;

            var rtvArray = new RenderTargetView[1];
            _context.OMGetRenderTargets(1, rtvArray, out DepthStencilView? dsv);
            dsv?.Dispose();
            if (rtvArray[0] != null)
            {
                using (var res = rtvArray[0].Resource)
                using (var tex = res?.QueryInterface<Texture2D>())
                {
                    if (tex != null)
                    {
                        _context.RSSetViewport(new Viewport(0, 0, tex.Description.Width, tex.Description.Height, 0, 1));
                    }
                }
                rtvArray[0].Dispose();
            }

            if (!_srvCache.TryGetValue(texture, out var srv))
            {
                srv = _device.CreateShaderResourceView(texture);
                _srvCache[texture] = srv;
            }

            _context.IASetInputLayout(_inputLayout);
            _context.IASetPrimitiveTopology(PrimitiveTopology.TriangleStrip);
            _context.IASetVertexBuffer(0, _vertexBuffer, Unsafe.SizeOf<VertexType>(), 0);

            bool usingEffect = effect.HasValue && effect.Value.IsValid;
            float geometryExpand = usingEffect ? effect.Value.GeometryExpand : 0f;
            bool expandUvs = usingEffect && effect.Value.ExpandUvs;

            UpdateVertexBuffer(destination, source, texture.Description.Width, texture.Description.Height, color, opacity, geometryExpand, expandUvs);
            UpdateMatrixBuffer(transform);

            _context.VSSetShader(_vertexShader);
            _context.PSSetShader(activePixelShader);
            _context.PSSetShaderResource(0, srv);
            _context.PSSetSampler(0, _samplerState);
            _context.VSSetConstantBuffer(0, _matrixBuffer);

            ApplyEffect(effect);

            if (_blendStates.TryGetValue(blendMode, out var blendState))
            {
                var factor = new Color4(blendRate, blendRate, blendRate, blendRate);
                _context.OMSetBlendState(blendState, factor, 0xFFFFFFFF);
            }
            else
            {
                var factor = new Color4(blendRate, blendRate, blendRate, blendRate);
                _context.OMSetBlendState(_blendStates[BlendMode.NORMAL], factor, 0xFFFFFFFF);
            }

            _context.Draw(4, 0);

            _context.PSSetShaderResource(0, null);
            _context.PSSetConstantBuffer(1, null);
            _context.OMSetBlendState(null);
        }

        private void ApplyEffect(SpriteEffect? effect)
        {
            if (!effect.HasValue || !effect.Value.IsValid)
            {
                _context.PSSetConstantBuffer(1, null);
                return;
            }

            var spriteEffect = effect.Value;

            if (spriteEffect.ConstantBuffer != null && spriteEffect.WriteConstants != null)
            {
                var mapped = _context.Map(spriteEffect.ConstantBuffer, 0, MapMode.WriteDiscard, MapFlags.None);
                spriteEffect.WriteConstants(mapped.DataPointer);
                _context.Unmap(spriteEffect.ConstantBuffer, 0);

                _context.PSSetConstantBuffer(1, spriteEffect.ConstantBuffer);
            }
            else
            {
                _context.PSSetConstantBuffer(1, null);
            }
        }

        private unsafe void UpdateVertexBuffer(RectangleF dest, RectangleF? source, int texWidth, int texHeight, Color color, float opacity, float geometryExpand, bool expandUvs)
        {
            float left = dest.Left;
            float right = dest.Right;
            float top = dest.Top;
            float bottom = dest.Bottom;

            float u1 = 0, v1 = 0, u2 = 1, v2 = 1;
            if (source.HasValue)
            {
                u1 = source.Value.Left / (float)texWidth;
                v1 = source.Value.Top / (float)texHeight;
                u2 = source.Value.Right / (float)texWidth;
                v2 = source.Value.Bottom / (float)texHeight;
            }

            if (geometryExpand > 0)
            {
                left -= geometryExpand;
                right += geometryExpand;
                top -= geometryExpand;
                bottom += geometryExpand;

                if (expandUvs)
                {
                    float uPad = geometryExpand / texWidth;
                    float vPad = geometryExpand / texHeight;
                    u1 -= uPad;
                    v1 -= vPad;
                    u2 += uPad;
                    v2 += vPad;
                }
            }

            var col = new Color4(color.R / 255f, color.G / 255f, color.B / 255f, (color.A / 255f) * opacity);

            // Triangle Strip: TopLeft, TopRight, BottomLeft, BottomRight
            var v0 = new VertexType(new Vector2(left, top), new Vector2(u1, v1), col);
            var v1_ = new VertexType(new Vector2(right, top), new Vector2(u2, v1), col);
            var v2_ = new VertexType(new Vector2(left, bottom), new Vector2(u1, v2), col);
            var v3 = new VertexType(new Vector2(right, bottom), new Vector2(u2, v2), col);

            var mapped = _context.Map(_vertexBuffer, 0, MapMode.WriteDiscard, MapFlags.None);
            VertexType* ptr = (VertexType*)mapped.DataPointer;
            ptr[0] = v0;
            ptr[1] = v1_;
            ptr[2] = v2_;
            ptr[3] = v3;
            _context.Unmap(_vertexBuffer, 0);
        }

        private unsafe void UpdateMatrixBuffer(Matrix3x2 transform)
        {
            var viewports = new Viewport[1];
            _context.RSGetViewports(viewports);
            float width = viewports[0].Width;
            float height = viewports[0].Height;

            // Standard 2D Ortho: Top-Left (0,0) to Bottom-Right (w,h)
            // X: (x / W) * 2 - 1
            // Y: 1 - (y / H) * 2

            Matrix4x4 projection = Matrix4x4.Identity;
            projection.M11 = 2.0f / width;
            projection.M22 = -2.0f / height;
            projection.M41 = -1.0f;
            projection.M42 = 1.0f;

            // Extend Matrix3x2 transform to 4x4
            Matrix4x4 world = Matrix4x4.Identity;
            world.M11 = transform.M11;
            world.M12 = transform.M12;
            world.M21 = transform.M21;
            world.M22 = transform.M22;
            world.M41 = transform.M31; // Translation X
            world.M42 = transform.M32; // Translation Y

            // Transpose because HLSL mul(vector, matrix) with row-major layout
            Matrix4x4 final = Matrix4x4.Transpose(world * projection);

            var mapped = _context.Map(_matrixBuffer, 0, MapMode.WriteDiscard, MapFlags.None);
            *(Matrix4x4*)mapped.DataPointer = final;
            _context.Unmap(_matrixBuffer, 0);
        }

        public void Dispose()
        {
            foreach (var srv in _srvCache.Values) srv.Dispose();
            _srvCache.Clear();

            foreach (var bs in _blendStates.Values) bs.Dispose();
            _blendStates.Clear();

            _samplerState?.Dispose();
            _matrixBuffer?.Dispose();
            _outlineBuffer?.Dispose();
            _dropShadowBuffer?.Dispose();
            _vertexBuffer?.Dispose();
            _inputLayout?.Dispose();
            _pixelShader?.Dispose();
            _grayscalePixelShader?.Dispose();
            _outlinePixelShader?.Dispose();
            _dropShadowPixelShader?.Dispose();
            _vertexShader?.Dispose();
        }
    }
}
