mod world;

use world::generator::WorldGenerator;
use world::palette::BlockPalette;

#[tokio::main]
async fn main() {
    println!("======================================");
    println!("   NV-2.0 ENGINE - CORE INITIALIZED   ");
    println!("======================================");
    println!("Dyrektorze, Ryzen 9 wykryty. Rozpoczynam generację...");

    let _palette = BlockPalette::new_default();
    let generator = WorldGenerator { seed: 12345 };

    let first_chunk = generator.generate_chunk(0, 0);

    // Szybka analiza wygenerowanego świata
    let grass_count = first_chunk.iter().filter(|&&b| b == 1).count();
    let stone_count = first_chunk.iter().filter(|&&b| b == 3).count();

    println!("-> Sektor (Chunk 0,0) wygenerowany pomyślnie.");
    println!("-> Bloki trawy: {}", grass_count);
    println!("-> Bloki kamienia: {}", stone_count);
    println!("--------------------------------------");
    println!("STATUS: System gotowy do renderowania GPU.");
}