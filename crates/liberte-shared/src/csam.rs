use image_hasher::{HashAlg, HasherConfig};
use serde::{Deserialize, Serialize};

use crate::error::CsamError;

#[derive(Serialize, Deserialize)]
struct BloomData {
    bitmap: Vec<u8>,
    bitmap_bits: u64,
    k_num: u32,
    sip_keys: [(u64, u64); 2],
}

// Perceptual hash + bloom filter for client-side CSAM detection.
// One-way: can check membership but can't reconstruct original images.
pub struct CsamFilter {
    bloom: bloomfilter::Bloom<[u8]>,
    hasher_config: HasherConfig,
}

impl CsamFilter {
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
            .hash_alg(HashAlg::DoubleGradient)
            .hash_size(16, 16); // 256-bit hash

        Ok(Self {
            bloom,
            hasher_config,
        })
    }

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

    // Returns Ok(()) if safe, Err if matched
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

    // Also checks horizontally flipped variant
    pub fn check_image_thorough(&self, image_bytes: &[u8]) -> Result<(), CsamError> {
        self.check_image(image_bytes)?;

        let img = image::load_from_memory(image_bytes)?;
        let flipped = img.fliph();
        let hasher = self.hasher_config.to_hasher();
        let hash = hasher.hash_image(&flipped);

        if self.bloom.check(hash.as_bytes()) {
            return Err(CsamError::ContentBlocked);
        }

        Ok(())
    }

    pub fn add_image_hash(&mut self, image_bytes: &[u8]) -> Result<(), CsamError> {
        let img = image::load_from_memory(image_bytes)?;
        let hasher = self.hasher_config.to_hasher();
        let hash = hasher.hash_image(&img);
        self.bloom.set(hash.as_bytes());
        Ok(())
    }

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
