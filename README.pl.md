# 🎮 NV_ENGINE

**Natywny desktopowy silnik wokselowy z generacją terenu opartą na AI.**

NV_ENGINE to silnik wokselowy napisany od zera w Ruście — renderowanie świata
w czasie rzeczywistym, proceduralna generacja terenu, interakcja, rozgrywka
z ekwipunkiem i craftingiem oraz narzędzia pipeline'u treści. Poza prototypem
terenu repo zawiera działającą pętlę rozgrywki: menu, komendy, zapis/odczyt,
obsługę przedmiotów, interakcję z blokami, symulację świata i renderowanie
sterowane GPU.

```
┌─────────────────────────────────────────────────────────────┐
│  NV_ENGINE                                                   │
│  ├── Core/        Runtime Rust — silnik, rozgrywka, renderer,│
│  │                symulacja świata, logika UI (wgpu, winit)  │
│  ├── Bridge/      Narzędzia treści .NET 8 — cięcie atlasów   │
│  │                i przygotowanie assetów                    │
│  ├── Assets/      Zasoby i pakowanie                         │
│  └── VulkanLayers/ Warstwy pomocnicze Vulkan                │
└─────────────────────────────────────────────────────────────┘
```

## Szybki start

```bash
cd Core
cargo run --release
```

Co zobaczysz:

- Silnik uruchamia się bez opóźnień
- Świat generuje się z roślinnością sterowaną AI
- Kwiaty, paprocie i kamyki rozmieszczane inteligentnie przez sieć neuronową
- AI uczy się w tle (bez wpływu na FPS)

## 🤖 AI — MeMLP (Modular embedded Multi-layer Perceptron Model)

Cały stos AI działa na **MeMLP** — modułowej, osadzonej sieci neuronowej,
która żyje wewnątrz silnika (czysty CPU, checkpointy JSON, bez chmury i GPU):

```
Wejście: 8 cech terenu (wysokość, nachylenie, temperatura bioma, wilgotność,
         odległość od wody, sąsiednia roślinność, światło, szum)
    ↓
Głowa roślinności (głębokie MLP 8 → 24 → 16 → 4, ReLU + softmax)
    ↓
Softmax (4 wyjścia: kwiat / paproć / patyk / kamyk)
    ↓
Rozmieszczenie ograniczone pewnością + dekoracje zależne od bioma (głowa biome)
```

Moduły — jeden plik checkpointu, implementacja w `Core/Src/world/memplp.rs`:

| Moduł | Kształt | Zadanie |
|---|---|---|
| `vegetation` | 8 → 24 → 16 → 4 | rozmieszczanie kwiat / paproć / patyk / kamyk |
| `biome` | 8 → 12 → 9 | klasyfikacja bioma (9 biomów, steruje dekoracjami) |
| `texture` | 8 → 12 → 6 | wybór stylu tekstur proceduralnych |

- **22 nowe typy roślin** — róże, tulipany, stokrotki, rośliny wodne, mech,
  patyki, kamyki i więcej
- **Uczy się od gracza** — stawianie/niszczenie roślinności jest zapisywane
  jako feedback treningowy (`Core/Src/world/ai_feedback.rs`)
- **Uczenie online** — prawdziwe dane klimatyczne (Open-Meteo) łączone z
  treningiem, z bezpiecznym fallbackiem offline
- **Wstecznie kompatybilny** — stare checkpointy z jedną warstwą ukrytą są
  wykrywane i migrowane automatycznie
- **Mały rozmiar** — checkpoint ~1 KB, ~0.3 µs na predykcję, miliony próbek
  treningowych/s w wątku tła (patrz `TEST_REPORT.md`)

> Reprodukowalność: `cargo test --release qa_benchmark_report -- --ignored --nocapture`
> ponownie uruchamia pełny benchmark MeMLP.

## 🧱 Rozgrywka

- Interakcja z blokami (stawianie / wydobywanie)
- Ekwipunek i crafting
- Komendy, menu, zapis / odczyt
- Symulacja świata oparta na chunkach

## 🛠️ Stack technologiczny

| Obszar | Technologia |
|---|---|
| Silnik i runtime | Rust (2021), wgpu 0.20, winit 0.30 |
| Pipeline treści | C# / .NET 8 (`Bridge/Tools`) |
| Narzędzia tekstur | Python (`generate_textures.py`) |
| Renderowanie | Renderer sterowany GPU, warstwy Vulkan |

## 📚 Dokumentacja

| Plik | Treść |
|---|---|
| `TECHNOLOGIES_AND_CURRENT_IMPLEMENTATION.md` | Przegląd rozwiązania i stack |
| `AI_IMPLEMENTATION_SUMMARY.md` | System roślinności AI — podsumowanie implementacji |
| `AI_TECHNICAL_DOCS.md` | Matematyka sieci neuronowej i szczegóły implementacji |
| `AI_PHASE2_ROADMAP.md` | Przyszłe plany (integracja internetowa, tekstury GPU, …) |
| `QUICKSTART.md` | Instrukcje budowania i uruchamiania |
| `CHANGELOG.md` | Co się zmieniło |

## 🚀 Roadmap (Faza 2)

- [ ] Pobieranie zestawów treningowych
- [ ] Generacja tekstur na GPU
- [ ] Edycja terenu w czasie rzeczywistym
- [ ] Udostępnianie modeli społeczności
- [ ] Nauka preferencji gracza

Szczegóły w `AI_PHASE2_ROADMAP.md`.
