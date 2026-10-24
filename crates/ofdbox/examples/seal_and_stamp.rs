//! 制章 → 盖章 → 验章 的完整示例。
//!
//! 运行：`cargo run -p ofdbox --example seal_and_stamp`
//!
//! 会在 `target/` 下生成：电子印章、签章者/制章者证书、签章者公钥（PEM）与
//! 盖好章的 OFD，便于用阅读器或其它工具测试。

use std::io::{Cursor, Write};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use der::Encode;
use der::asn1::{BitString, ObjectIdentifier};
use ofdbox::{
    DEFAULT_SIGNATURE_METHOD, OfdPackage, PasswordBackend, SealBuilder, SealFormat,
    SealPictureInput, SealSpec, SealStandard, SesSignatureVerifier, SignatureVerifiers,
    SigningRequest, Sm2SesSignatureProducer, StampAppearance, parse_seal_for_display,
    signer_in_seal, verify_seal,
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
    <ofd:DocInfo><ofd:DocID>seal-example</ofd:DocID></ofd:DocInfo>
    <ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot>
  </ofd:DocBody>
</ofd:OFD>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Document xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:CommonData>
    <ofd:MaxUnitID>2</ofd:MaxUnitID>
    <ofd:PageArea><ofd:PhysicalBox>0 0 210 297</ofd:PhysicalBox></ofd:PageArea>
    <ofd:DocumentRes>DocumentRes.xml</ofd:DocumentRes>
  </ofd:CommonData>
  <ofd:Pages><ofd:Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></ofd:Pages>
  <ofd:Annotations>Annotations.xml</ofd:Annotations>
</ofd:Document>"#;

const DOCUMENT_RES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="Res"><ofd:MultiMedias></ofd:MultiMedias></ofd:Res>"#;

const ANNOTATIONS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Annotations xmlns:ofd="http://www.ofdspec.org/2016"></ofd:Annotations>"#;

const CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016">
  <ofd:Content><ofd:Layer ID="1"/></ofd:Content>
</ofd:Page>"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target");
    std::fs::create_dir_all(&target)?;
    let target = std::fs::canonicalize(&target)?;

    // 1. 制章者与签章者各一对密钥（实际应由 CA 签发）。
    let (maker_cert, maker_secret) = make_keypair("CN=OFDBox Maker", [0x21; 32], 11)?;
    let (signer_cert, signer_secret) = make_keypair("CN=OFDBox Signer", [0x42; 32], 12)?;
    println!("1. 制章者与签章者证书就绪");

    // 2. 制章：38540 印章 + 代码生成的圆形印模。
    let seal_picture = make_seal_png();
    let spec = SealSpec::new(
        SealStandard::GbT38540,
        "SEAL-OFD-001",
        "示例电子印章",
        SealPictureInput {
            media_type: "PNG".to_owned(),
            data: seal_picture.clone(),
            width: 60,
            height: 40,
        },
    )
    .with_validity("20260101000000", "20260101000000", "20360101000000")
    .with_certificates(vec![signer_cert.clone()]);
    let seal =
        SealBuilder::new(spec, maker_cert.clone(), maker_secret.to_bytes().to_vec()).build()?;
    let seal_path = target.join("seal-38540.esl");
    std::fs::write(&seal_path, &seal)?;
    let display = parse_seal_for_display(&seal)?;
    println!(
        "2. 已生成印章 {}（{}，版本 {}，图片 {}x{}mm）",
        path_text(&seal_path),
        display.format().name(),
        display.header.version,
        display.picture.width,
        display.picture.height
    );
    verify_seal(&seal, SealFormat::GbT38540)?;
    assert!(signer_in_seal(&seal, &signer_cert)?);
    println!("   制章者签名与签章者列表校验通过");

    // 3. 写出证书与签章者公钥，便于外部验签。
    std::fs::write(target.join("seal-maker-cert.der"), &maker_cert)?;
    std::fs::write(target.join("seal-signer-cert.der"), &signer_cert)?;
    std::fs::write(
        target.join("seal-signer-public-key.pem"),
        public_key_pem(&signer_secret)?,
    )?;
    println!("3. 已写出制章者/签章者证书与签章者公钥（PEM）");

    // 4. 建一个带资源与注释入口的空文档。
    let template = build_package()?;
    let mut package = OfdPackage::from_bytes(template)?;
    let producer = Sm2SesSignatureProducer::new(
        seal.clone(),
        signer_cert.clone(),
        signer_secret.to_bytes().to_vec(),
    )?;
    let stamp = StampAppearance {
        page_id: "1".to_owned(),
        boundary: [75.0, 110.0, 60.0, 40.0],
        clip: None,
        image: seal_picture.clone(),
        image_format: "PNG".to_owned(),
        image_width: 60.0,
        image_height: 40.0,
    };
    let request = SigningRequest::new(DEFAULT_SIGNATURE_METHOD, "20261024050000").with_stamp(stamp);
    let mut output = Vec::new();
    let report = package.sign_package(&request, &producer, &mut Cursor::new(&mut output))?;
    let stamped_path = target.join("stamped-example.ofd");
    std::fs::write(&stamped_path, &output)?;
    println!(
        "4. 已盖章 {}（{}，保护 {} 个文件，含注释与印模）",
        path_text(&stamped_path),
        report.signature_id,
        report.protected.len()
    );

    // 5. 验章：签名值 + 引用摘要。
    let mut stamped = OfdPackage::open(&stamped_path)?;
    let index = stamped.parse_signatures()?;
    let entry = index.signatures().next().expect("签名记录");
    let signature = stamped.parse_signature(entry.base_loc().expect("BaseLoc"))?;
    let verifiers = SignatureVerifiers::new().with(Box::new(SesSignatureVerifier::new()));
    stamped.verify_signature(&signature, &verifiers)?;
    let checks = stamped.verify_references(&signature, &PasswordBackend::new(""))?;
    println!(
        "5. 验章通过：SES_Signature 有效，引用摘要 {}/{} 一致",
        checks
            .iter()
            .filter(|check| check.expected == check.actual)
            .count(),
        checks.len()
    );

    let signature_xml = stamped.read_entry("Doc_0/Signs/Sign_0/Signature.xml")?;
    let signature_xml = std::str::from_utf8(&signature_xml)?;
    println!(
        "   StampAnnot: {}",
        signature_xml
            .find("StampAnnot")
            .map(|_| "已写入")
            .unwrap_or("缺失")
    );
    Ok(())
}

