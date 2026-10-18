/// XML 读取模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReadOptions {
    /// 严格模式：命名空间、顺序、必选字段和未知节点都按标准校验。
    pub strict: bool,
}

impl ReadOptions {
    /// 宽松模式，读取时尽量容忍真实文件中的偏差。
    pub const LENIENT: Self = Self { strict: false };
    /// 严格模式。
    pub const STRICT: Self = Self { strict: true };
}

/// 缓存配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheOptions {
    /// 缓存上限，单位为字节；`0` 表示关闭缓存。
    pub max_bytes: usize,
}

impl Default for CacheOptions {
    fn default() -> Self {
        Self {
            max_bytes: 32 * 1024 * 1024,
        }
    }
}

impl CacheOptions {
    /// 关闭缓存。
    pub const DISABLED: Self = Self { max_bytes: 0 };
}
