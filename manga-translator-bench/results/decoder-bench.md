# Decode benchmark — gdk-pixbuf vs glycin vs image-rs

Times are wall-clock medians; input comes from disk with a warm page cache.
`out` is the produced pixel buffer: `bgra8-premult` is texture-ready for GTK/Cairo,
`rgba8`/`native` are the decoders' own output formats.
`vs gdk` compares against `gdk-pixbuf → BGRA premult` for the same file.

| | |
|---|---|
| CPU | AMD Ryzen 9 7950X3D 16-Core Processor |
| rustc 1.99.0 (b940084d7 2026-09-28) | — |
| gdk-pixbuf | 2.44.8 |
| glycin | crate 4.0.5, system loaders 2.2.5 (bwrap present) |
| image-rs | 0.25 |

**Cold start** (first decode in a fresh process, `full_q90.jpg`): glycin **166 ms** (ok=true), gdk-pixbuf **45 ms** (ok=true).


### Decode

| Decoder | File | Input | out | med ms | min ms | p95 ms | MPix/s | in MB/s | vs gdk |
|---|---|---:|---|---:|---:|---:|---:|---:|---:|
| gdk-pixbuf → BGRA premult (current) | crop.exr | 8.65 MB | bgra8-premult | 32.75 | 32.36 | 33.17 | 33.0 | 264.3 | +0 % |
| gdk-pixbuf (decode only) | crop.exr | 8.65 MB | pixbuf | 31.89 | 31.48 | 32.35 | 33.9 | 271.4 | -3 % |
| image-rs | crop.exr | 8.65 MB | rgba8 | 14.32 | 14.22 | 14.45 | 75.4 | 604.3 | -56 % |
| image-rs → BGRA premult | crop.exr | 8.65 MB | bgra8-premult | 15.07 | 14.97 | 15.26 | 71.7 | 574.5 | -54 % |
| glycin (sandboxed) | crop.exr | 8.65 MB | bgra8-premult | 44.85 | 44.35 | 45.97 | 24.1 | 193.0 | +37 % |
| glycin (native format) | crop.exr | 8.65 MB | native | 14.64 | 14.46 | 15.47 | 73.8 | 591.1 | -55 % |
| exr crate | crop.exr | 8.65 MB | native | 1.35 | 1.33 | 1.37 | 801.6 | 6423.5 | -96 % |
| gdk-pixbuf → BGRA premult (current) | full.bmp | 31.19 MB | bgra8-premult | 49.43 | 49.06 | 49.88 | 157.8 | 631.0 | +0 % |
| gdk-pixbuf (decode only) | full.bmp | 31.19 MB | pixbuf | 41.86 | 41.48 | 42.37 | 186.3 | 745.2 | -15 % |
| image-rs | full.bmp | 31.19 MB | rgba8 | 18.71 | 18.54 | 18.92 | 416.7 | 1666.9 | -62 % |
| image-rs → BGRA premult | full.bmp | 31.19 MB | bgra8-premult | 24.75 | 24.32 | 24.93 | 315.1 | 1260.3 | -50 % |
| glycin (sandboxed) | full.bmp | 31.19 MB | bgra8-premult | 184.59 | 184.39 | 185.05 | 42.2 | 169.0 | +273 % |
| glycin (native format) | full.bmp | 31.19 MB | native | 49.10 | 47.16 | 49.52 | 158.8 | 635.3 | -1 % |
| gdk-pixbuf → BGRA premult (current) | full.gif | 1.59 MB | bgra8-premult | 59.25 | 58.92 | 59.58 | 131.6 | 26.9 | +0 % |
| gdk-pixbuf (decode only) | full.gif | 1.59 MB | pixbuf | 50.78 | 50.36 | 51.17 | 153.6 | 31.3 | -14 % |
| image-rs | full.gif | 1.59 MB | rgba8 | 23.43 | 23.37 | 23.50 | 332.9 | 67.9 | -60 % |
| image-rs → BGRA premult | full.gif | 1.59 MB | bgra8-premult | 29.46 | 29.05 | 29.60 | 264.7 | 54.0 | -50 % |
| glycin (sandboxed) | full.gif | 1.59 MB | bgra8-premult | 186.48 | 186.06 | 186.70 | 41.8 | 8.5 | +215 % |
| glycin (native format) | full.gif | 1.59 MB | native | 51.33 | 50.97 | 51.57 | 151.9 | 31.0 | -13 % |
| gif crate | full.gif | 1.59 MB | native | 21.01 | 20.95 | 21.07 | 371.2 | 75.7 | -65 % |
| glycin (sandboxed) | full.jp2 | 5.31 MB | bgra8-premult | 1214.67 | 1213.56 | 1215.56 | 6.4 | 4.4 | — |
| glycin (native format) | full.jp2 | 5.31 MB | native | 1083.73 | 1079.61 | 1100.25 | 7.2 | 4.9 | — |
| hayro-jpeg2000 | full.jp2 | 5.31 MB | native | 822.20 | 820.97 | 825.09 | 9.5 | 6.5 | — |
| gdk-pixbuf → BGRA premult (current) | full.jxl | 0.82 MB | bgra8-premult | 283.98 | 283.31 | 286.73 | 27.5 | 2.9 | +0 % |
| gdk-pixbuf (decode only) | full.jxl | 0.82 MB | pixbuf | 275.67 | 274.92 | 276.49 | 28.3 | 3.0 | -3 % |
| glycin (sandboxed) | full.jxl | 0.82 MB | bgra8-premult | 405.59 | 404.95 | 414.03 | 19.2 | 2.0 | +43 % |
| glycin (native format) | full.jxl | 0.82 MB | native | 272.91 | 271.53 | 273.08 | 28.6 | 3.0 | -4 % |
| gdk-pixbuf → BGRA premult (current) | full.png | 4.09 MB | bgra8-premult | 54.27 | 54.03 | 56.26 | 143.7 | 75.3 | +0 % |
| gdk-pixbuf (decode only) | full.png | 4.09 MB | pixbuf | 46.78 | 46.66 | 47.26 | 166.7 | 87.4 | -14 % |
| image-rs | full.png | 4.09 MB | rgba8 | 30.98 | 30.88 | 31.05 | 251.7 | 132.0 | -43 % |
| image-rs → BGRA premult | full.png | 4.09 MB | bgra8-premult | 36.54 | 36.29 | 37.07 | 213.4 | 111.9 | -33 % |
| glycin (sandboxed) | full.png | 4.09 MB | bgra8-premult | 182.71 | 182.62 | 182.86 | 42.7 | 22.4 | +237 % |
| glycin (native format) | full.png | 4.09 MB | native | 47.91 | 47.81 | 48.41 | 162.8 | 85.3 | -12 % |
| png crate | full.png | 4.09 MB | native | 29.16 | 29.11 | 29.24 | 267.4 | 140.2 | -46 % |
| gdk-pixbuf → BGRA premult (current) | full.qoi | 4.04 MB | bgra8-premult | 35.60 | 35.28 | 36.07 | 219.0 | 113.4 | +0 % |
| gdk-pixbuf (decode only) | full.qoi | 4.04 MB | pixbuf | 27.78 | 27.61 | 28.45 | 280.7 | 145.3 | -22 % |
| image-rs | full.qoi | 4.04 MB | rgba8 | 24.76 | 24.63 | 24.86 | 314.9 | 163.0 | -30 % |
| image-rs → BGRA premult | full.qoi | 4.04 MB | bgra8-premult | 30.42 | 30.27 | 30.59 | 256.3 | 132.7 | -15 % |
| glycin (sandboxed) | full.qoi | 4.04 MB | bgra8-premult | 163.41 | 163.20 | 163.53 | 47.7 | 24.7 | +359 % |
| glycin (native format) | full.qoi | 4.04 MB | native | 28.82 | 26.78 | 29.87 | 270.6 | 140.0 | -19 % |
| qoi crate | full.qoi | 4.04 MB | native | 10.87 | 10.83 | 10.89 | 717.5 | 371.3 | -69 % |
| gdk-pixbuf → BGRA premult (current) | full.tiff | 4.63 MB | bgra8-premult | 87.00 | 86.77 | 88.11 | 89.6 | 53.2 | +0 % |
| gdk-pixbuf (decode only) | full.tiff | 4.63 MB | pixbuf | 80.10 | 79.47 | 82.49 | 97.3 | 57.7 | -8 % |
| image-rs | full.tiff | 4.63 MB | rgba8 | 46.35 | 46.27 | 46.46 | 168.2 | 99.8 | -47 % |
| image-rs → BGRA premult | full.tiff | 4.63 MB | bgra8-premult | 51.76 | 51.67 | 51.90 | 150.7 | 89.4 | -41 % |
| glycin (sandboxed) | full.tiff | 4.63 MB | bgra8-premult | 216.10 | 215.58 | 217.26 | 36.1 | 21.4 | +148 % |
| glycin (native format) | full.tiff | 4.63 MB | native | 82.35 | 81.14 | 82.90 | 94.7 | 56.2 | -5 % |
| tiff crate | full.tiff | 4.63 MB | native | 43.92 | 43.87 | 44.21 | 177.6 | 105.3 | -50 % |
| gdk-pixbuf → BGRA premult (current) | full.webp | 0.66 MB | bgra8-premult | 120.57 | 119.68 | 122.77 | 64.7 | 5.5 | +0 % |
| gdk-pixbuf (decode only) | full.webp | 0.66 MB | pixbuf | 111.70 | 111.32 | 112.09 | 69.8 | 5.9 | -7 % |
| image-rs | full.webp | 0.66 MB | rgba8 | 113.03 | 112.86 | 115.06 | 69.0 | 5.8 | -6 % |
| image-rs → BGRA premult | full.webp | 0.66 MB | bgra8-premult | 117.89 | 117.79 | 118.22 | 66.1 | 5.6 | -2 % |
| glycin (sandboxed) | full.webp | 0.66 MB | bgra8-premult | 248.42 | 248.11 | 248.89 | 31.4 | 2.7 | +106 % |
| glycin (native format) | full.webp | 0.66 MB | native | 112.94 | 112.71 | 113.45 | 69.0 | 5.8 | -6 % |
| image-webp | full.webp | 0.66 MB | native | 112.29 | 111.65 | 112.49 | 69.4 | 5.9 | -7 % |
| gdk-pixbuf → BGRA premult (current) | full_lossless.webp | 1.78 MB | bgra8-premult | 64.74 | 64.53 | 65.19 | 120.4 | 27.5 | +0 % |
| gdk-pixbuf (decode only) | full_lossless.webp | 1.78 MB | pixbuf | 57.13 | 56.96 | 57.63 | 136.5 | 31.1 | -12 % |
| image-rs | full_lossless.webp | 1.78 MB | rgba8 | 44.08 | 43.99 | 44.14 | 176.9 | 40.3 | -32 % |
| image-rs → BGRA premult | full_lossless.webp | 1.78 MB | bgra8-premult | 51.24 | 51.13 | 51.59 | 152.2 | 34.7 | -21 % |
| glycin (sandboxed) | full_lossless.webp | 1.78 MB | bgra8-premult | 192.69 | 191.59 | 193.57 | 40.5 | 9.2 | +198 % |
| glycin (native format) | full_lossless.webp | 1.78 MB | native | 58.18 | 57.64 | 58.85 | 134.0 | 30.6 | -10 % |
| image-webp | full_lossless.webp | 1.78 MB | native | 43.35 | 43.29 | 43.50 | 179.9 | 41.0 | -33 % |
| gdk-pixbuf → BGRA premult (current) | full_q75.jpg | 0.79 MB | bgra8-premult | 30.95 | 29.94 | 31.36 | 252.0 | 25.5 | +0 % |
| gdk-pixbuf (decode only) | full_q75.jpg | 0.79 MB | pixbuf | 23.92 | 23.21 | 24.48 | 326.0 | 33.0 | -23 % |
| image-rs | full_q75.jpg | 0.79 MB | rgba8 | 15.47 | 15.08 | 15.66 | 504.2 | 51.0 | -50 % |
| image-rs → BGRA premult | full_q75.jpg | 0.79 MB | bgra8-premult | 21.94 | 21.82 | 21.99 | 355.5 | 36.0 | -29 % |
| glycin (sandboxed) | full_q75.jpg | 0.79 MB | bgra8-premult | 148.22 | 147.97 | 149.19 | 52.6 | 5.3 | +379 % |
| glycin (native format) | full_q75.jpg | 0.79 MB | native | 24.03 | 22.16 | 24.72 | 324.5 | 32.9 | -22 % |
| zune-jpeg | full_q75.jpg | 0.79 MB | native | 11.62 | 10.95 | 11.75 | 671.0 | 67.9 | -62 % |
| gdk-pixbuf → BGRA premult (current) | full_q90.jpg | 1.23 MB | bgra8-premult | 35.54 | 35.18 | 37.83 | 219.4 | 34.6 | +0 % |
| gdk-pixbuf (decode only) | full_q90.jpg | 1.23 MB | pixbuf | 28.35 | 28.02 | 29.02 | 275.1 | 43.4 | -20 % |
| image-rs | full_q90.jpg | 1.23 MB | rgba8 | 19.96 | 19.60 | 20.13 | 390.6 | 61.6 | -44 % |
| image-rs → BGRA premult | full_q90.jpg | 1.23 MB | bgra8-premult | 26.78 | 26.49 | 26.95 | 291.2 | 46.0 | -25 % |
| glycin (sandboxed) | full_q90.jpg | 1.23 MB | bgra8-premult | 153.58 | 153.45 | 153.99 | 50.8 | 8.0 | +332 % |
| glycin (native format) | full_q90.jpg | 1.23 MB | native | 29.20 | 27.81 | 29.87 | 267.1 | 42.1 | -18 % |
| zune-jpeg | full_q90.jpg | 1.23 MB | native | 15.83 | 15.40 | 15.96 | 492.6 | 77.7 | -55 % |
| gdk-pixbuf → BGRA premult (current) | page.png | 1.57 MB | bgra8-premult | 24.80 | 24.38 | 25.40 | 157.2 | 63.5 | +0 % |
| gdk-pixbuf (decode only) | page.png | 1.57 MB | pixbuf | 21.77 | 20.99 | 22.12 | 179.1 | 72.3 | -12 % |
| image-rs | page.png | 1.57 MB | rgba8 | 12.89 | 12.84 | 12.97 | 302.5 | 122.2 | -48 % |
| image-rs → BGRA premult | page.png | 1.57 MB | bgra8-premult | 15.46 | 15.41 | 15.54 | 252.2 | 101.9 | -38 % |
| glycin (sandboxed) | page.png | 1.57 MB | bgra8-premult | 88.71 | 88.60 | 88.79 | 44.0 | 17.8 | +258 % |
| glycin (native format) | page.png | 1.57 MB | native | 21.83 | 21.56 | 22.80 | 178.6 | 72.1 | -12 % |
| png crate | page.png | 1.57 MB | native | 12.65 | 12.60 | 12.71 | 308.2 | 124.5 | -49 % |
| gdk-pixbuf → BGRA premult (current) | page.webp | 0.29 MB | bgra8-premult | 41.45 | 40.60 | 41.64 | 94.1 | 7.0 | +0 % |
| gdk-pixbuf (decode only) | page.webp | 0.29 MB | pixbuf | 37.38 | 37.20 | 38.17 | 104.3 | 7.7 | -10 % |
| image-rs | page.webp | 0.29 MB | rgba8 | 36.81 | 36.74 | 36.89 | 105.9 | 7.8 | -11 % |
| image-rs → BGRA premult | page.webp | 0.29 MB | bgra8-premult | 39.34 | 39.30 | 39.43 | 99.1 | 7.3 | -5 % |
| glycin (sandboxed) | page.webp | 0.29 MB | bgra8-premult | 99.48 | 99.40 | 99.63 | 39.2 | 2.9 | +140 % |
| glycin (native format) | page.webp | 0.29 MB | native | 37.54 | 36.65 | 38.88 | 103.9 | 7.7 | -9 % |
| image-webp | page.webp | 0.29 MB | native | 35.21 | 35.12 | 35.29 | 110.7 | 8.2 | -15 % |
| gdk-pixbuf → BGRA premult (current) | page_q90.jpg | 0.58 MB | bgra8-premult | 17.72 | 16.88 | 18.15 | 220.0 | 32.8 | +0 % |
| gdk-pixbuf (decode only) | page_q90.jpg | 0.58 MB | pixbuf | 14.12 | 13.84 | 14.96 | 276.2 | 41.1 | -20 % |
| image-rs | page_q90.jpg | 0.58 MB | rgba8 | 9.55 | 9.42 | 9.71 | 408.3 | 60.8 | -46 % |
| image-rs → BGRA premult | page_q90.jpg | 0.58 MB | bgra8-premult | 12.00 | 11.94 | 12.11 | 324.8 | 48.4 | -32 % |
| glycin (sandboxed) | page_q90.jpg | 0.58 MB | bgra8-premult | 76.23 | 76.11 | 76.42 | 51.1 | 7.6 | +330 % |
| glycin (native format) | page_q90.jpg | 0.58 MB | native | 14.93 | 14.21 | 15.23 | 261.1 | 38.9 | -16 % |
| zune-jpeg | page_q90.jpg | 0.58 MB | native | 7.72 | 7.63 | 7.74 | 505.0 | 75.2 | -56 % |
| gdk-pixbuf → BGRA premult (current) | small.png | 0.46 MB | bgra8-premult | 5.22 | 5.07 | 6.27 | 117.0 | 87.5 | +0 % |
| gdk-pixbuf (decode only) | small.png | 0.46 MB | pixbuf | 4.60 | 4.54 | 5.59 | 132.7 | 99.3 | -12 % |
| image-rs | small.png | 0.46 MB | rgba8 | 2.76 | 2.75 | 2.78 | 221.4 | 165.6 | -47 % |
| image-rs → BGRA premult | small.png | 0.46 MB | bgra8-premult | 3.17 | 3.16 | 3.18 | 192.9 | 144.3 | -39 % |
| glycin (sandboxed) | small.png | 0.46 MB | bgra8-premult | 15.30 | 15.15 | 15.38 | 39.9 | 29.9 | +193 % |
| glycin (native format) | small.png | 0.46 MB | native | 4.68 | 4.40 | 5.66 | 130.4 | 97.5 | -10 % |
| png crate | small.png | 0.46 MB | native | 2.73 | 2.72 | 2.74 | 223.9 | 167.5 | -48 % |
| gdk-pixbuf → BGRA premult (current) | small.webp | 0.08 MB | bgra8-premult | 11.45 | 11.19 | 12.33 | 53.3 | 7.4 | +0 % |
| gdk-pixbuf (decode only) | small.webp | 0.08 MB | pixbuf | 10.81 | 10.66 | 11.80 | 56.5 | 7.8 | -6 % |
| image-rs | small.webp | 0.08 MB | rgba8 | 10.06 | 10.03 | 10.11 | 60.7 | 8.4 | -12 % |
| image-rs → BGRA premult | small.webp | 0.08 MB | bgra8-premult | 10.49 | 10.44 | 10.53 | 58.2 | 8.0 | -8 % |
| glycin (sandboxed) | small.webp | 0.08 MB | bgra8-premult | 21.43 | 21.31 | 21.61 | 28.5 | 3.9 | +87 % |
| glycin (native format) | small.webp | 0.08 MB | native | 10.75 | 10.49 | 11.73 | 56.8 | 7.8 | -6 % |
| image-webp | small.webp | 0.08 MB | native | 9.99 | 9.97 | 10.01 | 61.1 | 8.4 | -13 % |
| gdk-pixbuf → BGRA premult (current) | small_q90.jpg | 0.16 MB | bgra8-premult | 3.76 | 3.62 | 4.74 | 162.4 | 41.5 | +0 % |
| gdk-pixbuf (decode only) | small_q90.jpg | 0.16 MB | pixbuf | 3.20 | 3.00 | 4.24 | 190.6 | 48.7 | -15 % |
| image-rs | small_q90.jpg | 0.16 MB | rgba8 | 2.06 | 2.01 | 2.08 | 296.4 | 75.7 | -45 % |
| image-rs → BGRA premult | small_q90.jpg | 0.16 MB | bgra8-premult | 2.43 | 2.41 | 2.47 | 251.4 | 64.2 | -35 % |
| glycin (sandboxed) | small_q90.jpg | 0.16 MB | bgra8-premult | 13.01 | 12.88 | 13.11 | 47.0 | 12.0 | +246 % |
| glycin (native format) | small_q90.jpg | 0.16 MB | native | 3.17 | 2.96 | 4.25 | 192.6 | 49.2 | -16 % |
| zune-jpeg | small_q90.jpg | 0.16 MB | native | 1.66 | 1.60 | 1.66 | 368.7 | 94.2 | -56 % |
