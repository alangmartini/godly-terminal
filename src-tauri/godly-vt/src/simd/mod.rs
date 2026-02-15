//! SIMD-accelerated scanning for terminal parsing.
//!
//! Provides `scan_for_control()` and `is_all_ascii()` with automatic
//! runtime dispatch to the best available instruction set:
//! - AVX2 (32 bytes/iter) on CPUs that support it
//! - SSE2 (16 bytes/iter) on all x86-64 CPUs
//! - Scalar fallback on non-x86 architectures

pub mod scalar;

#[cfg(target_arch = "x86_64")]
pub mod sse2;

#[cfg(target_arch = "x86_64")]
pub mod avx2;

/// Find the first control character in `data`.
///
/// A control character is any byte < 0x20 (C0 controls including ESC, NUL,
/// newline, tab, etc.) or 0x7F (DEL).
///
/// Returns `Some(index)` of the first control character, or `None` if the
/// entire slice contains only printable/high bytes (0x20..=0x7E, 0x80..=0xFF).
///
/// Uses SIMD acceleration when available (AVX2 > SSE2 > scalar).
#[inline]
pub fn scan_for_control(data: &[u8]) -> Option<usize> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { avx2::scan_for_control(data) };
        }
        // SSE2 is always available on x86-64
        return unsafe { sse2::scan_for_control(data) };
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        scalar::scan_for_control(data)
    }
}

