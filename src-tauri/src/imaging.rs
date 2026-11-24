use crate::paths::{allowed_photo, fingerprint, is_link};
use fast_image_resize::{images::Image, PixelType, Resizer};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufReader, Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
};

pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DECODE_BYTES: u64 = 160 * 1024 * 1024;
const MAX_PIXELS: u64 = 40_000_000;
const MAX_WORKING_BYTES: u64 = 256 * 1024 * 1024;
pub const THUMB_EDGE: u32 = 320;
pub const THUMB_VERSION: &str = "v2-320";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhotoMetadata {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub orientation: Option<u32>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub captured_at: Option<String>,
    pub iso: Option<String>,
    pub exposure: Option<String>,
    pub aperture: Option<String>,
    pub focal_length: Option<String>,
}

fn open_checked(path: &Path) -> Result<File, String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "Picture unavailable.")?;
    if !metadata.is_file()
        || is_link(&metadata)
        || !allowed_photo(path)
        || metadata.len() > MAX_FILE_BYTES
    {
        return Err("Picture unavailable or too large.".into());
    }
    let file = File::open(path).map_err(|_| "Picture unavailable.")?;
    let opened = file.metadata().map_err(|_| "Picture unavailable.")?;
    if fingerprint(&metadata) != fingerprint(&opened) || opened.len() > MAX_FILE_BYTES {
        return Err("Picture changed while opening.".into());
    }
    if !crate::paths::matches_open_path(&file, path) {
        return Err("Picture changed while opening.".into());
    }
    Ok(file)
}

fn reader(path: &Path) -> Result<ImageReader<BufReader<File>>, String> {
    let mut reader = ImageReader::new(BufReader::new(open_checked(path)?))
        .with_guessed_format()
        .map_err(|_| "Picture unavailable.")?;
    if reader.format().is_none() {
        reader.set_format(ImageFormat::from_path(path).map_err(|_| "Unsupported picture format.")?);
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    Ok(reader)
}

pub fn metadata(path: &Path) -> PhotoMetadata {
    let mut result = PhotoMetadata::default();
    let Ok(file) = open_checked(path) else {
        return result;
    };
    let mut buffered = BufReader::new(file);
    if let Ok(size) = imagesize::reader_size(&mut buffered) {
        result.width = u32::try_from(size.width).ok();
        result.height = u32::try_from(size.height).ok();
    } else if let Ok(reader) = reader(path) {
        if let Ok(decoder) = reader.into_decoder() {
            let (width, height) = decoder.dimensions();
            result.width = Some(width);
            result.height = Some(height);
        }
    }
    let has_exif = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["jpg", "jpeg", "png", "webp", "tif", "tiff"]
                .iter()
                .any(|kind| ext.eq_ignore_ascii_case(kind))
        });
    if has_exif && buffered.seek(SeekFrom::Start(0)).is_ok() {
        if let Ok(exif) = exif::Reader::new().read_from_container(&mut buffered) {
            let field = |tag| exif.get_field(tag, exif::In::PRIMARY);
            let display = |tag| {
                field(tag).map(|value| {
                    value
                        .display_value()
                        .with_unit(&exif)
                        .to_string()
                        .chars()
                        .take(256)
                        .collect::<String>()
                })
            };
            result.orientation =
                field(exif::Tag::Orientation).and_then(|value| value.value.get_uint(0));
            result.camera_make = display(exif::Tag::Make);
            result.camera_model = display(exif::Tag::Model);
            result.captured_at =
                display(exif::Tag::DateTimeOriginal).or_else(|| display(exif::Tag::DateTime));
            result.iso = display(exif::Tag::PhotographicSensitivity);
            result.exposure = display(exif::Tag::ExposureTime);
            result.aperture = display(exif::Tag::FNumber);
            result.focal_length = display(exif::Tag::FocalLength);
        }
    }
    if matches!(result.orientation, Some(5..=8)) {
        std::mem::swap(&mut result.width, &mut result.height);
    }
    result
}

