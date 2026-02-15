use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate, Weekday};
use filetime::{set_file_mtime, FileTime};
use image::{ImageBuffer, Rgb};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: cargo run --bin testdata_builder -- <output_dir> [--strategy family_us_weekends_2025] [--year 2025] [--seed 42]"
        );
        std::process::exit(1);
    }

    let output_dir = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&output_dir)?;

    let mut strategy = "family_us_weekends_2025".to_string();
    let mut year: i32 = 2025;
    let mut seed: u64 = 42;

    let mut idx = 2;
    while idx < args.len() {
        match args[idx].as_str() {
            "--strategy" if idx + 1 < args.len() => {
                strategy = args[idx + 1].clone();
                idx += 2;
            }
            "--year" if idx + 1 < args.len() => {
                year = args[idx + 1].parse()?;
                idx += 2;
            }
            "--seed" if idx + 1 < args.len() => {
                seed = args[idx + 1].parse()?;
                idx += 2;
            }
            _ => {
                idx += 1;
            }
        }
    }

    let plans = match strategy.as_str() {
        "family_us_weekends_2025" => build_family_us_weekend_plans(year, seed)?,
        "sample" => build_sample_plans(),
        other => {
            return Err(format!("unknown strategy: {}", other).into());
        }
    };

    let mut generated = 0_usize;
    for plan in plans {
        let dir_path = output_dir.join(&plan.album_dir_name);
        std::fs::create_dir_all(&dir_path)?;
        let file_path = dir_path.join(format!("{}.jpg", plan.marker));

        generate_marked_image(&file_path, &plan.marker)?;
        set_exif_metadata_in_jpeg(&file_path, &plan.exif)?;

        let ft = FileTime::from_unix_time(plan.mtime_unix, 0);
        set_file_mtime(&file_path, ft)?;
        generated += 1;
    }

    println!(
        "test data generated at {} (strategy={}, photos={})",
        output_dir.display(),
        strategy,
        generated
    );
    Ok(())
}

#[derive(Clone)]
struct PhotoPlan {
    album_dir_name: String,
    marker: String,
    exif: ExifMeta,
    mtime_unix: i64,
}

#[derive(Clone)]
struct ExifMeta {
    date_time_original: String,
    gps_lat: Option<f64>,
    gps_lng: Option<f64>,
}

#[derive(Clone)]
struct Park {
    name: &'static str,
    lat: f64,
    lng: f64,
}

fn build_family_us_weekend_plans(
    year: i32,
    seed: u64,
) -> Result<Vec<PhotoPlan>, Box<dyn std::error::Error>> {
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

    let mut day =
        NaiveDate::from_ymd_opt(year, 1, 1).ok_or_else(|| format!("invalid year {}", year))?;
    let end =
        NaiveDate::from_ymd_opt(year, 12, 31).ok_or_else(|| format!("invalid year {}", year))?;

    while day <= end {
        if day.weekday() == Weekday::Sat {
            let park = parks[rng.gen_range(0..parks.len())].clone();
            let album_dir = format!(
                "{:04}.{:02}.{:02}.{}",
                day.year(),
                day.month(),
                day.day(),
                park.name
            );

            let photo_count = rng.gen_range(10..=100);
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
