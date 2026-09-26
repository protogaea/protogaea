//! yespower 1.0 in plain Rust, ported from the reference implementation (`yespower-ref.c` by
//! Alexander Peslyak, 2-clause BSD) and checked against its test vectors and against the C
//! implementation. It needs no C compiler and builds for WebAssembly, for the spark client in the
//! browser. Only yespower 1.0 is ported, not the older yescrypt 0.5 variant.
//!
//! The reference works on 32-bit words decoded little-endian. Here the working memory (V, X and
//! the S-boxes) holds pairs of them as `u64` (`lo | hi << 32`), which is the same bytes and what
//! pwxform needs; blocks go back to 32-bit words only for Salsa20. The results are the same on
//! every host.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

const PWX_SIMPLE: usize = 2;
const PWX_GATHER: usize = 4;
const PWX_ROUNDS: usize = 3;
const S_WIDTH: u32 = 11;
/// Word pairs in one of the three S-boxes.
const S_PAIRS: usize = (1 << S_WIDTH) * PWX_SIMPLE;
/// The S-box index mask, in bytes as in the reference.
const S_MASK: u32 = ((1 << S_WIDTH) - 1) * PWX_SIMPLE as u32 * 8;
const SALSA_ROUNDS: usize = 2;
/// Word pairs in a 64-byte block.
const BLOCK: usize = 8;

/// The Salsa20 core on 16 words (in the reference's shuffled order), with 2 rounds.
fn salsa20(b: &mut [u32; 16]) {
    let mut x = [0u32; 16];
    for i in 0..16 {
        x[i * 5 % 16] = b[i];
    }
    for _ in 0..SALSA_ROUNDS / 2 {
        let r = |a: u32, n: u32| a.rotate_left(n);
        x[4] ^= r(x[0].wrapping_add(x[12]), 7);
        x[8] ^= r(x[4].wrapping_add(x[0]), 9);
        x[12] ^= r(x[8].wrapping_add(x[4]), 13);
        x[0] ^= r(x[12].wrapping_add(x[8]), 18);
        x[9] ^= r(x[5].wrapping_add(x[1]), 7);
        x[13] ^= r(x[9].wrapping_add(x[5]), 9);
        x[1] ^= r(x[13].wrapping_add(x[9]), 13);
        x[5] ^= r(x[1].wrapping_add(x[13]), 18);
        x[14] ^= r(x[10].wrapping_add(x[6]), 7);
        x[2] ^= r(x[14].wrapping_add(x[10]), 9);
        x[6] ^= r(x[2].wrapping_add(x[14]), 13);
        x[10] ^= r(x[6].wrapping_add(x[2]), 18);
        x[3] ^= r(x[15].wrapping_add(x[11]), 7);
        x[7] ^= r(x[3].wrapping_add(x[15]), 9);
        x[11] ^= r(x[7].wrapping_add(x[3]), 13);
        x[15] ^= r(x[11].wrapping_add(x[7]), 18);
        x[1] ^= r(x[0].wrapping_add(x[3]), 7);
        x[2] ^= r(x[1].wrapping_add(x[0]), 9);
        x[3] ^= r(x[2].wrapping_add(x[1]), 13);
        x[0] ^= r(x[3].wrapping_add(x[2]), 18);
        x[6] ^= r(x[5].wrapping_add(x[4]), 7);
        x[7] ^= r(x[6].wrapping_add(x[5]), 9);
        x[4] ^= r(x[7].wrapping_add(x[6]), 13);
        x[5] ^= r(x[4].wrapping_add(x[7]), 18);
        x[11] ^= r(x[10].wrapping_add(x[9]), 7);
        x[8] ^= r(x[11].wrapping_add(x[10]), 9);
        x[9] ^= r(x[8].wrapping_add(x[11]), 13);
        x[10] ^= r(x[9].wrapping_add(x[8]), 18);
        x[12] ^= r(x[15].wrapping_add(x[14]), 7);
        x[13] ^= r(x[12].wrapping_add(x[15]), 9);
        x[14] ^= r(x[13].wrapping_add(x[12]), 13);
        x[15] ^= r(x[14].wrapping_add(x[13]), 18);
    }
    for i in 0..16 {
        b[i] = b[i].wrapping_add(x[i * 5 % 16]);
    }
}

/// Salsa20 of one 64-byte block held as word pairs.
fn salsa20_block(block: &mut [u64]) {
    let mut w = [0u32; 16];
    for (m, v) in block.iter().enumerate() {
        w[2 * m] = *v as u32;
        w[2 * m + 1] = (v >> 32) as u32;
    }
    salsa20(&mut w);
    for (m, v) in block.iter_mut().enumerate() {
        *v = u64::from(w[2 * m]) | u64::from(w[2 * m + 1]) << 32;
    }
}

