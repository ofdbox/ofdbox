//! 国密 OID 常量与名称。
//!
//! 只收录本 crate 及 OFD 系列标准会用到的算法 OID；完整清单见工作区
//! `oid.md`，未实现的算法不注册，仅保留常量。

/// SM2 椭圆曲线公钥密码算法。
pub const SM2_CURVE_OID: &str = "1.2.156.10197.1.301";
/// SM2-1 数字签名算法。
pub const SM2_SIGN_OID: &str = "1.2.156.10197.1.301.1";
/// SM2-2 密钥交换协议。
pub const SM2_KEY_EXCHANGE_OID: &str = "1.2.156.10197.1.301.2";
/// SM2-3 公钥加密算法。
pub const SM2_ENCRYPT_OID: &str = "1.2.156.10197.1.301.3";
/// SM9 IBC 密码算法。
pub const SM9_OID: &str = "1.2.156.10197.1.302";
/// SM9 IBC 签名算法。
pub const SM9_SIGN_OID: &str = "1.2.156.10197.1.302.1";
/// SM3 密码杂凑算法。
pub const SM3_OID: &str = "1.2.156.10197.1.401";
/// SM3-HMAC。
pub const SM3_HMAC_OID: &str = "1.2.156.10197.1.401.2";
/// 基于 SM2 和 SM3 的签名。
pub const SM2_SM3_SIGN_OID: &str = "1.2.156.10197.1.501";
/// 基于 SM2 无证书机制和 SM3 的签名。
pub const SM2_CERTLESS_SM3_SIGN_OID: &str = "1.2.156.10197.1.501.1";
/// 基于 SM2 隐式证书机制和 SM3 的签名。
pub const SM2_IMPLICIT_SM3_SIGN_OID: &str = "1.2.156.10197.1.501.2";
/// 基于 SM9 和 SM3 的签名。
pub const SM9_SM3_SIGN_OID: &str = "1.2.156.10197.1.502";
/// 基于 RSA 和 SM3 的签名。
pub const RSA_SM3_SIGN_OID: &str = "1.2.156.10197.1.504";
/// SM4 分组密码算法。
pub const SM4_OID: &str = "1.2.156.10197.1.104";
/// SM4-ECB。
pub const SM4_ECB_OID: &str = "1.2.156.10197.1.104.1";
/// SM4-CBC。
pub const SM4_CBC_OID: &str = "1.2.156.10197.1.104.2";
/// SM4-CFB。
pub const SM4_CFB_OID: &str = "1.2.156.10197.1.104.3";
/// SM4-OFB。
pub const SM4_OFB_OID: &str = "1.2.156.10197.1.104.4";
/// SM4-MAC。
pub const SM4_MAC_OID: &str = "1.2.156.10197.1.104.5";
/// SM4-CTR。
pub const SM4_CTR_OID: &str = "1.2.156.10197.1.104.6";
/// 祖冲之序列密码算法。
pub const ZUC_OID: &str = "1.2.156.10197.1.201";
/// SM3-KDF 的注册名（标准未单独分配 OID）。
pub const SM3_KDF_ID: &str = "SM3-KDF";

/// 算法 OID 的可读名称。
pub fn algorithm_name(oid: &str) -> Option<&'static str> {
    Some(match oid {
        SM2_CURVE_OID => "SM2",
        SM2_SIGN_OID => "SM2-1",
        SM2_KEY_EXCHANGE_OID => "SM2-2",
        SM2_ENCRYPT_OID => "SM2-3",
        SM9_OID => "SM9",
        SM9_SIGN_OID => "SM9-1",
        SM3_OID => "SM3",
        SM3_HMAC_OID => "SM3-HMAC",
        SM2_SM3_SIGN_OID => "SM2+SM3",
        SM2_CERTLESS_SM3_SIGN_OID => "SM2-Certless+SM3",
        SM2_IMPLICIT_SM3_SIGN_OID => "SM2-Implicit+SM3",
        SM9_SM3_SIGN_OID => "SM9+SM3",
        RSA_SM3_SIGN_OID => "RSA+SM3",
        SM4_OID => "SM4",
        SM4_ECB_OID => "SM4-ECB",
        SM4_CBC_OID => "SM4-CBC",
        SM4_CFB_OID => "SM4-CFB",
        SM4_OFB_OID => "SM4-OFB",
        SM4_MAC_OID => "SM4-MAC",
        SM4_CTR_OID => "SM4-CTR",
        ZUC_OID => "ZUC",
        SM3_KDF_ID => "SM3-KDF",
        _ => return None,
    })
}
