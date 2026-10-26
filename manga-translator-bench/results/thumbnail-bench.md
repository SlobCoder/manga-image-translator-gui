# Thumbnail decode + cache encoding

Thumbnails are 112 px (the size the file browser requests) decoded from the full-size page.

| | |
|---|---|
| CPU | AMD Ryzen 9 7950X3D 16-Core Processor |
| rustc 1.99.0 (b940084d7 2026-09-28) | — |
| gdk-pixbuf | 2.44.8 |
| glycin | crate 4.0.5, system loaders 2.2.5 (bwrap present) |
| image-rs | 0.25 |


### Thumbnail decode (full page → 112 px)

| Decoder | File | Input | out | med ms | min ms | p95 ms | MPix/s | in MB/s | vs gdk |
|---|---|---:|---|---:|---:|---:|---:|---:|---:|
| gdk-pixbuf from_file_at_size | full.png | 4.09 MB | png bytes | 67.13 | 66.70 | 68.44 | 0.1 | 60.9 | +0 % |
| gdk-pixbuf from_file_at_size (no PNG) | full.png | 4.09 MB | pixbuf | 65.82 | 65.58 | 67.17 | 0.1 | 62.1 | -2 % |
| image-rs thumbnail | full.png | 4.09 MB | rgba8 | 35.51 | 35.34 | 35.55 | 0.2 | 115.2 | -47 % |
| glycin scale(112) → BGRA premult | full.png | 4.09 MB | bgra8-premult | 181.67 | 181.61 | 184.34 | 42.9 | 22.5 | +171 % |
| glycin scale(112) native | full.png | 4.09 MB | native | 48.06 | 45.74 | 49.14 | 162.3 | 85.1 | -28 % |
| gdk-pixbuf from_file_at_size | full_q90.jpg | 1.23 MB | png bytes | 54.65 | 54.26 | 56.46 | 0.1 | 22.5 | +0 % |
| gdk-pixbuf from_file_at_size (no PNG) | full_q90.jpg | 1.23 MB | pixbuf | 54.93 | 53.79 | 57.70 | 0.1 | 22.4 | +1 % |
| image-rs thumbnail | full_q90.jpg | 1.23 MB | rgba8 | 22.01 | 21.59 | 22.30 | 0.3 | 55.9 | -60 % |
| glycin scale(112) → BGRA premult | full_q90.jpg | 1.23 MB | bgra8-premult | 160.72 | 155.47 | 213.56 | 48.5 | 7.7 | +194 % |
| glycin scale(112) native | full_q90.jpg | 1.23 MB | native | 28.70 | 28.12 | 29.36 | 271.7 | 42.9 | -47 % |
| gdk-pixbuf from_file_at_size | full.webp | 0.66 MB | png bytes | 133.06 | 131.88 | 133.88 | 0.1 | 5.0 | +0 % |
| gdk-pixbuf from_file_at_size (no PNG) | full.webp | 0.66 MB | pixbuf | 130.70 | 129.71 | 131.73 | 0.1 | 5.1 | -2 % |
| image-rs thumbnail | full.webp | 0.66 MB | rgba8 | 118.73 | 118.45 | 118.82 | 0.1 | 5.6 | -11 % |
| glycin scale(112) → BGRA premult | full.webp | 0.66 MB | bgra8-premult | 246.97 | 246.53 | 247.35 | 31.6 | 2.7 | +86 % |
| glycin scale(112) native | full.webp | 0.66 MB | native | 112.20 | 111.89 | 112.62 | 69.5 | 5.9 | -16 % |
| gdk-pixbuf from_file_at_size | page_q90.jpg | 0.58 MB | png bytes | 21.70 | 20.52 | 22.88 | 0.5 | 26.8 | +0 % |
| gdk-pixbuf from_file_at_size (no PNG) | page_q90.jpg | 0.58 MB | pixbuf | 20.25 | 19.40 | 20.59 | 0.6 | 28.7 | -7 % |
| image-rs thumbnail | page_q90.jpg | 0.58 MB | rgba8 | 11.64 | 11.57 | 11.73 | 1.0 | 49.9 | -46 % |
| glycin scale(112) → BGRA premult | page_q90.jpg | 0.58 MB | bgra8-premult | 76.19 | 76.10 | 76.31 | 51.2 | 7.6 | +251 % |
| glycin scale(112) native | page_q90.jpg | 0.58 MB | native | 14.22 | 13.96 | 15.06 | 274.3 | 40.9 | -34 % |
| gdk-pixbuf from_file_at_size | small_q90.jpg | 0.16 MB | png bytes | 6.26 | 5.25 | 6.32 | 1.1 | 24.9 | +0 % |
| gdk-pixbuf from_file_at_size (no PNG) | small_q90.jpg | 0.16 MB | pixbuf | 4.52 | 4.46 | 5.50 | 1.5 | 34.5 | -28 % |
| image-rs thumbnail | small_q90.jpg | 0.16 MB | rgba8 | 2.62 | 2.60 | 2.66 | 2.6 | 59.5 | -58 % |
| glycin scale(112) → BGRA premult | small_q90.jpg | 0.16 MB | bgra8-premult | 13.08 | 12.95 | 13.16 | 46.7 | 11.9 | +109 % |
| glycin scale(112) native | small_q90.jpg | 0.16 MB | native | 3.18 | 3.02 | 4.21 | 191.9 | 49.0 | -49 % |

