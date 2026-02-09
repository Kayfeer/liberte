use image_hasher::{HashAlg, HasherConfig};
use serde::{Deserialize, Serialize};

use crate::error::CsamError;

/// Serializable representation of a Bloom filter.
#[derive(Serialize, Deserialize)]
struct BloomData {
    bitmap: Vec<u8>,
    bitmap_bits: u64,
    k_num: u32,
    sip_keys: [(u64, u64); 2],
}

/// Client-side CSAM detection filter using perceptual hashing + Bloom filter.
///
/// Before any image is sent (encrypted or not), the client computes a perceptual
/// hash and checks it against a local Bloom filter of known illegal content signatures.
///
/// The Bloom filter is one-way: you can check if a hash exists, but you cannot
/// reconstruct the original images from it.
pub struct CsamFilter {
    bloom: bloomfilter::Bloom<[u8]>,
    hasher_config: HasherConfig,
}

impl CsamFilter {
    /// Load Bloom filter from pre-built binary file.
    /// The file is distributed with the application.
    pub fn load(bloom_path: &std::path::Path) -> Result<Self, CsamError> {
        let data = std::fs::read(bloom_path)
            .map_err(|e| CsamError::BloomFilterError(format!("Failed to read bloom filter: {e}")))?;

        let bd: BloomData = bincode::deserialize(&data)
            .map_err(|e| CsamError::BloomFilterError(format!("Failed to deserialize: {e}")))?;

        let bloom = bloomfilter::Bloom::from_existing(
            &bd.bitmap,
            bd.bitmap_bits,
            bd.k_num,
            bd.sip_keys,
        );

        let hasher_config = HasherConfig::new()
            .hash_alg(HashAlg::DoubleGradient) // DCT-based, similar to pHash
            .hash_size(16, 16); // 256-bit hash

        Ok(Self {
            bloom,
            hasher_config,
        })
    }

    /// Create a new empty filter (for testing or bootstrapping).
    pub fn new_empty(expected_items: usize, false_positive_rate: f64) -> Self {
        let bloom = bloomfilter::Bloom::new_for_fp_rate(expected_items, false_positive_rate);
        let hasher_config = HasherConfig::new()
            .hash_alg(HashAlg::DoubleGradient)
            .hash_size(16, 16);

        Self {
            bloom,
            hasher_config,
        }
    }

    /// Check if an image matches known illegal content.
    /// Returns `Ok(())` if safe, `Err(CsamError::ContentBlocked)` if matched.
    pub fn check_image(&self, image_bytes: &[u8]) -> Result<(), CsamError> {
        let img = image::load_from_memory(image_bytes)?;
        let hasher = self.hasher_config.to_hasher();
        let hash = hasher.hash_image(&img);
        let hash_bytes = hash.as_bytes();

        if self.bloom.check(hash_bytes) {
            return Err(CsamError::ContentBlocked);
        }

        Ok(())
    }

    /// Thorough check: also check horizontally flipped variant.
    pub fn check_image_thorough(&self, image_bytes: &[u8]) -> Result<(), CsamError> {
        // Primary check
        self.check_image(image_bytes)?;

        // Also check flipped version
        let img = image::load_from_memory(image_bytes)?;
        let flipped = img.fliph();
        let hasher = self.hasher_config.to_hasher();
        let hash = hasher.hash_image(&flipped);

        if self.bloom.check(hash.as_bytes()) {
            return Err(CsamError::ContentBlocked);
        }

        Ok(())
    }

    /// Add a hash to the bloom filter (for building the filter)
    pub fn add_image_hash(&mut self, image_bytes: &[u8]) -> Result<(), CsamError> {
        let img = image::load_from_memory(image_bytes)?;
        let hasher = self.hasher_config.to_hasher();
        let hash = hasher.hash_image(&img);
        self.bloom.set(hash.as_bytes());
        Ok(())
    }

    /// Serialize the bloom filter to bytes for distribution.
    pub fn export(&self) -> Result<Vec<u8>, CsamError> {
        let bd = BloomData {
            bitmap: self.bloom.bitmap(),
            bitmap_bits: self.bloom.number_of_bits(),
            k_num: self.bloom.number_of_hash_functions(),
            sip_keys: self.bloom.sip_keys(),
        };
        bincode::serialize(&bd)
            .map_err(|e| CsamError::BloomFilterError(format!("Serialize failed: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_filter_passes() {
        let filter = CsamFilter::new_empty(1000, 0.0001);
        // A 1x1 red pixel PNG
        let img_data = create_test_image();
        assert!(filter.check_image(&img_data).is_ok());
    }

    fn create_test_image() -> Vec<u8> {
        use image::{ImageBuffer, Rgb};
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(10, 10, |_, _| {
            Rgb([255, 0, 0])
        });
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        buf
    }
}
