# OFDBox

OFDBox 是一个 Rust 实现的 OFD 文档工具箱，覆盖结构读写、加密验签、签章、页面渲染和格式转换。

当前已实现 GB/T 33190—2016 结构读写、GM/T 0099—2020 加密与签名结构、SM2/SM3/SM4 密码原语、印章识别验签，以及 `ofdbox-render` 的 OFD/PDF 前端、统一 Scene IR、CPU 位图后端和 PDF/OFD 文档后端。wgpu 后端和文档生成仍为占位。

## 能力范围

| 能力 | 状态 | 说明 |
| --- | --- | --- |
| 文档结构读取/写出 | 已完成 | GB/T 33190—2016 全量文件与结构模型，宽松读取、严格写出、缓存 |
| GM/T 0099 加密结构 | 已完成 | 签名扩展、`Encryptions.xml`、`DecryptSeed`、`EncryptedEntries`、`OFDEntries.xml` |
| 文档加密/解密 | 结构层完成 | 密码/证书、SM4、流式解密；密钥随会话释放 |
| 签名/验签 | 结构层完成 | References 摘要校验、SM2DSA、SignedData、印章注解 |
| 文档渲染 | 已完成基础能力 | OFD/PDF 前端 + `vello_cpu` 位图后端；PDF 后端可用；wgpu 占位 |
| 格式转换 | 基础完成 | OFD↔PDF、OFD/PDF→PNG；PDF→OFD 矢量链路仍有限制 |
| 文档生成 | 未开始 | 规划中 |

## 模块

| crate | 状态 | 职责 |
| --- | --- | --- |
| `ofdbox-core` | 已实现 | ZIP/XML 解析、OFD/GM 结构模型、严格写出、缓存、解密会话 |
| `ofdbox-crypto` | 已实现 | SM3-KDF、SM4-CBC、SM2 加解密/签名、证书链、SignedData |
| `ofdbox-seal` | 已实现 | 印章结构识别、图片提取、印章验签 |
| `ofdbox-render` | 已实现 | OFD/PDF 前端 → Scene IR → vello_cpu/PDF/OFD 后端；单文档转换 |
| `ofdbox` | 已实现 | 对外门面：常用类型、统一错误、feature 转发 |
| `ofdbox-cli` | 已实现 | 批量转换、目录遍历、页码/DPI/报告、性能汇总 |
| `ofdbox-generate` | 规划 | 结构化内容 → 排版 → OFD/PDF |

`ofdbox` 是对外门面；`ofdbox-render` 同时承担现有格式转换，不额外增加批处理包装层。

```rust
use ofdbox::prelude::*;

let mut package = OfdPackage::open("sample.ofd")?;
let document = package.parse_document()?;
```

## ofdbox-render

`ofdbox-render` 的转换链路：

```text
OFD ──OfdFrontend──┐
                   ├─→ Scene IR ─→ vello_cpu（PNG）
PDF ──PdfFrontend──┘             ├─→ PdfDocument（PDF）
                                 └─→ OfdBackend（OFD）
```

- OFD 前端：结构对象、颜色、渐变、图像、文字、裁剪、注释。
- PDF 前端：页面/内容流、路径、颜色、图片（Flate/JPEG/CCITT）、Form、基础文字。
- Scene IR：路径、图像、文字、字体资源、可选原文 `TextElement.text`。
- 后端：`vello_cpu`、PDF 1.4/1.7、OFD 1.1。
- OFD→PDF 优先嵌入 TrueType 字体并写 ToUnicode；不支持的字体回退轮廓。
- PDF→OFD 默认栅格，保证视觉一致；`--vector` 为实验性原生文字/路径模式。

详细限制：

- [`crates/ofdbox-render/PDF_FRONTEND.md`](crates/ofdbox-render/PDF_FRONTEND.md)
- [`crates/ofdbox-render/OFD_BACKEND.md`](crates/ofdbox-render/OFD_BACKEND.md)

### Feature 裁剪

默认全功能：`ofd + pdf + cpu + image`。可按需关闭：

