//! Every input mode against every output format, plus the file-safety rules.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use image::codecs::png::PngEncoder;
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageEncoder, ImageFormat, Rgb, Rgba};
use imaginify_core::*;

fn tmpdir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn job(path: &Path) -> Job {
    Job {
        path: path.to_path_buf(),
        crop: None,
    }
}

fn settings(format: OutputFormat) -> Settings {
    Settings {
        format,
        ..Settings::default()
    }
}

fn run(path: &Path, s: &Settings, out: &Path) -> Outcome {
    process_one(&job(path), s, out).unwrap_or_else(|e| panic!("{}: {e:#}", path.display()))
}

fn open(path: &Path) -> DynamicImage {
    image::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Gradient so encoders cannot collapse it to nothing.
fn sample(color: image::ColorType, w: u32, h: u32) -> DynamicImage {
    let rgba = ImageBuffer::from_fn(w, h, |x, y| {
        Rgba([(x * 7) as u8, (y * 5) as u8, 128, if x < w / 2 { 255 } else { 90 }])
    });
    let img = DynamicImage::ImageRgba8(rgba);
    match color {
        image::ColorType::L8 => DynamicImage::ImageLuma8(img.to_luma8()),
        image::ColorType::La8 => DynamicImage::ImageLumaA8(img.to_luma_alpha8()),
        image::ColorType::Rgb8 => DynamicImage::ImageRgb8(img.to_rgb8()),
        image::ColorType::Rgba8 => img,
        image::ColorType::L16 => DynamicImage::ImageLuma16(img.to_luma16()),
        image::ColorType::La16 => DynamicImage::ImageLumaA16(img.to_luma_alpha16()),
        image::ColorType::Rgb16 => DynamicImage::ImageRgb16(img.to_rgb16()),
        image::ColorType::Rgba16 => DynamicImage::ImageRgba16(img.to_rgba16()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn every_input_mode_to_every_output_format() {
    use image::ColorType::*;
    let dir = tmpdir();
    let out = dir.path().join("out");
    for color in [L8, La8, Rgb8, Rgba8, L16, La16, Rgb16, Rgba16] {
        let src = dir.path().join(format!("{color:?}.png"));
        sample(color, 40, 30).save(&src).unwrap();
        for fmt in [
            OutputFormat::Keep,
            OutputFormat::Jpeg,
            OutputFormat::Png,
            OutputFormat::Webp,
        ] {
            let o = run(&src, &settings(fmt), &out);
            let img = open(o.out.as_ref().unwrap());
            assert_eq!(img.dimensions(), (40, 30), "{color:?} -> {fmt:?}");
        }
    }
}

#[test]
fn every_source_format_keeps_its_format() {
    let dir = tmpdir();
    let out = dir.path().join("out");
    let img = sample(image::ColorType::Rgb8, 32, 24);
    for (name, fmt, want_ext) in [
        ("a.jpg", ImageFormat::Jpeg, "jpg"),
        ("b.jpeg", ImageFormat::Jpeg, "jpg"),
        ("c.png", ImageFormat::Png, "png"),
        ("d.webp", ImageFormat::WebP, "webp"),
        ("e.gif", ImageFormat::Gif, "gif"),
        ("f.bmp", ImageFormat::Bmp, "bmp"),
        ("g.tif", ImageFormat::Tiff, "tif"),
        ("h.tiff", ImageFormat::Tiff, "tiff"),
    ] {
        let src = dir.path().join(name);
        img.save_with_format(&src, fmt).unwrap();
        let o = run(
            &src,
            &Settings {
                keep_if_larger: false,
                ..settings(OutputFormat::Keep)
            },
            &out,
        );
        let path = o.out.unwrap();
        assert_eq!(path.extension().unwrap(), want_ext, "{name}");
        assert_eq!(
            image::ImageReader::open(&path)
                .unwrap()
                .with_guessed_format()
                .unwrap()
                .format(),
            Some(fmt),
            "{name}"
        );
        assert_eq!(open(&path).dimensions(), (32, 24));
    }
}

#[test]
fn sixteen_bit_to_jpeg_is_not_white() {
    let dir = tmpdir();
    let src = dir.path().join("deep.png");
    DynamicImage::ImageRgb16(ImageBuffer::from_pixel(8, 8, Rgb([40000u16, 40000, 40000])))
        .save(&src)
        .unwrap();
    let o = run(&src, &settings(OutputFormat::Jpeg), &dir.path().join("out"));
    let px = open(o.out.as_ref().unwrap()).to_rgb8().get_pixel(4, 4).0;
    // 40000 / 257 = 155.6
    assert!(px.iter().all(|&c| (150..=161).contains(&c)), "{px:?}");
}

#[test]
fn transparency_flattens_to_white_for_jpeg() {
    let dir = tmpdir();
    let src = dir.path().join("clear.png");
    DynamicImage::ImageRgba8(ImageBuffer::from_pixel(8, 8, Rgba([0, 0, 0, 0])))
        .save(&src)
        .unwrap();
    let o = run(&src, &settings(OutputFormat::Jpeg), &dir.path().join("out"));
    let px = open(o.out.as_ref().unwrap()).to_rgb8().get_pixel(4, 4).0;
    assert!(px.iter().all(|&c| c >= 250), "{px:?}");
}

#[test]
fn never_overwrites_the_source() {
    let dir = tmpdir();
    let src = dir.path().join("photo.jpg");
    sample(image::ColorType::Rgb8, 64, 48).save(&src).unwrap();
    let before = fs::read(&src).unwrap();
    // Output folder = source folder, empty suffix: the worst case.
    let s = Settings {
        suffix: String::new(),
        width: 32,
        ..settings(OutputFormat::Keep)
    };
    let o = run(&src, &s, dir.path());
    assert_eq!(o.out.unwrap(), dir.path().join("photo (2).jpg"));
    assert_eq!(fs::read(&src).unwrap(), before);
}

#[test]
fn never_overwrites_the_source_with_different_extension_case() {
    let dir = tmpdir();
    let src = dir.path().join("photo.JPG");
    sample(image::ColorType::Rgb8, 64, 48)
        .save_with_format(&src, ImageFormat::Jpeg)
        .unwrap();
    let before = fs::read(&src).unwrap();
    let s = Settings {
        suffix: String::new(),
        width: 32,
        ..settings(OutputFormat::Keep)
    };
    let o = run(&src, &s, dir.path());
    assert_eq!(fs::read(&src).unwrap(), before);
    assert_ne!(o.out.unwrap(), src);
}

#[test]
fn same_names_from_different_folders_do_not_collide() {
    let dir = tmpdir();
    let out = dir.path().join("out");
    let mut jobs = Vec::new();
    for sub in ["a", "b", "c"] {
        fs::create_dir(dir.path().join(sub)).unwrap();
        let src = dir.path().join(sub).join("IMG_1.jpg");
        sample(image::ColorType::Rgb8, 20, 20).save(&src).unwrap();
        jobs.push(job(&src));
    }
    let s = Settings {
        width: 10,
        ..settings(OutputFormat::Keep)
    };
    let outcomes = process_batch(&jobs, &s, &out, &AtomicBool::new(false), |_| {}).unwrap();
    let mut outs: Vec<PathBuf> = outcomes.into_iter().map(|o| o.out.unwrap()).collect();
    outs.sort();
    outs.dedup();
    assert_eq!(outs.len(), 3);
    // Run again: previous outputs are not replaced either.
    let again = process_batch(&jobs, &s, &out, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(again.iter().all(|o| !outs.contains(o.out.as_ref().unwrap())));
}

#[test]
fn does_not_enlarge_by_default() {
    let s = Settings {
        width: 1280,
        ..Settings::default()
    };
    assert_eq!(target_size(800, 600, &s), None);
    let s = Settings {
        width: 1280,
        no_enlarge: false,
        ..Settings::default()
    };
    assert_eq!(target_size(800, 600, &s), Some((1280, 960)));
    let s = Settings {
        width: 400,
        ..Settings::default()
    };
    assert_eq!(target_size(800, 600, &s), Some((400, 300)));
    let s = Settings {
        width: 1080,
        height: 1080,
        ..Settings::default()
    };
    assert_eq!(target_size(4000, 3000, &s), Some((1080, 810)));
    let s = Settings {
        width: 500,
        height: 500,
        keep_aspect: false,
        ..Settings::default()
    };
    assert_eq!(target_size(4000, 3000, &s), Some((500, 500)));
}

#[test]
fn crop_is_relative_to_each_image() {
    let c = Crop {
        x: 0.25,
        y: 0.5,
        w: 0.5,
        h: 0.5,
        ratio: None,
    };
    assert_eq!(crop_rect(100, 50, &c), Some((25, 25, 50, 25)));
    assert_eq!(crop_rect(4000, 3000, &c), Some((1000, 1500, 2000, 1500)));
    assert_eq!(
        crop_rect(
            100,
            50,
            &Crop {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
                ratio: None,
            }
        ),
        None
    );
    assert_eq!(
        crop_rect(
            100,
            50,
            &Crop {
                x: 0.9,
                y: 0.9,
                w: 0.5,
                h: 0.5,
                ratio: None,
            }
        ),
        Some((90, 45, 10, 5))
    );

    let dir = tmpdir();
    let src = dir.path().join("wide.png");
    sample(image::ColorType::Rgb8, 100, 50).save(&src).unwrap();
    let o = process_one(
        &Job {
            path: src,
            crop: Some(c),
        },
        &settings(OutputFormat::Png),
        dir.path(),
    )
    .unwrap();
    assert_eq!(open(o.out.as_ref().unwrap()).dimensions(), (50, 25));
}

fn p3_profile() -> Vec<u8> {
    use lcms2::{CIExyY, CIExyYTRIPLE, Profile, ToneCurve};
    let xy = |x, y| CIExyY { x, y, Y: 1.0 };
    let curve = ToneCurve::new(2.2);
    Profile::new_rgb(
        &xy(0.3127, 0.3290),
        &CIExyYTRIPLE {
            Red: xy(0.680, 0.320),
            Green: xy(0.265, 0.690),
            Blue: xy(0.150, 0.060),
        },
        &[&curve, &curve, &curve],
    )
    .unwrap()
    .icc()
    .unwrap()
}

#[test]
fn wide_gamut_profile_is_converted_to_srgb() {
    let p3 = p3_profile();

    let dir = tmpdir();
    let src = dir.path().join("p3.png");
    let pixel = [200u8, 100, 50];
    let buf = ImageBuffer::from_pixel(4, 4, Rgb(pixel));
    let mut bytes = Vec::new();
    let mut enc = PngEncoder::new(&mut bytes);
    enc.set_icc_profile(p3).unwrap();
    enc.write_image(&buf, 4, 4, image::ExtendedColorType::Rgb8).unwrap();
    fs::write(&src, bytes).unwrap();

    let o = run(&src, &settings(OutputFormat::Png), &dir.path().join("out"));
    let got = open(o.out.as_ref().unwrap()).to_rgb8().get_pixel(1, 1).0;
    // A saturated P3 orange is redder than the same numbers read as sRGB.
    assert!(got[0] > pixel[0] + 5, "expected converted pixel, got {got:?}");
}

#[test]
fn exif_orientation_is_applied() {
    let dir = tmpdir();
    let src = dir.path().join("rotated.jpg");
    let mut jpg = Vec::new();
    sample(image::ColorType::Rgb8, 60, 20)
        .write_to(&mut std::io::Cursor::new(&mut jpg), ImageFormat::Jpeg)
        .unwrap();
    // APP1 Exif block with Orientation = 6 (rotate 90 CW), little-endian TIFF.
    let mut tiff = b"II*\0\x08\0\0\0\x01\0".to_vec();
    tiff.extend_from_slice(&[0x12, 0x01, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0]);
    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend_from_slice(&tiff);
    let len = (app1.len() + 2) as u16;
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&app1);
    out.extend_from_slice(&jpg[2..]);
    fs::write(&src, out).unwrap();

    let o = run(&src, &settings(OutputFormat::Png), &dir.path().join("out"));
    assert_eq!(open(o.out.as_ref().unwrap()).dimensions(), (20, 60));
}

#[test]
fn animated_gif_keeps_its_frames() {
    use image::codecs::gif::{GifDecoder, GifEncoder};
    use image::{AnimationDecoder, Delay, Frame};
    let dir = tmpdir();
    let src = dir.path().join("anim.gif");
    {
        let mut enc = GifEncoder::new(fs::File::create(&src).unwrap());
        let frames = (0..3u8).map(|i| {
            Frame::from_parts(
                ImageBuffer::from_pixel(40, 40, Rgba([i * 80, 0, 0, 255])),
                0,
                0,
                Delay::from_numer_denom_ms(100, 1),
            )
        });
        enc.encode_frames(frames).unwrap();
    }
    let s = Settings {
        width: 20,
        ..settings(OutputFormat::Keep)
    };
    let o = run(&src, &s, &dir.path().join("out"));
    let dec = GifDecoder::new(std::io::BufReader::new(fs::File::open(o.out.unwrap()).unwrap())).unwrap();
    let frames = dec.into_frames().collect_frames().unwrap();
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0].buffer().dimensions(), (20, 20));

    let o = run(&src, &settings(OutputFormat::Png), &dir.path().join("out"));
    assert!(o.note.unwrap().contains("first frame"));
}

#[test]
fn keeps_the_original_when_reencoding_is_bigger() {
    let dir = tmpdir();
    let src = dir.path().join("small.jpg");
    let mut small = Vec::new();
    let img = sample(image::ColorType::Rgb8, 64, 64);
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut small, 10)
        .encode_image(&img)
        .unwrap();
    fs::write(&src, &small).unwrap();
    let s = Settings {
        quality: 100,
        ..settings(OutputFormat::Keep)
    };
    let o = run(&src, &s, &dir.path().join("out"));
    assert!(o.note.is_some());
    assert_eq!(fs::read(o.out.unwrap()).unwrap(), small);
}

#[test]
fn suffix_cannot_escape_the_output_folder() {
    let dir = tmpdir();
    let out = dir.path().join("out");
    let src = dir.path().join("a.png");
    sample(image::ColorType::Rgb8, 8, 8).save(&src).unwrap();
    let s = Settings {
        suffix: "/../../evil".into(),
        ..settings(OutputFormat::Png)
    };
    let o = run(&src, &s, &out);
    assert_eq!(o.out.unwrap().parent().unwrap(), out);
}

#[test]
fn scan_finds_images_recursively_and_skips_hidden_files() {
    let dir = tmpdir();
    fs::create_dir_all(dir.path().join("sub/deeper")).unwrap();
    for name in [
        "a.jpg",
        "sub/b.PNG",
        "sub/deeper/c.webp",
        "notes.txt",
        "._a.jpg",
        ".hidden.png",
    ] {
        fs::write(dir.path().join(name), b"x").unwrap();
    }
    let found = scan(&[dir.path().to_path_buf(), dir.path().join("a.jpg")]);
    let names: Vec<_> = found.iter().map(|p| p.file_name().unwrap().to_str().unwrap()).collect();
    assert_eq!(names, ["a.jpg", "b.PNG", "c.webp"]);
}

#[test]
fn cancel_stops_new_work() {
    let dir = tmpdir();
    let src = dir.path().join("a.png");
    sample(image::ColorType::Rgb8, 8, 8).save(&src).unwrap();
    let jobs = vec![job(&src); 5];
    let outcomes = process_batch(
        &jobs,
        &Settings::default(),
        &dir.path().join("out"),
        &AtomicBool::new(true),
        |_| {},
    )
    .unwrap();
    assert!(outcomes.iter().all(|o| o.cancelled && o.out.is_none()));
    assert!(fs::read_dir(dir.path().join("out")).unwrap().next().is_none());
}

#[test]
fn a_broken_file_fails_alone() {
    let dir = tmpdir();
    let bad = dir.path().join("bad.jpg");
    fs::write(&bad, b"not an image").unwrap();
    let good = dir.path().join("good.png");
    sample(image::ColorType::Rgb8, 8, 8).save(&good).unwrap();
    let outcomes = process_batch(
        &[job(&bad), job(&good)],
        &Settings::default(),
        &dir.path().join("out"),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert!(outcomes[0].error.is_some());
    assert!(outcomes[1].out.is_some());
}

#[test]
fn preview_is_small_and_reports_full_size() {
    let dir = tmpdir();
    let src = dir.path().join("big.png");
    sample(image::ColorType::Rgb8, 3000, 1000).save(&src).unwrap();
    let p = preview(&src, 1200).unwrap();
    assert_eq!((p.width, p.height), (3000, 1000));
    assert_eq!(p.mime, "image/jpeg");
    let img = image::load_from_memory(&p.data).unwrap();
    assert_eq!(img.dimensions(), (1200, 400));
}

/// The exact shapes `ui/main.js` sends to the `process` command.
#[test]
fn ui_json_matches_the_rust_types() {
    let s: Settings = serde_json::from_str(
        r#"{"width":1920,"height":0,"keepAspect":true,"noEnlarge":true,"quality":88,"format":"webp","suffix":"_optimised","keepIfLarger":true}"#,
    )
    .unwrap();
    assert_eq!((s.width, s.format, s.no_enlarge), (1920, OutputFormat::Webp, true));
    for f in ["keep", "jpeg", "png", "webp"] {
        let json = format!(
            r#"{{"width":0,"height":0,"keepAspect":true,"noEnlarge":true,"quality":88,"format":"{f}","suffix":"","keepIfLarger":true}}"#
        );
        serde_json::from_str::<Settings>(&json).unwrap();
    }
    let jobs: Vec<Job> = serde_json::from_str(
        r#"[{"path":"/a/b.jpg","crop":null},{"path":"/a/c.png","crop":{"x":0.1,"y":0.2,"w":0.5,"h":0.5}}]"#,
    )
    .unwrap();
    assert!(jobs[0].crop.is_none() && jobs[1].crop.is_some());
    let o = serde_json::to_value(Outcome {
        src: "/a".into(),
        out: None,
        size_in: 1,
        size_out: 2,
        note: None,
        error: None,
        cancelled: false,
    })
    .unwrap();
    for key in ["src", "out", "sizeIn", "sizeOut", "note", "error", "cancelled"] {
        assert!(o.get(key).is_some(), "{key}");
    }
}

fn three_frames(size: u32) -> Vec<image::Frame> {
    (0..3u8)
        .map(|i| {
            image::Frame::from_parts(
                ImageBuffer::from_pixel(size, size, Rgba([i * 80, 40, 200 - i * 60, 255])),
                0,
                0,
                image::Delay::from_numer_denom_ms(120, 1),
            )
        })
        .collect()
}

/// Loop count as a number; 0 = forever.
fn loops(l: image::metadata::LoopCount) -> u32 {
    match l {
        image::metadata::LoopCount::Infinite => 0,
        image::metadata::LoopCount::Finite(n) => n.get(),
    }
}

fn webp_frames(path: &Path) -> (usize, u32, (u32, u32)) {
    use image::codecs::webp::WebPDecoder;
    use image::AnimationDecoder;
    let dec = WebPDecoder::new(std::io::BufReader::new(fs::File::open(path).unwrap())).unwrap();
    assert!(dec.has_animation(), "{} is not animated", path.display());
    let count = loops(dec.loop_count());
    let frames = dec.into_frames().collect_frames().unwrap();
    (frames.len(), count, frames[0].buffer().dimensions())
}

#[test]
fn animated_webp_keeps_frames_and_loop_count() {
    let dir = tmpdir();
    let src = dir.path().join("anim.webp");
    {
        let frames = three_frames(40);
        let config = webp::WebPConfig::new().unwrap();
        let mut enc = webp::AnimEncoder::new(40, 40, &config);
        enc.set_loop_count(3);
        for (i, f) in frames.iter().enumerate() {
            enc.add_frame(webp::AnimFrame::from_rgba(f.buffer().as_raw(), 40, 40, i as i32 * 120));
        }
        fs::write(&src, &*enc.try_encode().unwrap()).unwrap();
    }
    let s = Settings {
        width: 20,
        ..settings(OutputFormat::Keep)
    };
    let o = run(&src, &s, &dir.path().join("out"));
    assert!(o.note.is_none(), "{:?}", o.note);
    assert_eq!(webp_frames(o.out.as_ref().unwrap()), (3, 3, (20, 20)));

    let o = run(&src, &settings(OutputFormat::Jpeg), &dir.path().join("out"));
    assert!(o.note.unwrap().contains("first frame"));
}

#[test]
fn animated_gif_to_webp_stays_animated_and_gif_loop_count_survives() {
    use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
    use image::AnimationDecoder;
    let dir = tmpdir();
    let src = dir.path().join("twice.gif");
    {
        let mut enc = GifEncoder::new(fs::File::create(&src).unwrap());
        enc.set_repeat(Repeat::Finite(2)).unwrap();
        enc.encode_frames(three_frames(30)).unwrap();
    }
    let o = run(&src, &settings(OutputFormat::Webp), &dir.path().join("out"));
    assert_eq!(webp_frames(o.out.as_ref().unwrap()).0, 3);
    let dec = image::codecs::webp::WebPDecoder::new(std::io::BufReader::new(fs::File::open(o.out.unwrap()).unwrap()))
        .unwrap();
    let delays: Vec<u32> = dec
        .into_frames()
        .collect_frames()
        .unwrap()
        .iter()
        .map(|f| f.delay().numer_denom_ms().0 / f.delay().numer_denom_ms().1.max(1))
        .collect();
    assert_eq!(delays, [120, 120, 120]);

    let o = run(&src, &settings(OutputFormat::Keep), &dir.path().join("out"));
    let dec = GifDecoder::new(std::io::BufReader::new(fs::File::open(o.out.unwrap()).unwrap())).unwrap();
    assert_eq!(loops(dec.loop_count()), 2);
}

#[test]
fn preview_of_an_animation_uses_the_first_frame() {
    let dir = tmpdir();
    let src = dir.path().join("anim.gif");
    {
        let mut enc = image::codecs::gif::GifEncoder::new(fs::File::create(&src).unwrap());
        enc.encode_frames(three_frames(50)).unwrap();
    }
    let p = preview(&src, 1200).unwrap();
    assert_eq!((p.width, p.height), (50, 50));
}

#[test]
fn animated_png_converts_to_animated_webp_and_warns_otherwise() {
    let dir = tmpdir();
    let src = dir.path().join("anim.png");
    {
        let mut enc = png::Encoder::new(fs::File::create(&src).unwrap(), 30, 30);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_animated(3, 0).unwrap();
        enc.set_frame_delay(1, 10).unwrap();
        let mut w = enc.write_header().unwrap();
        for f in three_frames(30) {
            w.write_image_data(f.buffer().as_raw()).unwrap();
        }
        w.finish().unwrap();
    }
    let o = run(&src, &settings(OutputFormat::Webp), &dir.path().join("out"));
    assert_eq!(webp_frames(o.out.as_ref().unwrap()).0, 3);

    let s = Settings {
        width: 15,
        ..settings(OutputFormat::Keep)
    };
    let o = run(&src, &s, &dir.path().join("out"));
    assert!(o.note.unwrap().contains("first frame"));
}

#[test]
fn huge_animation_canvas_fails_cleanly() {
    let dir = tmpdir();
    let src = dir.path().join("huge.gif");
    // 65535 x 65535 logical screen (16 GiB as RGBA) holding one 1x1 frame.
    let mut gif = b"GIF89a".to_vec();
    gif.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0x80, 0, 0]); // size, global table of 2 colours
    gif.extend_from_slice(&[0, 0, 0, 255, 255, 255]);
    gif.extend_from_slice(&[0x2C, 0, 0, 0, 0, 1, 0, 1, 0, 0]); // image descriptor 1x1
    gif.extend_from_slice(&[0x02, 0x02, 0x44, 0x01, 0x00, 0x3B]);
    fs::write(&src, gif).unwrap();
    let err = process_one(&job(&src), &Settings::default(), &dir.path().join("out")).unwrap_err();
    assert!(
        format!("{err:#}").contains("too large") || format!("{err:#}").contains("limit"),
        "{err:#}"
    );
    assert!(preview(&src, 1200).is_err());
}

