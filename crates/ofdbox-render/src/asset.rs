use std::sync::{Arc, OnceLock};

use crate::error::RenderError;
use crate::image::{DecodedImage, ImageFormat, decode_image};

/// 资源池内的稳定资源标识。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId(pub u64);

/// 可共享的字体资源。
#[derive(Debug)]
pub struct FontAsset {
    id: AssetId,
    data: Arc<[u8]>,
    face_index: u32,
}

impl FontAsset {
    pub(crate) fn new(id: AssetId, data: Arc<[u8]>, face_index: u32) -> Self {
        Self {
            id,
            data,
            face_index,
        }
    }

    /// 资源标识。
    pub fn id(&self) -> AssetId {
        self.id
    }

    /// 字体文件字节。
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// 字体集合内的索引。
    pub fn face_index(&self) -> u32 {
        self.face_index
    }
}

/// 可共享的图片资源。
#[derive(Debug)]
pub struct ImageAsset {
    id: AssetId,
    format: ImageFormat,
    data: Arc<[u8]>,
    mask: Option<(ImageFormat, Arc<[u8]>)>,
    decoded: OnceLock<Arc<DecodedImage>>,
}

impl ImageAsset {
    pub(crate) fn new(
        id: AssetId,
        format: ImageFormat,
        data: Arc<[u8]>,
        mask: Option<(ImageFormat, Arc<[u8]>)>,
    ) -> Self {
        Self {
            id,
            format,
            data,
            mask,
            decoded: OnceLock::new(),
        }
    }

    /// 资源标识。
    pub fn id(&self) -> AssetId {
        self.id
    }

    /// 编码格式。
    pub fn format(&self) -> &ImageFormat {
        &self.format
    }

    /// 编码字节。
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// 按需解码，并缓存结果。
    pub fn decoded(&self, max_pixels: u64) -> Result<Arc<DecodedImage>, RenderError> {
        if let Some(image) = self.decoded.get() {
            return Ok(Arc::clone(image));
        }
        let mut image = decode_image(&self.format, &self.data, max_pixels)?;
        if let Some((format, mask)) = &self.mask {
            let mask = decode_image(format, mask, max_pixels)?;
            image.apply_mask(&mask)?;
        }
        let image = Arc::new(image);
        let _ = self.decoded.set(Arc::clone(&image));
        Ok(image)
    }

    /// 已解码字节数；未解码时为 0。
    pub fn decoded_bytes(&self) -> usize {
        self.decoded
            .get()
            .map(|image| image.rgba.len())
            .unwrap_or(0)
    }
}
