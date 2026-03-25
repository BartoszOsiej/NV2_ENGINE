using System;
using System.Drawing;
using System.IO;
using System.Collections.Generic;

class Program
{
    static void Main()
    {
        string assetsPath = @"G:\NV_ENGINE\Assets\";
        string outputPath = Path.Combine(assetsPath, "Blocks");
        if (!Directory.Exists(outputPath)) Directory.CreateDirectory(outputPath);

        // MAPA NAZW DLA KAŻDEGO ATLASU
        var atlasMaps = new Dictionary<string, string[]>()
        {
            ["trawa_kamien.jpg"] = new[] {
                "grass_side", "grass_side_v2", "dirt", "dirt_dry",
                "stone_cracked", "stone_mix", "stone_smooth", "cobblestone",
                "mossy_stone", "andesite", "diorite", "gravel",
                "sand", "red_sand", "clay", "bedrock"
            },
            ["drewno_liscie.jpg"] = new[] {
                "log_oak", "log_spruce", "log_dark", "log_birch",
                "top_oak", "top_spruce", "top_dark", "top_birch",
                "leaves_oak", "leaves_spruce", "leaves_dark", "leaves_birch",
                "planks_oak", "planks_spruce", "planks_dark", "planks_birch"
            },
            ["rudy.jpg"] = new[] {
                "ore_coal", "ore_iron", "ore_gold", "ore_diamond",
                "ore_coal_deep", "ore_iron_deep", "ore_gold_deep", "ore_diamond_deep",
                "ore_obsidian", "ore_ignis", "ore_lapis", "ore_emerald", // TWOJA RUDA IGNIS!
                "ore_obsidian_v2", "ore_ignis_v2", "ore_quartz", "ore_emerald_deep"
            }
        };

        foreach (var entry in atlasMaps)
        {
            string fullPath = Path.Combine(assetsPath, entry.Key);
            if (!File.Exists(fullPath)) continue;

            using (Bitmap atlas = new Bitmap(fullPath))
            {
                int gridSize = 4; // Większość to 4x4
                int tileSizeW = atlas.Width / gridSize;
                int tileSizeH = atlas.Height / gridSize;

                for (int i = 0; i < entry.Value.Length; i++)
                {
                    int x = i % gridSize;
                    int y = i / gridSize;

                    Rectangle rect = new Rectangle(x * tileSizeW, y * tileSizeH, tileSizeW, tileSizeH);
                    using (Bitmap tile = atlas.Clone(rect, atlas.PixelFormat))
                    {
                        tile.Save(Path.Combine(outputPath, $"{entry.Value[i]}.png"), System.Drawing.Imaging.ImageFormat.Png);
                    }
                }
                Console.WriteLine($"Przetworzono: {entry.Key}");
            }
        }

        // SPECJALNA OBSŁUGA DLA under&workblocks (5x5)
        ProcessWorkBlocks(assetsPath, outputPath);
    }

    static void ProcessWorkBlocks(string path, string outPath)
    {
        string file = "under&workblocks.jpg";
        if (!File.Exists(Path.Combine(path, file))) return;

        string[] names = { "obsidian", "obsidian_v2", "bedrock_v2", "bedrock_v3", "glowstone", 
                           "crafting_table", "furnace", "dispenser", "blast_furnace", "smoker" }; // i tak dalej...

        using (Bitmap atlas = new Bitmap(Path.Combine(path, file)))
        {
            int gridSize = 5;
            int tW = atlas.Width / gridSize;
            int tH = atlas.Height / gridSize;

            for (int i = 0; i < names.Length; i++)
            {
                int x = i % gridSize; int y = i / gridSize;
                using (Bitmap tile = atlas.Clone(new Rectangle(x*tW, y*tH, tW, tH), atlas.PixelFormat))
                    tile.Save(Path.Combine(outPath, $"{names[i]}.png"), System.Drawing.Imaging.ImageFormat.Png);
            }
        }
        Console.WriteLine("Przetworzono bloki robocze.");
    }
}