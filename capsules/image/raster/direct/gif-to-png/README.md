# gif-to-png

Independent, zero-dependency Rust conversion from GIF87a/GIF89a exact-palette still raster to
Portable Network Graphics. GIF parsing covers LZW, interlace, global/local palettes,
transparency and animation disposal. ICO/CUR parsing validates the complete
directory and supports PNG plus common uncompressed DIB members.

Still-image GIF edges reject animation. ICO/CUR read edges explicitly select
the best member by area then bit depth and report that choice. GIF target edges
reject quantization, partial alpha and palettes above 256 colors.
