//! Encoding. Inputs are always L8, La8, Rgb8 or Rgba8 (see `decode::normalise`).

use std::io::Cursor;
use std::panic::{catch_unwind, AssertUnwindSafe};

use anyhow::{anyhow, bail, Result};
use image::codecs::gif::{GifEncoder, Repeat};
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::metadata::LoopCount;
use image::{DynamicImage, Frame, GenericImageView, GrayImage, ImageEncoder, ImageFormat, RgbImage};

use crate::Codec;

pub fn encode(img: &DynamicImage, codec: Codec, quality: u8) -> Result<Vec<u8>> {
    match codec {
        Codec::Jpeg => jpeg(img, quality),
        Codec::Png => png(img),
        Codec::Webp => webp(img, quality),
        Codec::Gif => gif(vec![Frame::new(img.to_rgba8())], LoopCount::Infinite),
        Codec::Tiff => write_with(img, ImageFormat::Tiff),
        Codec::Bmp => write_with(img, ImageFormat::Bmp),
    }
}

fn jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    // JPEG has no alpha: flatten onto white. Greyscale stays greyscale (smaller).
    let (space, w, h, pixels) = match img {
        DynamicImage::ImageLuma8(g) => (
            mozjpeg::ColorSpace::JCS_GRAYSCALE,
            g.width(),
            g.height(),
            g.as_raw().clone(),
        ),
        DynamicImage::ImageLumaA8(g) => {
            let flat = GrayImage::from_fn(g.width(), g.height(), |x, y| {
                let p = g.get_pixel(x, y).0;
                image::Luma([over_white(p[0], p[1])])
            });
            (
                mozjpeg::ColorSpace::JCS_GRAYSCALE,
                flat.width(),
                flat.height(),
                flat.into_raw(),
            )
        }
        DynamicImage::ImageRgba8(rgba) => {
            let flat = RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
                let [r, g, b, a] = rgba.get_pixel(x, y).0;
                image::Rgb([over_white(r, a), over_white(g, a), over_white(b, a)])
            });
            (
                mozjpeg::ColorSpace::JCS_RGB,
                flat.width(),
                flat.height(),
                flat.into_raw(),
            )
        }
        other => {
            let rgb = other.to_rgb8();
            (mozjpeg::ColorSpace::JCS_RGB, rgb.width(), rgb.height(), rgb.into_raw())
        }
    };
    // libjpeg reports errors by unwinding; keep that inside this function.
    let result = catch_unwind(AssertUnwindSafe(|| -> std::io::Result<Vec<u8>> {
        let mut c = mozjpeg::Compress::new(space);
        c.set_size(w as usize, h as usize);
        c.set_quality(quality as f32);
        c.set_progressive_mode();
        c.set_optimize_coding(true);
        if space == mozjpeg::ColorSpace::JCS_RGB {
            // Full-resolution colour at high quality keeps red/blue edges crisp.
            let chroma = if quality >= 90 { (1, 1) } else { (2, 2) };
            c.set_chroma_sampling_pixel_sizes(chroma, chroma);
        }
        let mut started = c.start_compress(Vec::new())?;
        started.write_scanlines(&pixels)?;
        started.finish()
    }));
    match result {
        Ok(Ok(data)) => Ok(data),
        Ok(Err(e)) => Err(anyhow!("JPEG encoding failed: {e}")),
        Err(_) => Err(anyhow!("JPEG encoding failed")),
    }
}

fn over_white(c: u8, a: u8) -> u8 {
    let (c, a) = (c as u32, a as u32);
    ((c * a + 255 * (255 - a) + 127) / 255) as u8
}

/// Fast zlib PNG for the preview pane.
pub fn png_fast(img: &DynamicImage) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let (w, h) = img.dimensions();
    PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::Adaptive).write_image(
        img.as_bytes(),
        w,
        h,
        img.color().into(),
    )?;
    Ok(out)
}

/// PNG is lossless, so "compression" means oxipng: it tries filter/deflate
/// combinations and lossless reductions (palette, greyscale, bit depth).
fn png(img: &DynamicImage) -> Result<Vec<u8>> {
    let raw = png_fast(img)?;
    Ok(oxipng::optimize_from_memory(&raw, &oxipng::Options::from_preset(2)).unwrap_or(raw))
}

