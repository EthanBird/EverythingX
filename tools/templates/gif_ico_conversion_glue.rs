
fn native_to_image(native: legacy_native::Image, source_channels: u8) -> Image {
    Image {
        width: native.width,
        height: native.height,
        source_channels,
        pixels: native
            .pixels
            .into_iter()
            .map(|pixel| Pixel { r: pixel.r, g: pixel.g, b: pixel.b, a: pixel.a })
            .collect(),
        warnings: Vec::new(),
    }
}

fn image_to_native(image: &Image) -> legacy_native::Image {
    legacy_native::Image {
        width: image.width,
        height: image.height,
        pixels: image
            .pixels
            .iter()
            .map(|pixel| legacy_native::Pixel { r: pixel.r, g: pixel.g, b: pixel.b, a: pixel.a })
            .collect(),
    }
}

fn legacy_error(error: legacy_native::Error) -> Error {
    match error {
        legacy_native::Error::Limit("pixel count") => Error::PixelLimitExceeded {
            pixels: DEFAULT_MAX_PIXELS.saturating_add(1),
            limit: DEFAULT_MAX_PIXELS,
        },
        other => Error::Legacy(other.to_string()),
    }
}

fn decode_gif(bytes: &[u8], options: &Options) -> Result<Image, Error> {
    let file = legacy_native::decode_gif(
        bytes,
        options.max_pixels,
        options.max_frames,
        options.max_input_bytes as usize,
    )
    .map_err(legacy_error)?;
    if file.frames.len() != 1 {
        return Err(Error::Unsupported(
            "animated GIF requires an explicit frame or sprite-sheet Capsule",
        ));
    }
    let frame = file.frames.into_iter().next().ok_or(Error::InvalidHeader("GIF has no frame"))?;
    Ok(native_to_image(frame.image, 4))
}

fn encode_gif(image: &Image) -> Result<Encoded, Error> {
    let bytes = legacy_native::encode_gif(&image_to_native(image)).map_err(legacy_error)?;
    let alpha = image.pixels.iter().any(|pixel| pixel.a != 255);
    Ok(Encoded {
        bytes,
        channels: if alpha { 4 } else { 3 },
        alpha_action: "preserved-for-binary-alpha-exact-palette",
    })
}

fn decode_icon(bytes: &[u8], options: &Options, kind: legacy_native::IconKind) -> Result<Image, Error> {
    let members = legacy_native::parse_icon(bytes, kind, options.max_members).map_err(legacy_error)?;
    let selected = legacy_native::select_best(&members).map_err(legacy_error)?;
    let mut image = if selected.png {
        let decoded = png_native::decode(
            selected.payload,
            &png_native::DecodeOptions {
                max_pixels: options.max_pixels,
                max_inflate_bytes: options.max_input_bytes,
                strict_crc: true,
                strict_trailing_data: true,
            },
        )
        .map_err(|error| match error {
            png_native::Error::Limit("pixel count") => Error::PixelLimitExceeded {
                pixels: options.max_pixels.saturating_add(1),
                limit: options.max_pixels,
            },
            other => Error::Png(other.to_string()),
        })?;
        Image {
            width: decoded.width,
            height: decoded.height,
            source_channels: decoded.source_channels,
            pixels: decoded
                .pixels
                .into_iter()
                .map(|pixel| Pixel {
                    r: ((pixel.r as u32 + 128) / 257) as u8,
                    g: ((pixel.g as u32 + 128) / 257) as u8,
                    b: ((pixel.b as u32 + 128) / 257) as u8,
                    a: ((pixel.a as u32 + 128) / 257) as u8,
                })
                .collect(),
            warnings: Vec::new(),
        }
    } else {
        native_to_image(
            legacy_native::decode_dib(selected.payload, options.max_pixels).map_err(legacy_error)?,
            4,
        )
    };
    if image.width != selected.width || image.height != selected.height {
        return Err(Error::InvalidHeader("ICO/CUR directory dimensions disagree with selected member"));
    }
    image.warnings.push(format!(
        "selected best member from {} entries: {}x{}, {} payload",
        members.len(),
        selected.width,
        selected.height,
        if selected.png { "PNG" } else { "DIB" }
    ));
    Ok(image)
}

fn encode_icon(
    image: &Image,
    options: &Options,
    kind: legacy_native::IconKind,
) -> Result<Encoded, Error> {
    let png = encode_png(image)?;
    let bytes = legacy_native::encode_icon(
        &png.bytes,
        image.width,
        image.height,
        kind,
        options.cursor_hotspot_x,
        options.cursor_hotspot_y,
    )
    .map_err(legacy_error)?;
    Ok(Encoded {
        bytes,
        channels: png.channels,
        alpha_action: "preserved-in-single-png-member",
    })
}
