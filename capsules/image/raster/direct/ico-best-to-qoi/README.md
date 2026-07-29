# ico-best-to-qoi

Independent, zero-dependency Rust conversion from best-ranked Windows ICO member to
Quite OK Image. GIF parsing covers LZW, interlace, global/local palettes,
transparency and animation disposal. ICO/CUR parsing validates the complete
directory and supports PNG plus common uncompressed DIB members.

Still-image GIF edges reject animation. ICO/CUR read edges explicitly select
the best member by area then bit depth and report that choice. GIF target edges
reject quantization, partial alpha and palettes above 256 colors.
