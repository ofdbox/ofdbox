use std::io::Cursor;

use jpeg_decoder::PixelFormat;

use crate::error::RenderError;

/// 图像的编码格式。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    /// PNG。
    Png,
    /// JPEG。
    Jpeg,
    /// BMP。
    Bmp,
    /// JBIG2。真实文件里也出现 `GBIG2` 拼写。
    Jbig2,
    /// 未支持的格式。
    Other(String),
}

impl ImageFormat {
    /// 按文件头魔数识别真实图片格式。
    ///
    /// 真实文件里经常出现扩展名/`Format` 与实际内容不一致的情况，优先使用内容。
    pub fn detect(data: &[u8]) -> Option<Self> {
        if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            Some(Self::Png)
        } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if data.starts_with(b"BM") {
            Some(Self::Bmp)
        } else if data.starts_with(&[0x97, b'J', b'B', b'2']) || data.first() == Some(&0x97) {
            Some(Self::Jbig2)
        } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
            Some(Self::Other("GIF".to_owned()))
        } else if data.starts_with(&[0x49, 0x49, 0x2A, 0x00])
            || data.starts_with(&[0x4D, 0x4D, 0x00, 0x2A])
        {
            Some(Self::Other("TIFF".to_owned()))
        } else if data.starts_with(&[0x00, 0x00, 0x00, 0x0C, b'j', b'P', b' ', b' ']) {
            Some(Self::Other("JP2".to_owned()))
        } else if data.starts_with(&[0xFF, 0x4F, 0xFF, 0x51]) {
            Some(Self::Other("J2K".to_owned()))
        } else {
            None
        }
    }

    /// 从 OFD 多媒体的 `Format` 属性解析。
    pub fn from_media_format(format: &str) -> Self {
        match format.trim().to_ascii_uppercase().as_str() {
            "PNG" => Self::Png,
            "JPG" | "JPEG" => Self::Jpeg,
            "BMP" => Self::Bmp,
            "JBIG2" | "GBIG2" | "JB2" => Self::Jbig2,
            other => Self::Other(other.to_owned()),
        }
    }
}

/// 解码后的 RGBA8 图像。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedImage {
    /// 宽度，像素。
    pub width: u32,
    /// 高度，像素。
    pub height: u32,
    /// 非预乘 RGBA8 像素。
    pub rgba: Vec<u8>,
}

impl DecodedImage {
    /// 将二值或灰度蒙版乘入 alpha 通道。
    pub fn apply_mask(&mut self, mask: &DecodedImage) -> Result<(), RenderError> {
        if mask.width != self.width || mask.height != self.height {
            return Err(RenderError::Image("图像蒙版尺寸不一致".to_owned()));
        }
        for (pixel, mask_pixel) in self
            .rgba
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(mask.rgba.as_chunks::<4>().0)
        {
            let factor =
                (u32::from(mask_pixel[0]) + u32::from(mask_pixel[1]) + u32::from(mask_pixel[2]))
                    / 3;
            pixel[3] = ((u32::from(pixel[3]) * factor) / 255) as u8;
        }
        Ok(())
    }
}

/// 解码图片并转换为 RGBA8。
pub fn decode_image(
    format: &ImageFormat,
    data: &[u8],
    max_pixels: u64,
) -> Result<DecodedImage, RenderError> {
    let image = match format {
        ImageFormat::Png => decode_png(data, max_pixels)?,
        ImageFormat::Jpeg => decode_jpeg(data, max_pixels)?,
        ImageFormat::Bmp => decode_bmp(data)?,
        ImageFormat::Jbig2 => decode_jbig2(data)?,
        ImageFormat::Other(format) => {
            return Err(RenderError::Unsupported(format!("图片格式: {format}")));
        }
    };
    let pixels = u64::from(image.width) * u64::from(image.height);
    if pixels > max_pixels {
        return Err(RenderError::Image("图片像素数超出限制".to_owned()));
    }
    Ok(image)
}

fn decode_png(data: &[u8], max_pixels: u64) -> Result<DecodedImage, RenderError> {
    let mut decoder = png::Decoder::new(Cursor::new(data));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|error| RenderError::Image(error.to_string()))?;
    let header = reader.info();
    let pixels = u64::from(header.width) * u64::from(header.height);
    if pixels > max_pixels {
        return Err(RenderError::Image("图片像素数超出限制".to_owned()));
    }
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| RenderError::Image("PNG 输出尺寸未知".to_owned()))?;
    let mut buffer = vec![0; size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| RenderError::Image(error.to_string()))?;
    let width = info.width;
    let height = info.height;
    let pixels = &buffer[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        png::ColorType::Grayscale => pixels
            .iter()
            .flat_map(|value| [*value, *value, *value, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => pixels
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect(),
        png::ColorType::Indexed => {
            return Err(RenderError::Image("PNG 调色板未展开".to_owned()));
        }
    };
    Ok(DecodedImage {
        width,
        height,
        rgba,
    })
}

fn decode_jpeg(data: &[u8], max_pixels: u64) -> Result<DecodedImage, RenderError> {
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(data));
    decoder
        .read_info()
        .map_err(|error| RenderError::Image(error.to_string()))?;
    let header = decoder
        .info()
        .ok_or_else(|| RenderError::Image("缺少 JPEG 图像信息".to_owned()))?;
    let pixel_count = u64::from(header.width) * u64::from(header.height);
    if pixel_count > max_pixels {
        return Err(RenderError::Image("图片像素数超出限制".to_owned()));
    }
    let pixels = decoder
        .decode()
        .map_err(|error| RenderError::Image(error.to_string()))?;
    let info = decoder
        .info()
        .ok_or_else(|| RenderError::Image("缺少 JPEG 图像信息".to_owned()))?;
    let rgba = match info.pixel_format {
        PixelFormat::L8 => pixels
            .iter()
            .flat_map(|value| [*value, *value, *value, 255])
            .collect(),
        PixelFormat::RGB24 => pixels
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        PixelFormat::CMYK32 => pixels
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|pixel| {
                let c = u16::from(pixel[0]);
                let m = u16::from(pixel[1]);
                let y = u16::from(pixel[2]);
                let k = u16::from(pixel[3]);
                [
                    (255 - ((c + k).min(255))) as u8,
                    (255 - ((m + k).min(255))) as u8,
                    (255 - ((y + k).min(255))) as u8,
                    255,
                ]
            })
            .collect(),
        PixelFormat::L16 => {
            return Err(RenderError::Image("不支持 16 位灰度 JPEG".to_owned()));
        }
    };
    Ok(DecodedImage {
        width: u32::from(info.width),
        height: u32::from(info.height),
        rgba,
    })
}

