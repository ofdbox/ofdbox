use std::sync::Arc;

use crate::error::RenderError;

/// 字体加载请求。
#[derive(Clone, Debug)]
pub struct FontRequest<'a> {
    /// 字体资源 ID（`Res/Font/@ID`）。
    pub font_id: Option<&'a str>,
    /// `FontName`。
    pub font_name: Option<&'a str>,
    /// `FamilyName`。
    pub family_name: Option<&'a str>,
    /// `Charset`。
    pub charset: Option<&'a str>,
    /// 是否粗体。
    pub bold: bool,
    /// 是否斜体。
    pub italic: bool,
    /// 是否衬线体。
    pub serif: bool,
    /// 是否等宽体。
    pub fixed_width: bool,
    /// 文档声明的内嵌字体文件路径。
    pub font_file: Option<&'a str>,
}

/// 调用方注入的字体来源。
///
/// 库不内置系统字体；没有嵌入字体的 OFD 需要调用方提供匹配逻辑。
pub trait FontProvider {
    /// 返回字体文件字节，未找到时返回 `None`。
    fn load(&mut self, request: &FontRequest<'_>) -> Result<Option<Arc<[u8]>>, RenderError>;
}

/// 用闭包/函数实现 [`FontProvider`]。
///
/// 调用方可以按 `font_id`、`font_name` 或 `font_file` 做精确映射：
///
/// ```ignore
/// let frontend = OfdFrontend::new(...).with_font_resolver(|request| {
///     if request.font_name == Some("Latha") {
///         Ok(Some(Arc::from(latha_bytes.as_slice())))
///     } else {
///         Ok(None)
///     }
/// });
/// ```
pub struct FnFontProvider<F> {
    resolver: F,
}

impl<F> FnFontProvider<F> {
    /// 创建闭包字体提供者。
    pub fn new(resolver: F) -> Self {
        Self { resolver }
    }
}

impl<F> FontProvider for FnFontProvider<F>
where
    F: for<'a> FnMut(&FontRequest<'a>) -> Result<Option<Arc<[u8]>>, RenderError>,
{
    fn load(&mut self, request: &FontRequest<'_>) -> Result<Option<Arc<[u8]>>, RenderError> {
        (self.resolver)(request)
    }
}

#[cfg(test)]
mod tests {
    use super::{FnFontProvider, FontProvider, FontRequest};
    use std::sync::Arc;

    #[test]
    fn closure_provider_receives_request() {
        let mut provider = FnFontProvider::new(|request: &FontRequest<'_>| {
            assert_eq!(request.font_id, Some("42"));
            assert_eq!(request.font_name, Some("Latha"));
            Ok(Some(Arc::<[u8]>::from(&b"font-bytes"[..])))
        });
        let request = FontRequest {
            font_id: Some("42"),
            font_name: Some("Latha"),
            family_name: None,
            charset: None,
            bold: false,
            italic: false,
            serif: false,
            fixed_width: false,
            font_file: None,
        };
        let bytes = provider.load(&request).expect("load").expect("font");
        assert_eq!(&bytes[..], b"font-bytes");
    }
}
