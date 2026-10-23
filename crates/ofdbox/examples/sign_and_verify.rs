//! 创建一个最小 OFD、生成 SM2 密钥、签名并验签。
//!
//! 运行：`cargo run -p ofdbox --example sign_and_verify`

use std::io::{Cursor, Write};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use der::Encode;
use der::asn1::{BitString, ObjectIdentifier};
use ofdbox::{
    DEFAULT_SIGNATURE_METHOD, OfdPackage, PasswordBackend, SignatureVerifiers, SigningRequest,
    Sm2SignedDataProducer, Sm2SignedDataVerifier,
};
use sm2::SecretKey;
use sm2::dsa::DerSignature;
use sm2::dsa::signature::{Keypair, Signer};
use x509_cert::Certificate;
use x509_cert::builder::{Builder, CertificateBuilder, profile::BuilderProfile};
use x509_cert::name::Name;
use x509_cert::serial_number::SerialNumber;
use x509_cert::spki::{
    AlgorithmIdentifierOwned, DynSignatureAlgorithmIdentifier, SubjectPublicKeyInfoOwned,
};
use x509_cert::time::Validity;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

const SM2_CURVE_OID: &str = "1.2.156.10197.1.301";
const SM2_SIGN_OID: &str = "1.2.156.10197.1.501";
const DIST_ID: &str = "1234567812345678";

const OFD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:OFD xmlns:ofd="http://www.ofdspec.org/2016" Version="1.0" DocType="OFD">
  <ofd:DocBody>
    <ofd:DocInfo><ofd:DocID>example-sign</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>2</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
</ofd:Document>"#;

const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Content><ofd:Layer ID="1"/></ofd:Content>
</ofd:Page>"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 生成密钥与自签证书（实际使用时应由 CA 签发）。
    let secret = SecretKey::from_slice(&[0x42u8; 32])?;
    let certificate = make_certificate(&secret)?;
    println!("1. SM2 密钥与自签证书就绪：证书 {} 字节", certificate.len());

    // 2. 新建一个最小 OFD 包。
    let package = build_package()?;
    println!("2. 新建 OFD：{} 字节，3 个条目", package.len());

    // 3. 签名。
    let mut document = OfdPackage::from_bytes(package)?;
    let request = SigningRequest::new(DEFAULT_SIGNATURE_METHOD, "2026-10-24T05:00:00Z");
    let producer = Sm2SignedDataProducer::new(certificate, secret.to_bytes().to_vec());
    let mut signed = Vec::new();
    let report = document.sign_package(&request, &producer, &mut Cursor::new(&mut signed))?;
    println!(
        "3. 签名完成：{} → {}，保护 {} 个文件",
        report.signature_id,
        report.signature_entry,
        report.protected.len()
    );
    for path in &report.protected {
        println!("   - {path}");
    }

    // 4. 验签。
    let mut document = OfdPackage::from_bytes(signed.clone())?;
    let index = document.parse_signatures()?;
    let entry = index.signatures().next().expect("签名记录");
    let base_loc = entry.base_loc().expect("BaseLoc").to_owned();
    let signature = document.parse_signature(&base_loc)?;
    let verifiers = SignatureVerifiers::new().with(Box::new(Sm2SignedDataVerifier::new()));
    document.verify_signature(&signature, &verifiers)?;
    println!("4. 签名值验证通过（GB/T 35275 SignedData + SM2）");

    // 5. 逐个校验保护文件的摘要。
    let checks = document.verify_references(&signature, &PasswordBackend::new(""))?;
    let matched = checks
        .iter()
        .filter(|check| check.expected == check.actual)
        .count();
    println!("5. 引用摘要 {matched}/{} 一致", checks.len());

    // 6. 把签名后的文件写到 target 目录。
    let target_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target");
    std::fs::create_dir_all(&target_dir)?;
    let output = target_dir.join("signed-example.ofd");
    std::fs::write(&output, &signed)?;
    let output = std::fs::canonicalize(&output)?;
    let display = output.to_string_lossy();
    let display = display.strip_prefix(r"\\?\").unwrap_or(&display);
    println!("6. 已写入 {display}");

    // 7. 从磁盘重新打开并验签，确认落盘文件可用。
    let mut saved = OfdPackage::open(&output)?;
    let index = saved.parse_signatures()?;
    let entry = index.signatures().next().expect("签名记录");
    let signature = saved.parse_signature(entry.base_loc().expect("BaseLoc"))?;
    saved.verify_signature(&signature, &verifiers)?;
    let checks = saved.verify_references(&signature, &PasswordBackend::new(""))?;
    println!(
        "7. 重新打开验签通过，引用摘要 {}/{} 一致",
        checks.iter().filter(|c| c.expected == c.actual).count(),
        checks.len()
    );
    Ok(())
}

fn build_package() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in [
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", CONTENT.as_bytes()),
    ] {
        writer.start_file(name, SimpleFileOptions::default())?;
        writer.write_all(data)?;
    }
    Ok(writer.finish()?.into_inner())
}

struct TestProfile {
    subject: Name,
}

impl BuilderProfile for TestProfile {
    fn get_issuer(&self, _subject: &Name) -> Name {
        self.subject.clone()
    }

    fn get_subject(&self) -> Name {
        self.subject.clone()
    }

    fn build_extensions(
        &self,
        _spk: x509_cert::spki::SubjectPublicKeyInfoRef<'_>,
        _issuer_spk: x509_cert::spki::SubjectPublicKeyInfoRef<'_>,
        _tbs: &x509_cert::certificate::TbsCertificate,
    ) -> x509_cert::builder::Result<Vec<x509_cert::ext::Extension>> {
        Ok(Vec::new())
    }
}

struct Sm2Signer {
    key: sm2::dsa::SigningKey,
}

impl Keypair for Sm2Signer {
    type VerifyingKey = sm2::dsa::VerifyingKey;

    fn verifying_key(&self) -> Self::VerifyingKey {
        self.key.verifying_key().clone()
    }
}

impl DynSignatureAlgorithmIdentifier for Sm2Signer {
    fn signature_algorithm_identifier(&self) -> x509_cert::spki::Result<AlgorithmIdentifierOwned> {
        Ok(AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM2_SIGN_OID),
            parameters: None,
        })
    }
}

impl Signer<DerSignature> for Sm2Signer {
    fn try_sign(&self, msg: &[u8]) -> sm2::dsa::signature::Result<DerSignature> {
        let signature: sm2::dsa::Signature = self.key.try_sign(msg)?;
        Ok(signature.to_der())
    }
}

fn make_certificate(secret: &SecretKey) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let signing = sm2::dsa::SigningKey::new(DIST_ID, secret)?;
    let public = secret.public_key().to_sec1_bytes();
    let spki = SubjectPublicKeyInfoOwned {
        algorithm: AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM2_CURVE_OID),
            parameters: None,
        },
        subject_public_key: BitString::from_bytes(&public)?,
    };
    let builder = CertificateBuilder::new(
        TestProfile {
            subject: Name::from_str("CN=OFDBox Example")?,
        },
        SerialNumber::from(1u32),
        Validity::from_now(Duration::from_secs(3600))?,
        spki,
    )?;
    let certificate: Certificate = builder.build::<_, DerSignature>(&Sm2Signer { key: signing })?;
    Ok(certificate.to_der()?)
}
