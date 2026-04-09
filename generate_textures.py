#!/usr/bin/env python3
"""
Texture Generation Script for NV_ENGINE

This script generates missing or variant graphics by transforming existing textures.
Useful for creating face-up versions, rotations, or variations of block textures.

Usage:
    python generate_textures.py <input_png> <output_png> <transformation>

Transformations:
    - rotate90: Rotate 90 degrees clockwise
    - rotate180: Rotate 180 degrees
    - rotate270: Rotate 270 degrees
    - flip_h: Flip horizontally
    - flip_v: Flip vertically
    - grayscale: Convert to grayscale
    - invert: Invert colors
    - darken: Make darker (for bottom faces)
    - brighten: Make brighter (for top faces)

Example:
    python generate_textures.py Assets/Blocks/drewno_liscie_1.png Assets/Blocks/drewno_top.png rotate90
"""

from PIL import Image, ImageOps
import sys
import os

def apply_transformation(img, transform):
    if transform == 'rotate90':
        return img.rotate(-90)
    elif transform == 'rotate180':
        return img.rotate(180)
    elif transform == 'rotate270':
        return img.rotate(90)
    elif transform == 'flip_h':
        return ImageOps.mirror(img)
    elif transform == 'flip_v':
        return ImageOps.flip(img)
    elif transform == 'grayscale':
        return ImageOps.grayscale(img)
    elif transform == 'invert':
        return ImageOps.invert(img)
    elif transform == 'darken':
        # Darken by reducing brightness
        return Image.eval(img, lambda x: int(x * 0.7))
    elif transform == 'brighten':
        # Brighten by increasing brightness
        return Image.eval(img, lambda x: min(255, int(x * 1.3)))
    else:
        print(f"Unknown transformation: {transform}")
        return img

def main():
    if len(sys.argv) != 4:
        print("Usage: python generate_textures.py <input_png> <output_png> <transformation>")
        print("See script header for available transformations.")
        sys.exit(1)

    input_path = sys.argv[1]
    output_path = sys.argv[2]
    transform = sys.argv[3]

    if not os.path.exists(input_path):
        print(f"Input file does not exist: {input_path}")
        sys.exit(1)

    try:
        img = Image.open(input_path).convert('RGBA')
        transformed = apply_transformation(img, transform)
        transformed.save(output_path, 'PNG')
        print(f"Generated: {output_path} from {input_path} with {transform}")
    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()