#[test]
fn wide_gamut_animation_is_converted_to_srgb() {
    let dir = tmpdir();
    let src = dir.path().join("p3-anim.png");
    let pixel = Rgba([200u8, 100, 50, 255]);
    {
        let mut info = png::Info::with_size(8, 8);
        info.color_type = png::ColorType::Rgba;
        info.bit_depth = png::BitDepth::Eight;
        info.icc_profile = Some(p3_profile().into());
        let mut enc = png::Encoder::with_info(fs::File::create(&src).unwrap(), info).unwrap();
        enc.set_animated(2, 0).unwrap();
        let mut w = enc.write_header().unwrap();
        // Frames must differ, or libwebp merges them into one still image.
        for frame in [pixel, Rgba([60, 160, 90, 255])] {
            w.write_image_data(ImageBuffer::from_pixel(8, 8, frame).as_raw())
                .unwrap();
        }
        w.finish().unwrap();
    }
    let o = run(&src, &settings(OutputFormat::Webp), &dir.path().join("out"));
    assert_eq!(
        webp_frames(o.out.as_ref().unwrap()).0,
        2,
        "must go through the animation path"
    );
    let img = open(o.out.as_ref().unwrap()).to_rgba8();
    let got = img.get_pixel(4, 4).0;
    assert!(got[0] > pixel.0[0] + 5, "expected converted pixel, got {got:?}");
}

