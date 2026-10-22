# Changelog

## Unreleased

### 变更

- 渲染、格式转换与文档生成迁至独立仓库 `ofdbox-folio`。
- 批量转换命令行迁至 `ofdbox-reader`，删除 `ofdbox-cli`。
- 删除 `ofdbox-render`；`ofdbox` 门面移除 render/ofd/pdf/cpu/image feature 与相关导出，只保留 core、crypto、seal。

## 0.0.1 - 2026-10-21

首个 alpha 版本。

### 新增

- `ofdbox-core`：OFD 结构读取/写出、缓存、加密会话。
- `ofdbox-crypto`：SM2/SM3/SM4、密码/证书解密、SignedData 验签。
- `ofdbox-seal`：印章格式识别、图片提取与验签。
- `ofdbox-render`：OFD/PDF 前端、Scene IR、vello_cpu、PDF 和 OFD 后端。
- `ofdbox`：对外门面、统一错误和细粒度 feature。
- `ofdbox-cli`：OFD/PDF/PNG 批量转换、相对目录、页码、DPI、报告和性能汇总。
- `ofdbox-render` feature：`ofd`、`pdf`、`cpu`、`image` 及独立图片编码器。
- `ofdbox` feature：`ofd`、`pdf`、`cpu`、`image`、`crypto`、`seal`。

### 已知限制

- PDF→OFD 默认栅格，`--vector` 仍为实验性。
- PDF 部分字体、裁剪和填充路径尚未完整支持。
- OFD→PDF 对 TTC/CFF 字体可能回退为轮廓。
- HarmonyOS 仅完成库/CLI 编译检查，未提供 HAP 打包。
- crates.io 发布需单独准备 package metadata 和 token。
