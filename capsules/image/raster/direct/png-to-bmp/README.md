# png-to-bmp

Independent, zero-dependency Rust conversion from Portable Network Graphics to Windows BMP raster. PNG parsing covers all legal color types and depths, all Deflate block types, five filters and Adam7. A 16-bit PNG source requires the explicit `allow_sample_scaling` option when targeting an 8-bit carrier.
