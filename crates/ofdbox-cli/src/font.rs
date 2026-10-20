use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ofdbox_render::{FontProvider, FontRequest, RenderError};

type FontCache = Arc<Mutex<HashMap<PathBuf, Option<Arc<[u8]>>>>>;

pub struct SystemFonts {
    cache: FontCache,
}

impl SystemFonts {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl FontProvider for SystemFonts {
    fn load(&mut self, request: &FontRequest<'_>) -> Result<Option<Arc<[u8]>>, RenderError> {
        let Some(path) = select_font(request) else {
            return Ok(None);
        };
        if let Some(cached) = self.cache.lock().expect("font cache").get(&path) {
            return Ok(cached.clone());
        }
        let bytes = std::fs::read(&path).ok().map(Arc::<[u8]>::from);
        self.cache
            .lock()
            .expect("font cache")
            .insert(path, bytes.clone());
        Ok(bytes)
    }
}

fn font_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if cfg!(windows) {
        roots.push(PathBuf::from("C:/Windows/Fonts"));
    }
    roots.extend([
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/Library/Fonts"),
    ]);
    roots
}

fn select_font(request: &FontRequest<'_>) -> Option<PathBuf> {
    let name = format!(
        "{} {}",
        request.font_name.unwrap_or_default(),
        request.family_name.unwrap_or_default()
    )
    .to_ascii_lowercase();
    let mut candidates: Vec<&str> = Vec::new();
    if name.contains("宋") || name.contains("song") || name.contains("simsun") {
        candidates.extend(["simsun.ttc", "simsun.ttf"]);
    } else if name.contains("黑") || name.contains("hei") {
        candidates.extend(["simhei.ttf", "msyh.ttc"]);
    } else if name.contains("楷") || name.contains("kai") {
        candidates.extend(["simkai.ttf", "simsun.ttc"]);
    } else if name.contains("雅黑") || name.contains("yahei") {
        candidates.extend(["msyh.ttc", "msyh.ttf"]);
    } else if name.contains("times") {
        candidates.extend(["times.ttf", "timesbd.ttf"]);
    } else if name.contains("tahoma") {
        candidates.extend(["tahoma.ttf", "tahomabd.ttf"]);
    } else if name.contains("courier") || name.contains("cour") {
        candidates.extend(["cour.ttf", "consola.ttf"]);
    } else if name.contains("arial") {
        candidates.extend(["arial.ttf", "arialbd.ttf"]);
    }
    if request.fixed_width {
        candidates.extend(["consola.ttf", "cour.ttf"]);
    }
    candidates.extend([
        "HarmonyOS_Sans_SC_Regular.ttf",
        "HarmonyOS_Sans_SC_Bold.ttf",
        "NotoSansCJK-Regular.ttc",
        "NotoSansSC-Regular.otf",
        "arial.ttf",
        "times.ttf",
        "msyh.ttc",
        "simsun.ttc",
    ]);
    for root in font_roots() {
        for candidate in &candidates {
            let path = root.join(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                if entry.path().extension().is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("ttf") || extension.eq_ignore_ascii_case("ttc")
                }) {
                    return Some(entry.path());
                }
            }
        }
    }
    None
}