pub fn thumbnail(path: &Path) -> Result<Vec<u8>, String> {
    let mut decoder = reader(path)?
        .into_decoder()
        .map_err(|_| "Unsupported or damaged picture.")?;
    let (width, height) = decoder.dimensions();
    if width == 0
        || height == 0
        || u64::from(width) * u64::from(height) > MAX_PIXELS
        || decoder.total_bytes() > MAX_DECODE_BYTES
    {
        return Err("Picture is too large to preview.".into());
    }
    let pixels = u64::from(width) * u64::from(height);
    let conversion = if matches!(
        decoder.color_type(),
        image::ColorType::Rgb8 | image::ColorType::Rgba8
    ) {
        0
    } else {
        pixels * 4
    };
    let compressed = fs::metadata(path)
        .map_err(|_| "Picture unavailable.")?
        .len();
    if compressed
        .saturating_add(decoder.total_bytes())
        .saturating_add(conversion)
        .saturating_add(24 * 1024 * 1024)
        > MAX_WORKING_BYTES
    {
        return Err("Picture exceeds the preview memory budget.".into());
    }
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let image = DynamicImage::from_decoder(decoder).map_err(|_| "Cannot decode picture.")?;
    let alpha = image.color().has_alpha();
    let mut preview = if width > THUMB_EDGE || height > THUMB_EDGE {
        let longest = u64::from(width.max(height));
        let target_width = (u64::from(width) * u64::from(THUMB_EDGE) / longest).max(1) as u32;
        let target_height = (u64::from(height) * u64::from(THUMB_EDGE) / longest).max(1) as u32;
        let pixel_type = if alpha {
            PixelType::U8x4
        } else {
            PixelType::U8x3
        };
        let pixels = if alpha {
            image.into_rgba8().into_raw()
        } else {
            image.into_rgb8().into_raw()
        };
        let source = Image::from_vec_u8(width, height, pixels, pixel_type)
            .map_err(|_| "Cannot resize picture.")?;
        let mut target = Image::new(target_width, target_height, pixel_type);
        Resizer::new()
            .resize(&source, &mut target, None)
            .map_err(|_| "Cannot resize picture.")?;
        let pixels = target.into_vec();
        if alpha {
            DynamicImage::ImageRgba8(
                image::RgbaImage::from_raw(target_width, target_height, pixels)
                    .ok_or("Cannot resize picture.")?,
            )
        } else {
            DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(target_width, target_height, pixels)
                    .ok_or("Cannot resize picture.")?,
            )
        }
    } else {
        image
    };
    preview.apply_orientation(orientation);
    let mut output = Cursor::new(Vec::new());
    if alpha {
        DynamicImage::ImageRgba8(preview.into_rgba8())
            .write_to(&mut output, ImageFormat::Png)
            .map_err(|_| "Cannot encode preview.")?;
    } else {
        let rgb = preview.into_rgb8();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 84)
            .encode_image(&rgb)
            .map_err(|_| "Cannot encode preview.")?;
    }
    Ok(output.into_inner())
}

pub fn full_image(path: &Path) -> Result<Vec<u8>, String> {
    let file = open_checked(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes).map_err(|_| "Cannot read picture.")?;
    if bytes.len() as u64 > MAX_FILE_BYTES { return Err("Picture exceeds the 64 MiB file limit.".into()); }
    let mut reader = ImageReader::new(Cursor::new(bytes.as_slice())).with_guessed_format().map_err(|_| "Cannot identify picture.")?;
    if reader.format().is_none() {
        reader.set_format(ImageFormat::from_path(path).map_err(|_| "Unsupported picture format.")?);
    }
    let format = reader.format();
    let mut limits = Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| "Unsupported or damaged picture.")?;
    let (width, height) = decoder.dimensions();
    let pixels = u64::from(width) * u64::from(height);
    if pixels == 0 || pixels > MAX_PIXELS || decoder.total_bytes() > MAX_DECODE_BYTES {
        return Err("Picture exceeds the full-resolution memory limit.".into());
    }
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    if matches!(format, Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP)) && orientation == image::metadata::Orientation::NoTransforms {
        drop(decoder);
        return Ok(bytes);
    }
    if (bytes.len() as u64).saturating_add(decoder.total_bytes()).saturating_add(pixels * 8) > MAX_WORKING_BYTES {
        return Err("Picture exceeds the full-resolution memory limit.".into());
    }
    let mut image = DynamicImage::from_decoder(decoder).map_err(|_| "Cannot decode picture.")?;
    drop(bytes);
    image.apply_orientation(orientation);
    let mut output = LimitedOutput(Vec::new());
    let rgba = image.into_rgba8();
    use image::ImageEncoder;
    image::codecs::png::PngEncoder::new(&mut output).write_image(rgba.as_raw(), rgba.width(), rgba.height(), image::ExtendedColorType::Rgba8).map_err(|_| "Cannot encode full-resolution picture within the memory limit.")?;
    Ok(output.0)
}

