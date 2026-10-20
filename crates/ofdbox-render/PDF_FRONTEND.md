# PDF 前端现状

## 已实现

- 加载内存/文件 PDF，读取页数、CropBox/MediaBox，页面坐标转换为 Scene 的毫米坐标。
- 图形状态：`q/Q`、`cm`、线宽/端点/连接/虚线、图形状态 alpha。
- 路径：`m/l/c/v/y/h/re`，`S/s/f/F/f*/B/B*/b/b*`，非零/奇偶填充。
- 颜色：`g/G/rg/RG/k/K`，裁剪 `W/W*` 与 `n`。
- XObject：Form 递归（最大深度 8）、图片（Flate 原始像素、DCTDecode JPEG、CCITTFaxDecode G3/G4、SMask）。
- 文字：嵌入的 TrueType `FontFile2`，Identity-H 与简单字体，`Tj/TJ/Tm/Td/TD/T*/'/"`，基础 ToUnicode（`beginbfchar`）。
- 示例：`pdf_frontend_batch` 批量把 PDF 转 PNG。

## 已知限制

- 未嵌入字体、`FontFile3`/CFF/Type1 字体不会渲染文字；当前没有 PDF 字体提供器回退。
- 不支持 JPX、JBIG2、LZW 等图片编码；扫描件优先支持 CCITT/JPEG。
- ToUnicode 只解析 `beginbfchar`，`beginbfrange`、多字符映射未处理。
- 文本字距/宽度优先使用字体表，未完整读取 `/Widths`、`/W`、`/DW`。
- 不支持 Shading、Type3 字体、注释、加密 PDF、页面 `/Rotate`。
- Form XObject 的 BBox 裁剪、透明度组、SMask 矩阵尚未实现。
- 复杂裁剪在 CTM 变化后的语义按当前用户空间近似处理。

## 验证

```powershell
cargo test -p ofdbox-render --lib frontend::pdf
cargo run -p ofdbox-render --release --example pdf_frontend_batch -- <PDF目录> <PNG输出目录>
```
