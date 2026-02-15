// Image protocol support for godly-vt
//
// Implements storage and management for inline terminal images from
// Kitty graphics, iTerm2, and Sixel protocols.
//
// Clean-room implementation from public specifications only:
// - Kitty: https://sw.kovidgoyal.net/kitty/graphics-protocol/
// - iTerm2: https://iterm2.com/documentation-images.html

#[cfg(feature = "images")]
pub mod kitty;
#[cfg(feature = "images")]
pub mod iterm2;
#[cfg(feature = "images")]
pub mod sixel;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// Default image storage quota: 256 MB.
const DEFAULT_QUOTA: usize = 256 * 1024 * 1024;

/// Maximum pixels for a single image (100 million = ~10K x 10K).
const MAX_SINGLE_IMAGE_PIXELS: u64 = 100_000_000;

/// A decoded image stored in the image cache.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    /// Raw RGBA pixel data (4 bytes per pixel).
    pub pixels: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Content hash for deduplication.
    pub content_hash: u64,
}

impl DecodedImage {
    /// Returns the size in bytes of the pixel data.
    pub fn byte_size(&self) -> usize {
        self.pixels.len()
    }
}

/// Reference from a grid cell to a region of an image.
///
/// Each cell that is part of an inline image holds one of these, describing
/// which portion of the image this cell represents using normalized
/// texture coordinates (0.0-1.0).
#[derive(Debug, Clone, PartialEq)]
pub struct ImageCellRef {
    /// Content hash pointing into `ImageStore.images`.
    pub image_hash: u64,
    /// Kitty placement ID (0 for non-Kitty protocols).
    pub placement_id: u32,
    /// Normalized X offset into the image (0.0 = left, 1.0 = right).
    pub tex_x: f32,
    /// Normalized Y offset into the image (0.0 = top, 1.0 = bottom).
    pub tex_y: f32,
    /// Normalized width of the cell's portion of the image.
    pub tex_w: f32,
    /// Normalized height of the cell's portion of the image.
    pub tex_h: f32,
    /// Z-index for layering (Kitty supports above/below text).
    pub z_index: i32,
}

/// Staging area for Kitty chunked image uploads.
#[derive(Debug, Clone)]
pub struct ImageUpload {
    /// Kitty image ID assigned by the application.
    pub image_id: u32,
    /// Kitty image number (for numbered references).
    pub image_number: u32,
    /// Accumulated raw data (base64-decoded, possibly compressed).
    pub data: Vec<u8>,
    /// Pixel format: 24 (RGB), 32 (RGBA), or 100 (PNG).
    pub format: u32,
    /// Expected width (for raw formats).
    pub width: u32,
    /// Expected height (for raw formats).
    pub height: u32,
    /// Whether data is zlib-compressed.
    pub compressed: bool,
}

/// Central store for all decoded images.
///
/// Images are stored deduplicated by content hash, with Arc references
/// allowing multiple cells to share the same image data. LRU eviction
/// enforces the memory quota.
#[derive(Debug)]
pub struct ImageStore {
    /// Deduplicated image storage keyed by content hash.
    images: HashMap<u64, Arc<DecodedImage>>,
    /// Kitty staging area for in-progress uploads.
    uploads: HashMap<u32, ImageUpload>,
    /// Total bytes currently used by all stored images.
    total_bytes: usize,
    /// Maximum allowed bytes for all stored images.
    quota: usize,
    /// LRU tracking: most recently used at back, least at front.
    lru: VecDeque<u64>,
    /// Kitty image ID to content hash mapping.
    id_to_hash: HashMap<u32, u64>,
    /// Next auto-assigned image ID for Kitty protocol.
    next_id: u32,
}

impl Default for ImageStore {
    fn default() -> Self {
        Self::new(DEFAULT_QUOTA)
    }
}

impl ImageStore {
    /// Creates a new ImageStore with the specified quota in bytes.
    pub fn new(quota: usize) -> Self {
        Self {
            images: HashMap::new(),
            uploads: HashMap::new(),
            total_bytes: 0,
            quota,
            lru: VecDeque::new(),
            id_to_hash: HashMap::new(),
            next_id: 1,
        }
    }

    /// Returns the total bytes currently stored.
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Returns the number of stored images.
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    /// Returns the storage quota in bytes.
    pub fn quota(&self) -> usize {
        self.quota
    }

