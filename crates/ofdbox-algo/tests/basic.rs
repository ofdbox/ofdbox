use ofdbox_algo::{
    AlgorithmError, AlgorithmRegistry, AlgorithmRole, Cipher, DigestAlgorithm, HmacSm3, Kdf, Mac,
    PublicKeyRef, RawSignatureAlgorithm, SM2_CURVE_OID, SM2_SIGN_OID, SM2_SM3_SIGN_OID,
    SM3_HMAC_OID, SM3_KDF_ID, SM3_OID, SM4_CBC_OID, SM4_CTR_OID, SM4_ECB_OID, SecretKeyRef,
    SignatureParams, Sm2RawSignature, Sm3Digest, Sm3Kdf, Sm4Cbc, Sm4Ctr, Sm4Ecb,
};

#[test]
fn sm3_matches_known_vector() {
    let digest = Sm3Digest.digest(b"abc");
    assert_eq!(
        hex(&digest),
        "66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0"
    );
    assert_eq!(Sm3Digest.output_len(), 32);
}

#[test]
fn sm3_kdf_matches_sample() {
    let key = Sm3Kdf.derive(b"12345678", 16).expect("kdf");
    assert_eq!(hex(&key), "34d6effbd5bdc7c8020f619bfd8303b5");
}

#[test]
fn sm4_cbc_roundtrip() {
    let key = [0x11u8; 16];
    let iv = [0x22u8; 16];
    let plaintext = b"OFDBox algorithm crate";
    let ciphertext = Sm4Cbc.encrypt(&key, Some(&iv), plaintext).expect("encrypt");
    assert_ne!(ciphertext, plaintext);
    let decrypted = Sm4Cbc
        .decrypt(&key, Some(&iv), &ciphertext)
        .expect("decrypt");
    assert_eq!(decrypted, plaintext);
    assert!(Sm4Cbc.decrypt(&key, Some(&iv), &ciphertext[..8]).is_err());
}

#[test]
fn sm2_signature_verifies() {
    let secret = sm2::SecretKey::from_slice(&[0x22u8; 32]).expect("secret");
    let public = secret.public_key().to_sec1_bytes();
    let signing = sm2::dsa::SigningKey::new("1234567812345678", &secret).expect("signing key");
    let message = b"OFDBox SM2";
    let signature: sm2::dsa::Signature = {
        use sm2::dsa::signature::Signer;
        signing.try_sign(message).expect("sign")
    };
    let key = PublicKeyRef::new(SM2_CURVE_OID, &public);
    let params = SignatureParams::default();
    Sm2RawSignature
        .verify(&key, message, &signature.to_der().to_vec(), &params)
        .expect("verify");
    assert!(
        Sm2RawSignature
            .verify(&key, b"tampered", &signature.to_der().to_vec(), &params)
            .is_err()
    );
}

#[test]
fn registry_lookup_and_roles() {
    let registry = AlgorithmRegistry::gm_default();
    assert!(registry.digest(SM3_OID).is_ok());
    assert!(registry.signature(SM2_SIGN_OID).is_ok());
    assert!(registry.signature(SM2_SM3_SIGN_OID).is_ok());
    assert!(registry.suite(SM2_SM3_SIGN_OID).is_ok());
    assert!(registry.cipher(SM4_CBC_OID).is_ok());
    assert!(registry.kdf(SM3_KDF_ID).is_ok());

    assert!(matches!(
        registry.digest(SM2_SIGN_OID),
        Err(AlgorithmError::RoleMismatch { .. })
    ));
    assert!(matches!(
        registry.signature("1.2.3.4"),
        Err(AlgorithmError::UnknownAlgorithm { .. })
    ));
}

#[test]
fn suite_verifies_sm2_sm3_signature() {
    let registry = AlgorithmRegistry::gm_default();
    let secret = sm2::SecretKey::from_slice(&[0x33u8; 32]).expect("secret");
    let public = secret.public_key().to_sec1_bytes();
    let signing = sm2::dsa::SigningKey::new("1234567812345678", &secret).expect("signing key");
    let message = b"OFDBox suite";
    let signature: sm2::dsa::Signature = {
        use sm2::dsa::signature::Signer;
        signing.try_sign(message).expect("sign")
    };
    let key = PublicKeyRef::new(SM2_CURVE_OID, &public);
    registry
        .verify_suite(
            SM2_SM3_SIGN_OID,
            &key,
            message,
            &signature.to_der().to_vec(),
            &SignatureParams::default(),
        )
        .expect("suite verify");
}

