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