    /// Compute a content hash for image data.
    ///
    /// Uses a simple but fast non-cryptographic hash suitable for
    /// deduplication.
    pub fn content_hash(data: &[u8]) -> u64 {
        // FNV-1a 64-bit hash — fast, good distribution for dedup
        let mut hash: u64 = 0xcbf29ce484222325;
        for &byte in data {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    /// Store a decoded image, deduplicating by content hash.
    ///
    /// Returns the content hash. If an image with the same content
    /// already exists, no new storage is allocated.
    pub fn store(&mut self, image: DecodedImage) -> u64 {
        let hash = image.content_hash;

        // Deduplication: if we already have this image, just touch LRU
        if self.images.contains_key(&hash) {
            self.touch_lru(hash);
            return hash;
        }

        let byte_size = image.byte_size();

        // Evict LRU images until we have room
        while self.total_bytes + byte_size > self.quota && !self.lru.is_empty() {
            self.evict_lru();
        }

        // If the single image exceeds quota, store it anyway but warn
        // (the caller should check MAX_SINGLE_IMAGE_PIXELS)
        self.total_bytes += byte_size;
        self.images.insert(hash, Arc::new(image));
        self.lru.push_back(hash);

        hash
    }

    /// Store an image and associate it with a Kitty image ID.
    pub fn store_with_id(&mut self, image_id: u32, image: DecodedImage) -> u64 {
        let hash = self.store(image);
        self.id_to_hash.insert(image_id, hash);
        hash
    }

    /// Look up an image by content hash.
    pub fn get(&self, hash: u64) -> Option<&Arc<DecodedImage>> {
        self.images.get(&hash)
    }

    /// Look up an image by Kitty image ID.
    pub fn get_by_id(&self, image_id: u32) -> Option<&Arc<DecodedImage>> {
        self.id_to_hash
            .get(&image_id)
            .and_then(|hash| self.images.get(hash))
    }

    /// Get the content hash for a Kitty image ID.
    pub fn hash_for_id(&self, image_id: u32) -> Option<u64> {
        self.id_to_hash.get(&image_id).copied()
    }

    /// Remove an image by content hash.
    pub fn remove(&mut self, hash: u64) -> bool {
        if let Some(image) = self.images.remove(&hash) {
            self.total_bytes -= image.byte_size();
            self.lru.retain(|&h| h != hash);
            // Clean up any ID mappings pointing to this hash
            self.id_to_hash.retain(|_, &mut h| h != hash);
            true
        } else {
            false
        }
    }

    /// Remove an image by Kitty image ID.
    pub fn remove_by_id(&mut self, image_id: u32) -> bool {
        if let Some(hash) = self.id_to_hash.remove(&image_id) {
            self.remove(hash)
        } else {
            false
        }
    }

    /// Start a Kitty chunked upload.
    pub fn begin_upload(&mut self, upload: ImageUpload) {
        let id = upload.image_id;
        self.uploads.insert(id, upload);
    }

    /// Append data to an in-progress Kitty upload.
    pub fn append_upload_data(&mut self, image_id: u32, data: &[u8]) {
        if let Some(upload) = self.uploads.get_mut(&image_id) {
            upload.data.extend_from_slice(data);
        }
    }

    /// Finish a Kitty upload and return the upload data for processing.
    pub fn finish_upload(&mut self, image_id: u32) -> Option<ImageUpload> {
        self.uploads.remove(&image_id)
    }

    /// Cancel an in-progress upload.
    pub fn cancel_upload(&mut self, image_id: u32) {
        self.uploads.remove(&image_id);
    }

    /// Allocate the next available Kitty image ID.
    pub fn next_image_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    /// Touch an entry in the LRU, moving it to the back (most recent).
    fn touch_lru(&mut self, hash: u64) {
        if let Some(pos) = self.lru.iter().position(|&h| h == hash) {
            self.lru.remove(pos);
            self.lru.push_back(hash);
        }
    }

    /// Evict the least recently used image.
    fn evict_lru(&mut self) {
        if let Some(hash) = self.lru.pop_front() {
            if let Some(image) = self.images.remove(&hash) {
                self.total_bytes -= image.byte_size();
                self.id_to_hash.retain(|_, &mut h| h != hash);
            }
        }
    }

    /// Validate that an image's dimensions are within limits.
    pub fn validate_dimensions(width: u32, height: u32) -> bool {
        let pixels = u64::from(width) * u64::from(height);
        pixels <= MAX_SINGLE_IMAGE_PIXELS && width > 0 && height > 0
    }
}

/// Calculate the cell grid positions for an image placement.
///
/// Given an image's pixel dimensions and the terminal's cell size,
/// computes `ImageCellRef` entries for each cell the image occupies.
///
/// # Arguments
/// * `image_hash` - Content hash of the image in the ImageStore
/// * `image_width` - Image width in pixels
/// * `image_height` - Image height in pixels
/// * `cell_width` - Terminal cell width in pixels
/// * `cell_height` - Terminal cell height in pixels
/// * `placement_id` - Kitty placement ID (0 for non-Kitty)
/// * `z_index` - Z-index for layering
///
/// # Returns
/// A 2D vector of `ImageCellRef` entries, indexed as `[row][col]`.
pub fn assign_image_to_cells(
    image_hash: u64,
    image_width: u32,
    image_height: u32,
    cell_width: u32,
    cell_height: u32,
    placement_id: u32,
    z_index: i32,
) -> Vec<Vec<ImageCellRef>> {
    if cell_width == 0 || cell_height == 0 || image_width == 0 || image_height == 0 {
        return vec![];
    }

    // How many cells does the image span?
    let cols = (image_width + cell_width - 1) / cell_width;
    let rows = (image_height + cell_height - 1) / cell_height;

    let mut result = Vec::with_capacity(rows as usize);

    for row in 0..rows {
        let mut row_refs = Vec::with_capacity(cols as usize);
        for col in 0..cols {
            let tex_x = (col * cell_width) as f32 / image_width as f32;
            let tex_y = (row * cell_height) as f32 / image_height as f32;
            let tex_w = cell_width as f32 / image_width as f32;
            let tex_h = cell_height as f32 / image_height as f32;

            row_refs.push(ImageCellRef {
                image_hash,
                placement_id,
                tex_x: tex_x.min(1.0),
                tex_y: tex_y.min(1.0),
                tex_w: tex_w.min(1.0 - tex_x.min(1.0)),
                tex_h: tex_h.min(1.0 - tex_y.min(1.0)),
                z_index,
            });
        }
        result.push(row_refs);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_image(width: u32, height: u32) -> DecodedImage {
        let pixels = vec![0u8; (width * height * 4) as usize];
        let content_hash = ImageStore::content_hash(&pixels);
        DecodedImage {
            pixels,
            width,
            height,
            content_hash,
        }
    }

    fn make_unique_image(width: u32, height: u32, seed: u8) -> DecodedImage {
        let mut pixels = vec![seed; (width * height * 4) as usize];
        // Make each image unique by varying a pixel
        if !pixels.is_empty() {
            pixels[0] = seed;
        }
        let content_hash = ImageStore::content_hash(&pixels);
        DecodedImage {
            pixels,
            width,
            height,
            content_hash,
        }
    }

    #[test]
    fn test_image_store_basic() {
        let mut store = ImageStore::new(1024 * 1024);
        let image = make_test_image(10, 10);
        let hash = store.store(image);

        assert_eq!(store.image_count(), 1);
        assert!(store.get(hash).is_some());
        assert_eq!(store.total_bytes(), 10 * 10 * 4);
    }

    #[test]
    fn test_image_store_deduplication() {
        let mut store = ImageStore::new(1024 * 1024);

        // Store same image twice — should deduplicate
        let image1 = make_test_image(10, 10);
        let image2 = make_test_image(10, 10);
        assert_eq!(image1.content_hash, image2.content_hash);

        let hash1 = store.store(image1);
        let hash2 = store.store(image2);

        assert_eq!(hash1, hash2);
        assert_eq!(store.image_count(), 1);
        assert_eq!(store.total_bytes(), 10 * 10 * 4);
    }

    #[test]
    fn test_image_store_lru_eviction() {
        // Quota of 1000 bytes — each 5x5 image is 100 bytes
        let mut store = ImageStore::new(1000);

        // Store 10 images (1000 bytes total)
        let mut hashes = Vec::new();
        for i in 0..10u8 {
            let image = make_unique_image(5, 5, i);
            hashes.push(store.store(image));
        }
        assert_eq!(store.image_count(), 10);

        // Store one more — should evict the LRU (first one)
        let image = make_unique_image(5, 5, 100);
        store.store(image);

        assert_eq!(store.image_count(), 10);
        assert!(store.get(hashes[0]).is_none(), "oldest image should be evicted");
        assert!(store.get(hashes[1]).is_some(), "second image should still exist");
    }

    #[test]
    fn test_image_store_lru_touch() {
        // Quota fits exactly 3 images of 100 bytes each
        let mut store = ImageStore::new(300);

        let img0 = make_unique_image(5, 5, 0);
        let img1 = make_unique_image(5, 5, 1);
        let img2 = make_unique_image(5, 5, 2);

        let hash0 = store.store(img0);
        let hash1 = store.store(img1);
        let hash2 = store.store(img2);

        // Touch img0 (making img1 the LRU)
        let img0_dup = make_unique_image(5, 5, 0);
        store.store(img0_dup);

        // Store one more — should evict img1 (the LRU after touching img0)
        let img3 = make_unique_image(5, 5, 3);
        store.store(img3);

        assert!(store.get(hash0).is_some(), "img0 was touched, should survive");
        assert!(store.get(hash1).is_none(), "img1 was LRU, should be evicted");
        assert!(store.get(hash2).is_some(), "img2 should survive");
    }

    #[test]
    fn test_image_store_remove() {
        let mut store = ImageStore::new(1024 * 1024);
        let image = make_test_image(10, 10);
        let hash = store.store(image);

        assert_eq!(store.image_count(), 1);
        assert!(store.remove(hash));
        assert_eq!(store.image_count(), 0);
        assert_eq!(store.total_bytes(), 0);
    }

    #[test]
    fn test_image_store_id_mapping() {
        let mut store = ImageStore::new(1024 * 1024);
        let image = make_test_image(10, 10);

        let hash = store.store_with_id(42, image);
        assert!(store.get_by_id(42).is_some());
        assert_eq!(store.hash_for_id(42), Some(hash));

        store.remove_by_id(42);
        assert!(store.get_by_id(42).is_none());
    }

    #[test]
    fn test_image_store_upload_lifecycle() {
        let mut store = ImageStore::new(1024 * 1024);

        let upload = ImageUpload {
            image_id: 1,
            image_number: 0,
            data: vec![1, 2, 3],
            format: 32,
            width: 2,
            height: 2,
            compressed: false,
        };

        store.begin_upload(upload);
        store.append_upload_data(1, &[4, 5, 6]);

        let finished = store.finish_upload(1).unwrap();
        assert_eq!(finished.data, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_image_store_cancel_upload() {
        let mut store = ImageStore::new(1024 * 1024);

        let upload = ImageUpload {
            image_id: 1,
            image_number: 0,
            data: vec![1, 2, 3],
            format: 32,
            width: 2,
            height: 2,
            compressed: false,
        };

        store.begin_upload(upload);
        store.cancel_upload(1);
        assert!(store.finish_upload(1).is_none());
    }

    #[test]
    fn test_validate_dimensions() {
        assert!(ImageStore::validate_dimensions(100, 100));
        assert!(ImageStore::validate_dimensions(10000, 10000));
        assert!(!ImageStore::validate_dimensions(0, 100));
        assert!(!ImageStore::validate_dimensions(100, 0));
        // 100001 * 1000 = 100_001_000 > MAX_SINGLE_IMAGE_PIXELS
        assert!(!ImageStore::validate_dimensions(100_001, 1000));
    }

    #[test]
    fn test_assign_image_to_cells_basic() {
        // 16x16 image, 8x8 cells = 2x2 grid
        let refs = assign_image_to_cells(12345, 16, 16, 8, 8, 0, 0);

        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].len(), 2);
        assert_eq!(refs[1].len(), 2);

        // Top-left cell
        assert_eq!(refs[0][0].image_hash, 12345);
        assert!((refs[0][0].tex_x - 0.0).abs() < f32::EPSILON);
        assert!((refs[0][0].tex_y - 0.0).abs() < f32::EPSILON);
        assert!((refs[0][0].tex_w - 0.5).abs() < f32::EPSILON);
        assert!((refs[0][0].tex_h - 0.5).abs() < f32::EPSILON);

        // Top-right cell
        assert!((refs[0][1].tex_x - 0.5).abs() < f32::EPSILON);
        assert!((refs[0][1].tex_y - 0.0).abs() < f32::EPSILON);

        // Bottom-left cell
        assert!((refs[1][0].tex_x - 0.0).abs() < f32::EPSILON);
        assert!((refs[1][0].tex_y - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_assign_image_to_cells_non_aligned() {
        // 20x12 image, 8x8 cells = 3 cols x 2 rows
        let refs = assign_image_to_cells(999, 20, 12, 8, 8, 1, -1);

        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].len(), 3);
        assert_eq!(refs[0][0].placement_id, 1);
        assert_eq!(refs[0][0].z_index, -1);
    }

    #[test]
    fn test_assign_image_to_cells_zero_inputs() {
        assert!(assign_image_to_cells(0, 0, 0, 8, 8, 0, 0).is_empty());
        assert!(assign_image_to_cells(0, 10, 10, 0, 0, 0, 0).is_empty());
    }

    #[test]
    fn test_content_hash_deterministic() {
        let data = b"hello world";
        assert_eq!(ImageStore::content_hash(data), ImageStore::content_hash(data));
    }

    #[test]
    fn test_content_hash_different_data() {
        let data1 = b"hello";
        let data2 = b"world";
        assert_ne!(ImageStore::content_hash(data1), ImageStore::content_hash(data2));
    }

    #[test]
    fn test_next_image_id() {
        let mut store = ImageStore::new(1024);
        assert_eq!(store.next_image_id(), 1);
        assert_eq!(store.next_image_id(), 2);
        assert_eq!(store.next_image_id(), 3);
    }
}
