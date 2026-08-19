use std::{
    env,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

const ICON_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];
const BACKGROUND: [u8; 3] = [0xe8, 0xeb, 0xf4];
const INK: [u8; 3] = [0x2f, 0x39, 0x4d];
const ACCENT: [u8; 3] = [0x53, 0x68, 0xa5];

fn main() {
    println!("cargo:rerun-if-changed=../../assets/cakify-mark.svg");
    println!("cargo:rerun-if-changed=build.rs");

    if !matches!(env::var("CARGO_CFG_TARGET_OS").as_deref(), Ok("windows")) {
        return;
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let icon_path = out_dir.join("cakify.ico");
    let resource_path = out_dir.join("cakify-icon.rc");
    write_icon(&icon_path).expect("write generated Cakify icon");
    write_resource(&resource_path, &icon_path).expect("write Cakify icon resource");
    embed_resource::compile(&resource_path, embed_resource::NONE)
        .manifest_optional()
        .expect("compile Cakify Windows resources");
}

fn write_icon(path: &Path) -> io::Result<()> {
    let images = ICON_SIZES
        .iter()
        .map(|&size| encode_icon_image(size))
        .collect::<Vec<_>>();
    let directory_size = 6 + images.len() * 16;
    let mut offset = u32::try_from(directory_size).expect("icon directory fits in u32");
    let mut file = File::create(path)?;

    write_u16(&mut file, 0)?;
    write_u16(&mut file, 1)?;
    write_u16(
        &mut file,
        u16::try_from(images.len()).expect("icon count fits in u16"),
    )?;
    for (&size, image) in ICON_SIZES.iter().zip(&images) {
        file.write_all(&[if size == 256 { 0 } else { size as u8 }])?;
        file.write_all(&[if size == 256 { 0 } else { size as u8 }])?;
        file.write_all(&[0, 0])?;
        write_u16(&mut file, 1)?;
        write_u16(&mut file, 32)?;
        write_u32(
            &mut file,
            u32::try_from(image.len()).expect("icon image fits in u32"),
        )?;
        write_u32(&mut file, offset)?;
        offset = offset
            .checked_add(u32::try_from(image.len()).expect("icon image fits in u32"))
            .expect("icon offset fits in u32");
    }
    for image in images {
        file.write_all(&image)?;
    }
    file.flush()
}

fn encode_icon_image(size: u32) -> Vec<u8> {
    let pixels = rasterize(size);
    let mask_stride = size.div_ceil(32) * 4;
    let pixel_bytes = size * size * 4;
    let mask_bytes = mask_stride * size;
    let mut image = Vec::with_capacity((40 + pixel_bytes + mask_bytes) as usize);

    push_u32(&mut image, 40);
    push_i32(&mut image, size as i32);
    push_i32(&mut image, (size * 2) as i32);
    push_u16(&mut image, 1);
    push_u16(&mut image, 32);
    push_u32(&mut image, 0);
    push_u32(&mut image, pixel_bytes);
    push_i32(&mut image, 0);
    push_i32(&mut image, 0);
    push_u32(&mut image, 0);
    push_u32(&mut image, 0);

    for y in (0..size).rev() {
        let row_start = (y * size * 4) as usize;
        let row_end = row_start + (size * 4) as usize;
        image.extend_from_slice(&pixels[row_start..row_end]);
    }
    image.resize(image.len() + mask_bytes as usize, 0);
    image
}

fn rasterize(size: u32) -> Vec<u8> {
    const SAMPLES: u32 = 4;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let mut alpha_sum = 0_u32;
            let mut rgb_sum = [0_u32; 3];
            for sample_y in 0..SAMPLES {
                for sample_x in 0..SAMPLES {
                    let point_x = (x as f32 + (sample_x as f32 + 0.5) / SAMPLES as f32)
                        / size as f32;
                    let point_y = (y as f32 + (sample_y as f32 + 0.5) / SAMPLES as f32)
                        / size as f32;
                    if let Some(color) = color_at(point_x, point_y) {
                        alpha_sum += 1;
                        for (sum, value) in rgb_sum.iter_mut().zip(color) {
                            *sum += u32::from(value);
                        }
                    }
                }
            }
            let sample_count = SAMPLES * SAMPLES;
            let color = if alpha_sum == 0 {
                [0, 0, 0]
            } else {
                rgb_sum.map(|value| (value / alpha_sum) as u8)
            };
            pixels.extend_from_slice(&[
                color[2],
                color[1],
                color[0],
                ((alpha_sum * 255) / sample_count) as u8,
            ]);
        }
    }
    pixels
}

fn color_at(x: f32, y: f32) -> Option<[u8; 3]> {
    if rounded_rect_contains(x, y, 0.0, 0.0, 1.0, 1.0, 0.25) {
        if rounded_rect_contains(x, y, 0.211, 0.227, 0.578, 0.133, 0.066)
            || rounded_rect_contains(x, y, 0.211, 0.641, 0.578, 0.133, 0.066)
        {
            return Some(INK);
        }
        if rounded_rect_contains(x, y, 0.211, 0.434, 0.383, 0.133, 0.066) {
            return Some(ACCENT);
        }
        return Some(BACKGROUND);
    }
    None
}

fn rounded_rect_contains(x: f32, y: f32, left: f32, top: f32, width: f32, height: f32, radius: f32) -> bool {
    if !(left..=left + width).contains(&x) || !(top..=top + height).contains(&y) {
        return false;
    }
    let nearest_x = x.clamp(left + radius, left + width - radius);
    let nearest_y = y.clamp(top + radius, top + height - radius);
    (x - nearest_x).powi(2) + (y - nearest_y).powi(2) <= radius.powi(2)
}

fn write_resource(resource_path: &Path, icon_path: &Path) -> io::Result<()> {
    let icon_path = icon_path.to_string_lossy().replace('\\', "/");
    fs::write(resource_path, format!("1 ICON \"{icon_path}\"\n"))
}

fn write_u16(writer: &mut impl Write, value: u16) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

fn write_u32(writer: &mut impl Write, value: u32) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(output: &mut Vec<u8>, value: i32) {
    output.extend_from_slice(&value.to_le_bytes());
}
