//! Imaginify image pipeline: decode, crop, resize, encode, and write outputs
//! without ever replacing an existing file (the user's sources included).
//!
//! Everything here is UI-free so it can be tested with `cargo test -p imaginify-core`.

mod decode;
mod encode;
mod geometry;
mod output;

use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::{Context, Result};
use image::{DynamicImage, Frame, GenericImageView};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

pub use decode::{
    decode, decode_first_frame, first_frame, Animation, Codec, Decoded, Pixels, MAX_ANIMATION_BYTES, MAX_INPUT_BYTES,
};
pub use geometry::{crop_rect, target_size};
pub use output::{sanitize_suffix, write_unique};

/// File extensions the app accepts (lowercase, no dot).
pub const SUPPORTED_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff", "gif"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Keep,
    Jpeg,
    Png,
    Webp,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Target width in px; 0 = not set.
    pub width: u32,
    /// Target height in px; 0 = not set.
    pub height: u32,
    pub keep_aspect: bool,
    /// Never make an image bigger than it already is.
    pub no_enlarge: bool,
    /// 1-100. JPEG/WebP quality; 100 on WebP means lossless.
    pub quality: u8,
    pub format: OutputFormat,
    pub suffix: String,
    /// When nothing about the image changes except the encoding and the result is
    /// bigger than the source, write a copy of the source instead.
    pub keep_if_larger: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            keep_aspect: true,
            no_enlarge: true,
            quality: 88,
            format: OutputFormat::Keep,
            suffix: "_optimised".into(),
            keep_if_larger: true,
        }
    }
}

/// Crop region as fractions (0..1) of the upright image, so it stays correct
/// whatever size the preview is drawn at.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Crop {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Locked shape (width / height in pixels), if one was chosen. Applied
    /// per image, so a 1:1 crop copied to a portrait photo stays square.
    #[serde(default)]
    pub ratio: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub path: PathBuf,
    pub crop: Option<Crop>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub src: PathBuf,
    pub out: Option<PathBuf>,
    pub size_in: u64,
    pub size_out: u64,
    pub note: Option<String>,
    pub error: Option<String>,
    pub cancelled: bool,
}

impl Outcome {
    fn failed(src: &Path, err: String) -> Self {
        Self {
            src: src.into(),
            out: None,
            size_in: 0,
            size_out: 0,
            note: None,
            error: Some(err),
            cancelled: false,
        }
    }
    fn cancelled(src: &Path) -> Self {
        Self {
            src: src.into(),
            out: None,
            size_in: 0,
            size_out: 0,
            note: None,
            error: None,
            cancelled: true,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub outcome: Outcome,
}

/// Expand files and folders (recursively) into supported image paths, in a
/// stable order, without duplicates. Hidden files (including macOS `._*`
/// resource forks on external drives) are skipped.
pub fn scan(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |p: PathBuf, out: &mut Vec<PathBuf>| {
        if seen.insert(p.clone()) {
            out.push(p);
        }
    };
    for p in paths {
        if p.is_dir() {
            let walker = walkdir::WalkDir::new(p).follow_links(false).sort_by_file_name();
            for entry in walker.into_iter().filter_map(Result::ok) {
                if entry.file_type().is_file() && is_supported(entry.path()) {
                    push(entry.into_path(), &mut out);
                }
            }
        } else if p.is_file() && is_supported(p) {
            push(p.clone(), &mut out);
        }
    }
    out
}

pub fn is_supported(path: &Path) -> bool {
    let hidden = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_none_or(|n| n.starts_with('.'));
    let ext_ok = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| SUPPORTED_EXTS.contains(&e.to_ascii_lowercase().as_str()));
    !hidden && ext_ok
}

/// A downscaled, upright, sRGB rendering for the preview pane, plus the size of
/// the full image (crop fractions are relative to that).
pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub mime: &'static str,
    pub data: Vec<u8>,
}

pub fn preview(path: &Path, max_side: u32) -> Result<Preview> {
    let img = decode::decode_first_frame(path)?;
    let (w, h) = img.dimensions();
    let scale = (max_side as f64 / w.max(h) as f64).min(1.0);
    let small = if scale < 1.0 {
        let sw = ((w as f64 * scale).round() as u32).max(1);
        let sh = ((h as f64 * scale).round() as u32).max(1);
        geometry::resize(&img, sw, sh)?
    } else {
        img
    };
    let (mime, data) = if small.color().has_alpha() {
        ("image/png", encode::png_fast(&small)?)
    } else {
        ("image/jpeg", encode::encode(&small, Codec::Jpeg, 85)?)
    };
    Ok(Preview {
        width: w,
        height: h,
        mime,
        data,
    })
}

