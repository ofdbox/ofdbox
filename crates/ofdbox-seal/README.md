# ofdbox-seal

电子印章（ESL）的结构识别、制章、解析与验章。

支持两种印章标准：

- **GB/T 38540—2020**：`SESeal` + `SES_Signature`
- **GM/T 0031—2014**：`SES_SealInfo` + `SES_SignInfo`

本 crate 只处理“印章/签章数据结构”，不负责 OFD 包读写；与 OFD 的集成由
`ofdbox-core` 的 `sign_package`、`parse_signature`、`verify_signature` 完成。

---

## 1. 证书角色

制章、签章、验章涉及三类证书，职责不同，不能混用：

| 证书 | 私钥持有者 | 用途 | 出现在哪里 |
|---|---|---|---|
| 制章证书（maker） | 制章系统 | 对 `SESeal` 签名，证明印章由谁制作 | 印章数据的证书字段（38540 `SESeal.cert` / 0031 外层签名容器） |
| 签章证书（signer） | 签章系统 | 对 `TBS_Sign` 签名 | 印章的签章者证书列表、`SES_Signature.cert` |
| TSA 证书 | 时间戳服务机构 | 对 `TimeStampToken` 签名 | 时间戳令牌内部，或验证方信任库 |

TSA 证书**不是**制章/签章的输入：它由 TSA 随令牌返回，或由验证方预置为信任锚。

---

## 2. 制章流程

```text
制章系统
  │
  ├─ 1. 准备制章密钥对、签章密钥对，取得对应证书
  │
  ├─ 2. 构造 SealSpec
  │     ├─ 标准（GbT38540 / GmT0031）
  │     ├─ 印章标识、名称、类型、版本
  │     ├─ 有效期
  │     ├─ 印章图片（PNG/GIF/JPG/BMP/SVG）
  │     └─ 签章者证书列表（类型 1：证书；类型 2：证书杂凑值）
  │
  ├─ 3. SealBuilder 编码印章信息并对它签名
  │     签名对象：印章信息（38540 为 sealInfo，0031 为 SealInfo）
  │
  └─ 4. 输出印章数据（.esl / 印章字节）
```

对应 API：

```rust
use ofdbox_seal::{
    SealBuilder, SealPictureInput, SealSpec, SealStandard,
};

let spec = SealSpec::new(
    SealStandard::GbT38540,
    "SEAL-001",          // 印章标识
    "测试印章",           // 印章名称
    SealPictureInput {
        media_type: "PNG".to_owned(),
        data: png_bytes,
        width: 280,
        height: 180,
    },
)
.with_validity("20260101000000", "20260101000000", "20360101000000")
.with_certificates(vec![signer_certificate_der]);

let seal = SealBuilder::new(
    spec,
    maker_certificate_der,
    maker_private_key,
)
.with_dist_id("1234567812345678") // 可选，SM2 签名者标识
.build()?;
```

制章只依赖制章证书/私钥和签章证书列表，**不需要 TSA**。

---

## 3. 签章流程

```text
签章系统
  │
  ├─ 1. dataHash = SM3(原文)
  │
  ├─ 2. 构造 TBS_Sign
  │     ├─ version（与印章版本一致）
  │     ├─ eseal（印章数据）
  │     ├─ timeInfo（签章时间，自报）
  │     ├─ dataHash
  │     └─ propertyInfo（原文路径等）
  │
  ├─ 3. 签章私钥对 TBS_Sign 签名 → signature
  │
  ├─ 4. 可选：对 signature 值打时间戳
  │     ── SM3(signature) → TSA ──▶ TimeStampToken
  │
  ├─ 5. 组装 SES_Signature / SES_SignInfo
  │     ├─ toSign
  │     ├─ cert（签章证书）
  │     ├─ signatureAlgID
  │     ├─ signature
  │     └─ timeStamp [0] BIT STRING（可选，GB/T 38540）
  │
  └─ 6. 写入 OFD 的 SignedValue.dat，并在 Signature.xml 中引用
```

`Signature.xml` 是最小签章描述，`SignedValue.dat` 才是真正的密码学签章值。

对应 API：

```rust
use std::io::Cursor;

use ofdbox_core::{
    DEFAULT_SIGNATURE_METHOD, OfdPackage, SigningRequest,
};
use ofdbox_seal::Sm2SesSignatureProducer;

let producer = Sm2SesSignatureProducer::new(
    seal_bytes,
    signer_certificate_der,
    signer_private_key,
)?;

let request = SigningRequest::new(DEFAULT_SIGNATURE_METHOD, "20261024050000");
let mut package = OfdPackage::from_bytes(package_bytes)?;
let mut output = Vec::new();
package.sign_package(&request, &producer, &mut Cursor::new(&mut output))?;
```

`Sm2SesSignatureProducer` 生成的是 38540 的四字段 `SES_Signature`：

```text
SEQUENCE {
    toSign,
    cert,
    signatureAlgID,
    signature
}
```

签章值产生后，才能向 TSA 请求时间戳并附加：

```rust
use ofdbox_seal::{attach_time_stamp, parse_ses_signature};

let ses = parse_ses_signature(&signed_value)?;
let token = request_timestamp_from_tsa(/* SM3(ses.signature) */)?;
let signed_with_timestamp = attach_time_stamp(&signed_value, &token)?;
```

`attach_time_stamp` 只负责把令牌写入 `timeStamp [0]`，不校验令牌；验证见第 5 节。