fn xor(dst: &mut [u64], src: &[u64]) {
    dst.iter_mut().zip(src).for_each(|(d, s)| *d ^= s);
}

/// BlockMix_{salsa20, 1} over two blocks.
fn blockmix_salsa(b: &mut [u64]) {
    let mut x = [0u64; BLOCK];
    x.copy_from_slice(&b[BLOCK..2 * BLOCK]);
    for i in 0..2 {
        xor(&mut x, &b[i * BLOCK..(i + 1) * BLOCK]);
        salsa20_block(&mut x);
        b[i * BLOCK..(i + 1) * BLOCK].copy_from_slice(&x);
    }
}

/// The pwxform state: the three S-boxes in one buffer of word pairs, the offsets of S0, S1 and S2
/// in it, and the write position `w`.
struct Pwx<'a> {
    s: &'a mut [u64],
    s0: usize,
    s1: usize,
    s2: usize,
    w: usize,
}

impl Pwx<'_> {
    /// pwxform of one block: X[j][k] is `x[j * PWX_SIMPLE + k]`.
    fn pwxform(&mut self, x: &mut [u64]) {
        let (s0, s1) = (self.s0, self.s1);
        let s = &mut *self.s;
        let mut w = self.w;
        for i in 0..PWX_ROUNDS {
            for j in 0..PWX_GATHER {
                let at = j * PWX_SIMPLE;
                let first = x[at];
                let p0 = s0 + ((first as u32 & S_MASK) >> 3) as usize;
                let p1 = s1 + (((first >> 32) as u32 & S_MASK) >> 3) as usize;
                for k in 0..PWX_SIMPLE {
                    let v = x[at + k];
                    x[at + k] = ((v >> 32) * (v & 0xffff_ffff)).wrapping_add(s[p0 + k]) ^ s[p1 + k];
                }
                if i == 0 || j < PWX_GATHER / 2 {
                    if j & 1 == 1 {
                        s[s1 + w..s1 + w + PWX_SIMPLE].copy_from_slice(&x[at..at + PWX_SIMPLE]);
                        w += PWX_SIMPLE;
                    } else {
                        s[s0 + w..s0 + w + PWX_SIMPLE].copy_from_slice(&x[at..at + PWX_SIMPLE]);
                    }
                }
            }
        }
        (self.s0, self.s1, self.s2) = (self.s2, self.s0, self.s1);
        self.w = w & (S_PAIRS - 1);
    }

    /// BlockMix_pwxform over 2r blocks: each through pwxform, chained, then Salsa20/2 of the last
    /// one. (The reference's further Salsa20 loop is empty with these settings.)
    fn blockmix(&mut self, b: &mut [u64]) {
        let blocks = b.len() / BLOCK;
        let mut x = [0u64; BLOCK];
        x.copy_from_slice(&b[(blocks - 1) * BLOCK..]);
        for i in 0..blocks {
            let bi = &mut b[i * BLOCK..(i + 1) * BLOCK];
            if blocks > 1 {
                xor(&mut x, bi);
            }
            self.pwxform(&mut x);
            bi.copy_from_slice(&x);
        }
        salsa20_block(&mut b[(blocks - 1) * BLOCK..]);
    }
}

/// The low 32 bits of the last block's first word.
fn integerify(x: &[u64]) -> u32 {
    x[x.len() - BLOCK] as u32
}

fn wrap(x: u32, i: u32) -> u32 {
    let n = 1u32 << (31 - i.leading_zeros());
    (x & (n - 1)) + (i - n)
}

/// B (32-bit words) into X (word pairs), undoing the reference's SIMD shuffle.
fn shuffle_in(b: &[u32], x: &mut [u64]) {
    for (k, block) in x.chunks_mut(BLOCK).enumerate() {
        let w = |i: usize| u64::from(b[k * 16 + i * 5 % 16]);
        for (m, v) in block.iter_mut().enumerate() {
            *v = w(2 * m) | w(2 * m + 1) << 32;
        }
    }
}

fn shuffle_out(x: &[u64], b: &mut [u32]) {
    for (k, block) in x.chunks(BLOCK).enumerate() {
        for (m, v) in block.iter().enumerate() {
            b[k * 16 + (2 * m) * 5 % 16] = *v as u32;
            b[k * 16 + (2 * m + 1) * 5 % 16] = (v >> 32) as u32;
        }
    }
}

