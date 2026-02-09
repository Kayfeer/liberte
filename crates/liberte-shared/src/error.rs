use thiserror::Error;

#[derive(Error, Debug)]
pub enum LiberteError {
    #[error("Crypto error: {0}")]
    Crypto(#[from] CryptoError),

    #[error("Identity error: {0}")]
    Identity(#[from] IdentityError),

    #[error("Noise handshake error: {0}")]
    Noise(#[from] NoiseError),

    #[error("CSAM filter error: {0}")]
    Csam(#[from] CsamError),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed")]
    EncryptionFailed,

    #[error("Decryption failed: invalid ciphertext or wrong key")]
    DecryptionFailed,

    #[error("Invalid key length")]
    InvalidKeyLength,
}

#[derive(Error, Debug)]
pub enum IdentityError {
    #[error("Invalid key bytes")]
    InvalidKeyBytes,

    #[error("Failed to generate keypair")]
    GenerationFailed,

    #[error("Key file error: {0}")]
    KeyFile(String),
}

#[derive(Error, Debug)]
pub enum NoiseError {
    #[error("Noise handshake error: {0}")]
    Handshake(String),

    #[error("Noise transport error: {0}")]
    Transport(String),
}

#[derive(Error, Debug)]
pub enum CsamError {
    #[error("Image processing error: {0}")]
    ImageError(#[from] image::ImageError),

    #[error("Bloom filter error: {0}")]
    BloomFilterError(String),

    #[error("Content blocked: matches known illegal content signature")]
    ContentBlocked,
}