---

## 4. 验章流程

```text
验证方
  │
  ├─ 1. 解析 SignedValue.dat → SES_Signature
  │
  ├─ 2. dataHash == SM3(原文) ?
  │
  ├─ 3. 用签章证书验 signature 对 TBS_Sign 的签名
  │
  ├─ 4. 签章证书是否在印章的签章者证书列表中 ?
  │
  ├─ 5. 用制章证书验印章的制章者签名
  │
  ├─ 6. 证书链、有效期、印章有效期
  │
  ├─ 7. 如果有 timeStamp：
  │     ├─ 解析 TimeStampToken（CMS SignedData）
  │     ├─ 校验 CMS 内部 messageDigest
  │     ├─ 用 TSA 证书验 CMS 签名
  │     ├─ TSA 证书链到受信根 + EKU timeStamping + 有效期
  │     └─ TSTInfo.messageImprint == SM3(signature 值) ?
  │
  └─ 输出：签章是否有效 + 可信时间（若验证了时间戳）
```

直接验证签章值：

```rust
use ofdbox_seal::{SesVerifyOptions, parse_ses_signature, verify_ses_signature};

let ses = parse_ses_signature(&signed_value)?;
let result = verify_ses_signature(
    &signed_value,
    &signature_xml_bytes,   // 原文：Signature.xml 字节
    &SesVerifyOptions::default(),
)?;
```

接入 OFD 的统一验签入口：

```rust
use ofdbox_core::SignatureVerifiers;
use ofdbox_seal::SesSignatureVerifier;

let verifiers = SignatureVerifiers::new()
    .with(Box::new(SesSignatureVerifier::new()));
package.verify_signature(&signature, &verifiers)?;
```

制章者证书与签章者证书是**两条独立的信任链**：

- 制章证书链证明“印章由谁制作”；
- 签章证书链证明“谁签了这份文件”。

---

## 5. 时间戳（GB/T 20520）

### 5.1 字段定义

38540 7.1.6 规定：

```text
timeStamp [0] BIT STRING OPTIONAL   -- 对签名值的时间戳，DER 编码
```

- 内容是 **TimeStampToken 的 DER 字节**，不是字符串；
- `BIT STRING` 前有一个 unused-bits 字节（通常为 0）；
- `[0]` 是显式上下文标签。

### 5.2 令牌来源

时间戳令牌只能由 TSA 签发：

```text
1. 签章完成，得到 signature 值
2. 计算 messageImprint = SM3(signature)
3. 向 TSA 发送 TimeStampReq
4. TSA 返回 TimeStampToken
5. 把 token 字节写入 SES_Signature.timeStamp
```

库不内置网络客户端；token 应由调用方获取后传入。

### 5.3 语义

时间戳证明的是：

> `SM3(signature)` 在 `genTime` 时刻已经存在。

因此它给出的是**可信时间上界**：

- 签名不晚于 `genTime` 产生；
- `timeInfo` / `SignatureDateTime` 是签章方自报时间，不可信；
- 只有 TSA 签名和证书链都通过，`genTime` 才可信。

时间戳同时支持长期验证（LTV）：证明签名发生在证书有效期内。

### 5.4 当前实现

已实现：

- `SesSignature.time_stamp`：解析 `timeStamp [0]` 中的令牌 DER；
- `attach_time_stamp`：把 TSA 令牌写入已有 `SES_Signature`（已有令牌会被替换）；
- `parse_time_stamp_token`：解析 CMS `SignedData` 与 `TSTInfo`；
- `verify_time_stamp_token`：校验 TSA 的 CMS 签名、`messageImprint` 与 `signature` 值一致；
- `SesVerifyOptions.check_timestamp` / `with_timestamp_check`：验章时可选开启时间戳验证；
- `SesSignatureVerifier::with_timestamp_check` / `with_timestamp_certificate_policy`：在 OFD 统一验签入口启用时间戳验证；
- `SesVerifyOptions.timestamp_certificate`：提供 TSA 信任策略时校验证书链与 `timeStamping` EKU。

限制：

- 只支持 **SM2 + SM3** 的令牌；
- 不提供 TSA 网络客户端，令牌必须由调用方从 TSA 获取；
- 未配置 TSA 信任策略时，只验证令牌签名与摘要，不建立信任。

---

## 6. 当前能力与限制

| 能力 | 状态 |
|---|---|
| 识别 GM/T 0031 / GB/T 38540 / 未知格式 | 支持 |
| 解析印章显示信息 | 支持 |
| 制章（38540、0031） | 支持 |
| 签章（38540 `SES_Signature`） | 支持 |
| 验章（签章者、制章者、证书链、有效期） | 支持 |
| 0031 印章制作 | 支持 |
| GM/T 0031 签章值（`SES_sign`）解析/验签/生成 | 支持 |
| `timeStamp` 字段读写 | 支持 |
| 20520 时间戳验签 | 支持（SM2/SM3，可选 TSA 信任链） |
| 盖章外观 / OFD 集成 | 由 `ofdbox-core` 完成 |

---

## 7. 测试

```bash
cargo test -p ofdbox-seal
```

外部语料兼容性测试集中在门面 crate：

```powershell
$env:OFDBOX_TESTDATA_DIR = "D:\OFDBox-Workspace\OFD文件"
cargo test -p ofdbox --test corpus_compat -- --ignored --nocapture
```

默认测试完全自包含：印章、证书、签名值均在测试中现场生成。