#[test]
fn oversized_target_fails_that_file_only() {
    let dir = tmpdir();
    let src = dir.path().join("a.png");
    sample(image::ColorType::Rgb8, 10, 10).save(&src).unwrap();
    let s = Settings {
        width: 100_000,
        height: 100_000,
        keep_aspect: false,
        no_enlarge: false,
        ..settings(OutputFormat::Png)
    };
    let err = process_one(&job(&src), &s, &dir.path().join("out")).unwrap_err();
    assert!(format!("{err:#}").contains("too large"), "{err:#}");
}

#[test]
fn locked_crop_shape_holds_on_other_aspect_ratios() {
    // A 1:1 crop drawn on a 4000x3000 photo, then copied to a portrait one.
    let c = Crop {
        x: 0.1,
        y: 0.1,
        w: 0.6,
        h: 0.8,
        ratio: Some(1.0),
    };
    assert_eq!(crop_rect(4000, 3000, &c), Some((400, 300, 2400, 2400)));
    let (_, _, w, h) = crop_rect(3000, 4000, &c).unwrap();
    assert_eq!(w, h);
    let c = Crop {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
        ratio: Some(16.0 / 9.0),
    };
    assert_eq!(crop_rect(1000, 1000, &c), Some((0, 218, 1000, 563)));
}

