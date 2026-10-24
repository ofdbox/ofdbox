# OFDBox

OFDBox 是一个 Rust 实现的 OFD 版式文档工具箱，依据 GB/T 33190—2016 与 GM/T 0099—2020，覆盖结构读写、加密验签和印章识别。

页面渲染、格式转换与文档生成已拆分为独立项目 [`ofdbox-folio`](https://github.com/ofdbox/ofdbox-folio)；桌面/Web 阅读器与批量转换命令行位于 [`ofdbox-reader`](https://github.com/ofdbox/ofdbox-reader)。

## 能力范围

| 能力 | 状态 | 说明 |
| --- | --- | --- |
| 文档结构读取/写出 | 已完成 | GB/T 33190—2016 全量文件与结构模型，宽松读取、严格写出、缓存 |
| GM/T 0099 加密结构 | 已完成 | 签名扩展、`Encryptions.xml`、`DecryptSeed`、`EncryptedEntries`、`OFDEntries.xml` |
| 文档加密/解密 | 已完成 | 口令/证书后端、SM4、流式解密；口令加密写出；密钥随会话释放 |
| 包写出/重打包 | 已完成 | `write_to`/`write_with` 原样复制 ZIP 元数据，可替换/删除/新增 entry；加密包可 `write_decrypted_to` 另存为未加密包 |
| 数字签名生成 | 已完成 | `Type=Sign`：按保护范围计算 References、生成 `Signature.xml`/`Signatures.xml`/`SignedValue.dat`（GB/T 35275 SignedData），支持追加签名 |
| 签名/签章验签 | 已完成 | 35275 SignedData、38540 `SES_Signature` 与厂商自定义格式按内容分派；可插拔 `SignatureVerifier` |
| 印章识别/验签 | 已完成 | GM/T 0031 与 GB/T 38540 印章格式识别、图片提取、制章者签名、证书链与签章者列表 |

## 模块

| crate | 职责 |
| --- | --- |
| `ofdbox-core` | ZIP/XML 解析、OFD/GM 结构模型、严格写出、缓存、解密会话 |
| `ofdbox-crypto` | SM3-KDF、SM4-CBC、SM2 加解密/签名、证书链、SignedData |
| `ofdbox-seal` | 印章结构识别、图片提取、印章验签 |
| `ofdbox` | 对外门面：常用类型、统一错误、feature 转发 |

## 使用

```rust
use ofdbox::prelude::*;

let mut package = OfdPackage::open("sample.ofd")?;
let document = package.parse_document()?;
```

签名与加密的完整流程见 `crates/ofdbox/examples/sign_and_verify.rs`：建包 → 签名（35275）→ 验签 → 口令加密 → 解密再验签。加密文档也可直接使用 `PasswordBackend` / `CertificateBackend` 与 `enable_decryption`；签章验签见 `ofdbox::SesSignatureVerifier`。

### Feature

默认启用 `crypto` 与 `seal`：

```toml
ofdbox = { path = "...", default-features = false, features = ["crypto"] }
ofdbox = { path = "...", default-features = false, features = ["seal"] }
```

## 标准覆盖

- **GB/T 33190—2016**：文档结构已覆盖。
- **GM/T 0099—2020**：签名扩展和加密结构已覆盖。
- **OFD/A、OFD-H**：未覆盖。

## 开发

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

集成测试可通过 `OFDBOX_TESTDATA_DIR` 指向 OFD 样例目录。

## 相关项目

- [`ofdbox-folio`](https://github.com/ofdbox/ofdbox-folio)：Flow IR / Fixed IR、OFD/PDF 读写、CPU 与 wgpu 渲染。
- [`ofdbox-reader`](https://github.com/ofdbox/ofdbox-reader)：桌面/Web 阅读器与 `ofdbox` 批量转换命令行。

## 许可证

MIT OR Apache-2.0，二选一。

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)
