use std::sync::Arc;

use ofdbox_render::{Group, ImageFormat, PoolBudget, ResourcePool, Scene, SceneKey, Size};

#[test]
fn deduplicates_resources_across_documents() {
    let pool = ResourcePool::new(PoolBudget::default());
    let first = pool.embedded_font(b"same-font-bytes", 0).expect("font");
    let second = pool.embedded_font(b"same-font-bytes", 0).expect("font");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.id(), second.id());

    let first = pool
        .embedded_image(ImageFormat::Png, b"same-image-bytes", None)
        .expect("image");
    let second = pool
        .embedded_image(ImageFormat::Png, b"same-image-bytes", None)
        .expect("image");
    assert!(Arc::ptr_eq(&first, &second));

    let stats = pool.stats();
    assert_eq!(stats.fonts, 1);
    assert_eq!(stats.images, 1);
}

#[test]
fn eviction_does_not_invalidate_live_resources() {
    let pool = ResourcePool::new(PoolBudget {
        font_bytes: 8,
        image_bytes: 8,
        scene_bytes: 8,
    });
    let first = pool.embedded_font(b"first-font-large", 0).expect("font");
    let second = pool.embedded_font(b"second-font-large", 0).expect("font");
    // 单个资源超出预算时不进入缓存，但句柄仍然有效。
    assert_eq!(pool.stats().fonts, 0);
    assert_eq!(first.data(), b"first-font-large");
    assert_eq!(second.data(), b"second-font-large");
}

#[test]
fn scenes_are_cached_by_revision() {
    let pool = ResourcePool::new(PoolBudget::default());
    let key = SceneKey {
        document: "doc".to_owned(),
        page: 0,
        revision: 1,
    };
    let build = || Scene {
        size: Size::new(10.0, 10.0),
        root: Group::default(),
        diagnostics: Vec::new(),
    };
    let first = pool.scene(key.clone(), || Ok(build())).expect("scene");
    let second = pool.scene(key, || Ok(build())).expect("scene");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(pool.stats().scenes, 1);

    let other = pool
        .scene(
            SceneKey {
                document: "doc".to_owned(),
                page: 0,
                revision: 2,
            },
            || Ok(build()),
        )
        .expect("scene");
    assert!(!Arc::ptr_eq(&first, &other));
}
