use ofdbox_core::OfdError;

/// 渲染错误。
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// OFD 结构读取或解析失败。
    #[error("OFD 结构错误: {0}")]
    Ofd(#[from] OfdError),
    /// 当前实现无法表达的场景特性。
    #[error("不支持的渲染特性: {0}")]
    Unsupported(String),
    /// 引用的资源缺失。
    #[error("缺少渲染资源: {0}")]
    MissingResource(String),
    /// 图片解码失败。
    #[error("图片解码失败: {0}")]
    Image(String),
    /// 字体解析失败。
    #[error("字体解析失败: {0}")]
    Font(String),
    /// 页面尺寸无法用于渲染。
    #[error("页面尺寸无效")]
    InvalidPage,
    /// 后端执行失败。
    #[error("渲染后端错误: {0}")]
    Backend(String),
    /// I/O 错误。
    #[error("I/O 错误: {0}")]
    Io(#[from] std::io::Error),
}
