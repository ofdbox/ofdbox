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
    /// 未支持的格式。
    Other(String),
}

impl ImageFormat {
    /// 从 OFD 多媒体的 `Format` 属性解析。
    pub fn from_media_format(format: &str) -> Self {
        match format.trim().to_ascii_uppercase().as_str() {
            "PNG" => Self::Png,
            "JPG" | "JPEG" => Self::Jpeg,
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
        ImageFormat::Png => decode_png(data)?,
        ImageFormat::Jpeg => decode_jpeg(data)?,
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

fn decode_png(data: &[u8]) -> Result<DecodedImage, RenderError> {
    let mut decoder = png::Decoder::new(Cursor::new(data));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|error| RenderError::Image(error.to_string()))?;
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

fn decode_jpeg(data: &[u8]) -> Result<DecodedImage, RenderError> {
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(data));
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
}
