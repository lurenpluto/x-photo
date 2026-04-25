use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use std::io::Write;

use chrono::{Datelike, NaiveDate, Utc, Weekday};
use filetime::{FileTime, set_file_mtime};
use image::{ImageBuffer, Rgb};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;

const USAGE: &str = "Usage: cargo run --bin testdata_builder -- <output_dir> [--strategy sample|family_us_weekends_small|family_us_weekends_medium|family_us_weekends_2025] [--year 2025] [--seed 42] [--clean] [--manifest <path>]";

#[derive(Debug)]
struct BuilderOptions {
    output_dir: PathBuf,
    strategy: String,
    year: i32,
    seed: u64,
    clean: bool,
    manifest_path: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }

    let options = match parse_options(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            std::process::exit(1);
        }
    };

    if options.clean && options.output_dir.exists() {
        std::fs::remove_dir_all(&options.output_dir)?;
    }
    std::fs::create_dir_all(&options.output_dir)?;

    let plans = build_plans(&options.strategy, options.year, options.seed)?;

    let total = plans.len();
    println!(
        "planning completed: strategy={} year={} seed={} total_photos={}",
        options.strategy, options.year, options.seed, total
    );

    let started_at = Instant::now();
    let mut last_progress_at = started_at;

    let mut generated = 0_usize;
    let mut generated_photos = Vec::with_capacity(total);
    for plan in &plans {
        let dir_path = options.output_dir.join(&plan.album_dir_name);
        std::fs::create_dir_all(&dir_path)?;
        let file_path = dir_path.join(format!("{}.jpg", plan.marker));

        generate_marked_image(&file_path, &plan.marker)?;
        set_exif_metadata_in_jpeg(&file_path, &plan.exif)?;

        let ft = FileTime::from_unix_time(plan.mtime_unix, 0);
        set_file_mtime(&file_path, ft)?;
        generated_photos.push(GeneratedPhoto {
            album_dir_name: plan.album_dir_name.clone(),
            marker: plan.marker.clone(),
            file_path: file_path.to_string_lossy().to_string(),
            relative_path: format!("{}/{}.jpg", plan.album_dir_name, plan.marker),
            exif: plan.exif.clone(),
            mtime_unix: plan.mtime_unix,
        });
        generated += 1;

        let now = Instant::now();
        if generated == total
            || generated == 1
            || generated.is_multiple_of(100)
            || now.duration_since(last_progress_at).as_secs_f64() >= 0.8
        {
            last_progress_at = now;
            render_progress(generated, total, started_at)?;
        }
    }

    println!();

    println!(
        "test data generated at {} (strategy={}, photos={})",
        options.output_dir.display(),
        options.strategy,
        generated
    );

    if let Some(manifest_path) = &options.manifest_path {
        write_manifest(&options, generated_photos, manifest_path)?;
        println!("manifest written to {}", manifest_path.display());
    }

    Ok(())
}

