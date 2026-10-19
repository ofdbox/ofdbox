use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use sm3::{Digest, Sm3};

use crate::asset::{AssetId, FontAsset, ImageAsset};
use crate::error::RenderError;
use crate::image::ImageFormat;
use crate::scene::Scene;

/// 资源池预算，按编码资源字节数计。
#[derive(Clone, Copy, Debug)]
pub struct PoolBudget {
    /// 字体缓存预算。
    pub font_bytes: usize,
    /// 图片缓存预算。
    pub image_bytes: usize,
    /// 场景缓存预算。
    pub scene_bytes: usize,
}

impl Default for PoolBudget {
    fn default() -> Self {
        Self {
            font_bytes: 32 * 1024 * 1024,
            image_bytes: 64 * 1024 * 1024,
            scene_bytes: 32 * 1024 * 1024,
        }
    }
}

/// 资源池统计。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolStats {
    /// 缓存中的字体数。
    pub fonts: usize,
    /// 字体缓存字节数。
    pub font_bytes: usize,
    /// 缓存中的图片数。
    pub images: usize,
    /// 图片缓存字节数。
    pub image_bytes: usize,
    /// 缓存中的场景数。
    pub scenes: usize,
    /// 场景缓存字节数。
    pub scene_bytes: usize,
}

/// 内容摘要，用于跨文档去重。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContentDigest(pub [u8; 32]);

/// 字体缓存键。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontKey {
    /// 字体文件内容的 SM3。
    pub digest: ContentDigest,
    /// 字体集合索引。
    pub face_index: u32,
}

/// 图片缓存键。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImageKey {
    /// 图片内容摘要。
    pub digest: ContentDigest,
    /// 图片编码格式。
    pub format: ImageFormat,
    /// 可选蒙版的内容摘要与格式。
    pub mask: Option<(ContentDigest, ImageFormat)>,
}

/// 场景缓存键。编辑器修改页面后应递增 `revision`。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SceneKey {
    /// 文档稳定标识。
    pub document: String,
    /// 页序号。
    pub page: u32,
    /// 页面内容修订号。
    pub revision: u64,
}

/// 跨文档共享的字体、图片与场景缓存。
///
/// 池只持有缓存条目的一份 `Arc`；场景或调用方持有的资源在淘汰后仍然有效。
pub struct ResourcePool {
    inner: Mutex<PoolInner>,
    budget: PoolBudget,
}

struct PoolInner {
    next_asset: u64,
    fonts: Lru<FontKey, Arc<FontAsset>>,
    images: Lru<ImageKey, Arc<ImageAsset>>,
    scenes: Lru<SceneKey, Arc<Scene>>,
}

impl ResourcePool {
    /// 创建资源池。
    pub fn new(budget: PoolBudget) -> Self {
        Self {
            inner: Mutex::new(PoolInner {
                next_asset: 1,
                fonts: Lru::new(budget.font_bytes),
                images: Lru::new(budget.image_bytes),
                scenes: Lru::new(budget.scene_bytes),
            }),
            budget,
        }
    }

    /// 预算设置。
    pub fn budget(&self) -> PoolBudget {
        self.budget
    }

    /// 按内容去重加载嵌入字体。
    pub fn embedded_font(
        &self,
        data: &[u8],
        face_index: u32,
    ) -> Result<Arc<FontAsset>, RenderError> {
        self.embedded_font_shared(Arc::from(data), face_index)
    }

    /// 使用已共享的字体字节加载字体，避免额外复制。
    pub fn embedded_font_shared(
        &self,
        data: Arc<[u8]>,
        face_index: u32,
    ) -> Result<Arc<FontAsset>, RenderError> {
        let key = FontKey {
            digest: ContentDigest(sm3(&data)),
            face_index,
        };
        let mut inner = self.lock()?;
        if let Some(asset) = inner.fonts.get(&key) {
            return Ok(asset);
        }
        let id = inner.allocate_id();
        let weight = data.len();
        let asset = Arc::new(FontAsset::new(id, data, face_index));
        if weight <= inner.fonts.budget {
            inner.fonts.insert(key, Arc::clone(&asset), weight);
        }
        Ok(asset)
    }

    /// 按内容去重加载嵌入图片。
    pub fn embedded_image(
        &self,
        format: ImageFormat,
        data: &[u8],
        mask: Option<(ImageFormat, &[u8])>,
    ) -> Result<Arc<ImageAsset>, RenderError> {
        let mask = mask.map(|(format, bytes)| (format, Arc::<[u8]>::from(bytes)));
        self.embedded_image_shared(format, Arc::from(data), mask)
    }

