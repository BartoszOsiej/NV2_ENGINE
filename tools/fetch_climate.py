#!/usr/bin/env python3
"""Generate `assets/climate.dat` — the real-world climate baseline for NV-2.0.

Downloads the NCEP/NCAR Reanalysis **1981–2010 monthly climatology** from
NOAA PSL (free, no login):

  * 2 m air temperature: `air.sig995.mon.ltm.1981-2010.nc`   (2.5° × 73×144)
  * precipitation rate:  `prate.sfc.mon.1981-2010.ltm.nc`    (Gaussian 94×192)

The precipitation field is regridded (bilinear) onto the temperature grid,
and both are written as a compact binary resource the engine embeds via
`include_bytes!`.  This gives every world seed a *real, offline, deterministic*
climate baseline — no network dependency at runtime.

Run:  python3 tools/fetch_climate.py [--out Core/assets/climate.dat]
"""

import argparse
import struct
import sys
from pathlib import Path

import h5py
import numpy as np

AIR_URL = (
    "https://downloads.psl.noaa.gov/Datasets/ncep.reanalysis.derived/surface/"
    "air.sig995.mon.ltm.1981-2010.nc"
)
PRATE_URL = (
    "https://downloads.psl.noaa.gov/Datasets/ncep.reanalysis.derived/surface_gauss/"
    "prate.sfc.mon.1981-2010.ltm.nc"
)

MAGIC = b"NV2CLIM1"
N_LAT = 73
N_LON = 144
N_MON = 12


def download(url: str, dest: Path) -> None:
    if dest.exists() and dest.stat().st_size > 10_000:
        print(f"  cached {dest.name} ({dest.stat().st_size} bytes)")
        return
    print(f"  downloading {url}")
    import urllib.request

    urllib.request.urlretrieve(url, dest)


def bilinear_sample(lat: np.ndarray, lon: np.ndarray, field: np.ndarray,
                    lat_q: float, lon_q: float) -> float:
    """Bilinear sample of a (lat, lon) field at arbitrary (lat_q, lon_q).

    Handles ascending or descending latitude arrays and longitude wrap.
    """
    lon_q = lon_q % 360.0
    # longitude brackets (ascending, wraps around 0°)
    i2 = int(np.searchsorted(lon, lon_q, side="left")) % len(lon)
    i1 = (i2 - 1) % len(lon)
    lon1, lon2 = float(lon[i1]), float(lon[i2])
    span = (lon2 - lon1) % 360.0 or 360.0
    fx = ((lon_q - lon1) % 360.0) / span
    # latitude brackets (ascending or descending)
    if lat[0] < lat[-1]:  # ascending
        j2 = int(np.searchsorted(lat, lat_q, side="left"))
    else:  # descending (90 → -90)
        j2 = len(lat) - int(np.searchsorted(lat[::-1], lat_q, side="left"))
    j2 = int(max(1, min(j2, len(lat) - 1)))
    j1 = j2 - 1
    lat1, lat2 = float(lat[j1]), float(lat[j2])
    fy = (lat_q - lat1) / (lat2 - lat1) if lat2 != lat1 else 0.0
    v = (field[j1, i1] * (1 - fx) + field[j1, i2] * fx) * (1 - fy) + \
        (field[j2, i1] * (1 - fx) + field[j2, i2] * fx) * fy
    return float(v)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="Core/assets/climate.dat")
    args = ap.parse_args()

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    tmp = Path("/tmp")
    air_f = tmp / "ncep_air_ltm.nc"
    prate_f = tmp / "ncep_prate_ltm.nc"

    print("step 1/4 — downloading NCEP climatologies")
    download(AIR_URL, air_f)
    download(PRATE_URL, prate_f)

    print("step 2/4 — loading")
    air_units = b""
    prate_units = b""
    with h5py.File(air_f, "r") as d:
        lat = np.array(d["lat"], dtype=np.float64)          # 73, 90..-90
        lon = np.array(d["lon"], dtype=np.float64)          # 144, 0..357.5
        air = np.array(d["air"], dtype=np.float64)          # (12, 73, 144)
        air_units = d["air"].attrs.get("units", b"")
        print("  air units attr:", air_units[:40])
    with h5py.File(prate_f, "r") as d:
        plat = np.array(d["lat"], dtype=np.float64)         # 94 (Gaussian)
        plon = np.array(d["lon"], dtype=np.float64)         # 192
        prate = np.array(d["prate"], dtype=np.float64)      # (12, 94, 192)
        prate_units = d["prate"].attrs.get("units", b"")
        print("  prate units attr:", prate_units[:40])

    # NCEP air is Kelvin; precipitation is kg/m²/s (≈ mm/s).
    if b"K" in air_units or air.min() > 150.0:
        print("  converting air Kelvin → °C")
        air = air - 273.15
    if b"mm/s" in prate_units or b"kg" in prate_units or prate.max() < 0.01:
        print("  converting prate mm/s → mm/day")
        prate = prate * 86400.0

    print("step 3/4 — regridding precipitation onto the 2.5° grid")
    t_mon = np.zeros((N_MON, N_LAT, N_LON))
    p_mon = np.zeros((N_MON, N_LAT, N_LON))
    for m in range(N_MON):
        for j in range(N_LAT):
            qlat = lat[j]
            for i in range(N_LON):
                qlon = lon[i]
                t_mon[m, j, i] = air[m, j, i]
                p_mon[m, j, i] = bilinear_sample(plat, plon, prate[m], qlat, qlon)

    # clamp regridding artefacts (tiny negatives at polar cells)
    p_mon = np.maximum(p_mon, 0.0)
    t_ann = t_mon.mean(axis=0)
    p_ann = p_mon.mean(axis=0)

    print("step 4/4 — writing", out)
    with open(out, "wb") as f:
        f.write(MAGIC)
        f.write(struct.pack("<HH", N_LAT, N_LON))
        f.write(np.round(t_ann * 100.0).astype("<i2").tobytes())
        f.write(np.round(p_ann * 100.0).astype("<u2").tobytes())
        f.write(np.round(t_mon * 100.0).astype("<i2").tobytes())
        f.write(np.round(p_mon * 100.0).astype("<u2").tobytes())

    print(f"  wrote {out} ({out.stat().st_size} bytes)")
    print(f"  T range: {t_ann.min():.1f}..{t_ann.max():.1f} °C  (annual mean)")
    print(f"  P range: {p_ann.min():.2f}..{p_ann.max():.2f} mm/day")
    # sanity spot-checks against well-known real climates
    def spot(latq, lonq, name):
        t = bilinear_sample(lat, lon, t_ann, latq, lonq)
        p = bilinear_sample(lat, lon, p_ann, latq, lonq)
        print(f"  spot {name:14s} ({latq:5.1f},{lonq:6.1f}): {t:5.1f}°C, {p:4.2f} mm/day")
    spot(24.0, 15.0, "Sahara")
    spot(-3.0, -60.0, "Amazon")
    spot(60.0, 90.0, "Siberia")
    spot(51.0, 0.0, "London")
    spot(-23.0, -43.0, "Rio")
    return 0


if __name__ == "__main__":
    sys.exit(main())
