using System;
using System.IO;
using System.Drawing;
using System.Drawing.Imaging;
using System.Collections.Generic;
using System.Runtime.Versioning;

[SupportedOSPlatform("windows")]
class Program
{
    static void Main()
    {
        string exeDir = AppContext.BaseDirectory;
        string? engineRoot = FindEngineRoot(exeDir);

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

            var tiles = GetTileRects(name, atlas);

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
    static List<Rectangle>? GetTileRects(string name, Bitmap atlas)
    {
        if (name.Contains("drewno", StringComparison.OrdinalIgnoreCase) ||
            name.Contains("trawa", StringComparison.OrdinalIgnoreCase) ||
            name.Contains("rudy", StringComparison.OrdinalIgnoreCase))
        {
            if (name.Contains("drewno_liscie", StringComparison.OrdinalIgnoreCase))
            {
                // drewno_liscie.png: 1024x1024, 4x4 grid
                // Col x-starts: 133, 330, 530, 730   widths: 160, 162, 163, 160
                // Row y-starts: 107, 287, 466, 642   heights: 136, 135, 132, 114
                int[] xs = {133, 330, 530, 730};
                int[] ws = {160, 162, 163, 160};
                int[] ys = {107, 287, 466, 642};
                int[] hs = {136, 135, 132, 114};
                return BuildUniformGrid(xs, ws, ys, hs);
            }
            else if (name.Contains("trawa_kamien", StringComparison.OrdinalIgnoreCase))
            {
                // trawa_kamien.png: 1024x1024, 4x4 grid
                // Col x-starts: 11, 262, 517, 771   widths: 237, 241, 240, 240
                // Row y-starts: 9, 249, 488, 724   heights: 227, 228, 225, 226
                int[] xs = {11, 262, 517, 771};
                int[] ws = {237, 241, 240, 240};
                int[] ys = {9, 249, 488, 724};
                int[] hs = {227, 228, 225, 226};
                return BuildUniformGrid(xs, ws, ys, hs);
            }
            else if (name.Contains("rudy", StringComparison.OrdinalIgnoreCase))
            {
                // rudy.png: 1536x1024, intended 4x4 grid, but with complex per-tile spacing.
                return AnalyzeAtlasByContent(atlas, 4, 4);
            }
            else
            {
                return AnalyzeAtlasForUniformGrid(atlas);
            }
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

    // Analyzes the atlas bitmap to detect a uniform 4x4 grid of tiles
    static List<Rectangle> AnalyzeAtlasForUniformGrid(Bitmap atlas)
    {
        int width = atlas.Width;
        int height = atlas.Height;

        // Scan horizontal line at y = height / 2 to find column boundaries
        int scanY = height / 2;
        var colStarts = new List<int>();
        Color prevColor = atlas.GetPixel(0, scanY);
        for (int x = 1; x < width; x++)
        {
            Color currColor = atlas.GetPixel(x, scanY);
            if (ColorDifference(prevColor, currColor) > 100) // threshold for change
            {
                if (colStarts.Count < 4) // limit to 4 columns
                    colStarts.Add(x);
            }
            prevColor = currColor;
        }
        if (colStarts.Count != 4) colStarts = new List<int> { 0, width/4, width/2, 3*width/4 }; // fallback

        // Scan vertical line at x = width / 2 to find row boundaries
        int scanX = width / 2;
        var rowStarts = new List<int>();
        prevColor = atlas.GetPixel(scanX, 0);
        for (int y = 1; y < height; y++)
        {
            Color currColor = atlas.GetPixel(scanX, y);
            if (ColorDifference(prevColor, currColor) > 100)
            {
                if (rowStarts.Count < 4) // limit to 4 rows
                    rowStarts.Add(y);
            }
            prevColor = currColor;
        }
        if (rowStarts.Count != 4) rowStarts = new List<int> { 0, height/4, height/2, 3*height/4 }; // fallback

        // Compute widths and heights
        var colWidths = new List<int>();
        for (int i = 0; i < colStarts.Count; i++)
        {
            int start = colStarts[i];
            int end = (i < colStarts.Count - 1) ? colStarts[i+1] : width;
            colWidths.Add(end - start);
        }

        var rowHeights = new List<int>();
        for (int i = 0; i < rowStarts.Count; i++)
        {
            int start = rowStarts[i];
            int end = (i < rowStarts.Count - 1) ? rowStarts[i+1] : height;
            rowHeights.Add(end - start);
        }

        return BuildUniformGrid(colStarts.ToArray(), colWidths.ToArray(), rowStarts.ToArray(), rowHeights.ToArray());
    }

    static List<Rectangle> AnalyzeAtlasByContent(Bitmap atlas, int expectedCols, int expectedRows)
    {
        int width = atlas.Width;
        int height = atlas.Height;
        Color bg = atlas.GetPixel(0, 0);

        bool[] colActive = new bool[width];
        bool[] rowActive = new bool[height];

        for (int x = 0; x < width; x++)
        {
            for (int y = 0; y < height; y++)
            {
                var c = atlas.GetPixel(x, y);
                if (c.A > 20 && (Math.Abs(c.R - bg.R) > 10 || Math.Abs(c.G - bg.G) > 10 || Math.Abs(c.B - bg.B) > 10))
                {
                    colActive[x] = true;
                    rowActive[y] = true;
                }
            }
        }

        var colStarts = new List<int>();
        var colWidths = new List<int>();
        for (int x = 0; x < width;)
        {
            if (!colActive[x]) { x++; continue; }
            int start = x;
            while (x < width && colActive[x]) x++;
            colStarts.Add(start);
            colWidths.Add(x - start);
        }

        var rowStarts = new List<int>();
        var rowHeights = new List<int>();
        for (int y = 0; y < height;)
        {
            if (!rowActive[y]) { y++; continue; }
            int start = y;
            while (y < height && rowActive[y]) y++;
            rowStarts.Add(start);
            rowHeights.Add(y - start);
        }

        if (colStarts.Count == expectedCols && rowStarts.Count == expectedRows)
            return BuildUniformGrid(colStarts.ToArray(), colWidths.ToArray(), rowStarts.ToArray(), rowHeights.ToArray());

        // Fallback to even grid when content detection fails.
        int[] xs = new int[expectedCols];
        int[] ws = new int[expectedCols];
        for (int i = 0; i < expectedCols; i++)
        {
            xs[i] = i * (width / expectedCols);
            ws[i] = (i + 1) * (width / expectedCols) - xs[i];
        }

        int[] ys = new int[expectedRows];
        int[] hs = new int[expectedRows];
        for (int i = 0; i < expectedRows; i++)
        {
            ys[i] = i * (height / expectedRows);
            hs[i] = (i + 1) * (height / expectedRows) - ys[i];
        }

        return BuildUniformGrid(xs, ws, ys, hs);
    }

    // Computes the difference between two colors
    static int ColorDifference(Color c1, Color c2)
    {
        return Math.Abs(c1.R - c2.R) + Math.Abs(c1.G - c2.G) + Math.Abs(c1.B - c2.B);
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