/// Check if all bytes in `data` are ASCII (< 0x80).
///
/// Uses SIMD acceleration when available (AVX2 > SSE2 > scalar).
#[inline]
pub fn is_all_ascii(data: &[u8]) -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { avx2::is_all_ascii(data) };
        }
        return unsafe { sse2::is_all_ascii(data) };
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        scalar::is_all_ascii(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- scan_for_control correctness tests ----

    #[test]
    fn scan_empty() {
        assert_eq!(scan_for_control(&[]), None);
    }

    #[test]
    fn scan_single_printable() {
        assert_eq!(scan_for_control(b"A"), None);
    }

    #[test]
    fn scan_single_control() {
        assert_eq!(scan_for_control(&[0x00]), Some(0));
        assert_eq!(scan_for_control(&[0x1B]), Some(0));
        assert_eq!(scan_for_control(&[0x7F]), Some(0));
    }

    #[test]
    fn scan_all_byte_values() {
        // Test every single byte value individually
        for b in 0u8..=255 {
            let data = [b];
            let expected = if b < 0x20 || b == 0x7F { Some(0) } else { None };
            assert_eq!(
                scan_for_control(&data),
                expected,
                "byte 0x{:02X}: expected {:?}",
                b,
                expected
            );
        }
    }

    #[test]
    fn scan_matches_scalar_for_all_bytes() {
        // Property test: SIMD dispatch must match scalar for every byte
        for b in 0u8..=255 {
            let data = [b];
            assert_eq!(
                scan_for_control(&data),
                scalar::scan_for_control(&data),
                "mismatch at byte 0x{:02X}",
                b
            );
        }
    }

    #[test]
    fn scan_boundary_sizes() {
        // Test sizes that exercise SIMD boundary conditions:
        // 1, 15, 16, 17, 31, 32, 33, 63, 64, 65
        let sizes = [1, 15, 16, 17, 31, 32, 33, 63, 64, 65, 128, 255, 256];

        for &size in &sizes {
            // All printable - should return None
            let data: Vec<u8> = (0..size).map(|i| b'A' + (i as u8 % 26)).collect();
            assert_eq!(
                scan_for_control(&data),
                None,
                "size {}: expected None for all printable",
                size
            );

            // Control at the very end
            let mut data_end = data.clone();
            data_end[size - 1] = 0x1B;
            assert_eq!(
                scan_for_control(&data_end),
                Some(size - 1),
                "size {}: expected control at end",
                size
            );

            // Control at the beginning
            let mut data_start = data.clone();
            data_start[0] = 0x0A;
            assert_eq!(
                scan_for_control(&data_start),
                Some(0),
                "size {}: expected control at start",
                size
            );

            // Verify SIMD matches scalar
            assert_eq!(
                scan_for_control(&data),
                scalar::scan_for_control(&data),
                "size {}: SIMD/scalar mismatch for all printable",
                size
            );
            assert_eq!(
                scan_for_control(&data_end),
                scalar::scan_for_control(&data_end),
                "size {}: SIMD/scalar mismatch for control at end",
                size
            );
        }
    }

    #[test]
    fn scan_control_at_simd_boundaries() {
        // Place control char at positions that are SIMD-significant
        let positions = [0, 1, 14, 15, 16, 17, 30, 31, 32, 33, 47, 48];
        for &pos in &positions {
            let mut data = vec![b'X'; 64];
            data[pos] = 0x1B;
            assert_eq!(
                scan_for_control(&data),
                Some(pos),
                "control at position {}",
                pos
            );
        }
    }

    #[test]
    fn scan_high_bytes_not_control() {
        // Bytes 0x80-0xFF should not be detected as control
        let data: Vec<u8> = (0x80..=0xFF).collect();
        assert_eq!(scan_for_control(&data), None);

        // Mix of printable and high bytes
        let data: Vec<u8> = (0x20..=0xFF).filter(|&b| b != 0x7F).collect();
        assert_eq!(scan_for_control(&data), None);
    }

    #[test]
    fn scan_realistic_terminal_output() {
        // Simulates "ls -la" output with newlines
        let data = b"total 42\ndrwxr-xr-x  2 user group 4096 Jan  1 12:00 .\n";
        assert_eq!(scan_for_control(data), Some(8)); // first \n
    }

    // ---- is_all_ascii correctness tests ----

    #[test]
    fn ascii_empty() {
        assert!(is_all_ascii(&[]));
    }

    #[test]
    fn ascii_single_bytes() {
        for b in 0u8..=127 {
            assert!(is_all_ascii(&[b]), "byte 0x{:02X} should be ASCII", b);
        }
        for b in 128u8..=255 {
            assert!(!is_all_ascii(&[b]), "byte 0x{:02X} should not be ASCII", b);
        }
    }

    #[test]
    fn ascii_matches_scalar_for_all_bytes() {
        for b in 0u8..=255 {
            let data = [b];
            assert_eq!(
                is_all_ascii(&data),
                scalar::is_all_ascii(&data),
                "mismatch at byte 0x{:02X}",
                b
            );
        }
    }

    #[test]
    fn ascii_boundary_sizes() {
        let sizes = [1, 15, 16, 17, 31, 32, 33, 63, 64, 65, 128, 255, 256];

        for &size in &sizes {
            // All ASCII
            let data: Vec<u8> = (0..size).map(|i| b'A' + (i as u8 % 26)).collect();
            assert!(
                is_all_ascii(&data),
                "size {}: expected true for all ASCII",
                size
            );

            // Non-ASCII at the end
            let mut data_end = data.clone();
            data_end[size - 1] = 0x80;
            assert!(
                !is_all_ascii(&data_end),
                "size {}: expected false for non-ASCII at end",
                size
            );

            // Non-ASCII at the start
            let mut data_start = data.clone();
            data_start[0] = 0xFF;
            assert!(
                !is_all_ascii(&data_start),
                "size {}: expected false for non-ASCII at start",
                size
            );

            // Verify SIMD matches scalar
            assert_eq!(
                is_all_ascii(&data),
                scalar::is_all_ascii(&data),
                "size {}: SIMD/scalar mismatch (all ASCII)",
                size
            );
            assert_eq!(
                is_all_ascii(&data_end),
                scalar::is_all_ascii(&data_end),
                "size {}: SIMD/scalar mismatch (non-ASCII at end)",
                size
            );
        }
    }

    #[test]
    fn ascii_non_ascii_at_simd_boundaries() {
        let positions = [0, 1, 14, 15, 16, 17, 30, 31, 32, 33, 47, 48];
        for &pos in &positions {
            let mut data = vec![b'X'; 64];
            data[pos] = 0xC0;
            assert!(
                !is_all_ascii(&data),
                "non-ASCII at position {} not detected",
                pos
            );
        }
    }

    #[test]
    fn large_buffer_property_test() {
        // Generate a large buffer and verify SIMD matches scalar
        let mut data: Vec<u8> = (0..1024).map(|i| (i % 128) as u8 + 0x20).collect();
        // Make sure no 0x7F
        for b in data.iter_mut() {
            if *b == 0x7F {
                *b = 0x20;
            }
        }
        assert_eq!(
            scan_for_control(&data),
            scalar::scan_for_control(&data),
        );
        assert_eq!(
            is_all_ascii(&data),
            scalar::is_all_ascii(&data),
        );

        // Insert control chars at various points
        data[500] = 0x0A;
        assert_eq!(
            scan_for_control(&data),
            scalar::scan_for_control(&data),
        );

        // Insert non-ASCII
        data[200] = 0xC0;
        assert_eq!(
            is_all_ascii(&data),
            scalar::is_all_ascii(&data),
        );
    }
}
