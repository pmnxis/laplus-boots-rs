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
#![feature(inline_const_pat)]
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
use embedded_io::Read;
// use embedded_io::*;
#[allow(unused_imports)]
use hex_literal::hex;
use panic_abort as _;

use crate::boards::Board;
use crate::types::ota::RequestForm;

#[entry]
fn main() -> ! {
    let board = make_static!(Board, boards::Board::init());
    let mut rx_buf: [u8; 512] = [0; 512];

    let rx = unsafe { &mut *board.hardware.rx.get() };

    loop {
        // this may not work. cortex-m0+ not support un-anligned access on 16/32bit memory.

        let _n = rx.read(&mut rx_buf);

        if let Ok(cmd) = crate::types::ota::test_packet(&rx_buf) {
            match cmd {
                RequestForm::WriteChunk(chunk) => {
                    let _k = chunk.try_flash(board);
                }
                _ => todo!(),
            }
        }
    }
}
