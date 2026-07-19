
fn decode_png(bytes: &[u8], options: &Options) -> Result<Image, Error> {
    let decoded = png_native::decode(bytes, &png_native::DecodeOptions {
        max_pixels: options.max_pixels,
        max_inflate_bytes: options.max_input_bytes,
        strict_crc: true,
        strict_trailing_data: options.strict_trailing_data,
    }).map_err(|error| match error {
        png_native::Error::Limit("pixel count") => Error::PixelLimitExceeded {
            pixels: options.max_pixels.saturating_add(1),
            limit: options.max_pixels,
        },
        other => Error::Png(other.to_string()),
    })?;
    if decoded.source_bit_depth == 16 && !options.allow_sample_scaling {
        return Err(Error::Unsupported("16-bit PNG to an 8-bit carrier requires allow_sample_scaling"));
    }
    let mut warnings = decoded.warnings;
    if decoded.source_bit_depth == 16 {
        warnings.push("PNG 16-bit samples explicitly scaled to 8-bit code values".into());
    }
    let pixels = decoded.pixels.into_iter().map(|pixel| Pixel {
        r: ((pixel.r as u32 + 128) / 257) as u8,
        g: ((pixel.g as u32 + 128) / 257) as u8,
        b: ((pixel.b as u32 + 128) / 257) as u8,
        a: ((pixel.a as u32 + 128) / 257) as u8,
    }).collect();
    Ok(Image { width: decoded.width, height: decoded.height, source_channels: decoded.source_channels, pixels, warnings })
}

fn encode_png(image: &Image) -> Result<Encoded, Error> {
    let alpha = image.pixels.iter().any(|pixel| pixel.a != 255);
    let native = png_native::Image {
        width: image.width,
        height: image.height,
        source_channels: if alpha { 4 } else { 3 },
        source_bit_depth: 8,
        source_color_type: if alpha { 6 } else { 2 },
        interlaced: false,
        pixels: image.pixels.iter().map(|pixel| png_native::Pixel16 {
            r: pixel.r as u16 * 257,
            g: pixel.g as u16 * 257,
            b: pixel.b as u16 * 257,
            a: pixel.a as u16 * 257,
        }).collect(),
        warnings: Vec::new(),
    };
    let bytes = png_native::encode(&native, png_native::Filter::Adaptive).map_err(|error| Error::Png(error.to_string()))?;
    Ok(Encoded { bytes, channels: if alpha { 4 } else { 3 }, alpha_action: "preserved" })
}
