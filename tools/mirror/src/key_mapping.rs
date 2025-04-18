// cspell:words hkdf
use hkdf::Hkdf;
use near_crypto::{ED25519PublicKey, ED25519SecretKey, PublicKey, Secp256K1PublicKey, SecretKey};
use near_primitives::types::AccountId;
use near_primitives::utils::derive_near_implicit_account_id;
use near_primitives_core::account::id::AccountType;
use sha2::Sha256;

// there is nothing special about this key, it's just some randomly generated one.
// We will ensure that every account in the target chain has at least one full access
// key by adding this one (when preparing the records file, or when sending a create account tx)
// if one doesn't exist
const DEFAULT_EXTRA_KEY: SecretKey = SecretKey::ED25519(ED25519SecretKey([
    213, 175, 27, 65, 239, 63, 64, 126, 187, 96, 90, 207, 42, 75, 1, 199, 109, 5, 0, 67, 207, 80,
    147, 19, 53, 126, 142, 30, 162, 168, 97, 155, 119, 161, 145, 134, 247, 30, 152, 37, 178, 129,
    174, 62, 225, 47, 43, 131, 212, 59, 200, 4, 158, 143, 3, 235, 237, 190, 51, 82, 253, 38, 36,
    145,
]));

pub fn default_extra_key(secret: Option<&[u8; crate::secret::SECRET_LEN]>) -> SecretKey {
    match secret {
        Some(s) => map_key(&DEFAULT_EXTRA_KEY.public_key(), Some(s)),
        None => DEFAULT_EXTRA_KEY,
    }
}

fn ed25519_map_secret(
    buf: &mut [u8],
    public: &ED25519PublicKey,
    secret: Option<&[u8; crate::secret::SECRET_LEN]>,
) {
    match secret {
        Some(secret) => {
            let hk = Hkdf::<Sha256>::new(None, secret);
            hk.expand(&public.0, buf).unwrap();
        }
        None => {
            buf.copy_from_slice(&public.0);
        }
    };
}

fn map_ed25519(
    public: &ED25519PublicKey,
    secret: Option<&[u8; crate::secret::SECRET_LEN]>,
) -> ED25519SecretKey {
    let mut buf = [0; ed25519_dalek::KEYPAIR_LENGTH];

    ed25519_map_secret(&mut buf[..ed25519_dalek::SECRET_KEY_LENGTH], public, secret);

    let secret_key = ed25519_dalek::SigningKey::from_bytes(
        <&[u8; ed25519_dalek::SECRET_KEY_LENGTH]>::try_from(
            &buf[..ed25519_dalek::SECRET_KEY_LENGTH],
        )
        .unwrap(),
    );
    let public_key = ed25519_dalek::VerifyingKey::from(&secret_key);

    buf[ed25519_dalek::SECRET_KEY_LENGTH..].copy_from_slice(public_key.as_bytes());
    ED25519SecretKey(buf)
}

fn secp256k1_from_slice(buf: &mut [u8], public: &Secp256K1PublicKey) -> secp256k1::SecretKey {
    match secp256k1::SecretKey::from_slice(buf) {
        Ok(s) => s,
        Err(_) => {
            tracing::warn!(target: "mirror", "Something super unlikely occurred! SECP256K1 key mapped from {:?} is too large. Flipping most significant bit.", public);
            // If we got an error, it means that either `buf` is all zeros, or that when interpreted as a 256-bit
            // int, it is larger than the order of the secp256k1 curve. Since the order of the curve starts with 0xFF,
            // in either case flipping the first bit should work, and we can unwrap() below.
            buf[0] ^= 0x80;
            secp256k1::SecretKey::from_slice(buf).unwrap()
        }
    }
}

fn map_secp256k1(
    public: &Secp256K1PublicKey,
    secret: Option<&[u8; crate::secret::SECRET_LEN]>,
) -> secp256k1::SecretKey {
    let mut buf = [0; secp256k1::constants::SECRET_KEY_SIZE];

    match secret {
        Some(secret) => {
            let hk = Hkdf::<Sha256>::new(None, secret);
            hk.expand(public.as_ref(), &mut buf).unwrap();
        }
        None => {
            buf.copy_from_slice(&public.as_ref()[..secp256k1::constants::SECRET_KEY_SIZE]);
        }
    };

    secp256k1_from_slice(&mut buf, public)
}

// This maps the public key to a secret key so that we can sign
// transactions on the target chain.  If secret is None, then we just
// use the bytes of the public key directly, otherwise we feed the
// public key to a key derivation function.
pub fn map_key(key: &PublicKey, secret: Option<&[u8; crate::secret::SECRET_LEN]>) -> SecretKey {
    match key {
        PublicKey::ED25519(k) => SecretKey::ED25519(map_ed25519(k, secret)),
        PublicKey::SECP256K1(k) => SecretKey::SECP256K1(map_secp256k1(k, secret)),
    }
}

// If it's a NEAR-implicit account, interprets it as an ed25519 public key,
// maps that and then returns the resulting implicit account. Otherwise does nothing.
// We do this so that transactions creating an implicit account
// by sending money will generate an account that we can control.
// TODO: return a Cow<> since this usually just clones the account.
pub fn map_account(
    account_id: &AccountId,
    secret: Option<&[u8; crate::secret::SECRET_LEN]>,
) -> AccountId {
    match account_id.get_account_type() {
        AccountType::NearImplicitAccount => {
            let public_key =
                PublicKey::from_near_implicit_account(account_id).expect("must be implicit");
            let mapped_key = map_key(&public_key, secret);
            derive_near_implicit_account_id(&mapped_key.public_key().unwrap_as_ed25519())
        }
        // TODO(eth-implicit) map to a new ETH address
        AccountType::EthImplicitAccount => account_id.clone(),
        AccountType::NamedAccount => account_id.clone(),
    }
}
