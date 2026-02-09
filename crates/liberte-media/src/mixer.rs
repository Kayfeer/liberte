/// Client-side audio mixer for mesh mode.
///
/// In full mesh mode, each client receives audio streams from all other
/// participants. The mixer combines these streams into a single output
/// for playback.

/// Mix multiple audio frames (f32 samples) into a single output frame.
/// Uses simple additive mixing with clipping prevention.
pub fn mix_frames(frames: &[Vec<f32>]) -> Vec<f32> {
    if frames.is_empty() {
        return Vec::new();
    }

    let max_len = frames.iter().map(|f| f.len()).max().unwrap_or(0);
    let mut output = vec![0.0f32; max_len];
    let num_sources = frames.len() as f32;

    for frame in frames {
        for (i, &sample) in frame.iter().enumerate() {
            output[i] += sample;
        }
    }

    // Normalize to prevent clipping (simple approach)
    if num_sources > 1.0 {
        let scale = 1.0 / num_sources.sqrt(); // Use sqrt for more natural mixing
        for sample in &mut output {
            *sample *= scale;
        }
    }

    // Hard clip to [-1.0, 1.0]
    for sample in &mut output {
        *sample = sample.clamp(-1.0, 1.0);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mix_empty() {
        let result = mix_frames(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_mix_single_frame() {
        let frame = vec![0.5, -0.5, 0.25];
        let result = mix_frames(&[frame.clone()]);
        assert_eq!(result, frame);
    }

    #[test]
    fn test_mix_two_frames() {
        let frame1 = vec![0.5, 0.3];
        let frame2 = vec![0.3, 0.2];
        let result = mix_frames(&[frame1, frame2]);
        // Each sample is (sum) * (1/sqrt(2))
        let scale = 1.0 / 2.0f32.sqrt();
        assert!((result[0] - 0.8 * scale).abs() < 0.001);
        assert!((result[1] - 0.5 * scale).abs() < 0.001);
    }

    #[test]
    fn test_mix_clipping_prevention() {
        let frame1 = vec![1.0, 1.0];
        let frame2 = vec![1.0, 1.0];
        let result = mix_frames(&[frame1, frame2]);
        // Should not exceed 1.0
        for &sample in &result {
            assert!(sample <= 1.0);
            assert!(sample >= -1.0);
        }
    }

    #[test]
    fn test_mix_different_lengths() {
        let frame1 = vec![0.5, 0.3, 0.1];
        let frame2 = vec![0.3, 0.2];
        let result = mix_frames(&[frame1, frame2]);
        assert_eq!(result.len(), 3);
    }
}
