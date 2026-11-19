//! A corrupt scan decodes as C libjpeg-turbo decodes it. A scan that ends before its last MCU: the
//! MCU in which the data ends decodes from zero bits, and every MCU after it holds zero
//! coefficients, which a baseline file shows as mid gray, past every restart interval: no RST
//! marker follows the end. A code that no Huffman table holds: it
//! is symbol 0 after 17 bits, and the decode goes on.

use libjpeg_turbo_rs::encode::pipeline::{compress_with_params, CompressParams};
use libjpeg_turbo_rs::{compress, decompress, Decoder, PixelFormat, Subsampling};

const SIDE: usize = 64;

fn pattern() -> Vec<u8> {
    let mut pixels: Vec<u8> = Vec::with_capacity(SIDE * SIDE * 3);
    for y in 0..SIDE {
        for x in 0..SIDE {
            pixels.push(((x * 5 + y * 3) % 256) as u8);
            pixels.push(((x * 3 + y * 7) % 256) as u8);
            pixels.push(((x * 7 + y * 11) % 256) as u8);
        }
    }
    pixels
}

/// The offset of the first entropy-coded byte, after the SOS segment.
fn scan_start(jpeg: &[u8]) -> usize {
    let sos: usize = jpeg
        .windows(2)
        .position(|marker| marker == [0xFF, 0xDA])
        .expect("an SOS marker");
    sos + 2 + usize::from(u16::from_be_bytes([jpeg[sos + 2], jpeg[sos + 3]]))
}

fn rows_of(jpeg: &[u8]) -> Vec<u8> {
    let decoder: Decoder = Decoder::new(jpeg).expect("a header");
    let mut data: Vec<u8> = vec![0; SIDE * SIDE * 3];
    let mut rows: Vec<&mut [u8]> = data.chunks_exact_mut(SIDE * 3).collect();
    decoder
        .decode_image_into_rows(&mut rows)
        .expect("a decode of rows");
    data
}

#[test]
fn a_scan_cut_short_decodes_its_missing_mcus_to_gray() {
    let pixels: Vec<u8> = pattern();
    for restart in [0, 3] {
        let params =
            CompressParams::new(&pixels, SIDE, SIDE, PixelFormat::Rgb, 90, Subsampling::S420)
                .restart_interval(restart);
        let clean: Vec<u8> = compress_with_params(&params).expect("an encode");
        let start: usize = scan_start(&clean);
        let cut: usize = start + (clean.len() - 2 - start) / 2;
        let short: Vec<u8> = [&clean[..cut], &[0xFF, 0xD9][..]].concat();

        let whole: Vec<u8> = decompress(&clean).expect("a clean decode").data;
        let row: usize = SIDE * 3;
        for (name, data) in [
            ("decode", decompress(&short).expect("a decode").data),
            ("rows", rows_of(&short)),
        ] {
            assert_eq!(
                data[..row],
                whole[..row],
                "{name}, restart {restart}: the first row"
            );
            assert!(
                data[data.len() - row..].iter().all(|&sample| sample == 128),
                "{name}, restart {restart}: the last row is mid gray"
            );
        }
    }
}

#[test]
fn a_code_no_table_holds_decodes_on() {
    let clean: Vec<u8> = compress(
        &pattern(),
        SIDE,
        SIDE,
        PixelFormat::Rgb,
        90,
        Subsampling::S420,
    )
    .expect("an encode");
    let start: usize = scan_start(&clean);
    let at: usize = start + (clean.len() - 2 - start) / 2;
    let ones: [u8; 16] = [0xFF, 0x00].repeat(8).try_into().expect("16 bytes");
    let corrupt: Vec<u8> = [&clean[..at], &ones[..], &clean[at..]].concat();

    let whole: Vec<u8> = decompress(&clean).expect("a clean decode").data;
    let row: usize = SIDE * 3;
    for (name, data) in [
        ("decode", decompress(&corrupt).expect("a decode").data),
        ("rows", rows_of(&corrupt)),
    ] {
        assert_eq!(data.len(), whole.len(), "{name}: the size");
        assert_eq!(data[..row], whole[..row], "{name}: the first row");
    }
}
