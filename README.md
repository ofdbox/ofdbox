# OFDBox

OFDBox 是一个用 Rust 实现的 OFD 文档处理库。项目目标是覆盖 OFD 的结构读写、密码应用、签章验签、渲染、格式转换和文档生成。

当前仓库已实现 GB/T 33190—2016 的结构读写、GM/T 0099—2020 的加密解密与签名结构、`ofdbox-crypto` 中的 SM2/SM3/SM4 密码原语、`ofdbox-seal` 的印章识别与验签，以及 `ofdbox-render` 的 OFD 场景前端与 CPU 位图后端。PDF/wgpu 后端、渲染能力补全、格式转换和文档生成尚未实现。

## 目标范围

| 能力 | 状态 | 说明 |
| --- | --- | --- |
| 基本结构读取/写入 | 已完成 | GB/T 33190—2016 全部根文件和具名复杂类型；宽松解析、严格校验、规范化写出；按需读取与缓存 |
| GM/T 0099 描述结构 | 已完成 | 签名扩展、`Encryptions.xml`、`DecryptSeed`、`EncryptedEntries`、`OFDEntries.xml` 已解析/写出 |
| 文档加密/解密 | 已完成（结构层） | 口令/证书、单层与多重懒解密、加密写出、流式解密；密钥内存在 drop 时擦除 |
| 签章/验签 | 已完成（结构层） | 签名结构、References 摘要校验、SM2DSA、GB/T 35275 SignedData 默认验签和自定义验签注入 |
| 文档渲染 | 部分完成 | OFD 前端 + `vello_cpu` 位图后端；PDF/wgpu 后端为占位 |
| 格式转换 | 未开始 | 规划 OFD ↔ PDF、OFD ↔ 图片 |
| 文档生成 | 未开始 | 规划由结构化数据自动排版生成 OFD/PDF |

“已完成”指结构层面的读取、解析和序列化，不代表密码学或渲染能力。

## 已实现的结构

`ofdbox-core` 覆盖：

- `OFD.xml`、`Document.xml`、`Content.xml`
- `Res`、`Annotations`/`PageAnnot`
- `Signatures`/`Signature`（含 0099 扩展字段）
- `DocVersion`、`Attachments`、`CustomTags`、`Extensions`
- 颜色、底纹、渐变、文字/路径/图像载荷、Clips、Region

行为约定：

- **宽松读取**：容忍命名空间变体、混合前缀、元素乱序、未知节点和厂商拼写差异。
- **严格写出**：从类型化视图生成标准命名空间、XSD 顺序和规范化属性；等于 XSD 默认值的属性/元素省略。
- **按需读取**：ZIP entry 和 XML 语法树分别做字节上限 LRU 缓存，不解压整个文档。
- **完全静默**：库不输出日志；严格模式的偏差通过 `Error` 中的诊断返回。
- **纯 Rust / WASM**：ZIP 使用纯 Rust 解压后端，核心 I/O 抽象为 `Read + Seek`，`wasm32-unknown-unknown` 编译通过。

## 规划中的模块

| crate | 状态 | 职责 |
| --- | --- | --- |
| `ofdbox-core` | 已实现 | 容器、XML 语法树、类型化视图、严格写出、缓存、透明解密会话 |
| `ofdbox-crypto` | 已实现 | SM3-KDF、SM4-CBC、SM2 加解密/签名、口令/证书后端、方案注册表、加密写出、SignedData 验签 |
| `ofdbox-seal` | 已实现 | 电子印章结构识别、图片提取、印章验签、证书链与有效期验证 |
| `ofdbox-render` | 已实现（首期） | 统一场景 IR、共享资源池、OFD 前端、`vello_cpu` 位图后端 |
| `ofdbox-convert` | 规划 | OFD ↔ PDF、OFD ↔ 图片 |
| `ofdbox-convert` | 规划 | OFD ↔ PDF、OFD ↔ 图片 |
| `ofdbox-generate` | 规划 | 结构化数据 → 布局 → OFD/PDF |
| `ofdbox-cli` | 规划 | 命令行工具 |

模块拆分以实际里程碑为准，避免提前创建空 crate。

## 使用

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

WASM 或内存场景使用 `OfdPackage::from_bytes(bytes)`。

加密文档：

```rust
use ofdbox_core::OfdPackage;
use ofdbox_crypto::{CertificateBackend, PasswordBackend};

let mut package = OfdPackage::open("encrypted.ofd")?;
package.enable_decryption(Box::new(PasswordBackend::new("password")))?;

// 或者使用 SM2 私钥
// package.enable_decryption(Box::new(CertificateBackend::new(private_key)))?;

let root = package.parse_root()?;
let document = package.parse_document()?;
```

解密按需发生：启用时只解出文件密钥和明密文映射表，页面与资源在读取时解密并进入独立明文缓存。`close_decryption()` 会释放会话并清空明文缓存。

## 批量转图

```bash
cargo run -p ofdbox-render --release --example render_batch -- <输入目录> <输出目录>
```

递归查找输入目录下的 `.ofd`，每个文档输出到独立子目录，页面命名为 `page_0001.png`。
无法读取或渲染的文档/页面会跳过，并记录到输出目录的 `_skipped.txt`。

## 标准覆盖

- **GB/T 33190—2016**：核心结构已覆盖。
- **GM/T 0099—2020**：签名描述扩展和加密描述结构已覆盖；密码运算未实现。
- **OFD/A、OFD-H 及其他衍生标准**：未覆盖。

GM/T 0099 没有随标准提供 XSD，严格规则按正文、图和真实样本手工整理；冲突点记录在工作区根目录的 `GM-T-0099-2020-对象结构.md`。

## 构建与测试

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo check --target wasm32-unknown-unknown
```

真实样本测试通过环境变量启用：

```bash
export OFDBOX_TESTDATA_DIR=/path/to/OFD/samples
cargo test
```

也可以把路径写入仓库根目录的 `.env`（已在 `.gitignore` 中忽略）。未设置时相关测试自动跳过。

## 里程碑

| 阶段 | 内容 | 状态 |
| --- | --- | --- |
| M0 | 2016 结构读写、0099 描述结构、缓存、WASM 编译 | 已完成 |
| M1 | SM2/SM3/SM4、解密、加密写出、References 校验、SignedData 验签 | 已完成（合成 SignedData 验证） |
| M2 | 页面渲染、图片导出、PDF 导出 | 进行中（OFD 前端与位图后端已完成） |
| M3 | OFD ↔ PDF、OFD ↔ 图片转换 | 规划 |
| M4 | 结构化数据布局与 OFD/PDF 生成 | 规划 |

## 许可证

本项目采用 MIT 或 Apache-2.0 双协议，任选其一：

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)

SPDX 标识：`MIT OR Apache-2.0`。
