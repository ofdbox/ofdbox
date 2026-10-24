//! 公共印章容器：格式识别、解析与显示视图。

pub(crate) mod encoding;
pub(crate) mod parse;
pub(crate) mod spec;
pub(crate) mod verify;

pub use parse::{
    SealDisplay, SealHeader, SealPicture, SealProperty, fingerprint_seal, identify_seal_format,
    parse_seal_for_display,
};
pub use spec::{SealBuilder, SealPictureInput, SealSpec, SealStandard};
pub use verify::{
    SealVerification, SealVerifyOptions, signer_in_seal, verify_seal, verify_seal_with_options,
};