/// The first loop of SMix over N blocks of `x.len()` pairs. Without `pwx` it fills the S-boxes
/// (`v`) with BlockMix_salsa.
fn smix1(b: &mut [u32], n: u32, v: &mut [u64], x: &mut [u64], mut pwx: Option<&mut Pwx>) {
    let s = x.len();
    shuffle_in(b, x);
    if let Some(p) = pwx.as_deref_mut() {
        for k in 1..s / 16 {
            let (prev, cur) = x.split_at_mut(k * 16);
            cur[..16].copy_from_slice(&prev[(k - 1) * 16..]);
            p.blockmix(&mut cur[..16]);
        }
    }
    for i in 0..n {
        let at = i as usize * s;
        v[at..at + s].copy_from_slice(x);
        if i > 1 {
            let j = wrap(integerify(x), i) as usize;
            xor(x, &v[j * s..(j + 1) * s]);
        }
        match pwx.as_deref_mut() {
            Some(p) => p.blockmix(x),
            None => blockmix_salsa(x),
        }
    }
    shuffle_out(x, b);
}

fn smix2(b: &mut [u32], n: u32, nloop: u32, v: &mut [u64], x: &mut [u64], pwx: &mut Pwx) {
    let s = x.len();
    shuffle_in(b, x);
    for _ in 0..nloop {
        let j = (integerify(x) & (n - 1)) as usize;
        let vj = &mut v[j * s..(j + 1) * s];
        xor(x, vj);
        if nloop != 2 {
            vj.copy_from_slice(x);
        }
        pwx.blockmix(x);
    }
    shuffle_out(x, b);
}

fn hmac(key: &[u8], data: &[&[u8]]) -> [u8; 32] {
    let mut m = HmacSha256::new_from_slice(key).expect("HMAC takes any key length");
    for d in data {
        m.update(d);
    }
    m.finalize().into_bytes().into()
}

/// PBKDF2-HMAC-SHA256 with one iteration.
fn pbkdf2_1(password: &[u8], salt: &[u8], out: &mut [u8]) {
    for (i, chunk) in out.chunks_mut(32).enumerate() {
        let t = hmac(password, &[salt, &(i as u32 + 1).to_be_bytes()]);
        chunk.copy_from_slice(&t[..chunk.len()]);
    }
}

/// A yespower 1.0 hasher whose memory (128·N·r bytes and the S-boxes) is kept between hashes.
pub struct Hasher {
    n: u32,
    r: usize,
    pers: Vec<u8>,
    v: Vec<u64>,
    b: Vec<u32>,
    x: Vec<u64>,
    s: Vec<u64>,
}

impl Hasher {
    /// N must be a power of two from 1,024 to 524,288 and r from 8 to 32, as in the reference.
    pub fn new(n: u32, r: u32, pers: &[u8]) -> Self {
        assert!(
            (1024..=512 * 1024).contains(&n) && n.is_power_of_two() && (8..=32).contains(&r),
            "yespower parameters out of range"
        );
        let r = r as usize;
        Self {
            n,
            r,
            pers: pers.to_vec(),
            v: vec![0; 16 * r * n as usize],
            b: vec![0; 32 * r],
            x: vec![0; 16 * r],
            s: vec![0; 3 * S_PAIRS],
        }
    }

    pub fn hash(&mut self, input: &[u8]) -> [u8; 32] {
        let r = self.r;
        let prehash: [u8; 32] = Sha256::digest(input).into();
        let mut bytes = vec![0u8; 128 * r];
        pbkdf2_1(&prehash, &self.pers, &mut bytes);
        // The first 32 bytes of B before SMix are the message of the final HMAC.
        let head: [u8; 32] = bytes[..32].try_into().expect("32 bytes");
        for (w, c) in self.b.iter_mut().zip(bytes.as_chunks::<4>().0) {
            *w = u32::from_le_bytes(*c);
        }

        // SMix. Its first loop at r = 1 fills the S-boxes, 768 blocks of 128 bytes.
        smix1(
            &mut self.b,
            (3 * S_PAIRS / 16) as u32,
            &mut self.s,
            &mut self.x[..16],
            None,
        );
        let mut pwx = Pwx {
            s: &mut self.s,
            s0: 0,
            s1: S_PAIRS,
            s2: 2 * S_PAIRS,
            w: 0,
        };
        let n = self.n;
        let nloop_rw = (n.div_ceil(3) + 1) & !1;
        let nloop_all = nloop_rw;
        smix1(&mut self.b, n, &mut self.v, &mut self.x, Some(&mut pwx));
        smix2(&mut self.b, n, nloop_rw, &mut self.v, &mut self.x, &mut pwx);
        // The reference's second SMix2 runs `nloop_all - nloop_rw` = 0 loops for yespower 1.0.
        smix2(
            &mut self.b,
            n,
            nloop_all - nloop_rw,
            &mut self.v,
            &mut self.x,
            &mut pwx,
        );

        let mut last = [0u8; 64];
        for (c, w) in last
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(&self.b[32 * r - 16..])
        {
            *c = w.to_le_bytes();
        }
        hmac(&last, &[&head])
    }
}