#[test]
fn role_names_are_stable() {
    assert_eq!(AlgorithmRole::Digest.name(), "digest");
    assert_eq!(AlgorithmRole::SignatureSuite.name(), "signature-suite");
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn algo_signs_and_verifies() {
    let secret = sm2::SecretKey::from_slice(&[0x44u8; 32]).expect("secret");
    let public = secret.public_key().to_sec1_bytes();
    let params = SignatureParams::default();
    let secret_bytes = secret.to_bytes();
    let secret_ref = SecretKeyRef::new(SM2_CURVE_OID, &secret_bytes);
    let signature = Sm2RawSignature
        .sign(&secret_ref, b"OFDBox sign", &params)
        .expect("sign");
    assert!(!signature.is_empty());

    let key = PublicKeyRef::new(SM2_CURVE_OID, &public);
    Sm2RawSignature
        .verify(&key, b"OFDBox sign", &signature, &params)
        .expect("verify");
    assert!(
        Sm2RawSignature
            .verify(&key, b"tampered", &signature, &params)
            .is_err()
    );
}

#[test]
fn algo_rejects_invalid_signing_key() {
    let params = SignatureParams::default();
    let secret_ref = SecretKeyRef::new(SM2_CURVE_OID, &[0u8; 8]);
    assert!(matches!(
        Sm2RawSignature.sign(&secret_ref, b"data", &params),
        Err(AlgorithmError::InvalidKey)
    ));
}

#[test]
fn registry_signs_and_verifies_suite() {
    let registry = AlgorithmRegistry::gm_default();
    let secret = sm2::SecretKey::from_slice(&[0x55u8; 32]).expect("secret");
    let public = secret.public_key().to_sec1_bytes();
    let secret_bytes = secret.to_bytes();
    let params = SignatureParams::default();
    let secret_ref = SecretKeyRef::new(SM2_CURVE_OID, &secret_bytes);

    let signature = registry
        .sign_suite(SM2_SM3_SIGN_OID, &secret_ref, b"suite sign", &params)
        .expect("sign suite");
    let key = PublicKeyRef::new(SM2_CURVE_OID, &public);
    registry
        .verify_suite(SM2_SM3_SIGN_OID, &key, b"suite sign", &signature, &params)
        .expect("verify suite");
}

#[test]
fn sm2_encrypt_roundtrip() {
    let secret = sm2::SecretKey::from_slice(&[0x66u8; 32]).expect("secret");
    let public = secret.public_key().to_sec1_bytes();
    let mut rng = getrandom::SysRng;
    let ciphertext =
        ofdbox_algo::sm2_encrypt(&public, b"OFDBox plaintext", &mut rng).expect("encrypt");
    let plaintext = ofdbox_algo::sm2_decrypt(&secret.to_bytes(), &ciphertext).expect("decrypt");
    assert_eq!(plaintext, b"OFDBox plaintext");
}

#[cfg(feature = "x509")]
#[test]
fn converts_spki_to_public_key() {
    use sm2::pkcs8::EncodePublicKey;

    let secret = sm2::SecretKey::from_slice(&[0x77u8; 32]).expect("secret");
    let signing = sm2::dsa::SigningKey::new("1234567812345678", &secret).expect("signing key");
    let spki = signing.verifying_key().to_public_key_der().expect("spki");
    let key = ofdbox_algo::public_key_from_spki(spki.as_bytes()).expect("public key");
    assert_eq!(key.algorithm_oid, SM2_CURVE_OID);
    assert_eq!(key.key_bytes, secret.public_key().to_sec1_bytes().to_vec());
}

#[test]
fn sm4_ecb_and_ctr_roundtrip() {
    let key = [0x11u8; 16];
    let iv = [0x22u8; 16];
    let plaintext = b"OFDBox SM4 modes";

    let ecb = Sm4Ecb.encrypt(&key, None, plaintext).expect("ecb encrypt");
    assert_eq!(
        Sm4Ecb.decrypt(&key, None, &ecb).expect("ecb decrypt"),
        plaintext
    );

    let ctr = Sm4Ctr
        .encrypt(&key, Some(&iv), plaintext)
        .expect("ctr encrypt");
    assert_eq!(ctr.len(), plaintext.len());
    assert_eq!(
        Sm4Ctr.decrypt(&key, Some(&iv), &ctr).expect("ctr decrypt"),
        plaintext
    );
}

#[test]
fn hmac_sm3_marks_and_verifies() {
    let tag = HmacSm3.mac(b"key", b"data").expect("mac");
    assert_eq!(tag.len(), 32);
    HmacSm3.verify(b"key", b"data", &tag).expect("verify");
    assert!(HmacSm3.verify(b"key", b"modified", &tag).is_err());
    assert_ne!(HmacSm3.mac(b"other", b"data").expect("mac"), tag);
}

#[test]
fn registry_exposes_new_modes() {
    let registry = AlgorithmRegistry::gm_default();
    assert!(registry.cipher(SM4_ECB_OID).is_ok());
    assert!(registry.cipher(SM4_CTR_OID).is_ok());
    assert!(registry.mac(SM3_HMAC_OID).is_ok());
}
