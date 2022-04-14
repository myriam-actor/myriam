use crypto_box::aead::Aead;
use thiserror::Error;

use crate::identity::{PublicIdentity, SelfIdentity};

/// size in bytes of NaCl's `crypto_box` public and secret keys
pub const KEY_BYTES: usize = 32;

/// size in bytes of NaCl's `crypto_box` nonce
pub const NONCE_BYTES: usize = 24;

pub fn try_decrypt(
    cipher: Vec<u8>,
    nonce: &[u8; NONCE_BYTES],
    public_id: &PublicIdentity,
    self_id: &SelfIdentity,
) -> Result<Vec<u8>, DecryptionError> {
    let decrypt_box = crypto_box::Box::new(public_id.key(), self_id.secret());
    Ok(decrypt_box.decrypt(nonce.into(), cipher.as_slice())?)
}

pub fn try_encrypt(
    message: &[u8],
    public_id: &PublicIdentity,
    self_id: &SelfIdentity,
) -> Result<(Vec<u8>, [u8; NONCE_BYTES]), EncryptionError> {
    let mut rng = crypto_box::rand_core::OsRng;
    let encrypt_box = crypto_box::Box::new(public_id.key(), self_id.secret());
    let nonce = crypto_box::generate_nonce(&mut rng);

    Ok((encrypt_box.encrypt(&nonce, message)?, nonce.into()))
}

#[derive(Debug, Error)]
pub enum DecryptionError {
    #[error("failed to decrypt message with the given keys: {0}")]
    Mismatch(#[from] crypto_box::aead::Error),
}

#[derive(Debug, Error)]
pub enum EncryptionError {
    #[error("failed to encrypt message with the given keys: {0}")]
    Mismatch(#[from] crypto_box::aead::Error),
}

#[cfg(test)]
mod tests {
    use crate::{
        crypto::{try_decrypt, try_encrypt},
        identity::SelfIdentity,
    };

    #[test]
    fn can_decrypt() {
        let alice = SelfIdentity::new();
        let bob = SelfIdentity::new();

        let message = b"a message";

        let (cipher, nonce) = try_encrypt(message, bob.public_identity(), &alice).unwrap();

        assert_eq!(
            message,
            &try_decrypt(cipher, &nonce, alice.public_identity(), &bob).unwrap()[..]
        );
    }
}