fn parse_options(args: Vec<String>) -> Result<BuilderOptions, String> {
    if args.is_empty() {
        return Err("missing output_dir".to_string());
    }

    let output_dir = PathBuf::from(&args[0]);
    let mut options = BuilderOptions {
        output_dir,
        strategy: "family_us_weekends_2025".to_string(),
        year: 2025,
        seed: 42,
        clean: false,
        manifest_path: None,
    };

    let mut idx = 1;
    while idx < args.len() {
        match args[idx].as_str() {
            "--help" | "-h" => return Err("help requested".to_string()),
            "--strategy" => {
                options.strategy = parse_required_value(&args, idx, "--strategy")?.to_string();
                idx += 2;
            }
            "--year" => {
                let value = parse_required_value(&args, idx, "--year")?;
                options.year = value
                    .parse()
                    .map_err(|e| format!("invalid --year value '{value}': {e}"))?;
                idx += 2;
            }
            "--seed" => {
                let value = parse_required_value(&args, idx, "--seed")?;
                options.seed = value
                    .parse()
                    .map_err(|e| format!("invalid --seed value '{value}': {e}"))?;
                idx += 2;
            }
            "--clean" => {
                options.clean = true;
                idx += 1;
            }
            "--manifest" => {
                options.manifest_path = Some(PathBuf::from(parse_required_value(
                    &args,
                    idx,
                    "--manifest",
                )?));
                idx += 2;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(options)
}

fn parse_required_value<'a>(args: &'a [String], idx: usize, flag: &str) -> Result<&'a str, String> {
    let value = args
        .get(idx + 1)
        .ok_or_else(|| format!("{flag} requires a value"))?;
    if value.starts_with("--") {
        return Err(format!("{flag} requires a value"));
    }
    Ok(value)
}

fn render_progress(
    generated: usize,
    total: usize,
    started_at: Instant,
) -> Result<(), Box<dyn std::error::Error>> {
    let width = 34_usize;
    let ratio = if total == 0 {
        1.0
    } else {
        generated as f64 / total as f64
    }
    .clamp(0.0, 1.0);

    let filled = (ratio * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    let bar = format!("{}{}", "#".repeat(filled), "-".repeat(empty));

    let elapsed = started_at.elapsed().as_secs_f64();
    let rate = if elapsed > 0.0 {
        generated as f64 / elapsed
    } else {
        0.0
    };
    let remaining = total.saturating_sub(generated);
    let eta_secs = if rate > 0.0 {
        (remaining as f64 / rate).round() as u64
    } else {
        0
    };

    print!(
        "\rprogress [{}] {:>6.2}% ({}/{}) rate={:>6.1} img/s eta={}s",
        bar,
        ratio * 100.0,
        generated,
        total,
        rate,
        eta_secs,
    );
    std::io::stdout().flush()?;
    Ok(())
}

#[derive(Clone)]
struct PhotoPlan {
    album_dir_name: String,
    marker: String,
    exif: ExifMeta,
    mtime_unix: i64,
}

#[derive(Clone, Serialize)]
struct ExifMeta {
    date_time_original: String,
    gps_lat: Option<f64>,
    gps_lng: Option<f64>,
}

#[derive(Clone, Serialize)]
struct GeneratedPhoto {
    album_dir_name: String,
    marker: String,
    file_path: String,
    relative_path: String,
    exif: ExifMeta,
    mtime_unix: i64,
}

#[derive(Serialize)]
struct GeneratedManifest {
    strategy: String,
    year: i32,
    seed: u64,
    generated_at: String,
    output_dir: String,
    total_photos: usize,
    albums: Vec<GeneratedAlbum>,
    photos: Vec<GeneratedPhoto>,
}

#[derive(Serialize)]
struct GeneratedAlbum {
    album_dir_name: String,
    photo_count: usize,
}

#[derive(Clone, Copy)]
struct FamilyWeekendSpec {
    max_weekends: Option<usize>,
    min_photos_per_trip: u32,
    max_photos_per_trip: u32,
}

#[derive(Clone)]
struct Park {
    name: &'static str,
    lat: f64,
    lng: f64,
}

fn build_plans(
    strategy: &str,
    year: i32,
    seed: u64,
) -> Result<Vec<PhotoPlan>, Box<dyn std::error::Error>> {
    match strategy {
        "sample" => Ok(build_sample_plans()),
        "family_us_weekends_small" => build_family_us_weekend_plans(
            year,
            seed,
            FamilyWeekendSpec {
                max_weekends: Some(4),
                min_photos_per_trip: 3,
                max_photos_per_trip: 6,
            },
        ),
        "family_us_weekends_medium" => build_family_us_weekend_plans(
            year,
            seed,
            FamilyWeekendSpec {
                max_weekends: Some(12),
                min_photos_per_trip: 8,
                max_photos_per_trip: 16,
            },
        ),
        "family_us_weekends_2025" | "family_us_weekends" => build_family_us_weekend_plans(
            year,
            seed,
            FamilyWeekendSpec {
                max_weekends: None,
                min_photos_per_trip: 10,
                max_photos_per_trip: 100,
            },
        ),
        other => Err(format!("unknown strategy: {other}").into()),
    }
}

fn write_manifest(
    options: &BuilderOptions,
    photos: Vec<GeneratedPhoto>,
    manifest_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = manifest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut album_counts = BTreeMap::new();
    for photo in &photos {
        *album_counts
            .entry(photo.album_dir_name.clone())
            .or_insert(0_usize) += 1;
    }
    let albums = album_counts
        .into_iter()
        .map(|(album_dir_name, photo_count)| GeneratedAlbum {
            album_dir_name,
            photo_count,
        })
        .collect();

    let manifest = GeneratedManifest {
        strategy: options.strategy.clone(),
        year: options.year,
        seed: options.seed,
        generated_at: Utc::now().to_rfc3339(),
        output_dir: options.output_dir.to_string_lossy().to_string(),
        total_photos: photos.len(),
        albums,
        photos,
    };
    let bytes = serde_json::to_vec_pretty(&manifest)?;
    std::fs::write(manifest_path, bytes)?;
    Ok(())
}

fn build_family_us_weekend_plans(
    year: i32,
    seed: u64,
    spec: FamilyWeekendSpec,
) -> Result<Vec<PhotoPlan>, Box<dyn std::error::Error>> {
    if spec.min_photos_per_trip > spec.max_photos_per_trip {
        return Err("invalid family weekend photo count range".into());
    }

    let parks = vec![
        Park {
            name: "Yellowstone.National.Park",
            lat: 44.60,
            lng: -110.50,
        },
        Park {
            name: "Yosemite.National.Park",
            lat: 37.86,
            lng: -119.54,
        },
        Park {
            name: "Grand.Canyon.National.Park",
            lat: 36.10,
            lng: -112.11,
        },
        Park {
            name: "Zion.National.Park",
            lat: 37.30,
            lng: -113.03,
        },
        Park {
            name: "Great.Smoky.Mountains",
            lat: 35.65,
            lng: -83.50,
        },
        Park {
            name: "Rocky.Mountain.National.Park",
            lat: 40.34,
            lng: -105.68,
        },
        Park {
            name: "Acadia.National.Park",
            lat: 44.35,
            lng: -68.21,
        },
        Park {
            name: "Olympic.National.Park",
            lat: 47.80,
            lng: -123.70,
        },
        Park {
            name: "Glacier.National.Park",
            lat: 48.70,
            lng: -113.80,
        },
        Park {
            name: "Bryce.Canyon.National.Park",
            lat: 37.62,
            lng: -112.16,
        },
    ];

    let mut rng = StdRng::seed_from_u64(seed);
    let mut plans = Vec::new();
    let mut weekend_count = 0_usize;

    let mut day =
        NaiveDate::from_ymd_opt(year, 1, 1).ok_or_else(|| format!("invalid year {}", year))?;
    let end =
        NaiveDate::from_ymd_opt(year, 12, 31).ok_or_else(|| format!("invalid year {}", year))?;

    while day <= end {
        if day.weekday() == Weekday::Sat {
            if let Some(max_weekends) = spec.max_weekends
                && weekend_count >= max_weekends
            {
                break;
            }
            weekend_count += 1;
            let park = parks[rng.gen_range(0..parks.len())].clone();
            let album_dir = format!(
                "{:04}.{:02}.{:02}.{}",
                day.year(),
                day.month(),
                day.day(),
                park.name
            );

            let photo_count = rng.gen_range(spec.min_photos_per_trip..=spec.max_photos_per_trip);
            let trip_lat_center = park.lat + rng.gen_range(-0.03_f64..0.03_f64);
            let trip_lng_center = park.lng + rng.gen_range(-0.03_f64..0.03_f64);

            for i in 0..photo_count {
                let hour = rng.gen_range(6..=23);
                let minute = rng.gen_range(0..=59);
                let second = rng.gen_range(0..=59);
                let dt = format!(
                    "{:04}:{:02}:{:02} {:02}:{:02}:{:02}",
                    day.year(),
                    day.month(),
                    day.day(),
                    hour,
                    minute,
                    second
                );

                let lat = trip_lat_center + rng.gen_range(-0.004_f64..0.004_f64);
                let lng = trip_lng_center + rng.gen_range(-0.004_f64..0.004_f64);
                let marker = format!(
                    "IMG_{:04}{:02}{:02}_{:03}",
                    day.year(),
                    day.month(),
                    day.day(),
                    i + 1
                );

                plans.push(PhotoPlan {
                    album_dir_name: album_dir.clone(),
                    marker,
                    exif: ExifMeta {
                        date_time_original: dt,
                        gps_lat: Some(lat),
                        gps_lng: Some(lng),
                    },
                    mtime_unix: day
                        .and_hms_opt(hour, minute, second)
                        .map(|v| v.and_utc().timestamp())
                        .unwrap_or(1_735_689_600),
                });
            }
        }
        day = day.succ_opt().ok_or("date overflow")?;
    }

    Ok(plans)
}

fn build_sample_plans() -> Vec<PhotoPlan> {
    vec![PhotoPlan {
        album_dir_name: "2020.02.01.Sample.Album".to_string(),
        marker: "IMG_SAMPLE_001".to_string(),
        exif: ExifMeta {
            date_time_original: "2020:02:01 10:11:12".to_string(),
            gps_lat: Some(37.86),
            gps_lng: Some(-119.54),
        },
        mtime_unix: 1_580_513_600,
    }]
}

fn generate_marked_image(path: &Path, marker: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut img = ImageBuffer::from_pixel(1280, 720, Rgb([255_u8, 255_u8, 255_u8]));
    draw_marker(&mut img, marker, 40, 80, 6, Rgb([10, 10, 10]));
    img.save(path)?;
    Ok(())
}

fn draw_marker(
    img: &mut ImageBuffer<Rgb<u8>, Vec<u8>>,
    text: &str,
    x: u32,
    y: u32,
    scale: u32,
    color: Rgb<u8>,
) {
    let mut cursor = x;
    for c in text.chars() {
        let glyph = glyph_5x7(c);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if (bits >> (4 - col)) & 1 == 1 {
                    fill_block(
                        img,
                        cursor + (col as u32) * scale,
                        y + (row as u32) * scale,
                        scale,
                        color,
                    );
                }
            }
        }
        cursor += 6 * scale;
    }
}

