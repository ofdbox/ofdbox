use std::str::FromStr;
use std::time::Duration;

use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::content_info::{CmsVersion, ContentInfo};
use cms::signed_data::{
    CertificateSet, EncapsulatedContentInfo, SignedData, SignerIdentifier, SignerInfo, SignerInfos,
};
use der::Tag;
use der::asn1::{Any, AnyRef, BitString, ObjectIdentifier, OctetString, SetOfVec};
use der::{Decode, Encode};
use ofdbox_core::SignatureVerifier;
use ofdbox_crypto::{Sm2SignedDataVerifier, verify_signed_data};
use sm2::SecretKey;
use sm2::dsa::DerSignature;
use sm2::dsa::signature::{Keypair, Signer};
use sm3::{Digest, Sm3};
use x509_cert::Certificate;
use x509_cert::attr::{Attribute, Attributes};
use x509_cert::builder::{Builder, CertificateBuilder, profile::BuilderProfile};
use x509_cert::name::Name;
use x509_cert::serial_number::SerialNumber;
use x509_cert::spki::{
    AlgorithmIdentifierOwned, DynSignatureAlgorithmIdentifier, SubjectPublicKeyInfoOwned,
};
use x509_cert::time::Validity;

const SM2_CURVE_OID: &str = "1.2.156.10197.1.301";
const SM2_SIGN_OID: &str = "1.2.156.10197.1.501";
const SM3_OID: &str = "1.2.156.10197.1.401";
const ID_SIGNED_DATA: &str = "1.2.840.113549.1.7.2";
const ID_DATA: &str = "1.2.840.113549.1.7.1";
const ID_MESSAGE_DIGEST: &str = "1.2.840.113549.1.9.4";
const DIST_ID: &str = "1234567812345678";

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

fn make_certificate() -> (Vec<u8>, SecretKey) {
    let secret = SecretKey::from_slice(&[0x66u8; 32]).expect("secret");
    let signing = sm2::dsa::SigningKey::new(DIST_ID, &secret).expect("signing");
    let public = secret.public_key().to_sec1_bytes();
    let spki = SubjectPublicKeyInfoOwned {
        algorithm: AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM2_CURVE_OID),
            parameters: None,
        },
        subject_public_key: BitString::from_bytes(&public).expect("bit string"),
    };
    let builder = CertificateBuilder::new(
        TestProfile {
            subject: Name::from_str("CN=OFDBox Test").expect("name"),
        },
        SerialNumber::from(1u32),
        Validity::from_now(Duration::from_secs(3600)).expect("validity"),
        spki,
    )
    .expect("builder");
    let certificate: Certificate = builder
        .build::<_, DerSignature>(&Sm2Signer { key: signing })
        .expect("certificate");
    (certificate.to_der().expect("der"), secret)
}

#[test]
fn verifies_detached_signed_data() {
    let (certificate_der, secret) = make_certificate();
    let certificate = Certificate::from_der(&certificate_der).expect("certificate");
    let content = b"<Signature>content</Signature>";
    let digest = Sm3::digest(content);
    let attribute = Attribute {
        oid: ObjectIdentifier::new_unwrap(ID_MESSAGE_DIGEST),
        values: SetOfVec::try_from(vec![
            Any::new(Tag::OctetString, digest.as_slice()).expect("digest any"),
        ])
        .expect("attribute values"),
    };
    let attributes: Attributes = SetOfVec::try_from(vec![attribute]).expect("attributes");
    let signed_bytes = attributes.to_der().expect("attributes der");
    let signing = sm2::dsa::SigningKey::new(DIST_ID, &secret).expect("signing");
    let signature: sm2::dsa::Signature = signing.try_sign(&signed_bytes).expect("signature");

    let signer = SignerInfo {
        version: CmsVersion::V1,
        sid: SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
            issuer: certificate.tbs_certificate().issuer().clone(),
            serial_number: certificate.tbs_certificate().serial_number().clone(),
        }),
        digest_alg: AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM3_OID),
            parameters: None,
        },
        signed_attrs: Some(attributes),
        signature_algorithm: AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM2_SIGN_OID),
            parameters: None,
        },
        signature: OctetString::new(signature.to_der().to_vec()).expect("signature octets"),
        unsigned_attrs: None,
    };
    let signed_data = SignedData {
        version: CmsVersion::V1,
        digest_algorithms: SetOfVec::try_from(vec![AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(SM3_OID),
            parameters: None,
        }])
        .expect("digest algorithms"),
        encap_content_info: EncapsulatedContentInfo {
            econtent_type: ObjectIdentifier::new_unwrap(ID_DATA),
            econtent: None,
        },
        certificates: Some(
            CertificateSet::try_from(vec![CertificateChoices::Certificate(certificate)])
                .expect("certificates"),
        ),
        crls: None,
        signer_infos: SignerInfos::try_from(vec![signer]).expect("signer infos"),
    };
    let signed_data_der = signed_data.to_der().expect("signed data");
    let content_info = ContentInfo {
        content_type: ObjectIdentifier::new_unwrap(ID_SIGNED_DATA),
        content: Any::from(AnyRef::try_from(signed_data_der.as_slice()).expect("any ref")),
    };
    let signed_value = content_info.to_der().expect("content info");

    verify_signed_data(content, &signed_value, DIST_ID).expect("verify");
    let verifier = Sm2SignedDataVerifier::new();
    verifier
        .verify(SM2_SIGN_OID, content, &signed_value)
        .expect("verifier");
    let mut tampered = content.to_vec();
    tampered[0] ^= 1;
    assert!(verify_signed_data(&tampered, &signed_value, DIST_ID).is_err());
}