#[test]
fn tiny_file_claiming_a_gigantic_image_fails_cleanly() {
    let dir = tmpdir();
    let src = dir.path().join("liar.png");
    {
        // Header only: 1,000,000 x 2,000,000,000 RGBA (8 PB decoded).
        let mut enc = png::Encoder::new(fs::File::create(&src).unwrap(), 1_000_000, 2_000_000_000);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        // The decoder reads the header up to the first data chunk.
        w.write_chunk(png::chunk::IDAT, &[0x78, 0x9c]).unwrap();
        std::mem::forget(w);
    }
    assert!(fs::metadata(&src).unwrap().len() < 100);
    let err = preview(&src, 1200).err().expect("preview must fail");
    assert!(format!("{err:#}").contains("too large"), "{err:#}");
    assert!(process_one(&job(&src), &Settings::default(), &dir.path().join("out")).is_err());
}

/// An extended WebP (VP8X + the VP8L image from a simple lossless file) with
/// an EXIF chunk that declares `exif_size` bytes and holds `exif_data`.
fn extended_webp(exif_size: u32, exif_data: &[u8]) -> Vec<u8> {
    let mut simple = Vec::new();
    sample(image::ColorType::Rgba8, 8, 8)
        .write_to(&mut std::io::Cursor::new(&mut simple), ImageFormat::WebP)
        .unwrap();
    assert_eq!(&simple[12..16], b"VP8L");
    let mut body = b"WEBP".to_vec();
    body.extend_from_slice(b"VP8X");
    body.extend_from_slice(&10u32.to_le_bytes());
    body.extend_from_slice(&[0x08 | 0x10, 0, 0, 0, 7, 0, 0, 7, 0, 0]); // EXIF + alpha flags, 8x8 canvas
    body.extend_from_slice(&simple[12..]);
    body.extend_from_slice(b"EXIF");
    body.extend_from_slice(&exif_size.to_le_bytes());
    body.extend_from_slice(exif_data);
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

#[test]
fn webp_chunk_claiming_more_than_the_file_fails_cleanly() {
    let dir = tmpdir();
    let src = dir.path().join("liar.webp");
    fs::write(&src, extended_webp(0xFFFF_FF00, &[0; 16])).unwrap();
    let err = preview(&src, 1200).err().expect("preview must fail");
    assert!(format!("{err:#}").contains("corrupt"), "{err:#}");
    assert!(process_one(&job(&src), &Settings::default(), &dir.path().join("out")).is_err());

    // The same layout with an honest EXIF chunk still opens.
    let ok = dir.path().join("honest.webp");
    fs::write(&ok, extended_webp(16, &[0; 16])).unwrap();
    let o = run(&ok, &settings(OutputFormat::Png), &dir.path().join("out"));
    assert_eq!(open(o.out.as_ref().unwrap()).dimensions(), (8, 8));
}

#[test]
fn animation_with_too_many_frames_fails_cleanly() {
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Frame};
    let dir = tmpdir();
    let src = dir.path().join("many.gif");
    {
        let mut enc = GifEncoder::new(fs::File::create(&src).unwrap());
        let frames = (0..10_001u32).map(|i| {
            Frame::from_parts(
                ImageBuffer::from_pixel(1, 1, Rgba([(i % 256) as u8, 0, 0, 255])),
                0,
                0,
                Delay::from_numer_denom_ms(20, 1),
            )
        });
        enc.encode_frames(frames).unwrap();
    }
    let err = process_one(&job(&src), &Settings::default(), &dir.path().join("out")).unwrap_err();
    assert!(format!("{err:#}").contains("too many frames"), "{err:#}");
}

