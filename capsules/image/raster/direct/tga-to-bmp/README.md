# tga-to-bmp

Independent, zero-dependency Rust conversion from Truevision TGA raster to
Windows BMP raster. The directory can be copied out of EverythingX and built
or tested on its own. It contains its own parser, encoder, options, errors,
report, conformance fixture and runnable defaults; `everythingx/` is optional.

Version 0.1 targets the Raster Wave A RGBA8/RGB8 domain. It preserves accepted
pixel code values and coordinates exactly. PPM transparency is rejected by
default and requires an explicit lossy policy to discard or composite alpha.
