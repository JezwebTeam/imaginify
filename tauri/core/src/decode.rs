//! Decoding: every input ends up upright, 8-bit, and in sRGB, so the encoders
//! only ever see L8, La8, Rgb8 or Rgba8.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use image::codecs::gif::GifDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::metadata::{LoopCount, Orientation};
use image::{AnimationDecoder, DynamicImage, Frame, ImageDecoder, ImageFormat, ImageReader, Limits};
use lcms2::{ColorSpaceSignature, Flags, InfoType, Intent, Locale, PixelFormat, Profile, Transform};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Jpeg,
    Png,
    Webp,
    Gif,
    Tiff,
    Bmp,
}

impl Codec {
    fn from_format(f: ImageFormat) -> Option<Self> {
        Some(match f {
            ImageFormat::Jpeg => Self::Jpeg,
            ImageFormat::Png => Self::Png,
            ImageFormat::WebP => Self::Webp,
            ImageFormat::Gif => Self::Gif,
            ImageFormat::Tiff => Self::Tiff,
            ImageFormat::Bmp => Self::Bmp,
            _ => return None,
        })
    }

    /// Output extension. TIFF keeps the source's spelling (`.tif` / `.tiff`).
    pub fn extension(self, src: &Path) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Gif => "gif",
            Self::Bmp => "bmp",
            Self::Tiff => {
                let src_ext = src.extension().and_then(|e| e.to_str()).unwrap_or("");
                if src_ext.eq_ignore_ascii_case("tiff") {
                    "tiff"
                } else {
                    "tif"
                }
            }
        }
    }
}

/// Cap on the decoded frames of one animation (RGBA bytes, all frames), so a
/// long or huge GIF/WebP/APNG fails with a message instead of exhausting memory.
/// Up to four images decode at once.
pub const MAX_ANIMATION_BYTES: usize = 512 * 1024 * 1024;

/// Cap on one decoded still image (about 268 MP as RGBA, 357 MP as RGB).
pub const MAX_STILL_BYTES: u64 = 1024 * 1024 * 1024;

pub struct Animation {
    /// Composited full-canvas RGBA frames.
    pub frames: Vec<Frame>,
    pub loop_count: LoopCount,
}

pub enum Pixels {
    Still(DynamicImage),
    Animated(Animation),
}

pub struct Decoded {
    pub pixels: Pixels,
    /// Format detected from the file's content, not its extension.
    pub codec: Codec,
}

/// Decode the whole image, every frame of an animation included.
pub fn decode(path: &Path) -> Result<Decoded> {
    decode_inner(path, false)
}

/// Decode only what the preview needs: the first frame of an animation.
pub fn decode_first_frame(path: &Path) -> Result<DynamicImage> {
    match decode_inner(path, true)?.pixels {
        Pixels::Still(img) => Ok(img),
        Pixels::Animated(anim) => first_frame(anim),
    }
}

pub fn first_frame(anim: Animation) -> Result<DynamicImage> {
    let frame = anim
        .frames
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("image has no frames"))?;
    Ok(DynamicImage::ImageRgba8(frame.into_buffer()))
}

fn decode_inner(path: &Path, first_only: bool) -> Result<Decoded> {
    let reader = ImageReader::open(path)
        .with_context(|| format!("cannot open {}", path.display()))?
        .with_guessed_format()?;
    let format = reader.format().ok_or_else(|| anyhow!("not a recognised image file"))?;
    let codec = Codec::from_format(format).ok_or_else(|| anyhow!("unsupported image format {format:?}"))?;

    let open = || -> Result<BufReader<File>> { Ok(BufReader::new(File::open(path)?)) };
    // Animated formats are decoded frame by frame. The declared canvas is
    // checked before any frame is decoded, so a hostile header cannot ask for
    // a huge allocation.
    let anim = match codec {
        Codec::Gif => {
            let mut decoder = GifDecoder::new(open()?)?;
            decoder.set_limits(Limits::default())?;
            let dims = decoder.dimensions();
            Some(collect_animation(decoder, dims, None, first_only)?)
        }
        Codec::Webp => {
            let mut decoder = WebPDecoder::new(open()?)?;
            if decoder.has_animation() {
                let dims = decoder.dimensions();
                let icc = decoder.icc_profile().ok().flatten();
                Some(collect_animation(decoder, dims, icc, first_only)?)
            } else {
                None
            }
        }
        Codec::Png => {
            let mut decoder = PngDecoder::with_limits(open()?, Limits::default())?;
            if decoder.is_apng()? {
                let dims = decoder.dimensions();
                let icc = decoder.icc_profile().ok().flatten();
                Some(collect_animation(decoder.apng()?, dims, icc, first_only)?)
            } else {
                None
            }
        }
        _ => None,
    };
    if let Some(anim) = anim {
        if anim.frames.len() > 1 {
            return Ok(Decoded {
                pixels: Pixels::Animated(anim),
                codec,
            });
        }
        return Ok(Decoded {
            pixels: Pixels::Still(normalise(first_frame(anim)?, None)),
            codec,
        });
    }

    let mut decoder = reader.into_decoder()?;
    // `into_decoder` only checks dimension caps; reserve the decoded size
    // against a budget here, or a tiny file declaring a gigantic image makes
    // the allocation abort the whole app (an abort is not catchable).
    let mut budget = Limits::default();
    budget.max_alloc = Some(MAX_STILL_BYTES);
    budget.reserve(decoder.total_bytes()).map_err(|_| {
        anyhow!(
            "image too large to open (over {} MB decoded)",
            MAX_STILL_BYTES / (1024 * 1024)
        )
    })?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    // A broken colour profile should not stop the image from opening.
    let icc = decoder.icc_profile().ok().flatten();
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    Ok(Decoded {
        pixels: Pixels::Still(normalise(img, icc.as_deref())),
        codec,
    })
}

