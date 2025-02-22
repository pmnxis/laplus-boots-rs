/*
 * SPDX-FileCopyrightText: © 2025 Jinwoo Park (pmnxis@gmail.com)
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

pub(crate) const BOOTLOADER_ORIGIN: usize = env_to_array::hex_env_to_usize!("FLASH_ORIGIN");
const BOOTLOADER_LENGTH: usize = env_to_array::hex_env_to_usize!("FLASH_LENGTH");
const FLASH_BASE: usize = embassy_stm32::flash::FLASH_BASE;
const FLASH_SIZE: usize = embassy_stm32::flash::FLASH_SIZE;

pub const REMAIN_OFFSET: usize = BOOTLOADER_ORIGIN + BOOTLOADER_LENGTH - FLASH_BASE;
const REMAIN_SIZE: usize = FLASH_SIZE - REMAIN_OFFSET;

pub const WRITE_CHUNK_SIZE: usize = 256;
pub const CHUNK_BIT_IDX: usize = WRITE_CHUNK_SIZE.trailing_zeros() as usize;
const BYTE_BIT_IDX: usize = 8_u8.trailing_zeros() as usize;
const MAX_PAGE: usize = REMAIN_SIZE / WRITE_CHUNK_SIZE;
const PAGE_BITMAP_SIZE: usize = (MAX_PAGE + 7) / 8;

#[repr(C)]
#[derive(Clone, PartialEq, Eq)]
pub struct SectionMark {
    pub bitmap: [u8; PAGE_BITMAP_SIZE],
}

impl SectionMark {
    pub fn new() -> Self {
        Self {
            bitmap: [0u8; PAGE_BITMAP_SIZE],
        }
    }

    pub fn mark_offset(&mut self, offset: u32) {
        let p = offset - (REMAIN_OFFSET as u32);
        self.bitmap[(p as usize) >> (CHUNK_BIT_IDX + BYTE_BIT_IDX)] |=
            1 << ((p >> CHUNK_BIT_IDX) & 0x7);
    }

    #[allow(unused)]
    pub fn unmark_offset(&mut self, offset: u32) {
        let p = offset - (REMAIN_OFFSET as u32);
        self.bitmap[(p as usize) >> (CHUNK_BIT_IDX + BYTE_BIT_IDX)] &=
            !(1 << ((p >> CHUNK_BIT_IDX) & 0x7));
    }

    #[allow(unused)]
    pub fn clear(&mut self) {
        for i in 0..PAGE_BITMAP_SIZE {
            self.bitmap[i] = 0;
        }
    }

    #[allow(unused)]
    pub fn popcount(&self) -> usize {
        let mut ret = 0;
        for d in self.bitmap {
            ret += d.count_zeros();
        }
        ret as usize
    }
}
