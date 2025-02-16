/*
 * SPDX-FileCopyrightText: © 2023 Jinwoo Park (pmnxis@gmail.com)
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

// #![deny(unsafe_code)]
// #![deny(warnings)]
#![feature(const_trait_impl)]
#![feature(async_fn_in_trait)]
#![allow(stable_features)]
#![feature(type_alias_impl_trait)]
#![feature(impl_trait_in_assoc_type)]
#![no_main]
#![no_std]

// use defmt_rtt as _;
// use embassy_stm32::gpio::{Level, Output, Speed};
// use {defmt_rtt as _, panic_probe as _};
// use rtic::app;
// use rtic_monotonics::systick::prelude::*;

// systick_monotonic!(Mono, 1_000);

pub mod pac {
    pub use embassy_stm32::pac::Interrupt as interrupt;
    pub use embassy_stm32::pac::*;
}

mod boards;

pub(crate) mod types;

#[allow(unused_imports)]
use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherCoreWrapper, StreamCipherSeek};
#[allow(unused_imports)]
use chacha20::ChaCha20;
use cortex_m_rt::entry;
#[allow(unused_imports)]
use hex_literal::hex;
use panic_abort as _;

use crate::boards::Board;

#[repr(u32)]
pub enum LaplusBootsCmd {
    Ack,
    GetSerialNumber,
    FlashStart,
    FlashContinuous,
    FlashFinished,
}

pub struct FlashStart {
    pub nonce: [u32; 12],
}

pub struct FlashContinuousBlock {
    pub addr: u32,
    pub checksum: u32,
    pub data: [u8; 128],
}

impl FlashContinuousBlock {
    fn try_flash(&self, board: &Board) -> bool {
        let flash = unsafe { &mut *board.hardware.flash.get() };
        let crc = unsafe { &mut *board.hardware.crc.get() };
        let cipher = unsafe { &mut *board.shared_resource.cipher.get() };

        let mut data = self.data;
        let actual = crc.feed_words(unsafe {
            core::mem::transmute::<&[u8], &[u32]>(&data)
            // core::slice::from_raw_parts((&data as *const _) as *const u32, 128 / core::mem::size_of::<u32>())
        });

        if actual != self.checksum {
            return false;
        }

        cipher.apply_keystream(&mut data); // decrypt

        let _ = flash.bank1_region.blocking_write(self.addr, &data);

        true
    }
}

#[entry]
fn main() -> ! {
    let board = make_static!(Board, boards::Board::init());
    let mut rx_buf: [u8; 512] = [0; 512];

    let rx = unsafe { &mut *board.hardware.rx.get() };

    loop {
        // this may not work. cortex-m0+ not support un-anligned access on 16/32bit memory.
        let _n = rx.blocking_read(&mut rx_buf);
        let _a: &FlashContinuousBlock =
            unsafe { &*(rx_buf.as_ptr() as *const FlashContinuousBlock) };
        let _b = _a.try_flash(board);
    }
}
