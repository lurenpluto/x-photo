use std::path::{Path, PathBuf};

use filetime::{set_file_mtime, FileTime};
use image::{ImageBuffer, Rgb};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --bin testdata_builder -- <output_dir>");
        std::process::exit(1);
    }

    let out = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&out)?;

    let plans = vec![
        (
            "2020.02.01.大沙河长廊.人才公园.流花山",
            "IMG_A1",
            "2020:02:01 10:11:12",
        ),
        ("2020_02_02_深圳湾公园", "IMG_B2", "2020:02:02 11:12:13"),
        ("misc_bucket", "IMG_C3", ""),
    ];

    for (dir, marker, exif_time) in plans {
        let dir_path = out.join(dir);
        std::fs::create_dir_all(&dir_path)?;
        let file_path = dir_path.join(format!("{}.jpg", marker));
        generate_marked_image(&file_path, marker)?;

        let ft = FileTime::from_unix_time(1_580_513_600, 0);
        set_file_mtime(&file_path, ft)?;

        if !exif_time.is_empty() {
            set_exif_datetime_in_jpeg(&file_path, exif_time)?;
        }
    }

    println!("test data generated at {}", out.display());
    Ok(())
}

fn generate_marked_image(path: &Path, marker: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut img = ImageBuffer::from_pixel(1280, 720, Rgb([255_u8, 255_u8, 255_u8]));
    draw_marker(&mut img, marker, 40, 80, 8, Rgb([10, 10, 10]));
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
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
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
        _ => [
            0b11111, 0b00001, 0b00110, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
    }
}

fn set_exif_datetime_in_jpeg(
    path: &Path,
    date_time_original: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if date_time_original.len() != 19 {
        return Err(format!(
            "invalid exif datetime format '{}', expect YYYY:MM:DD HH:MM:SS",
            date_time_original
        )
        .into());
    }

    let mut jpeg = std::fs::read(path)?;
    if jpeg.len() < 4 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        return Err(format!("{} is not a valid jpeg file", path.display()).into());
    }

    let exif_payload = build_exif_payload(date_time_original);
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

fn build_exif_payload(date_time_original: &str) -> Vec<u8> {
    let dt = format!("{}\0", date_time_original).into_bytes();

    let tiff_header_len = 8_u32;
    let ifd0_entries = 2_u16;
    let ifd0_len = 2_u32 + (ifd0_entries as u32) * 12 + 4;
    let dt0_offset = tiff_header_len + ifd0_len;
    let exif_ifd_offset = dt0_offset + (dt.len() as u32);
    let exif_ifd_entries = 1_u16;
    let exif_ifd_len = 2_u32 + (exif_ifd_entries as u32) * 12 + 4;
    let dto_offset = exif_ifd_offset + exif_ifd_len;

    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II*");
    tiff.push(0x00);
    tiff.extend_from_slice(&8_u32.to_le_bytes());

    tiff.extend_from_slice(&ifd0_entries.to_le_bytes());
    append_ifd_entry(&mut tiff, 0x0132, 2, dt.len() as u32, dt0_offset);
    append_ifd_entry(&mut tiff, 0x8769, 4, 1, exif_ifd_offset);
    tiff.extend_from_slice(&0_u32.to_le_bytes());

    tiff.extend_from_slice(&dt);

    tiff.extend_from_slice(&exif_ifd_entries.to_le_bytes());
    append_ifd_entry(&mut tiff, 0x9003, 2, dt.len() as u32, dto_offset);
    tiff.extend_from_slice(&0_u32.to_le_bytes());
    tiff.extend_from_slice(&dt);

    let mut payload = Vec::with_capacity(6 + tiff.len());
    payload.extend_from_slice(b"Exif\0\0");
    payload.extend_from_slice(&tiff);
    payload
}

fn append_ifd_entry(buf: &mut Vec<u8>, tag: u16, ty: u16, count: u32, value_or_offset: u32) {
    buf.extend_from_slice(&tag.to_le_bytes());
    buf.extend_from_slice(&ty.to_le_bytes());
    buf.extend_from_slice(&count.to_le_bytes());
    buf.extend_from_slice(&value_or_offset.to_le_bytes());
}