fn fill_block(img: &mut ImageBuffer<Rgb<u8>, Vec<u8>>, x: u32, y: u32, size: u32, color: Rgb<u8>) {
    for dx in 0..size {
        for dy in 0..size {
            let px = x + dx;
            let py = y + dy;
            if px < img.width() && py < img.height() {
                *img.get_pixel_mut(px, py) = color;
            }
        }
    }
}

fn glyph_5x7(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        '_' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        _ => [
            0b11111, 0b00001, 0b00110, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
    }
}

fn set_exif_metadata_in_jpeg(
    path: &Path,
    meta: &ExifMeta,
) -> Result<(), Box<dyn std::error::Error>> {
    if meta.date_time_original.len() != 19 {
        return Err(format!(
            "invalid exif datetime format '{}', expect YYYY:MM:DD HH:MM:SS",
            meta.date_time_original
        )
        .into());
    }

    let mut jpeg = std::fs::read(path)?;
    if jpeg.len() < 4 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        return Err(format!("{} is not a valid jpeg file", path.display()).into());
    }

    let exif_payload = build_exif_payload(meta);
    let mut out = Vec::with_capacity(jpeg.len() + exif_payload.len() + 4);
    out.extend_from_slice(&jpeg[0..2]);
    out.push(0xFF);
    out.push(0xE1);
    let seg_len = (exif_payload.len() + 2) as u16;
    out.extend_from_slice(&seg_len.to_be_bytes());
    out.extend_from_slice(&exif_payload);
    out.extend_from_slice(&jpeg.split_off(2));
    std::fs::write(path, out)?;
    Ok(())
}