fn webp_config(quality: u8) -> Result<webp::WebPConfig> {
    let mut config = webp::WebPConfig::new().map_err(|_| anyhow!("WebP encoder unavailable"))?;
    config.method = 6;
    config.quality = quality as f32;
    if quality >= 100 {
        config.lossless = 1;
    }
    Ok(config)
}

/// WebP's hard limit on each side, in pixels.
const WEBP_MAX_SIDE: u32 = 16383;

fn check_webp_size(w: u32, h: u32) -> Result<()> {
    if w > WEBP_MAX_SIDE || h > WEBP_MAX_SIDE {
        bail!("{w} x {h} is too big for WebP (max {WEBP_MAX_SIDE} px per side): set a width or height, or choose another format");
    }
    Ok(())
}

fn webp(img: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    let (w, h) = img.dimensions();
    check_webp_size(w, h)?;
    let (layout, pixels) = if img.color().has_alpha() {
        (webp::PixelLayout::Rgba, img.to_rgba8().into_raw())
    } else {
        (webp::PixelLayout::Rgb, img.to_rgb8().into_raw())
    };
    let mem = webp::Encoder::new(&pixels, layout, w, h)
        .encode_advanced(&webp_config(quality)?)
        .map_err(|e| anyhow!("WebP encoding failed: {e:?}"))?;
    Ok(mem.to_vec())
}

/// Frame duration as browsers play it: GIF delays of 10 ms or less (including
/// 0) are shown for 100 ms. Writing that duration into the WebP keeps a
/// converted GIF playing at the speed people saw it at.
fn delay_ms(frame: &Frame) -> u32 {
    let (num, den) = frame.delay().numer_denom_ms();
    match num / den.max(1) {
        ms if ms <= 10 => 100,
        ms => ms,
    }
}

/// Animated WebP. All frames must be the same size (they are: composited
/// canvases with the same crop and resize applied).
pub fn webp_animated(frames: &[Frame], loop_count: LoopCount, quality: u8) -> Result<Vec<u8>> {
    let first = frames.first().ok_or_else(|| anyhow!("animation has no frames"))?;
    let (w, h) = first.buffer().dimensions();
    check_webp_size(w, h)?;
    let config = webp_config(quality)?;
    let mut enc = webp::AnimEncoder::new(w, h, &config);
    enc.set_loop_count(match loop_count {
        LoopCount::Infinite => 0,
        LoopCount::Finite(n) => i32::try_from(n.get()).unwrap_or(0),
    });
    let mut timestamp: i32 = 0;
    for frame in frames {
        enc.add_frame(webp::AnimFrame::from_rgba(frame.buffer().as_raw(), w, h, timestamp));
        timestamp = timestamp.saturating_add(i32::try_from(delay_ms(frame)).unwrap_or(i32::MAX));
    }
    let mem = enc
        .try_encode()
        .map_err(|e| anyhow!("WebP animation encoding failed: {e:?}"))?;
    Ok(mem.to_vec())
}

/// GIF, animated when there is more than one frame.
pub fn gif(frames: Vec<Frame>, loop_count: LoopCount) -> Result<Vec<u8>> {
    let animated = frames.len() > 1;
    let mut out = Vec::new();
    {
        let mut enc = GifEncoder::new_with_speed(&mut out, 10);
        if animated {
            enc.set_repeat(match loop_count {
                LoopCount::Infinite => Repeat::Infinite,
                LoopCount::Finite(n) => Repeat::Finite(u16::try_from(n.get()).unwrap_or(u16::MAX)),
            })?;
        }
        enc.encode_frames(frames)?;
    }
    Ok(out)
}

fn write_with(img: &DynamicImage, format: ImageFormat) -> Result<Vec<u8>> {
    // The TIFF encoder has no grey+alpha mode; widen it.
    let widened;
    let img = if format == ImageFormat::Tiff && matches!(img, DynamicImage::ImageLumaA8(_)) {
        widened = DynamicImage::ImageRgba8(img.to_rgba8());
        &widened
    } else {
        img
    };
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, format)?;
    Ok(out.into_inner())
}