fn too_large() -> anyhow::Error {
    anyhow!(
        "animation too large to process (more than {} MB of frames)",
        MAX_ANIMATION_BYTES / (1024 * 1024)
    )
}

fn collect_animation<'a>(
    decoder: impl AnimationDecoder<'a>,
    (width, height): (u32, u32),
    icc: Option<Vec<u8>>,
    first_only: bool,
) -> Result<Animation> {
    let canvas = (width as u64) * (height as u64) * 4;
    if canvas > MAX_ANIMATION_BYTES as u64 {
        return Err(too_large());
    }
    let loop_count = decoder.loop_count();
    // Frames are RGBA; build the colour transform once for all of them.
    let to_srgb_rgba = icc.as_deref().and_then(|icc| srgb_transform(icc, true));
    let mut frames = Vec::new();
    let mut bytes = 0usize;
    for frame in decoder.into_frames() {
        let mut frame = frame?;
        bytes = bytes.saturating_add(frame.buffer().as_raw().len());
        if bytes > MAX_ANIMATION_BYTES {
            return Err(too_large());
        }
        if let Some(t) = &to_srgb_rgba {
            t.transform_in_place(frame.buffer_mut());
        }
        frames.push(frame);
        if first_only {
            break;
        }
    }
    Ok(Animation { frames, loop_count })
}

/// Reduce to 8 bits per channel (scaling 16-bit and float properly, unlike a
/// raw cast) and convert colour images to sRGB using their embedded profile.
fn normalise(img: DynamicImage, icc: Option<&[u8]>) -> DynamicImage {
    let color = img.color();
    let mut img = match (color.has_color(), color.has_alpha()) {
        // Already 8-bit: keep the buffer instead of copying it.
        _ if matches!(
            img,
            DynamicImage::ImageLuma8(_)
                | DynamicImage::ImageLumaA8(_)
                | DynamicImage::ImageRgb8(_)
                | DynamicImage::ImageRgba8(_)
        ) =>
        {
            img
        }
        (false, false) => DynamicImage::ImageLuma8(img.to_luma8()),
        (false, true) => DynamicImage::ImageLumaA8(img.to_luma_alpha8()),
        (true, false) => DynamicImage::ImageRgb8(img.to_rgb8()),
        (true, true) => DynamicImage::ImageRgba8(img.to_rgba8()),
    };
    if let Some(icc) = icc {
        to_srgb(&mut img, icc);
    }
    img
}

/// Transform from an embedded RGB profile to sRGB, or `None` when the profile
/// is already sRGB, not RGB, or unreadable.
fn srgb_transform(icc: &[u8], alpha: bool) -> Option<Transform<u8, u8>> {
    let src = Profile::new_icc(icc).ok()?;
    if src.color_space() != ColorSpaceSignature::RgbData || is_srgb(&src) {
        return None;
    }
    let srgb = Profile::new_srgb();
    let format = if alpha { PixelFormat::RGBA_8 } else { PixelFormat::RGB_8 };
    let flags = if alpha { Flags::COPY_ALPHA } else { Flags::default() };
    Transform::new_flags(&src, format, &srgb, format, Intent::Perceptual, flags).ok()
}

fn to_srgb(img: &mut DynamicImage, icc: &[u8]) {
    match img {
        DynamicImage::ImageRgb8(buf) => {
            if let Some(t) = srgb_transform(icc, false) {
                t.transform_in_place(buf);
            }
        }
        DynamicImage::ImageRgba8(buf) => {
            if let Some(t) = srgb_transform(icc, true) {
                t.transform_in_place(buf);
            }
        }
        _ => {}
    }
}

fn is_srgb(p: &Profile) -> bool {
    p.info(InfoType::Description, Locale::none())
        .is_some_and(|d| d.to_ascii_lowercase().contains("srgb"))
}