/// 4x4 RGB + one alpha sample TIFF, every pixel (128, 0, 0, 128), with the
/// alpha marked associated (premultiplied) or unassociated.
fn rgba_tiff(associated: bool) -> Vec<u8> {
    use tiff::encoder::{colortype, TiffEncoder};
    use tiff::tags::ExtraSamples;
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut enc = TiffEncoder::new(&mut bytes).unwrap();
        let mut img = enc.new_image::<colortype::RGB8>(4, 4).unwrap();
        let kind = if associated {
            ExtraSamples::AssociatedAlpha
        } else {
            ExtraSamples::UnassociatedAlpha
        };
        img.extra_samples(&[kind]).unwrap();
        img.write_data(&[128u8, 0, 0, 128].repeat(16)).unwrap();
    }
    bytes.into_inner()
}

#[test]
fn associated_alpha_tiff_is_unpremultiplied() {
    let dir = tmpdir();
    for (associated, want) in [(true, [255u8, 0, 0, 128]), (false, [128, 0, 0, 128])] {
        let src = dir.path().join(format!("alpha-{associated}.tif"));
        fs::write(&src, rgba_tiff(associated)).unwrap();
        let o = run(&src, &settings(OutputFormat::Png), &dir.path().join("out"));
        let px = open(o.out.as_ref().unwrap()).to_rgba8().get_pixel(1, 1).0;
        assert!(
            px.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 1),
            "associated={associated}: got {px:?}, want {want:?}"
        );
    }
}

