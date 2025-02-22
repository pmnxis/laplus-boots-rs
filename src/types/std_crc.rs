/*
 * SPDX-FileCopyrightText: © 2025 Jinwoo Park (pmnxis@gmail.com)
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

use crc::{Algorithm, Crc};
const LAPLUS_CRC: Algorithm<u32> = Algorithm {
    width: 32,
    poly: 0x4C11DB7,
    init: super::CRC_POLY_INIT,
    refin: true,
    refout: false,
    xorout: 0x0000,
    check: 0,
    residue: 0x0000,
};

pub fn std_crc(bytes: &[u8]) -> u32 {
    let crc = Crc::<u32>::new(&LAPLUS_CRC);
    let mut digest = crc.digest();
    digest.update(bytes);
    digest.finalize()
}
