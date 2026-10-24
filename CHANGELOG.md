# Changelog

## Unreleased

### 新增

- `ofdbox-core`：`OfdPackage::write_ofd_entries` 生成并签署 `OFDEntries.xml`：按引用图列出夹带文件，写入 `FileList` 与 `SignedValueLoc`，签名值由 `SignedValueProducer` 产出；`EntriesWriteOptions` 控制 ID、元数据、签名值路径与是否覆盖。
- `ofdbox-core`：`OfdPackage::verify_ofd_entries` 校验 `OFDEntries.xml`：`FileList` 中的文件必须存在且不重复，`SignedValueLoc` 指向的签名值按 GB/T 35275 验证其对 `OFDEntries.xml` 字节的签名；并沿 `OFD.xml`、`Encryptions.xml` 遍历文档引用图，报告未列入 `FileList` 的夹带文件以及“列入但仍被引用”的不一致。
- `ofdbox-core`：盖章外观支持一次写多枚章（`SigningRequest::with_stamps`），跨页/同页均可；注释文件按页复用，单元 ID 连续分配；`Clip` 只在显式提供时写入。
- `ofdbox-core`：盖章外观 `StampAppearance` + `SigningRequest::with_stamp`：自动写 `StampAnnot`、页面注释（`Type="Stamp"`）、`DocumentRes` 图片资源与印模文件，并把它们纳入 `References`；要求 `Document.xml` 已声明 `DocumentRes` 与 `Annotations`。
- `ofdbox`：示例 `seal_and_stamp` 演示制章 → 盖章 → 验章，写出印章、制章者/签章者证书、签章者公钥（PEM）与盖章文件到 `target/`。
- `ofdbox-seal`：`SealBuilder` 按 GB/T 38540 或 GM/T 0031 生成 `SESeal`：印章图片（PNG/GIF/JPG/BMP/SVG + 毫米宽高）、证书列表或证书杂凑值列表、有效期，制章者 SM2 签名；生成结果可用 `verify_seal` 验回并做 `signer_in_seal` 匹配。
- `ofdbox-seal`：`Sm2SesSignatureProducer` 用已有 `SESeal` + 签章者证书/私钥生成 GB/T 38540 `SES_Signature`；`sign_package` 按 `SignedValueProducer::signature_type()` 写出 `Type=Seal` 的签名记录。
- `ofdbox-seal`：GB/T 38540 `SES_Signature` 解析与验签（`parse_ses_signature` / `verify_ses_signature` / `SesSignatureVerifier`），验证原文杂凑值、签章者签名、内嵌印章与签章者证书列表；43 份真实 `Type=Seal` 样本全部验通。
- `ofdbox-core`：`OfdPackage::write_to` / `write_with` / `to_bytes` / `save` 重打包与编辑写出。未修改的 entry 直接复制压缩数据并保留压缩方式、时间、权限、extra field、文件注释与包注释；非 UTF-8 名称改用原始名称字节复制。
- `ofdbox-core`：`OfdPackage::write_decrypted_to` 解密另存为未加密包，返回被移除条目与签名/完整性引用告警。
- `ofdbox-core`：`PackageEdits`（替换/删除/新增）与 `PackageWriteOptions`（保留条目、压缩方式、时间、目录条目）。
- `ofdbox-core`：`canonical_digest_method` 把 `CheckMethod` 的 OID、子弧、大小写写法归一化为 `SM3`/`MD5`/`SHA1`。
- `ofdbox-core`：`SignedValueFormat` 与 `detect_signed_value_format`，按内容嗅探 GB/T 35275 `SignedData`、GB/T 38540 `SES_Signature` 与厂商自定义格式。
- `ofdbox-core`：`SignatureVerifiers` 注册表，`verify_signature` 按「格式 → 字节认领 → 签名方法」分派到自定义验签器。
- `ofdbox-core`：`sign_package` 生成 `Type=Sign` 数字签名：按 `ProtectScope` 选文件、算 `References`、组装 `Signature.xml`、更新 `Signatures.xml`、必要时给 `OFD.xml` 补链接，再经 `PackageEdits` 写包；签名值由 `SignedValueProducer` 产出。
- `ofdbox-crypto`：`Sm2SignedDataProducer` 产出 GB/T 35275 `SignedData`（`.2.2`、SM3、纯 SM2 `.301.1`），并内置 `messageDigest`/`contentType` 签名属性。
- `ofdbox-crypto`：摘要校验支持 MD5 与 SHA1，用于历史文档的 `References`。
- `ofdbox`：门面导出 `EncryptOptions`、`EncryptionScheme` 与 `encrypt_package`，示例演示“签名 → 口令加密 → 解密验签”全流程。