#[test]
fn bytes_after_the_webp_riff_are_ignored() {
    // Browsers and libwebp ignore data after the RIFF; so does Imaginify,
    // including a "chunk header" there that claims ~4 GiB.
    let dir = tmpdir();
    let out = dir.path().join("out");
    for (name, tail) in [
        ("text.webp", b"trailing comment".to_vec()),
        (
            "fake-chunk.webp",
            [b"ICCP".as_slice(), &0xFFFF_FF00u32.to_le_bytes()].concat(),
        ),
    ] {
        let src = dir.path().join(name);
        let mut bytes = extended_webp(16, &[0; 16]);
        bytes.extend_from_slice(&tail);
        fs::write(&src, bytes).unwrap();
        let o = run(&src, &settings(OutputFormat::Png), &out);
        assert_eq!(open(o.out.as_ref().unwrap()).dimensions(), (8, 8), "{name}");
    }
}

#[test]
fn resize_with_a_huge_intermediate_fails_cleanly() {
    let dir = tmpdir();
    let src = dir.path().join("line.png");
    DynamicImage::ImageRgb8(ImageBuffer::from_pixel(100_000, 1, Rgb([9, 9, 9])))
        .save(&src)
        .unwrap();
    let s = Settings {
        width: 1,
        height: 100_000,
        keep_aspect: false,
        no_enlarge: false,
        ..settings(OutputFormat::Png)
    };
    let err = process_one(&job(&src), &s, &dir.path().join("out")).unwrap_err();
    assert!(format!("{err:#}").contains("too much memory"), "{err:#}");
}

