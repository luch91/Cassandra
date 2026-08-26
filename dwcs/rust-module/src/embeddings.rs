
//! Embedding table for semantic scoring.
//!
//! 12k-word GloVe 50d vocabulary quantized to int8, addressed by FNV-1a32
//! word hashes in an open-addressing table. Blobs are embedded with
//! include_bytes! so the module stays self-contained (no filesystem access).
//! All arithmetic is fixed-shape and allocation-free.

pub const D: usize = 50;
pub const TABLE_SIZE: usize = 16384;

static KEYS: &[u8] = include_bytes!("embeddings/keys.bin");
static VALS: &[u8] = include_bytes!("embeddings/vals.bin");
static VECS: &[u8] = include_bytes!("embeddings/vectors.bin");

/// Global dequantization scale (max |value| / 127 from training time).
const SCALE: f32 = 0.042986613;

fn read_u32_le(bytes: &[u8], index: usize) -> u32 {
    let o = index * 4;
    u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]])
}

fn fnv1a32(word: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in word.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    if h == 0 {
        0xffff_ffff
    } else {
        h
    }
}

/// Returns the dequantized 50-dim vector for `word`, or None if OOV.
pub fn lookup(word: &str) -> Option<[f32; D]> {
    let w = word.trim_matches(|c: char| c.is_ascii_punctuation());
    if w.is_empty() {
        return None;
    }
    // ascii-lowercase in place (table was built on lowercase ascii tokens)
    let bytes = w.as_bytes();
    let mut buf = [0u8; 32];
    let len = if bytes.len() > 32 { 32 } else { bytes.len() };
    for i in 0..len {
        let mut b = bytes[i];
        if b.is_ascii_uppercase() {
            b += 32;
        }
        buf[i] = b;
    }
    let lower = core::str::from_utf8(&buf[..len]).unwrap_or(w);
    let h = fnv1a32(lower);
    let mut slot = (h as usize) & (TABLE_SIZE - 1);
    for _ in 0..64 {
        let key = read_u32_le(KEYS, slot);
        if key == 0 {
            return None;
        }
        if key == h {
            let idx = u16::from_le_bytes([VALS[slot * 2], VALS[slot * 2 + 1]]) as usize;
            if idx == 0 {
                return None;
            }
            let base = (idx - 1) * D;
            let mut out = [0f32; D];
            for d in 0..D {
                out[d] = VECS[base + d] as i8 as f32 * SCALE;
            }
            return Some(out);
        }
        slot = (slot + 1) & (TABLE_SIZE - 1);
    }
    None
}
