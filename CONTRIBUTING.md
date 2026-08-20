# Contributing

Dziekuje, ze chcesz sie przyczynic do projektu.

## Setup

1. Uzyj toolchainu z plikow CI (patrz .github/workflows).
2. Format, lint, testy:
   - Rust: cargo fmt, clippy -D warnings, test
   - Python: ruff (jesli skonfigurowany) + pytest
   - TS/JS: eslint + tsc --noEmit

## Zglaszanie problemow

Sprawdz najpierw istniejace issue. Opisz: oczekiwany vs aktualny wynik, kroki reprodukcji, srodowisko.

## Pull requesty

- 1 temat = 1 PR, maly rozmiar
- Zielony CI wymagany (wymuszone przez ochrone galezi)
- Testy dla nowych zachowan
- Opisz "dlaczego"

Licencja: MIT (patrz LICENSE).