fn build_package() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in [
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", DOCUMENT.as_bytes()),
        ("Doc_0/DocumentRes.xml", DOCUMENT_RES.as_bytes()),
        ("Doc_0/Annotations.xml", ANNOTATIONS.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", CONTENT.as_bytes()),
    ] {
        writer.start_file(name, SimpleFileOptions::default())?;
        writer.write_all(data)?;
    }
    Ok(writer.finish()?.into_inner())
}

fn public_key_pem(secret: &SecretKey) -> Result<String, Box<dyn std::error::Error>> {
    let public = secret.public_key().to_sec1_bytes();
    let spki = SubjectPublicKeyInfoOwned {
        algorithm: AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM2_CURVE_OID),
            parameters: None,
        },
        subject_public_key: BitString::from_bytes(&public)?,
    };
    let der = spki.to_der()?;
    let encoded = base64(&der);
    let mut pem = String::from("-----BEGIN PUBLIC KEY-----\n");
    for chunk in encoded.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(chunk)?);
        pem.push('\n');
    }
    pem.push_str("-----END PUBLIC KEY-----\n");
    Ok(pem)
}

fn base64(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(TABLE[(first >> 2) as usize] as char);
        output.push(TABLE[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[(((second & 0x0F) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(third & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    output
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
    fn try_sign(&self, message: &[u8]) -> sm2::dsa::signature::Result<DerSignature> {
        let signature: sm2::dsa::Signature = self.key.try_sign(message)?;
        Ok(signature.to_der())
    }
}

fn make_keypair(
    subject: &str,
    seed: [u8; 32],
    serial: u32,
) -> Result<(Vec<u8>, SecretKey), Box<dyn std::error::Error>> {
    let secret = SecretKey::from_slice(&seed)?;
    let signing = sm2::dsa::SigningKey::new(DIST_ID, &secret)?;
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
            subject: Name::from_str(subject)?,
        },
        SerialNumber::from(serial),
        Validity::from_now(Duration::from_secs(3600))?,
        spki,
    )?;
    let certificate: Certificate = builder.build::<_, DerSignature>(&Sm2Signer { key: signing })?;
    Ok((certificate.to_der()?, secret))
}

fn path_text(path: &Path) -> String {
    let text = path.display().to_string();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned()
}

const SEAL_RED: [u8; 4] = [200, 30, 30, 255];

/// 用代码画一枚方形公章风格的印模（透明底、红色边框与文字），返回 PNG 字节。
fn make_seal_png() -> Vec<u8> {
    const WIDTH: i32 = 280;
    const HEIGHT: i32 = 180;
    let mut pixels = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    draw_border(&mut pixels, WIDTH, HEIGHT, 6);
    draw_centered(&mut pixels, WIDTH, HEIGHT, "OFDBox", 40, 6);
    draw_centered(&mut pixels, WIDTH, HEIGHT, "Test", 96, 6);
    encode_png(WIDTH as u32, HEIGHT as u32, &pixels)
}

fn paint(pixels: &mut [u8], width: i32, height: i32, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= width || y >= height {
        return;
    }
    let index = ((y * width + x) * 4) as usize;
    pixels[index..index + 4].copy_from_slice(&color);
}

fn draw_border(pixels: &mut [u8], width: i32, height: i32, thickness: i32) {
    const MARGIN: i32 = 16;
    let left = MARGIN;
    let top = MARGIN;
    let right = width - MARGIN;
    let bottom = height - MARGIN;
    for y in top..bottom {
        for x in left..right {
            let edge = x < left + thickness
                || x >= right - thickness
                || y < top + thickness
                || y >= bottom - thickness;
            if edge {
                paint(pixels, width, height, x, y, SEAL_RED);
            }
        }
    }
}

fn draw_centered(pixels: &mut [u8], width: i32, height: i32, text: &str, y: i32, scale: i32) {
    let text_width = text.chars().count() as i32 * 6 * scale - scale;
    draw_text(
        pixels,
        width,
        height,
        text,
        (width - text_width) / 2,
        y,
        scale,
    );
}

fn draw_text(pixels: &mut [u8], width: i32, height: i32, text: &str, x: i32, y: i32, scale: i32) {
    let mut cursor = x;
    for character in text.chars() {
        let rows = glyph(character);
        for (row, bits) in rows.iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        paint(
                            pixels,
                            width,
                            height,
                            cursor + column * scale + dx,
                            y + row as i32 * scale + dy,
                            SEAL_RED,
                        );
                    }
                }
            }
        }
        cursor += 6 * scale;
    }
}

