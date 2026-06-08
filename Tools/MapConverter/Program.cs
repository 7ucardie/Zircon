#!/usr/bin/env dotnet-script
// map-converter — convert Zircon binary .map files to JSON format
//
// Usage:
//   map-converter <source.map> [output.map.json]
//   map-converter --batch <maps-dir> [output-dir]
//
// If output is omitted the JSON is written alongside the source with a .map.json extension.
// --batch converts all *.map files in a directory.

using System.Text.Json;

if (args.Length == 0 || args[0] is "-h" or "--help")
{
    Console.Error.WriteLine("Usage:");
    Console.Error.WriteLine("  map-converter <source.map> [output.map.json]");
    Console.Error.WriteLine("  map-converter --batch <maps-dir> [output-dir]");
    return 1;
}

if (args[0] == "--batch")
{
    string inDir = args.Length >= 2 ? args[1] : ".";
    string outDir = args.Length >= 3 ? args[2] : inDir;

    if (!Directory.Exists(inDir))
    {
        Console.Error.WriteLine($"Directory not found: {inDir}");
        return 1;
    }

    Directory.CreateDirectory(outDir);

    int converted = 0;
    int skipped = 0;
    foreach (string src in Directory.EnumerateFiles(inDir, "*.map"))
    {
        string name = Path.GetFileNameWithoutExtension(src);
        string dst = Path.Combine(outDir, name + ".map.json");
        try
        {
            ExportMapJson(src, dst);
            Console.WriteLine($"  {Path.GetFileName(src)} → {Path.GetFileName(dst)}");
            converted++;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"  SKIP {Path.GetFileName(src)}: {ex.Message}");
            skipped++;
        }
    }

    Console.WriteLine($"Done: {converted} converted, {skipped} skipped.");
    return skipped > 0 ? 2 : 0;
}
else
{
    string src = args[0];
    string dst = args.Length >= 2 ? args[1] : Path.ChangeExtension(src, ".map.json");

    if (!File.Exists(src))
    {
        Console.Error.WriteLine($"File not found: {src}");
        return 1;
    }

    ExportMapJson(src, dst);
    Console.WriteLine($"{src} → {dst}");
    return 0;
}

static void ExportMapJson(string binaryPath, string jsonPath)
{
    byte[] data = File.ReadAllBytes(binaryPath);

    if (data.Length < 28)
        throw new InvalidDataException("File too short to be a valid .map file.");

    int width  = data[23] << 8 | data[22];
    int height = data[25] << 8 | data[24];
    int offset = 28 + width * height / 4 * 3;

    if (offset + (long)width * height * 14 > data.Length)
        throw new InvalidDataException("File truncated: cell data extends past end of file.");

    // Build layout rows: '.' = walkable, '#' = blocked.
    var rows = new char[height][];
    for (int y = 0; y < height; y++)
    {
        rows[y] = new char[width];
        Array.Fill(rows[y], '#');
    }

    for (int x = 0; x < width; x++)
        for (int y = 0; y < height; y++)
        {
            byte flag = data[offset + (x * height + y) * 14];
            if ((flag & 0x03) == 0x03)
                rows[y][x] = '.';
        }

    var options = new JsonWriterOptions { Indented = true };
    Directory.CreateDirectory(Path.GetDirectoryName(jsonPath) ?? ".");
    using FileStream fs = File.Create(jsonPath);
    using var writer = new Utf8JsonWriter(fs, options);

    writer.WriteStartObject();
    writer.WriteNumber("width",  width);
    writer.WriteNumber("height", height);
    writer.WriteStartArray("layout");
    for (int y = 0; y < height; y++)
        writer.WriteStringValue(new string(rows[y]));
    writer.WriteEndArray();
    writer.WriteEndObject();
}
