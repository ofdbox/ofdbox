# OFD 后端现状

## 已实现

- 多页 Scene 写成 OFD 1.1 ZIP 包：`OFD.xml`、`Document.xml`、逐页 `Content.xml`。
- 路径对象：M/L/Q/B/A/C，Fill/Stroke、RGB 颜色、线宽、端点、连接、虚线。
- 图片对象：复制原始图片资源到 `Doc_0/Res`，通过 `DocumentRes.xml` 引用；整页栅格模式就是单个覆盖全页的 `ImageObject`。
- 文字对象：`TextElement.text` 存在时写原生 `TextObject` + `FontFile` + `TextCode`；没有原文时回退字形轮廓。
- 路径对象坐标已转换到页面坐标，`CTM` 写单位矩阵；不再依赖 `Boundary` 平移。
- 页面尺寸、对象边界和 CTM 从 Scene 计算。

## 已知限制

- 组裁剪、透明度、渐变尚未写入；渐变填充退化为第一个色标。
- PDF 来源的填充路径仍有兼容问题；复杂 PDF 默认不要用 `--vector`。
- 文字位置只用首个字形 X/Y 和字体默认 advance；复杂字距/旋转需要补 `DeltaX`。
- 字体名仍是合成的 `FontN`，原 FontName/FamilyName 未保留；CFF/TTC 嵌入依赖阅读器。
- 图片 SMask 未单独拆分；带透明通道的图片按原始文件写入。
- 未实现签章、注释、附件、书签、文档元数据。
- 尚未做 XSD 严格校验，只通过包结构和回读 XML 测试。

## Feature\n\n- `ofd`：启用 OFD 前端/后端和 ZIP 写出；不强制启用图片解码。\n- OFD 图片资源可解析，但要渲染图片需对应 `image-*`。\n\n## 验证

```powershell
cargo test -p ofdbox-render --lib backend::ofd
```