struct LimitedOutput(Vec<u8>);

impl Write for LimitedOutput {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(data.len()) > MAX_FILE_BYTES as usize {
            return Err(std::io::Error::other("Full-resolution output limit"));
        }
        self.0.extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resizes_opaque_and_transparent_pictures() {
        let dir = tempfile::tempdir().unwrap();
        for (name, source) in [
            ("opaque.bmp", DynamicImage::new_rgb8(640, 480)),
            ("alpha.png", DynamicImage::new_rgba8(480, 640)),
        ] {
            let path = dir.path().join(name);
            source.save(&path).unwrap();
            let path = fs::canonicalize(path).unwrap();
            let bytes = thumbnail(&path).unwrap();
            let result = image::load_from_memory(&bytes).unwrap();
            assert_eq!(result.width().max(result.height()), THUMB_EDGE);
            assert_eq!(result.color().has_alpha(), source.color().has_alpha());
            assert_eq!(metadata(&path).width, Some(source.width()));
        }
    }

    #[test]
    fn extracts_exif_and_applies_orientation_after_resize() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("rotated.jpg");
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .encode_image(&DynamicImage::new_rgb8(640, 480))
            .unwrap();
        let mut exif = b"Exif\0\0II\x2a\0\x08\0\0\0".to_vec();
        exif.extend_from_slice(&2u16.to_le_bytes());
        exif.extend_from_slice(&[0x0f, 0x01, 0x02, 0x00, 0x06, 0, 0, 0, 38, 0, 0, 0]);
        exif.extend_from_slice(&[0x12, 0x01, 0x03, 0x00, 0x01, 0, 0, 0, 6, 0, 0, 0]);
        exif.extend_from_slice(&[0, 0, 0, 0]);
        exif.extend_from_slice(b"Canon\0");
        let mut bytes = jpeg[..2].to_vec();
        bytes.extend_from_slice(&[0xff, 0xe1]);
        bytes.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        bytes.extend_from_slice(&exif);
        bytes.extend_from_slice(&jpeg[2..]);
        fs::write(&file, bytes).unwrap();
        let path = fs::canonicalize(file).unwrap();
        let meta = metadata(&path);
        assert_eq!(meta.orientation, Some(6));
        assert_eq!((meta.width, meta.height), (Some(480), Some(640)));
        assert!(meta.camera_make.unwrap().contains("Canon"));
        let preview = image::load_from_memory(&thumbnail(&path).unwrap()).unwrap();
        assert_eq!((preview.width(), preview.height()), (240, 320));
    }

    #[test]
    fn supported_static_formats_produce_thumbnails() {
        let dir = tempfile::tempdir().unwrap();
        for (name, format) in [
            ("test.gif", ImageFormat::Gif),
            ("test.tiff", ImageFormat::Tiff),
            ("test.ico", ImageFormat::Ico),
            ("test.tga", ImageFormat::Tga),
            ("test.qoi", ImageFormat::Qoi),
            ("test.ppm", ImageFormat::Pnm),
        ] {
            let path = dir.path().join(name);
            let source = if matches!(format, ImageFormat::Gif | ImageFormat::Ico) {
                DynamicImage::new_rgba8(64, 48)
            } else {
                DynamicImage::new_rgb8(64, 48)
            };
            source.save_with_format(&path, format).unwrap();
            let path = fs::canonicalize(path).unwrap();
            let preview = image::load_from_memory(&thumbnail(&path).unwrap()).unwrap();
            assert_eq!((preview.width(), preview.height()), (64, 48), "{name}");
        }
    }
    #[test]
    fn full_image_preserves_original_resolution_and_png_bytes() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["original.png", "original.bmp", "original.jpg"] {
            let path = dir.path().join(name);
            DynamicImage::new_rgb8(1280, 720).save(&path).unwrap();
            let path = fs::canonicalize(path).unwrap();
            let bytes = full_image(&path).unwrap();
            let decoded = image::load_from_memory(&bytes).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (1280,720));
            if !name.ends_with(".bmp") { assert_eq!(bytes,fs::read(path).unwrap()); }
        }
    }
}