/// PNG of 4x4 `pixel` carrying `icc`.
fn png_with_profile(path: &Path, pixel: [u8; 3], icc: Vec<u8>) {
    let buf = ImageBuffer::from_pixel(4, 4, Rgb(pixel));
    let mut bytes = Vec::new();
    let mut enc = PngEncoder::new(&mut bytes);
    enc.set_icc_profile(icc).unwrap();
    enc.write_image(&buf, 4, 4, image::ExtendedColorType::Rgb8).unwrap();
    fs::write(path, bytes).unwrap();
}

#[test]
fn every_rgb_profile_is_honoured_srgb_included() {
    use lcms2::{CIExyY, CIExyYTRIPLE, Profile, ToneCurve};
    let dir = tmpdir();
    let out = dir.path().join("out");

    // Linear-light sRGB primaries: mid grey 128 is much lighter in sRGB.
    let xy = |x, y| CIExyY { x, y, Y: 1.0 };
    let linear = ToneCurve::new(1.0);
    let primaries = CIExyYTRIPLE {
        Red: xy(0.64, 0.33),
        Green: xy(0.30, 0.60),
        Blue: xy(0.15, 0.06),
    };
    let linear_srgb = Profile::new_rgb(&xy(0.3127, 0.3290), &primaries, &[&linear, &linear, &linear])
        .unwrap()
        .icc()
        .unwrap();
    let src = dir.path().join("linear.png");
    png_with_profile(&src, [128, 128, 128], linear_srgb);
    let px = open(run(&src, &settings(OutputFormat::Png), &out).out.as_ref().unwrap())
        .to_rgb8()
        .get_pixel(1, 1)
        .0;
    assert!(px.iter().all(|&c| (180..=196).contains(&c)), "linear grey -> {px:?}");

    // A real sRGB profile leaves the pixels where they were.
    let src = dir.path().join("srgb.png");
    png_with_profile(&src, [200, 100, 50], Profile::new_srgb().icc().unwrap());
    let px = open(run(&src, &settings(OutputFormat::Png), &out).out.as_ref().unwrap())
        .to_rgb8()
        .get_pixel(1, 1)
        .0;
    assert!(
        px.iter().zip([200u8, 100, 50]).all(|(a, b)| a.abs_diff(b) <= 2),
        "sRGB -> {px:?}"
    );
}

#[test]
fn webp_chunk_nested_in_an_animation_frame_fails_cleanly() {
    let dir = tmpdir();
    let src = dir.path().join("nested.webp");
    let mut body = b"WEBP".to_vec();
    body.extend_from_slice(b"VP8X");
    body.extend_from_slice(&10u32.to_le_bytes());
    body.extend_from_slice(&[0x02 | 0x20, 0, 0, 0, 7, 0, 0, 7, 0, 0]); // animation + ICC flags, 8x8
    body.extend_from_slice(b"ANIM");
    body.extend_from_slice(&6u32.to_le_bytes());
    body.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    // A frame whose only sub-chunk claims ~4 GiB but the frame holds 8 bytes of it.
    body.extend_from_slice(b"ANMF");
    body.extend_from_slice(&(16u32 + 8).to_le_bytes());
    body.extend_from_slice(&[0, 0, 0, 0, 0, 0, 7, 0, 0, 7, 0, 0, 100, 0, 0, 0]);
    body.extend_from_slice(b"ICCP");
    body.extend_from_slice(&0xFFFF_FF00u32.to_le_bytes());
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&body);
    fs::write(&src, bytes).unwrap();
    let err = preview(&src, 1200).err().expect("preview must fail");
    assert!(format!("{err:#}").contains("corrupt"), "{err:#}");
}

#[test]
fn long_thin_image_preview_fails_cleanly() {
    // 20 million x 1: small to decode (20 MB), but squeezing it into a
    // preview would need ~2 GB of filter coefficients.
    let dir = tmpdir();
    let src = dir.path().join("thread.png");
    DynamicImage::ImageLuma8(ImageBuffer::from_pixel(20_000_000, 1, image::Luma([7u8])))
        .save(&src)
        .unwrap();
    let err = preview(&src, 1200).err().expect("preview must fail");
    assert!(format!("{err:#}").contains("too much memory"), "{err:#}");
}