fn decode_bmp(data: &[u8]) -> Result<DecodedImage, RenderError> {
    let image = bmp::from_reader(&mut Cursor::new(data))
        .map_err(|error| RenderError::Image(error.to_string()))?;
    let width = image.get_width();
    let height = image.get_height();
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y);
            rgba.extend_from_slice(&[pixel.r, pixel.g, pixel.b, 255]);
        }
    }
    Ok(DecodedImage {
        width,
        height,
        rgba,
    })
}

fn decode_jbig2(data: &[u8]) -> Result<DecodedImage, RenderError> {
    let pages = justbig2::decode(data)
        .or_else(|_| justbig2::decode_embedded(data))
        .map_err(|error| RenderError::Image(error.to_string()))?;
    let page = pages
        .into_iter()
        .next()
        .ok_or_else(|| RenderError::Image("JBIG2 没有页面".to_owned()))?;
    if page.width == 0 || page.height == 0 {
        return Err(RenderError::Image("JBIG2 页面尺寸无效".to_owned()));
    }
    let mut rgba = Vec::with_capacity(page.width as usize * page.height as usize * 4);
    for y in 0..page.height {
        for x in 0..page.width {
            let color = if page.get_pixel(x, y) == 0 { 255 } else { 0 };
            rgba.extend_from_slice(&[color, color, color, 255]);
        }
    }
    Ok(DecodedImage {
        width: page.width,
        height: page.height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::{DecodedImage, ImageFormat, decode_image};

    #[test]
    fn applies_grayscale_mask() {
        let mut image = DecodedImage {
            width: 1,
            height: 1,
            rgba: vec![10, 20, 30, 200],
        };
        let mask = DecodedImage {
            width: 1,
            height: 1,
            rgba: vec![128, 128, 128, 255],
        };
        image.apply_mask(&mask).expect("mask");
        assert_eq!(image.rgba[3], 100);
    }

    #[test]
    fn unsupported_format_is_explicit() {
        assert!(decode_image(&ImageFormat::Other("JP2".to_owned()), &[], 10).is_err());
    }

    #[test]
    fn detects_formats_by_magic() {
        assert_eq!(
            ImageFormat::detect(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
            Some(ImageFormat::Png)
        );
        assert_eq!(
            ImageFormat::detect(&[0xFF, 0xD8, 0xFF, 0xE0]),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(ImageFormat::detect(b"BM\x00\x00"), Some(ImageFormat::Bmp));
        assert_eq!(
            ImageFormat::detect(b"GIF89a"),
            Some(ImageFormat::Other("GIF".to_owned()))
        );
        assert_eq!(
            ImageFormat::detect(&[0x49, 0x49, 0x2A, 0x00]),
            Some(ImageFormat::Other("TIFF".to_owned()))
        );
        assert_eq!(ImageFormat::detect(b"not-an-image"), None);
    }

    #[test]
    fn parses_bmp_and_jbig2_format_names() {
        assert_eq!(ImageFormat::from_media_format("BMP"), ImageFormat::Bmp);
        assert_eq!(ImageFormat::from_media_format("GBIG2"), ImageFormat::Jbig2);
        assert_eq!(ImageFormat::from_media_format("JBIG2"), ImageFormat::Jbig2);
    }

    #[test]
    fn decodes_24_bit_bmp() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"BM");
        bytes.extend_from_slice(&70u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&54u32.to_le_bytes());
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(&2i32.to_le_bytes());
        bytes.extend_from_slice(&2i32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&24u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        for _ in 0..2 {
            bytes.extend_from_slice(&[255, 0, 0, 255, 0, 0, 0, 0]);
        }
        let image = decode_image(&ImageFormat::Bmp, &bytes, 16).expect("bmp");
        assert_eq!((image.width, image.height), (2, 2));
        assert_eq!(&image.rgba[..4], &[0, 0, 255, 255]);
    }

    #[test]
    fn rejects_invalid_jbig2() {
        assert!(decode_image(&ImageFormat::Jbig2, b"not-jbig2", 100).is_err());
    }
}
