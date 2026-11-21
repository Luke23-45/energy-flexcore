// FIM-F02 — Hand-written SHA-1 (FIPS 180-4 shape), pinned by test vectors.
//
// HONESTY HEADER (FIM standing decision 1,
// docs/releases/implementation_ledger/README.md): this is a hand-written
// minimal codec added under FIM-F02 because the RFC 6455
// |Sec-WebSocket-Accept| computation REQUIRES SHA-1 exactly (no
// substitute hash is permitted by the RFC). The committed `Cargo.lock`
// resolves a `sha1` crate (0.11.0) only transitively; its 0.11-line API
// could NOT be verified from inside this repository (no build, no
// downloads, no documentation access — the FIM hard boundary), so the
// algorithm is implemented HERE as a small pinned module instead of
// taking an unverifiable dependency. The implementation below is the
// textbook FIPS 180-4 compression function (padded message schedule,
// 80-step rounds with the four round functions and the four 32-bit
// constants); it is pinned by the FIPS 180-4 / RFC 6455 test vectors in
// the tests module and is used ONLY for the RFC 6455 handshake accept
// key.
//
// NOT A CRYPTOGRAPHIC-STRENGTH CLAIM: SHA-1 is collision-broken and is
// acceptable here ONLY because RFC 6455 mandates it for the handshake
// integrity check, where it provides no security property at all (the
// handshake is not an authentication mechanism). Nothing in this crate
// uses SHA-1 for authentication, signatures, or certificates.
//
// WRITTEN-UNRUN: this module has never compiled or executed; execution is
// a HUMAN LAB GATE (conventions §1.3). The vectors below are published
// constants (FIPS 180-4 examples and the RFC 6455 §1.3 example key) —
// they pin the implementation, they are not a conformance claim about
// anything else.

/// Computes the 20-byte SHA-1 digest of `message` (FIPS 180-4).
pub fn sha1(message: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];

    // Message padding: 0x80, zeros, 64-bit big-endian bit length.
    let bit_length = (message.len() as u64).wrapping_mul(8);
    let mut padded = message.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());

    // Compression: one 512-bit block at a time.
    for block in padded.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, word) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];

        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut digest = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        digest[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn fips_180_4_vectors() {
        // The FIPS 180-4 / RFC 3174 published vectors.
        assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            hex(&sha1(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        // The empty message.
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        // A message longer than one block (forces multi-block padding).
        assert_eq!(
            hex(&sha1(&[b'a'; 1_000])),
            "291e9a6c66994949b57ba5e650361e98fc36b1ba"
        );
    }

    #[test]
    fn rfc_6455_example_accept_key_vector() {
        // RFC 6455 §1.3: the example client key and the expected accept
        // value. This is the handshake vector the transport pins.
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let accept_input = format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
        assert_eq!(
            hex(&sha1(accept_input.as_bytes())),
            "b37a4f2cc0624f1690f64606cf385945b2bec4ea"
        );
    }
}