    /// 使用已共享的图片字节加载图片，避免额外复制。
    pub fn embedded_image_shared(
        &self,
        format: ImageFormat,
        data: Arc<[u8]>,
        mask: Option<(ImageFormat, Arc<[u8]>)>,
    ) -> Result<Arc<ImageAsset>, RenderError> {
        let key = ImageKey {
            digest: ContentDigest(sm3(&data)),
            format: format.clone(),
            mask: mask
                .as_ref()
                .map(|(mask_format, bytes)| (ContentDigest(sm3(bytes)), mask_format.clone())),
        };
        let mut inner = self.lock()?;
        if let Some(asset) = inner.images.get(&key) {
            return Ok(asset);
        }
        let id = inner.allocate_id();
        let weight = data.len() + mask.as_ref().map_or(0, |(_, bytes)| bytes.len());
        let asset = Arc::new(ImageAsset::new(id, format, data, mask));
        if weight <= inner.images.budget {
            inner.images.insert(key, Arc::clone(&asset), weight);
        }
        Ok(asset)
    }

    /// 读取或构建页面场景。
    pub fn scene(
        &self,
        key: SceneKey,
        build: impl FnOnce() -> Result<Scene, RenderError>,
    ) -> Result<Arc<Scene>, RenderError> {
        if let Some(scene) = self.lock()?.scenes.get(&key) {
            return Ok(scene);
        }
        let scene = Arc::new(build()?);
        let weight = scene.estimated_bytes();
        let mut inner = self.lock()?;
        if weight <= inner.scenes.budget {
            inner.scenes.insert(key, Arc::clone(&scene), weight);
        }
        Ok(scene)
    }

    /// 清空所有缓存。
    pub fn trim(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.fonts.trim();
            inner.images.trim();
            inner.scenes.trim();
        }
    }

    /// 当前统计。
    pub fn stats(&self) -> PoolStats {
        let Ok(inner) = self.inner.lock() else {
            return PoolStats::default();
        };
        PoolStats {
            fonts: inner.fonts.len(),
            font_bytes: inner.fonts.bytes,
            images: inner.images.len(),
            image_bytes: inner.images.bytes,
            scenes: inner.scenes.len(),
            scene_bytes: inner.scenes.bytes,
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, PoolInner>, RenderError> {
        self.inner
            .lock()
            .map_err(|_| RenderError::Backend("资源池锁已失效".to_owned()))
    }
}

impl PoolInner {
    fn allocate_id(&mut self) -> AssetId {
        let id = AssetId(self.next_asset);
        self.next_asset += 1;
        id
    }
}

fn sm3(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sm3::new();
    hasher.update(data);
    hasher.finalize().into()
}

struct Lru<K, V> {
    entries: HashMap<K, Entry<V>>,
    bytes: usize,
    budget: usize,
    clock: u64,
}

struct Entry<V> {
    value: V,
    weight: usize,
    used: u64,
}

impl<K: Eq + Hash + Clone, V: Clone> Lru<K, V> {
    fn new(budget: usize) -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            budget,
            clock: 0,
        }
    }

    fn get(&mut self, key: &K) -> Option<V> {
        self.clock += 1;
        let entry = self.entries.get_mut(key)?;
        entry.used = self.clock;
        Some(entry.value.clone())
    }

    fn insert(&mut self, key: K, value: V, weight: usize) {
        if let Some(old) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(old.weight);
        }
        while self.bytes + weight > self.budget && !self.entries.is_empty() {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| key.clone());
            let Some(oldest) = oldest else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(removed.weight);
            }
        }
        self.clock += 1;
        self.entries.insert(
            key,
            Entry {
                value,
                weight,
                used: self.clock,
            },
        );
        self.bytes += weight;
    }

    fn trim(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{PoolBudget, ResourcePool};
    use crate::image::ImageFormat;

    #[test]
    fn image_key_includes_mask() {
        let pool = ResourcePool::new(PoolBudget::default());
        let data = std::sync::Arc::<[u8]>::from(&b"image"[..]);
        let first = pool
            .embedded_image_shared(
                ImageFormat::Png,
                data.clone(),
                Some((
                    ImageFormat::Png,
                    std::sync::Arc::<[u8]>::from(&b"mask-1"[..]),
                )),
            )
            .expect("first image");
        let second = pool
            .embedded_image_shared(
                ImageFormat::Png,
                data,
                Some((
                    ImageFormat::Png,
                    std::sync::Arc::<[u8]>::from(&b"mask-2"[..]),
                )),
            )
            .expect("second image");
        assert_ne!(first.id(), second.id());
    }
}
