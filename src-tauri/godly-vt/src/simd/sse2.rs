/// SSE2 SIMD implementation for control character scanning.
///
/// Processes 16 bytes per iteration using SSE2 intrinsics.
/// SSE2 is always available on x86-64 so no runtime detection is needed.
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// Scan for the first control character (< 0x20 or == 0x7F) using SSE2.
///
/// # Safety
/// Requires SSE2 support (always available on x86-64).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
pub unsafe fn scan_for_control(data: &[u8]) -> Option<usize> {
    let len = data.len();
    let mut i = 0;

    if len >= 16 {
        // DEL vector: all bytes = 0x7F
        let del = _mm_set1_epi8(0x7F_u8 as i8);

        while i + 16 <= len {
            let chunk = _mm_loadu_si128(data.as_ptr().add(i) as *const __m128i);

            // Detect bytes < 0x20: subtract 0x20 with unsigned saturation.
            // If byte < 0x20, result is 0. If byte >= 0x20, result is byte - 0x20.
            // We want bytes where the result IS zero (those are control chars).
            let sub = _mm_subs_epu8(chunk, _mm_set1_epi8(0x1F_u8 as i8));
            let is_control = _mm_cmpeq_epi8(sub, _mm_setzero_si128());

            // Detect 0x7F (DEL)
            let is_del = _mm_cmpeq_epi8(chunk, del);

            // Combine: control OR del
            let combined = _mm_or_si128(is_control, is_del);
            let mask = _mm_movemask_epi8(combined);

            if mask != 0 {
                return Some(i + mask.trailing_zeros() as usize);
            }

            i += 16;
        }
    }

    // Handle remaining bytes with scalar fallback
    while i < len {
        let b = data[i];
        if b < 0x20 || b == 0x7F {
            return Some(i);
        }
        i += 1;
    }

    None
}

/// Check if all bytes are ASCII (< 0x80) using SSE2.
///
/// # Safety
/// Requires SSE2 support (always available on x86-64).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
pub unsafe fn is_all_ascii(data: &[u8]) -> bool {
    let len = data.len();
    let mut i = 0;

    if len >= 16 {
        while i + 16 <= len {
            let chunk = _mm_loadu_si128(data.as_ptr().add(i) as *const __m128i);
            // movemask extracts the high bit of each byte.
            // If any byte >= 0x80, the corresponding bit will be set.
            let mask = _mm_movemask_epi8(chunk);
            if mask != 0 {
                return false;
            }
            i += 16;
        }
    }

    // Handle remaining bytes
    while i < len {
        if data[i] >= 0x80 {
            return false;
        }
        i += 1;
    }

    true
}