fn build_exif_payload(meta: &ExifMeta) -> Vec<u8> {
    let dt = format!("{}\0", meta.date_time_original).into_bytes();

    let with_gps = meta.gps_lat.is_some() && meta.gps_lng.is_some();
    let ifd0_entries = if with_gps { 3_u16 } else { 2_u16 };
    let tiff_header_len = 8_u32;
    let ifd0_len = 2_u32 + (ifd0_entries as u32) * 12 + 4;
    let dt0_offset = tiff_header_len + ifd0_len;
    let exif_ifd_offset = dt0_offset + dt.len() as u32;

    let exif_ifd_entries = 1_u16;
    let exif_ifd_len = 2_u32 + (exif_ifd_entries as u32) * 12 + 4;
    let dto_offset = exif_ifd_offset + exif_ifd_len;

    let gps_ifd_offset = if with_gps {
        dto_offset + dt.len() as u32
    } else {
        0_u32
    };
    let gps_ifd_entries = 4_u16;
    let gps_ifd_len = if with_gps {
        2_u32 + (gps_ifd_entries as u32) * 12 + 4
    } else {
        0_u32
    };
    let gps_lat_offset = gps_ifd_offset + gps_ifd_len;
    let gps_lng_offset = gps_lat_offset + 24;

    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II*");
    tiff.push(0x00);
    tiff.extend_from_slice(&8_u32.to_le_bytes());

    tiff.extend_from_slice(&ifd0_entries.to_le_bytes());
    append_ifd_entry(&mut tiff, 0x0132, 2, dt.len() as u32, dt0_offset);
    append_ifd_entry(&mut tiff, 0x8769, 4, 1, exif_ifd_offset);
    if with_gps {
        append_ifd_entry(&mut tiff, 0x8825, 4, 1, gps_ifd_offset);
    }
    tiff.extend_from_slice(&0_u32.to_le_bytes());

    tiff.extend_from_slice(&dt);

    tiff.extend_from_slice(&exif_ifd_entries.to_le_bytes());
    append_ifd_entry(&mut tiff, 0x9003, 2, dt.len() as u32, dto_offset);
    tiff.extend_from_slice(&0_u32.to_le_bytes());
    tiff.extend_from_slice(&dt);

    if with_gps {
        let lat = meta.gps_lat.unwrap_or(0.0);
        let lng = meta.gps_lng.unwrap_or(0.0);
        let lat_ref = if lat >= 0.0 { b'N' } else { b'S' };
        let lng_ref = if lng >= 0.0 { b'E' } else { b'W' };
        let lat_rats = to_dms_rational(lat.abs());
        let lng_rats = to_dms_rational(lng.abs());

        tiff.extend_from_slice(&gps_ifd_entries.to_le_bytes());
        append_ifd_entry_ascii_inline(&mut tiff, 0x0001, lat_ref);
        append_ifd_entry(&mut tiff, 0x0002, 5, 3, gps_lat_offset);
        append_ifd_entry_ascii_inline(&mut tiff, 0x0003, lng_ref);
        append_ifd_entry(&mut tiff, 0x0004, 5, 3, gps_lng_offset);
        tiff.extend_from_slice(&0_u32.to_le_bytes());

        append_rational_triplet(&mut tiff, lat_rats);
        append_rational_triplet(&mut tiff, lng_rats);
    }

    let mut payload = Vec::with_capacity(6 + tiff.len());
    payload.extend_from_slice(b"Exif\0\0");
    payload.extend_from_slice(&tiff);
    payload
}