#[test]
fn webp_pixel_bytes_that_look_like_a_chunk_header_fail_cleanly() {
    // Every chunk is honestly sized, but the 8-byte VP8L payload inside the
    // frame reads as an ICCP header declaring ~4 GiB, followed by another
    // sub-chunk. image-webp's frame index misreads it; Imaginify never asks
    // that decoder for metadata, so nothing large is allocated.
    let dir = tmpdir();
    let src = dir.path().join("lookalike.webp");
    let mut frame = vec![0, 0, 0, 0, 0, 0, 7, 0, 0, 7, 0, 0, 100, 0, 0, 0];
    frame.extend_from_slice(b"VP8L");
    frame.extend_from_slice(&8u32.to_le_bytes());
    frame.extend_from_slice(b"ICCP");
    frame.extend_from_slice(&0xFFFF_FF00u32.to_le_bytes());
    frame.extend_from_slice(b"JUNK");
    frame.extend_from_slice(&0u32.to_le_bytes());
    let mut body = b"WEBP".to_vec();
    body.extend_from_slice(b"VP8X");
    body.extend_from_slice(&10u32.to_le_bytes());
    body.extend_from_slice(&[0x02 | 0x20, 0, 0, 0, 7, 0, 0, 7, 0, 0]);
    body.extend_from_slice(b"ANIM");
    body.extend_from_slice(&6u32.to_le_bytes());
    body.extend_from_slice(&[0; 6]);
    body.extend_from_slice(b"ANMF");
    body.extend_from_slice(&(frame.len() as u32).to_le_bytes());
    body.extend_from_slice(&frame);
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&body);
    fs::write(&src, bytes).unwrap();
    assert!(preview(&src, 1200).is_err());
    assert!(process_one(&job(&src), &Settings::default(), &dir.path().join("out")).is_err());
}

/// Extended still WebP: VP8X, optional ICCP, the VP8L image (w x h), optional EXIF.
fn still_webp_with(icc: Option<Vec<u8>>, exif: Option<Vec<u8>>, w: u32, h: u32) -> Vec<u8> {
    let mut simple = Vec::new();
    DynamicImage::ImageRgba8(ImageBuffer::from_pixel(w, h, Rgba([200, 100, 50, 255])))
        .write_to(&mut std::io::Cursor::new(&mut simple), ImageFormat::WebP)
        .unwrap();
    let flags = 0x10 | if icc.is_some() { 0x20 } else { 0 } | if exif.is_some() { 0x08 } else { 0 };
    let dim = |v: u32| (v - 1).to_le_bytes();
    let mut body = b"WEBP".to_vec();
    body.extend_from_slice(b"VP8X");
    body.extend_from_slice(&10u32.to_le_bytes());
    body.extend_from_slice(&[flags, 0, 0, 0]);
    body.extend_from_slice(&dim(w)[..3]);
    body.extend_from_slice(&dim(h)[..3]);
    let mut chunk = |fourcc: &[u8], data: &[u8]| {
        body.extend_from_slice(fourcc);
        body.extend_from_slice(&(data.len() as u32).to_le_bytes());
        body.extend_from_slice(data);
        if data.len() % 2 == 1 {
            body.push(0);
        }
    };
    if let Some(icc) = &icc {
        chunk(b"ICCP", icc);
    }
    chunk(b"VP8L", &simple[20..]);
    if let Some(exif) = &exif {
        chunk(b"EXIF", exif);
    }
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

#[test]
fn webp_profile_and_orientation_are_still_applied() {
    let dir = tmpdir();
    let out = dir.path().join("out");

    let src = dir.path().join("p3.webp");
    fs::write(&src, still_webp_with(Some(p3_profile()), None, 8, 8)).unwrap();
    let px = open(run(&src, &settings(OutputFormat::Png), &out).out.as_ref().unwrap())
        .to_rgb8()
        .get_pixel(1, 1)
        .0;
    assert!(px[0] > 205, "P3 WebP not converted: {px:?}");

    // EXIF orientation 6 (rotate 90 CW), little-endian TIFF block.
    let mut tiff = b"II*\0\x08\0\0\0\x01\0".to_vec();
    tiff.extend_from_slice(&[0x12, 0x01, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0]);
    let src = dir.path().join("rotated.webp");
    fs::write(&src, still_webp_with(None, Some(tiff), 12, 4)).unwrap();
    let o = run(&src, &settings(OutputFormat::Png), &out);
    assert_eq!(open(o.out.as_ref().unwrap()).dimensions(), (4, 12));
}

#[test]
fn oversized_input_file_is_refused_before_decoding() {
    let dir = tmpdir();
    let src = dir.path().join("padded.png");
    sample(image::ColorType::Rgb8, 4, 4).save(&src).unwrap();
    // Sparse: costs no disk, but the file reports 600 MB.
    fs::OpenOptions::new()
        .write(true)
        .open(&src)
        .unwrap()
        .set_len(600 * 1024 * 1024)
        .unwrap();
    let err = preview(&src, 1200).err().expect("preview must fail");
    assert!(format!("{err:#}").contains("file too large"), "{err:#}");
}

#[test]
fn sixteen_bit_animated_png_opens_with_a_note() {
    let dir = tmpdir();
    let src = dir.path().join("deep-anim.png");
    {
        let mut enc = png::Encoder::new(fs::File::create(&src).unwrap(), 6, 6);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Sixteen);
        enc.set_animated(2, 0).unwrap();
        enc.set_frame_delay(1, 10).unwrap();
        let mut w = enc.write_header().unwrap();
        for v in [40000u16, 20000] {
            let frame: Vec<u8> = [v, 0, 0, 65535]
                .iter()
                .flat_map(|c| c.to_be_bytes())
                .collect::<Vec<_>>()
                .repeat(36);
            w.write_image_data(&frame).unwrap();
        }
        w.finish().unwrap();
    }
    let p = preview(&src, 1200).unwrap();
    assert_eq!((p.width, p.height), (6, 6));
    let o = run(&src, &settings(OutputFormat::Webp), &dir.path().join("out"));
    assert!(o.note.unwrap().contains("16-bit animated PNG"));
    let px = open(o.out.as_ref().unwrap()).to_rgba8().get_pixel(2, 2).0;
    assert!(px[0].abs_diff(156) <= 3, "first frame red ~156, got {px:?}");
}