/// Process one image and write it into `out_dir` under a name that does not
/// exist yet. Never replaces any file.
pub fn process_one(job: &Job, s: &Settings, out_dir: &Path) -> Result<Outcome> {
    let src = &job.path;
    let size_in = fs::metadata(src)
        .with_context(|| format!("cannot read {}", src.display()))?
        .len();
    let decoded = decode(src)?;
    let out_codec = match s.format {
        OutputFormat::Keep => decoded.codec,
        OutputFormat::Jpeg => Codec::Jpeg,
        OutputFormat::Png => Codec::Png,
        OutputFormat::Webp => Codec::Webp,
    };
    let quality = s.quality.clamp(1, 100);
    let mut note = decoded.note.map(String::from);
    let mut changed = false;

    let data = match decoded.pixels {
        Pixels::Animated(anim) if matches!(out_codec, Codec::Gif | Codec::Webp) => {
            let mut frames = Vec::with_capacity(anim.frames.len());
            let mut bytes = 0usize;
            for frame in anim.frames {
                let delay = frame.delay();
                let img = DynamicImage::ImageRgba8(frame.into_buffer());
                let img = geometry::apply(img, job.crop, s, &mut changed)?.into_rgba8();
                bytes = bytes.saturating_add(img.as_raw().len());
                if bytes > MAX_ANIMATION_BYTES {
                    anyhow::bail!(
                        "resized animation too large (more than {} MB of frames)",
                        MAX_ANIMATION_BYTES / (1024 * 1024)
                    );
                }
                frames.push(Frame::from_parts(img, 0, 0, delay));
            }
            if out_codec == Codec::Gif {
                encode::gif(frames, anim.loop_count)?
            } else {
                encode::webp_animated(&frames, anim.loop_count, quality)?
            }
        }
        Pixels::Animated(anim) => {
            note = Some(
                "Animated image: only the first frame was kept (choose WebP, or keep a GIF or WebP as it is, to keep the animation)"
                    .into(),
            );
            let img = geometry::apply(first_frame(anim)?, job.crop, s, &mut changed)?;
            encode::encode(&img, out_codec, quality)?
        }
        Pixels::Still(img) => {
            let img = geometry::apply(img, job.crop, s, &mut changed)?;
            encode::encode(&img, out_codec, quality)?
        }
    };

    let data = if s.keep_if_larger && !changed && out_codec == decoded.codec && data.len() as u64 >= size_in {
        note = Some("Kept the original: re-encoding made it larger".into());
        fs::read(src)?
    } else {
        data
    };

    fs::create_dir_all(out_dir).with_context(|| format!("cannot create {}", out_dir.display()))?;
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".into());
    let base = format!("{stem}{}", sanitize_suffix(&s.suffix));
    let out = write_unique(out_dir, &base, out_codec.extension(src), &data)?;
    Ok(Outcome {
        src: src.clone(),
        out: Some(out),
        size_in,
        size_out: data.len() as u64,
        note,
        error: None,
        cancelled: false,
    })
}

/// Worker count: parallel, but capped so a batch of 50-megapixel photos does
/// not decode a dozen at once and exhaust memory.
fn worker_count() -> usize {
    std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(1, 4)
}

/// Process every job in parallel. `on_progress` runs on worker threads after
/// each job. Setting `cancel` stops new jobs from starting; jobs already
/// running finish (outputs are written atomically, so none is left half-written).
pub fn process_batch<F>(
    jobs: &[Job],
    s: &Settings,
    out_dir: &Path,
    cancel: &AtomicBool,
    on_progress: F,
) -> Result<Vec<Outcome>>
where
    F: Fn(Progress) + Sync,
{
    fs::create_dir_all(out_dir).with_context(|| format!("cannot create {}", out_dir.display()))?;
    let pool = rayon::ThreadPoolBuilder::new().num_threads(worker_count()).build()?;
    let total = jobs.len();
    let done = AtomicUsize::new(0);
    let outcomes = pool.install(|| {
        jobs.par_iter()
            .map(|job| {
                let outcome = if cancel.load(Ordering::Relaxed) {
                    Outcome::cancelled(&job.path)
                } else {
                    match catch_unwind(AssertUnwindSafe(|| process_one(job, s, out_dir))) {
                        Ok(Ok(o)) => o,
                        Ok(Err(e)) => Outcome::failed(&job.path, format!("{e:#}")),
                        Err(_) => Outcome::failed(&job.path, "internal error while processing this image".into()),
                    }
                };
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                on_progress(Progress {
                    done: n,
                    total,
                    outcome: outcome.clone(),
                });
                outcome
            })
            .collect()
    });
    Ok(outcomes)
}
