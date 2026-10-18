/// GB/T 33190-2016 规定的 OFD 命名空间。
pub const OFD_NAMESPACE: &str = "http://www.ofdspec.org/2016";

/// 一个命名空间 URI 的分类结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NamespaceClass {
    /// 标准 URI。
    Canonical,
    /// 已知变体，例如缺少 `/2016`、`https`、结尾多斜杠。
    KnownVariant,
    /// 没有命名空间。
    Missing,
    /// 不是 OFD 命名空间。
    Unknown,
}

/// 对命名空间 URI 做宽松分类。
pub(crate) fn classify(uri: Option<&str>) -> NamespaceClass {
    let Some(uri) = uri else {
        return NamespaceClass::Missing;
    };
    let uri = uri.trim();
    if uri.is_empty() {
        return NamespaceClass::Missing;
    }
    if uri == OFD_NAMESPACE {
        return NamespaceClass::Canonical;
    }
    if is_known_variant(uri) {
        NamespaceClass::KnownVariant
    } else {
        NamespaceClass::Unknown
    }
}

fn is_known_variant(uri: &str) -> bool {
    let lower = uri.to_ascii_lowercase();
    let Some(rest) = lower
        .strip_prefix("http://")
        .or_else(|| lower.strip_prefix("https://"))
    else {
        return false;
    };
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    matches!(rest, "ofdspec.org" | "ofdspec.org/2016")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_variants() {
        assert_eq!(classify(Some(OFD_NAMESPACE)), NamespaceClass::Canonical);
        assert_eq!(
            classify(Some("http://www.ofdspec.org/")),
            NamespaceClass::KnownVariant
        );
        assert_eq!(
            classify(Some("http://www.ofdspec.org")),
            NamespaceClass::KnownVariant
        );
        assert_eq!(
            classify(Some("https://ofdspec.org/2016/")),
            NamespaceClass::KnownVariant
        );
        assert_eq!(classify(None), NamespaceClass::Missing);
        assert_eq!(classify(Some("urn:other")), NamespaceClass::Unknown);
    }
}
