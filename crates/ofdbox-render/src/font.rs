use crate::error::RenderError;

/// 字体加载请求。
#[derive(Clone, Debug)]
pub struct FontRequest<'a> {
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
}

/// 调用方注入的字体来源。
///
/// 库不内置系统字体；没有嵌入字体的 OFD 需要调用方提供匹配逻辑。
pub trait FontProvider {
    /// 返回字体文件字节，未找到时返回 `None`。
    fn load(&mut self, request: &FontRequest<'_>) -> Result<Option<Vec<u8>>, RenderError>;
}