```toml
ofdbox-render = { path = "...", default-features = false, features = ["ofd"] }
ofdbox-render = { path = "...", default-features = false, features = ["pdf"] }
ofdbox-render = { path = "...", default-features = false, features = ["ofd", "cpu", "image-ccitt"] }
```

- `ofd`：OFD 结构/前端/后端，含 ZIP；图片解码按 `image-*` 选择。
- `pdf`：PDF 前端/后端，包含 PNG 编码；CCITT 扫描页需 `image-ccitt`。
- `cpu`：`vello_cpu` 位图后端。
- `image`：全部图片编码器；也可单独启用 `image-png`、`image-jpeg`、`image-bmp`、`image-tiff`、`image-jbig2`、`image-ccitt`。
- OFD 的图片资源结构解析不依赖解码器；没有对应 `image-*` 时，图片渲染会返回“不支持”诊断。

`ofdbox-cli` 默认使用全功能，不受最小集影响。`ofdbox-render` 已通过 `wasm32-unknown-unknown` 编译检查。
门面同样支持细粒度裁剪：

```toml
ofdbox = { path = "...", default-features = false, features = ["pdf", "image-ccitt"] }
ofdbox = { path = "...", default-features = false, features = ["ofd", "cpu", "image"] }
ofdbox = { path = "...", default-features = false, features = ["crypto"] }
```

`ofdbox` 的 `render` 是 `ofd + pdf + cpu + image` 的合集；也可以只启用其中一项。

`ofdbox-cli` 默认使用全功能，不受最小集影响。`ofdbox-render` 已通过 `wasm32-unknown-unknown` 编译检查。

## CLI 使用

```bash
cargo run -p ofdbox-cli -- convert <输入...> -t <pdf|ofd|png> -o <输出目录>
```

示例：

```bash
# OFD 批量转 PDF，保留相对目录
ofdbox convert ./OFD文件 -t pdf -o ./pdf_export --report ./report.txt

# PDF/OFD 转 PNG
ofdbox convert ./pdf_export -t png -o ./png --dpi 200

# PDF 转 OFD（默认栅格；矢量加 --vector）
ofdbox convert ./a.pdf -t ofd -o ./out
```

参数：`--pages 1-3,5`、`--dpi`、`--force`、`--no-recursive`、`--quiet`、`--report`。详见 [`crates/ofdbox-cli/README.md`](crates/ofdbox-cli/README.md)。

## 库使用

```rust
use ofdbox_core::OfdPackage;

fn main() -> Result<(), ofdbox_core::OfdError> {
    let mut package = OfdPackage::open("sample.ofd")?;
    let root = package.parse_root()?;
    println!("{}", root.to_xml()?);
    let document = package.parse_document()?;
    if let Some(pages) = document.pages() {
        for page in pages.pages().take(1) {
            let content = package.parse_page_for(&page)?;
            println!("{}", content.to_xml()?);
        }
    }
    Ok(())
}
```

WASM/内存场景使用 `OfdPackage::from_bytes(bytes)`。加密文档见 `ofdbox_crypto` 的 `PasswordBackend` / `CertificateBackend`。

## 标准覆盖

- **GB/T 33190—2016**：文档结构已覆盖。
- **GM/T 0099—2020**：签名扩展和加密结构已覆盖。
- **OFD/A、OFD-H**：未覆盖。

## 开发

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo check --target wasm32-unknown-unknown
```

集成测试可通过 `OFDBOX_TESTDATA_DIR` 指向 OFD 样例目录。

## 里程碑

| 阶段 | 内容 | 状态 |
| --- | --- | --- |
| M0 | 2016 结构读写、0099 加密结构、缓存、WASM 基础 | 已完成 |
| M1 | SM2/SM3/SM4、加密写回、References、SignedData | 基本完成 |
| M2 | 页面渲染、图片、PDF 后端 | 基本完成；OFD/PDF 前端和两个文档后端已落地 |
| M3 | OFD↔PDF、OFD/PDF→PNG | 基础完成；PDF→OFD 矢量仍需完善 |
| M4 | 结构化内容 → OFD/PDF 生成 | 规划 |

## 许可证

MIT OR Apache-2.0，二选一。

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)





