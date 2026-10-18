use std::str::FromStr;
use std::time::Duration;

use der::Encode;
use der::asn1::BitString;
use getrandom::SysRng;
use ofdbox_crypto::{
    decrypt_file_key_with_private_key, encrypt_file_key_with_certificate,
    public_key_from_certificate,
};
use sm2::SecretKey;
use sm2::dsa::DerSignature;
use sm2::dsa::signature::{Keypair, Signer};
use x509_cert::Certificate;
use x509_cert::builder::{Builder, CertificateBuilder, profile::BuilderProfile};
use x509_cert::der::asn1::ObjectIdentifier;
use x509_cert::name::Name;
use x509_cert::serial_number::SerialNumber;
use x509_cert::spki::{
    AlgorithmIdentifierOwned, DynSignatureAlgorithmIdentifier, SubjectPublicKeyInfoOwned,
};
use x509_cert::time::Validity;

const SM2_CURVE_OID: &str = "1.2.156.10197.1.301";
const SM2_SIGN_OID: &str = "1.2.156.10197.1.501";

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

#[test]
fn certificate_key_wrapping_roundtrip() {
    let secret = SecretKey::from_slice(&[0x44u8; 32]).expect("secret");
    let signing = sm2::dsa::SigningKey::new("1234567812345678", &secret).expect("signing");
    let public = secret.public_key().to_sec1_bytes();
    let spki = SubjectPublicKeyInfoOwned {
        algorithm: AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM2_CURVE_OID),
            parameters: None,
        },
        subject_public_key: BitString::from_bytes(&public).expect("bit string"),
    };
    let profile = TestProfile {
        subject: Name::from_str("CN=OFDBox Test").expect("name"),
    };
    let builder = CertificateBuilder::new(
        profile,
        SerialNumber::from(1u32),
        Validity::from_now(Duration::from_secs(3600)).expect("validity"),
        spki,
    )
    .expect("builder");
    let certificate: Certificate = builder
        .build::<_, DerSignature>(&Sm2Signer { key: signing })
        .expect("certificate");
    let certificate_der = certificate.to_der().expect("der");

    assert_eq!(
        public_key_from_certificate(&certificate_der).expect("public key"),
        &*public
    );

    let file_key = [0x55u8; 16];
    let wrapped =
        encrypt_file_key_with_certificate(&certificate_der, &file_key, &mut SysRng).expect("wrap");
    let unwrapped =
        decrypt_file_key_with_private_key(&secret.to_bytes(), &wrapped).expect("unwrap");
    assert_eq!(&*unwrapped, &file_key);
}
