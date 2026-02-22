use std::collections::VecDeque;

const RING_BUFFER_SIZE: usize = 1024 * 1024; // 1MB

pub struct RingBuffer {
    buf: VecDeque<u8>,
}

impl RingBuffer {
    pub fn new() -> Self {
        Self {
            buf: VecDeque::with_capacity(RING_BUFFER_SIZE),
        }
    }

    /// Append data to the ring buffer, evicting oldest bytes if the buffer
    /// would exceed 1MB. If the incoming data itself exceeds 1MB, only the
    /// last 1MB is kept.
    pub fn append(&mut self, data: &[u8]) {
        if data.len() >= RING_BUFFER_SIZE {
            self.buf.clear();
            self.buf.extend(&data[data.len() - RING_BUFFER_SIZE..]);
            return;
        }
        let needed = self.buf.len() + data.len();
        if needed > RING_BUFFER_SIZE {
            let to_remove = needed - RING_BUFFER_SIZE;
            self.buf.drain(..to_remove);
        }
        self.buf.extend(data);
    }

    /// Drain all bytes from the buffer, returning them as a Vec.
    pub fn drain_all(&mut self) -> Vec<u8> {
        self.buf.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer_is_empty() {
        let buf = RingBuffer::new();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_append_small_data() {
        let mut buf = RingBuffer::new();
        buf.append(b"hello");
        assert_eq!(buf.len(), 5);
        assert!(!buf.is_empty());
    }

    #[test]
    fn test_drain_all_returns_data() {
        let mut buf = RingBuffer::new();
        buf.append(b"hello world");
        let data = buf.drain_all();
        assert_eq!(data, b"hello world");
        assert!(buf.is_empty());
    }

    #[test]
    fn test_multiple_appends() {
        let mut buf = RingBuffer::new();
        buf.append(b"hello ");
        buf.append(b"world");
        let data = buf.drain_all();
        assert_eq!(data, b"hello world");
    }

    #[test]
    fn test_eviction_when_full() {
        let mut buf = RingBuffer::new();
        // Fill to exactly 1MB
        let chunk = vec![b'A'; RING_BUFFER_SIZE];
        buf.append(&chunk);
        assert_eq!(buf.len(), RING_BUFFER_SIZE);

        // Append 100 more bytes — should evict first 100
        let extra = vec![b'B'; 100];
        buf.append(&extra);
        assert_eq!(buf.len(), RING_BUFFER_SIZE);

        let data = buf.drain_all();
        // First bytes should now be 'A' (after eviction of first 100)
        assert_eq!(data[0], b'A');
        // Last 100 bytes should be 'B'
        assert!(data[RING_BUFFER_SIZE - 100..].iter().all(|&b| b == b'B'));
    }

    #[test]
    fn test_data_larger_than_buffer_keeps_last_1mb() {
        let mut buf = RingBuffer::new();
        // Pre-fill some data
        buf.append(b"will be replaced");

        // Append 2MB of data
        let large = vec![b'X'; 2 * RING_BUFFER_SIZE];
        buf.append(&large);
        assert_eq!(buf.len(), RING_BUFFER_SIZE);

        let data = buf.drain_all();
        assert!(data.iter().all(|&b| b == b'X'));
    }

    #[test]
    fn test_drain_empties_buffer() {
        let mut buf = RingBuffer::new();
        buf.append(b"some data");
        let _ = buf.drain_all();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_drain_empty_buffer() {
        let mut buf = RingBuffer::new();
        let data = buf.drain_all();
        assert!(data.is_empty());
    }

    #[test]
    fn test_append_empty_data() {
        let mut buf = RingBuffer::new();
        buf.append(b"hello");
        buf.append(b"");
        assert_eq!(buf.len(), 5);
        let data = buf.drain_all();
        assert_eq!(data, b"hello");
    }

    #[test]
    fn test_exactly_1mb_no_eviction() {
        let mut buf = RingBuffer::new();
        let half = vec![b'A'; RING_BUFFER_SIZE / 2];
        buf.append(&half);
        buf.append(&half);
        assert_eq!(buf.len(), RING_BUFFER_SIZE);
        let data = buf.drain_all();
        assert!(data.iter().all(|&b| b == b'A'));
    }

    #[test]
    fn test_one_byte_over_triggers_eviction() {
        let mut buf = RingBuffer::new();
        let full = vec![b'A'; RING_BUFFER_SIZE];
        buf.append(&full);
        assert_eq!(buf.len(), RING_BUFFER_SIZE);

        buf.append(b"B");
        assert_eq!(buf.len(), RING_BUFFER_SIZE);

        let data = buf.drain_all();
        // First byte was evicted; last byte is 'B'
        assert_eq!(*data.last().unwrap(), b'B');
        assert_eq!(data[0], b'A');
    }

    #[test]
    fn test_incremental_fill_and_eviction() {
        let mut buf = RingBuffer::new();
        // Fill in 1KB increments
        let chunk = vec![0u8; 1024];
        for _ in 0..1024 {
            buf.append(&chunk);
        }
        assert_eq!(buf.len(), RING_BUFFER_SIZE);

        // One more chunk should evict exactly 1KB
        let new_chunk = vec![1u8; 1024];
        buf.append(&new_chunk);
        assert_eq!(buf.len(), RING_BUFFER_SIZE);

        let data = buf.drain_all();
        // Last 1024 bytes should be the new chunk
        assert!(data[RING_BUFFER_SIZE - 1024..].iter().all(|&b| b == 1));
    }

    #[test]
    fn test_data_exactly_1mb_replaces_all() {
        let mut buf = RingBuffer::new();
        buf.append(b"original data");
        let exact = vec![b'Z'; RING_BUFFER_SIZE];
        buf.append(&exact);
        assert_eq!(buf.len(), RING_BUFFER_SIZE);
        let data = buf.drain_all();
        assert!(data.iter().all(|&b| b == b'Z'));
    }
}