### 修复

- 0099 严格校验：`Encryptions` 检查 `EncryptInfo/@ID` 唯一，`EncryptedEntries` 的 `EncryptEntry` 要求 `EPath`；新增重复 ID、缺 `EPath`、篡改密文与密钥描述的负例测试。
- 签名的 `BaseLoc` / `SignedValue` 写成不带前导 `/` 的包内全路径时，先按相对路径解析，找不到再回退到包根解析；`References` 里指向目录（以 `/` 结尾）的条目跳过，不再报缺少条目。
- `OFDEntries` 引用图补标准 `ST_Loc` 字段 `SchemaLoc`（`CustomTag` 的 schema 路径）；只按标准定义的字段名收集，不按 `Loc` 后缀猜测厂商自定义字段。
- `OFDEntries.xml` 按表 D.1 把 `File/@FileLoc` 作为属性读写（此前误作子元素）；`CreatorName`/`Version`/`CreationDate` 仍为根元素属性。严格模式下子元素 `FileLoc` 与缺失 `FileLoc` 属性都会报错。
- 盖章外观的元素定位与插入改用 `XmlDocument` 语法树和源字节区间，不再按字符串查找标签；同时修正 `XmlDocument` 元素 `Span` 只覆盖开始标签的问题（结束事件时补齐到结束标签之后）。
- 盖章外观：页 ID 与图片格式写入 XML 前转义；注释/资源文件名改用数字 ID，不再把页 ID 带进包内路径；支持 `<ofd:Annotations/>`、`<ofd:MultiMedias/>` 等自闭合容器并跳过 XML 注释；同一页已有 `<ofd:Page>` 时复用其注释文件，不再追加重复条目；`MaxUnitID` 缺失改为报错，并校验盖章页 ID 存在于 `Document/Pages`、盖章/裁剪/图片尺寸为有限正数。
- 按 GM/T 0099 附录 C.1，`UserInfo` 缺少 `IVValue` 时按 16 个 0 处理，不再跳过该用户。
- 按 GM/T 0099 附录 C.4 与图 C.3，`EncryptEntry/@DecryptSeedLoc` 改为属性读写（仍接受子元素旧写法），解密时按条目取独立密钥；单个条目解不开不再影响其他条目。
- `zip` 依赖启用 `unreserved` 特性，可读取并复制含 Info-ZIP Unix extra field 的包。
- `Provider` 属性缺失时回退读取同名子元素（`ProviderName`/`Version`/`Company`），真实样本有 3 份使用子元素写法。
- GB/T 35275（沿用 GM/T 0010）的标识 `1.2.156.10197.6.1.4.2.2`（signedData）与纯 SM2 签名算法标识 `1.2.156.10197.1.301.1` 均被接受；四份铁路电子客票 `Sign` 样本通过端到端验签。
- `CheckMethod` 按 GB/T 33190 表 68 作为自由字符串校验，不再限定 `MD5`/`SHA1`/`SM3`（真实文件大量写 SM3 的 OID 与子弧写法）；生成的签名默认写 `1.2.156.10197.1.401`，与真实文件一致。
- 示例与签名用例的 `SignatureDateTime` 改为 14 位 `YYYYMMDDHHMMSS`，与真实签名文件一致。
- `signedAttrs` 补充 `signingTime`（UTCTime），与真实签名文件的属性集合一致（contentType + signingTime + messageDigest）。
- `ofdbox-seal` 兼容厂商写法：BIT STRING 的 unused bits 非 0 时仍按字节串解释；`SES_Signature` 末尾容忍全零填充；签章者证书列表类型 3（厂商扩展）无法比对时跳过匹配但仍验证印章签名。
- CI 的 feature matrix 改用现存 crate 与 feature，删除已无二进制目标的 release 工作流。

### 变更

- 测试改为按标准自行构造：盖章测试用 `SealBuilder` 现场生成印章，加密测试用 `encrypt_package` 现场造包，不再依赖外部样张；外部语料只保留一个 `ofdbox/tests/corpus_compat.rs`（`#[ignore]`，需 `OFDBOX_TESTDATA_DIR`），统一做读包、重打包、验签与引用摘要的兼容性统计。
- 35275 `SignedData` 测试补齐负例：detached 内容篡改、缺 `signedAttrs`、未知摘要算法、缺证书、签名值篡改，以及旧 SM2 签名 OID `.301.1` 的正向兼容。
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
