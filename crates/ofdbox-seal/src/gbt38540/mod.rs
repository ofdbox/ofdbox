//! GB/T 38540—2020 的签章数据结构与生成。

pub(crate) mod build;
pub(crate) mod producer;
pub(crate) mod signature;

pub use producer::Sm2SesSignatureProducer;
pub use signature::{
    SesSignature, SesSignatureVerifier, SesVerification, SesVerifyOptions, parse_ses_signature,
    verify_ses_signature,
};
