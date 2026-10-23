# Changelog

## Unreleased

### 新增

- `ofdbox-core`：`OfdPackage::write_to` / `write_with` / `to_bytes` / `save` 重打包与编辑写出。未修改的 entry 直接复制压缩数据并保留压缩方式、时间、权限、extra field、文件注释与包注释；非 UTF-8 名称改用原始名称字节复制。
- `ofdbox-core`：`OfdPackage::write_decrypted_to` 解密另存为未加密包，返回被移除条目与签名/完整性引用告警。
- `ofdbox-core`：`PackageEdits`（替换/删除/新增）与 `PackageWriteOptions`（保留条目、压缩方式、时间、目录条目）。
- `ofdbox-core`：`canonical_digest_method` 把 `CheckMethod` 的 OID、子弧、大小写写法归一化为 `SM3`/`MD5`/`SHA1`。
- `ofdbox-crypto`：摘要校验支持 MD5 与 SHA1，用于历史文档的 `References`。

### 修复

- 按 GM/T 0099 附录 C.1，`UserInfo` 缺少 `IVValue` 时按 16 个 0 处理，不再跳过该用户。
- 按 GM/T 0099 附录 C.4 与图 C.3，`EncryptEntry/@DecryptSeedLoc` 改为属性读写（仍接受子元素旧写法），解密时按条目取独立密钥；单个条目解不开不再影响其他条目。
- `zip` 依赖启用 `unreserved` 特性，可读取并复制含 Info-ZIP Unix extra field 的包。
- `Provider` 属性缺失时回退读取同名子元素（`ProviderName`/`Version`/`Company`），真实样本有 3 份使用子元素写法。
- CI 的 feature matrix 改用现存 crate 与 feature，删除已无二进制目标的 release 工作流。

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
