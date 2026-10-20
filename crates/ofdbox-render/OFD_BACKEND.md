# OFD 后端现状

## 已实现

- 多页 Scene 写成 OFD 1.1 ZIP 包：`OFD.xml`、`Document.xml`、逐页 `Content.xml`。
- 路径对象：M/L/Q/B/A/C，Fill/Stroke、RGB 颜色、线宽、端点、连接、虚线。
- 图片对象：复制原始图片资源到 `Doc_0/Res`，通过 `DocumentRes.xml` 引用。
- 文字对象：嵌入 `FontFile` 到 `PublicRes.xml`，通过字形反查 Unicode 写 `TextCode`。
- 页面尺寸、对象边界和 CTM 从 Scene 计算。

## 已知限制

- 组裁剪、透明度、渐变尚未写入；渐变填充退化为第一个色标。
- 文字位置仅使用首个字形 X/Y 和字体默认 advance，复杂字距/旋转需后续补 `DeltaX`。
- 字体反查失败的字形写为替换字符；CFF/TTC 字体可写入，但能否被阅读器正确使用取决于字体格式。
- 图片 SMask 未单独拆分；带透明通道的图片按原始文件写入。
- 未实现签章、注释、附件、书签、文档元数据。
- 尚未做 XSD 严格校验，只通过包结构和回读 XML 测试。

## 验证

```powershell
cargo test -p ofdbox-render --lib backend::ofd
```
