//! Decoding: every input ends up upright, 8-bit, and in sRGB, so the encoders
//! only ever see L8, La8, Rgb8 or Rgba8.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use image::codecs::gif::GifDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::metadata::{LoopCount, Orientation};
use image::{AnimationDecoder, DynamicImage, Frame, ImageDecoder, ImageFormat, ImageReader, Limits};
use lcms2::{ColorSpaceSignature, Flags, Intent, PixelFormat, Profile, Transform};

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

/// Largest image file Imaginify opens. Far above any photo, and it bounds what
/// decoders that buffer the whole encoded file can allocate.
pub const MAX_INPUT_BYTES: u64 = 512 * 1024 * 1024;

/// Most frames one animation may have; each frame costs memory beyond its
/// pixels, so a file of millions of 1x1 frames must not get through.
pub const MAX_FRAMES: usize = 10_000;
/// Bytes charged per frame on top of its pixels (Frame, Vec slot, allocation).
const FRAME_OVERHEAD: usize = 1024;

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
    /// Something the user should know about how the file was read.
    pub note: Option<&'static str>,
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

    // Some decoders (WebP, JPEG) read the whole encoded file into memory, so
    // a small image padded to gigabytes must be refused before they run.
    let input_len = std::fs::metadata(path)?.len();
    if input_len > MAX_INPUT_BYTES {
        bail!("file too large to open (over {} MB)", MAX_INPUT_BYTES / (1024 * 1024));
    }
    let mut note = None;
    let open = || -> Result<BufReader<File>> { Ok(BufReader::new(File::open(path)?)) };
    let webp = if codec == Codec::Webp {
        Some(read_webp_container(path)?)
    } else {
        None
    };
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
            let decoder = WebPDecoder::new(open()?)?;
            if decoder.has_animation() {
                let dims = decoder.dimensions();
                let icc = webp.as_ref().and_then(|m| m.icc.clone());
                Some(collect_animation(decoder, dims, icc, first_only)?)
            } else {
                None
            }
        }
        Codec::Png => {
            let mut decoder = PngDecoder::with_limits(open()?, Limits::default())?;
            let color = decoder.color_type();
            let sixteen_bit = color.bytes_per_pixel() > color.channel_count();
            if decoder.is_apng()? && sixteen_bit {
                // The APNG frame reader rejects 16-bit samples; open the PNG's
                // default image instead (16-bit is fine there) and say so.
                note = Some("16-bit animated PNG: only the first frame was kept");
                None
            } else if decoder.is_apng()? {
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
                note: None,
            });
        }
        return Ok(Decoded {
            pixels: Pixels::Still(normalise(first_frame(anim)?, None, false)),
            codec,
            note: None,
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
    let (orientation, icc) = match &webp {
        Some(meta) => (
            meta.exif
                .as_deref()
                .and_then(Orientation::from_exif_chunk)
                .unwrap_or(Orientation::NoTransforms),
            meta.icc.clone(),
        ),
        None => (
            decoder.orientation().unwrap_or(Orientation::NoTransforms),
            // A broken colour profile should not stop the image from opening.
            decoder.icc_profile().ok().flatten(),
        ),
    };
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    let premultiplied = codec == Codec::Tiff && tiff_has_associated_alpha(path);
    Ok(Decoded {
        pixels: Pixels::Still(normalise(img, icc.as_deref(), premultiplied)),
        codec,
        note,
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
        if frames.len() >= MAX_FRAMES {
            bail!("animation has too many frames to process (more than {MAX_FRAMES})");
        }
        let mut frame = frame?;
        bytes = bytes.saturating_add(frame.buffer().as_raw().len() + FRAME_OVERHEAD);
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
/// raw cast), undo premultiplied alpha, and convert colour images to sRGB
/// using their embedded profile.
fn normalise(img: DynamicImage, icc: Option<&[u8]>, premultiplied: bool) -> DynamicImage {
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
    if premultiplied {
        unpremultiply(&mut img);
    }
    if let Some(icc) = icc {
        to_srgb(&mut img, icc);
    }
    img
}

/// Transform from an embedded RGB profile to sRGB, or `None` when the profile
/// is already sRGB, not RGB, or unreadable.
fn srgb_transform(icc: &[u8], alpha: bool) -> Option<Transform<u8, u8>> {
    let src = Profile::new_icc(icc).ok()?;
    // Every RGB profile is converted, sRGB included: a profile's description
    // is not proof of its curves (a "linear sRGB" profile is not sRGB).
    if src.color_space() != ColorSpaceSignature::RgbData {
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

/// Reject a WebP whose RIFF chunks claim more bytes than the file holds. The
/// WebP decoder allocates a metadata chunk's declared size before reading it,
/// so a tiny file declaring a multi-gigabyte EXIF/ICC chunk would otherwise
/// abort the app.
/// Metadata Imaginify reads itself from a validated WebP: the WebP decoder's
/// own metadata index can misread pixel bytes as a chunk header and then
/// allocate whatever size that "header" claims, so its `icc_profile()` and
/// `orientation()` (EXIF) are never called.
#[derive(Default)]
struct WebpMeta {
    icc: Option<Vec<u8>>,
    exif: Option<Vec<u8>>,
}

fn read_webp_container(path: &Path) -> Result<WebpMeta> {
    let corrupt = || anyhow!("corrupt or truncated WebP file");
    let len = std::fs::metadata(path)?.len();
    let mut f = BufReader::new(File::open(path)?);
    let mut header = [0u8; 12];
    f.read_exact(&mut header).map_err(|_| corrupt())?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WEBP" {
        return Err(corrupt());
    }
    let riff_end = 8 + u64::from(u32::from_le_bytes([header[4], header[5], header[6], header[7]]));
    if riff_end > len {
        return Err(corrupt());
    }
    // Bytes after the RIFF are ignored, as browsers and libwebp do. (The WebP
    // decoder allocates from declared sizes only for metadata, which is never
    // read through it; pixel data reads are bounded by the real bytes.)
    let mut meta = WebpMeta::default();
    let mut pos = 12u64;
    while pos + 8 <= riff_end {
        f.seek(SeekFrom::Start(pos))?;
        let mut chunk = [0u8; 8];
        f.read_exact(&mut chunk).map_err(|_| corrupt())?;
        let size = u64::from(u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]));
        let data_end = pos + 8 + size;
        if data_end > riff_end || metadata_too_large(&chunk[0..4], size) {
            return Err(corrupt());
        }
        // Animation frames hold their own sub-chunks, which the decoder also
        // indexes: each must fit inside its frame.
        if &chunk[0..4] == b"ANMF" {
            if size < 16 {
                return Err(corrupt());
            }
            let mut sub = pos + 8 + 16; // after the 16-byte frame header
            while sub + 8 <= data_end {
                f.seek(SeekFrom::Start(sub))?;
                let mut sub_chunk = [0u8; 8];
                f.read_exact(&mut sub_chunk).map_err(|_| corrupt())?;
                let sub_size = u64::from(u32::from_le_bytes([
                    sub_chunk[4],
                    sub_chunk[5],
                    sub_chunk[6],
                    sub_chunk[7],
                ]));
                let sub_end = sub + 8 + sub_size;
                if sub_end > data_end || metadata_too_large(&sub_chunk[0..4], sub_size) {
                    return Err(corrupt());
                }
                sub = sub_end + (sub_size & 1);
            }
        }
        // ICC and EXIF are top-level chunks inside the RIFF (first one wins);
        // their size is already capped by `metadata_too_large`.
        let slot = match &chunk[0..4] {
            b"ICCP" if meta.icc.is_none() => Some(&mut meta.icc),
            b"EXIF" if meta.exif.is_none() => Some(&mut meta.exif),
            _ => None,
        };
        if let Some(slot) = slot {
            let mut data = vec![0u8; size as usize];
            f.seek(SeekFrom::Start(pos + 8))?;
            f.read_exact(&mut data).map_err(|_| corrupt())?;
            *slot = Some(data);
        }
        pos = data_end + (size & 1); // chunks are padded to an even length
    }
    Ok(meta)
}

/// Metadata the decoder reads whole into memory: cap it well above any real
/// ICC profile, EXIF block or XMP packet.
fn metadata_too_large(fourcc: &[u8], size: u64) -> bool {
    const MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;
    matches!(fourcc, b"ICCP" | b"EXIF" | b"XMP ") && size > MAX_METADATA_BYTES
}

/// TIFF `ExtraSamples = 1` means the colour samples are premultiplied by
/// alpha. The decoder passes them through as if they were straight alpha.
fn tiff_has_associated_alpha(path: &Path) -> bool {
    let Ok(file) = File::open(path) else { return false };
    let Ok(mut decoder) = tiff::decoder::Decoder::new(BufReader::new(file)) else {
        return false;
    };
    decoder
        .get_tag_u16_vec(tiff::tags::Tag::ExtraSamples)
        .is_ok_and(|v| v.first() == Some(&1))
}

fn unpremultiply(img: &mut DynamicImage) {
    fn straight(c: u8, a: u8) -> u8 {
        if a == 0 {
            0
        } else {
            ((u32::from(c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8
        }
    }
    match img {
        DynamicImage::ImageRgba8(buf) => {
            for p in buf.pixels_mut() {
                let a = p.0[3];
                for c in &mut p.0[..3] {
                    *c = straight(*c, a);
                }
            }
        }
        DynamicImage::ImageLumaA8(buf) => {
            for p in buf.pixels_mut() {
                p.0[0] = straight(p.0[0], p.0[1]);
            }
        }
        _ => {}
    }
}
