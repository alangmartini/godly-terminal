/// AVX2 SIMD implementation for control character scanning.
///
/// Processes 32 bytes per iteration using AVX2 intrinsics.
/// Requires runtime detection via `is_x86_feature_detected!("avx2")`.
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// Scan for the first control character (< 0x20 or == 0x7F) using AVX2.
///
/// # Safety
/// Requires AVX2 support. Caller must verify via `is_x86_feature_detected!("avx2")`.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn scan_for_control(data: &[u8]) -> Option<usize> {
    let len = data.len();
    let mut i = 0;

    if len >= 32 {
        let del = _mm256_set1_epi8(0x7F_u8 as i8);

        while i + 32 <= len {
            let chunk = _mm256_loadu_si256(data.as_ptr().add(i) as *const __m256i);

            // Detect bytes < 0x20: subtract 0x1F with unsigned saturation,
            // then check if result is zero.
            let sub = _mm256_subs_epu8(chunk, _mm256_set1_epi8(0x1F_u8 as i8));
            let is_control = _mm256_cmpeq_epi8(sub, _mm256_setzero_si256());

            // Detect 0x7F (DEL)
            let is_del = _mm256_cmpeq_epi8(chunk, del);

            // Combine: control OR del
            let combined = _mm256_or_si256(is_control, is_del);
            let mask = _mm256_movemask_epi8(combined);

            if mask != 0 {
                return Some(i + mask.trailing_zeros() as usize);
            }

            i += 32;
        }
    }

    // Fall through to SSE2 for 16-31 byte remainder
    if i + 16 <= len {
        let threshold_128 = _mm_set1_epi8(0x1F_u8 as i8);
        let del_128 = _mm_set1_epi8(0x7F_u8 as i8);

        let chunk = _mm_loadu_si128(data.as_ptr().add(i) as *const __m128i);
        let sub = _mm_subs_epu8(chunk, threshold_128);
        let is_control = _mm_cmpeq_epi8(sub, _mm_setzero_si128());
        let is_del = _mm_cmpeq_epi8(chunk, del_128);
        let combined = _mm_or_si128(is_control, is_del);
        let mask = _mm_movemask_epi8(combined);

        if mask != 0 {
            return Some(i + mask.trailing_zeros() as usize);
        }
        i += 16;
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

/// Check if all bytes are ASCII (< 0x80) using AVX2.
///
/// # Safety
/// Requires AVX2 support. Caller must verify via `is_x86_feature_detected!("avx2")`.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn is_all_ascii(data: &[u8]) -> bool {
    let len = data.len();
    let mut i = 0;

    if len >= 32 {
        while i + 32 <= len {
            let chunk = _mm256_loadu_si256(data.as_ptr().add(i) as *const __m256i);
            let mask = _mm256_movemask_epi8(chunk);
            if mask != 0 {
                return false;
            }
            i += 32;
        }
    }

    // Handle 16-31 byte remainder with SSE2
    if i + 16 <= len {
        let chunk = _mm_loadu_si128(data.as_ptr().add(i) as *const __m128i);
        let mask = _mm_movemask_epi8(chunk);
        if mask != 0 {
            return false;
        }
        i += 16;
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
