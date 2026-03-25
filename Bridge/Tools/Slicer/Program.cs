using System;
using System.IO;
using System.Drawing;
using System.Drawing.Imaging;

class Program
{
    static void Main()
    {
        string exeDir = AppContext.BaseDirectory;
        string engineRoot = FindEngineRoot(exeDir);

        if (engineRoot == null)
        {
            Console.WriteLine("Nie znaleziono katalogu NV_ENGINE.");
            return;
        }

        string atlasDir = Path.Combine(engineRoot, "Assets", "Atlas");
        string outputDir = Path.Combine(engineRoot, "Assets", "Blocks");

        Directory.CreateDirectory(outputDir);

        Console.WriteLine($"Atlas dir:   {atlasDir}");
        Console.WriteLine($"Output dir:  {outputDir}");
        Console.WriteLine();

        foreach (var file in Directory.GetFiles(atlasDir, "*.png"))
        {
            string name = Path.GetFileNameWithoutExtension(file);
            using Bitmap atlas = new Bitmap(file);

            var tiles = GetTileRects(name);

            if (tiles == null)
            {
                Console.WriteLine($"[SKIP] Brak definicji dla '{name}'");
                continue;
            }

            int index = 0;
            foreach (var rect in tiles)
            {
                if (rect.Right > atlas.Width || rect.Bottom > atlas.Height)
                {
                    Console.WriteLine($"  [WARN] Tile {index} poza atlasem, pomijam.");
                    index++;
                    continue;
                }

                using Bitmap tile = atlas.Clone(rect, PixelFormat.Format32bppArgb);
                string outPath = Path.Combine(outputDir, $"{name}_{index}.png");
                tile.Save(outPath, ImageFormat.Png);
                Console.WriteLine($"  -> {name}_{index}.png  ({rect.Width}x{rect.Height} @ {rect.X},{rect.Y})");
                index++;
            }

            Console.WriteLine($"[OK] {name}: {index} kafelków");
            Console.WriteLine();
        }

        Console.WriteLine("Gotowe.");
    }

    // Returns exact Rectangle for every tile in each atlas,
    // derived from pixel-level analysis of the actual images.
    static List<Rectangle>? GetTileRects(string name)
    {
        // ── drewno_liscie.png ── 1024x1024, 4x4 uniform
        // Col starts: 133, 330, 530, 730  widths: ~160
        // Row starts: 107, 287, 466, 642  heights: ~136
        if (name.Contains("drewno", StringComparison.OrdinalIgnoreCase))
        {
            int[] xs = { 133, 330, 530, 730 };
            int[] ws = { 160, 162, 163, 160 };
            int[] ys = { 107, 287, 466, 642 };
            int[] hs = { 136, 135, 132, 114 };
            return BuildUniformGrid(xs, ws, ys, hs);
        }

        // ── trawa_kamien.png ── 1024x1024, 4x4 uniform
        // Col starts: 11, 262, 517, 771  widths: ~237-240
        // Row starts: 9, 249, 488, 724   heights: ~227
        if (name.Contains("trawa", StringComparison.OrdinalIgnoreCase))
        {
            int[] xs = { 11,  262, 517, 771 };
            int[] ws = { 237, 241, 240, 240 };
            int[] ys = { 9,   249, 488, 724 };
            int[] hs = { 227, 228, 225, 226 };
            return BuildUniformGrid(xs, ws, ys, hs);
        }

        // ── rudy.png ── 1536x1024, 4x4 uniform (large side margins ~366px)
        // Col starts: 366, 570, 777, 983  widths: ~186
        // Row starts: 137, 324, 513, 682  heights: ~161
        if (name.Contains("rudy", StringComparison.OrdinalIgnoreCase))
        {
            int[] xs = { 366, 570, 777, 983 };
            int[] ws = { 186, 187, 186, 187 };
            int[] ys = { 137, 324, 513, 682 };
            int[] hs = { 161, 162, 143, 127 };
            return BuildUniformGrid(xs, ws, ys, hs);
        }

        // ── under_workblocks.png ── 1536x1024, IRREGULAR layout
        // 5 rows, variable columns per row: 5, 5, 4, 5, 4
        // All coordinates measured pixel-precisely from image analysis
        if (name.Contains("under", StringComparison.OrdinalIgnoreCase))
        {
            // Col x-starts and widths (5 possible columns)
            int[] xs = { 367, 537, 706, 874, 1041 };
            int[] ws = { 141, 142, 138, 140, 131  };

            // Row definitions: (y, height, which col indices are present)
            var rowDefs = new (int Y, int H, int[] Cols)[]
            {
                (121, 121, new[] { 0, 1, 2, 3, 4 }),
                (278, 113, new[] { 0, 1, 2, 3, 4 }),
                (429, 118, new[] { 0, 1, 2, 3    }),
                (581,  95, new[] { 0, 1, 2, 3, 4 }),
                (707, 109, new[] { 0, 1, 2, 3    }),
            };

            var rects = new List<Rectangle>();
            foreach (var (ry, rh, cols) in rowDefs)
                foreach (int ci in cols)
                    rects.Add(new Rectangle(xs[ci], ry, ws[ci], rh));

            return rects;
        }

        return null;
    }

    // Builds a grid of Rectangles from per-column and per-row arrays
    static List<Rectangle> BuildUniformGrid(int[] xs, int[] ws, int[] ys, int[] hs)
    {
        var rects = new List<Rectangle>();
        for (int row = 0; row < ys.Length; row++)
            for (int col = 0; col < xs.Length; col++)
                rects.Add(new Rectangle(xs[col], ys[row], ws[col], hs[row]));
        return rects;
    }

    static string? FindEngineRoot(string start)
    {
        DirectoryInfo? dir = new DirectoryInfo(start);
        while (dir != null)
        {
            if (dir.Name.Equals("NV_ENGINE", StringComparison.OrdinalIgnoreCase))
                return dir.FullName;
            dir = dir.Parent;
        }
        return null;
    }
}