fn to_dms_rational(value: f64) -> [(u32, u32); 3] {
    let deg = value.floor();
    let min_full = (value - deg) * 60.0;
    let min = min_full.floor();
    let sec = (min_full - min) * 60.0;
    [
        (deg as u32, 1),
        (min as u32, 1),
        ((sec * 10_000.0).round() as u32, 10_000),
    ]
}

fn append_rational_triplet(buf: &mut Vec<u8>, triplet: [(u32, u32); 3]) {
    for (num, den) in triplet {
        buf.extend_from_slice(&num.to_le_bytes());
        buf.extend_from_slice(&den.to_le_bytes());
    }
}

fn append_ifd_entry(buf: &mut Vec<u8>, tag: u16, ty: u16, count: u32, value_or_offset: u32) {
    buf.extend_from_slice(&tag.to_le_bytes());
    buf.extend_from_slice(&ty.to_le_bytes());
    buf.extend_from_slice(&count.to_le_bytes());
    buf.extend_from_slice(&value_or_offset.to_le_bytes());
}

fn append_ifd_entry_ascii_inline(buf: &mut Vec<u8>, tag: u16, one_char: u8) {
    buf.extend_from_slice(&tag.to_le_bytes());
    buf.extend_from_slice(&2_u16.to_le_bytes());
    buf.extend_from_slice(&2_u32.to_le_bytes());
    buf.push(one_char);
    buf.push(0_u8);
    buf.push(0_u8);
    buf.push(0_u8);
}