/// 5x7 点阵字形，仅覆盖示例用到的字符。
fn glyph(character: char) -> [u8; 7] {
    match character {
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'o' => [
            0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'x' => [
            0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'e' => [
            0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110,
        ],
        's' => [
            0b00000, 0b00000, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110,
        ],
        't' => [
            0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00100, 0b00011,
        ],
        _ => [0; 7],
    }
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(((width * 4 + 1) * height) as usize);
    for y in 0..height {
        raw.push(0);
        let start = (y * width * 4) as usize;
        raw.extend_from_slice(&rgba[start..start + (width * 4) as usize]);
    }

    let mut output = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    output.extend(png_chunk(b"IHDR", &header));
    output.extend(png_chunk(b"IDAT", &zlib_stored(&raw)));
    output.extend(png_chunk(b"IEND", &[]));
    output
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len() + 12);
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    output.extend_from_slice(kind);
    output.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    output.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    output
}

fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut output = vec![0x78, 0x01];
    let mut chunks = data.chunks(65_535).peekable();
    while let Some(chunk) = chunks.next() {
        output.push(u8::from(chunks.peek().is_none()));
        let length = chunk.len() as u16;
        output.extend_from_slice(&length.to_le_bytes());
        output.extend_from_slice(&(!length).to_le_bytes());
        output.extend_from_slice(chunk);
    }
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

fn adler32(data: &[u8]) -> u32 {
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
