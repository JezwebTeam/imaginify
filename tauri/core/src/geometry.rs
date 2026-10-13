//! Crop and resize.

use anyhow::{bail, Result};
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::{DynamicImage, GenericImageView};

use crate::{Crop, Settings};

/// Largest output, in pixels (about 1 GB as RGBA), so an oversized target
/// fails that file with a message instead of exhausting memory.
pub const MAX_OUTPUT_PIXELS: u64 = 268_000_000;

/// Pixel rectangle `(x, y, w, h)` for a fractional crop, or `None` when the
/// crop is empty or covers the whole image.
pub fn crop_rect(width: u32, height: u32, c: &Crop) -> Option<(u32, u32, u32, u32)> {
    let to_px = |frac: f64, size: u32| ((frac.clamp(0.0, 1.0) * size as f64).round() as u32).min(size);
    let (x0, y0) = (to_px(c.x, width), to_px(c.y, height));
    let (x1, y1) = (to_px(c.x + c.w, width), to_px(c.y + c.h, height));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (mut x, mut y, mut w, mut h) = (x0, y0, x1 - x0, y1 - y0);
    // Hold a locked shape: shrink the longer side around the crop's centre.
    if let Some(ratio) = c.ratio.filter(|r| r.is_finite() && *r > 0.0) {
        let current = w as f64 / h as f64;
        if current > ratio {
            let nw = ((h as f64 * ratio).round() as u32).clamp(1, w);
            x += (w - nw) / 2;
            w = nw;
        } else if current < ratio {
            let nh = ((w as f64 / ratio).round() as u32).clamp(1, h);
            y += (h - nh) / 2;
            h = nh;
        }
    }
    if (x, y, w, h) == (0, 0, width, height) {
        return None;
    }
    Some((x, y, w, h))
}

/// Output size for an image of `w` x `h`, or `None` to leave the size alone.
pub fn target_size(w: u32, h: u32, s: &Settings) -> Option<(u32, u32)> {
    if s.width == 0 && s.height == 0 {
        return None;
    }
    let (tw, th) = if s.keep_aspect {
        let rw = s.width as f64 / w as f64;
        let rh = s.height as f64 / h as f64;
        let mut ratio = match (s.width, s.height) {
            (_, 0) => rw,
            (0, _) => rh,
            _ => rw.min(rh),
        };
        if s.no_enlarge {
            ratio = ratio.min(1.0);
        }
        let scale = |v: u32| ((v as f64 * ratio).round() as u32).max(1);
        (scale(w), scale(h))
    } else {
        let mut tw = if s.width > 0 { s.width } else { w };
        let mut th = if s.height > 0 { s.height } else { h };
        if s.no_enlarge {
            tw = tw.min(w);
            th = th.min(h);
        }
        (tw, th)
    };
    ((tw, th) != (w, h)).then_some((tw, th))
}

pub fn resize(img: &DynamicImage, w: u32, h: u32) -> Result<DynamicImage> {
    let mut dst = DynamicImage::new(w, h, img.color());
    let opts = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));
    Resizer::new().resize(img, &mut dst, &opts)?;
    Ok(dst)
}

/// Crop, then resize. Sets `changed` when either actually happened.
pub fn apply(img: DynamicImage, crop: Option<Crop>, s: &Settings, changed: &mut bool) -> Result<DynamicImage> {
    let (w, h) = img.dimensions();
    let img = match crop.and_then(|c| crop_rect(w, h, &c)) {
        Some((x, y, cw, ch)) => {
            *changed = true;
            img.crop_imm(x, y, cw, ch)
        }
        None => img,
    };
    let (w, h) = img.dimensions();
    match target_size(w, h, s) {
        Some((tw, th)) => {
            if u64::from(tw) * u64::from(th) > MAX_OUTPUT_PIXELS {
                bail!(
                    "output size {tw} x {th} is too large (limit {} megapixels)",
                    MAX_OUTPUT_PIXELS / 1_000_000
                );
            }
            *changed = true;
            resize(&img, tw, th)
        }
        None => Ok(img),
    }
}