Thumbnail cache encoders, 112×112 RGBA:

### 

| Encoder | size | med ms | MPix/s | vs png Fast |
|---|---:|---:|---:|---:|
| png Fast (fdeflate) | 12.6 KiB | 0.023 | 294.3 | 1.0x |
| png Balanced | 9.0 KiB | 0.576 | 11.7 | 25.2x |
| png High | 9.0 KiB | 1.132 | 5.9 | 49.6x |
| png NoCompression | 26.4 KiB | 0.003 | 2055.0 | 0.1x |
| image PNG | 12.6 KiB | 0.023 | 298.4 | 1.0x |
| image JPEG q85 (RGB) | 3.5 KiB | 0.107 | 62.9 | 4.7x |
| image-webp lossless | 7.2 KiB | 0.041 | 164.9 | 1.8x |
| gif (speed 30) | 5.5 KiB | 0.237 | 28.4 | 10.4x |
| qoi | 9.3 KiB | 0.010 | 678.1 | 0.4x |
| tiff (deflate) | 8.9 KiB | 0.383 | 17.5 | 16.8x |
| memcpy (no cache encoding) | 26.2 KiB | 0.000 | 16800.0 | 0.0x |

Page encoders, 1064×1510 RGBA:

### 

| Encoder | size | med ms | MPix/s | vs png Fast |
|---|---:|---:|---:|---:|
| png Fast (fdeflate) | 578.8 KiB | 1.884 | 324.2 | 1.0x |
| png Balanced | 360.2 KiB | 42.281 | 14.4 | 22.4x |
| png High | 358.9 KiB | 83.763 | 7.3 | 44.5x |
| png NoCompression | 2386.5 KiB | 0.167 | 3652.9 | 0.1x |
| image PNG | 578.8 KiB | 1.873 | 326.0 | 1.0x |
| image JPEG q85 (RGB) | 134.0 KiB | 9.539 | 64.0 | 5.1x |
| image-webp lossless | 291.3 KiB | 2.050 | 297.9 | 1.1x |
| gif (speed 30) | 241.9 KiB | 18.473 | 33.1 | 9.8x |
| qoi | 436.8 KiB | 1.552 | 393.6 | 0.8x |
| tiff (deflate) | 375.1 KiB | 28.218 | 21.6 | 15.0x |
| memcpy (no cache encoding) | 2385.7 KiB | 0.044 | 13893.0 | 0.0x |
