//! Property-based test suite using proptest
//!
//! Run with: cargo test --test property_tests

use blvm_commons::validation::content_hash::ContentHashValidator;
use proptest::prelude::*;

proptest! {
    /// Property: Hash function is deterministic
    #[test]
    fn test_hash_determinism(
        content in prop::collection::vec(any::<u8>(), 0..10000)
    ) {
        let validator = ContentHashValidator::new();
        let hash1 = validator.compute_file_hash(&content);
        let hash2 = validator.compute_file_hash(&content);

        prop_assert_eq!(hash1, hash2, "Hash must be deterministic");
    }

    /// Property: Hash format is always valid
    #[test]
    fn test_hash_format(
        content in prop::collection::vec(any::<u8>(), 0..100000)
    ) {
        let validator = ContentHashValidator::new();
        let hash = validator.compute_file_hash(&content);

        prop_assert!(hash.starts_with("sha256:"), "Hash must start with 'sha256:'");
        prop_assert_eq!(hash.len(), 71, "Hash must be 71 characters");
    }
}

#[test]
fn test_empty_hash() {
    let validator = ContentHashValidator::new();
    let hash = validator.compute_file_hash(&[]);

    let expected = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    assert_eq!(hash, expected, "Empty content must produce known hash");
}

use blvm_commons::validation::version_pinning::VersionPinningValidator;

proptest! {
    /// Property: Version parsing is deterministic
    #[test]
    fn test_version_parsing_determinism(
        version in prop::string::string_regex(r"v\d+\.\d+\.\d+(-[a-zA-Z0-9]+)?(\+[a-zA-Z0-9]+)?").unwrap()
    ) {
        let validator = VersionPinningValidator::default();
        let content = format!("// @blvm-spec-version: {}", version);

        let refs1 = validator.parse_version_references("test.rs", &content);
        let refs2 = validator.parse_version_references("test.rs", &content);

        match (refs1, refs2) {
            (Ok(r1), Ok(r2)) => prop_assert_eq!(r1, r2, "Version parsing must be deterministic"),
            (Err(e1), Err(e2)) => prop_assert_eq!(e1.to_string(), e2.to_string(), "Error messages must be deterministic"),
            _ => prop_assert!(false, "Version parsing results must match (both Ok or both Err)"),
        }
    }

    /// Property: Valid semantic versions are always parsed correctly
    #[test]
    fn test_semantic_version_parsing(
        major in 0u64..1000,
        minor in 0u64..1000,
        patch in 0u64..1000
    ) {
        let validator = VersionPinningValidator::default();
        let version = format!("v{}.{}.{}", major, minor, patch);
        let content = format!("// @blvm-spec-version: {}", version);

        let refs = validator.parse_version_references("test.rs", &content);

        prop_assert!(refs.is_ok(), "Valid semantic version must parse successfully");
    }
}

use blvm_commons::github::cross_layer_status::CrossLayerStatusChecker;

proptest! {
    /// Property: Test count extraction handles various formats
    #[test]
    fn test_count_extraction_formats(
        count in 0usize..100000
    ) {
        let formats = vec![
            format!("{} tests", count),
            format!("Tests: {}", count),
            format!("cargo test: {}", count),
            format!("{} passed", count),
            format!("passed: {}", count),
        ];

        for format in formats {
            let result = CrossLayerStatusChecker::extract_test_count_from_name(&format);
            prop_assert_eq!(result, Some(count), "Format '{}' should extract count {}", format, count);
        }
    }

    /// Property: Test count extraction is case-insensitive
    #[test]
    fn test_count_extraction_case_insensitive(
        count in 1usize..1000
    ) {
        let formats = vec![
            format!("{} TESTS", count),
            format!("{} Tests", count),
            format!("{} tests", count),
        ];

        for format in formats {
            let result = CrossLayerStatusChecker::extract_test_count_from_name(&format);
            prop_assert_eq!(result, Some(count), "Case should not matter for '{}'", format);
        }
    }
}

use blvm_commons::crypto::signatures::SignatureManager;
use rand::SeedableRng;
use rand::rngs::StdRng;
use secp256k1::{PublicKey, Secp256k1, SecretKey};

proptest! {
    /// Property: Signature creation and verification round-trip
    #[test]
    fn test_signature_round_trip(
        message in prop::string::string_regex(".*").unwrap(),
        seed in prop::array::uniform32(any::<u8>())
    ) {
        let manager = SignatureManager::new();
        let secp = Secp256k1::new();
        let mut rng = StdRng::from_seed(seed);
        let secret_key = SecretKey::new(&mut rng);
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);

        let signature = manager.create_signature(&message, &secret_key).unwrap();
        let verified = manager.verify_signature(&message, &signature, &public_key).unwrap();

        prop_assert!(verified, "Signature should verify for original message");
    }

    /// Property: Signature verification fails for different messages
    #[test]
    fn test_signature_different_message(
        message1 in prop::string::string_regex(".+").unwrap(),
        message2 in prop::string::string_regex(".+").unwrap(),
        seed in prop::array::uniform32(any::<u8>())
    ) {
        prop_assume!(message1 != message2);

        let manager = SignatureManager::new();
        let secp = Secp256k1::new();
        let mut rng = StdRng::from_seed(seed);
        let secret_key = SecretKey::new(&mut rng);
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);

        let signature = manager.create_signature(&message1, &secret_key).unwrap();
        let verified = manager.verify_signature(&message2, &signature, &public_key).unwrap();

        prop_assert!(!verified, "Signature should not verify for different message");
    }
}
