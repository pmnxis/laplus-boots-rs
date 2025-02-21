/*
 * SPDX-FileCopyrightText: © 2023 Jinwoo Park (pmnxis@gmail.com)
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

#![feature(const_trait_impl)]
#![feature(async_fn_in_trait)]
#![allow(stable_features)]
#![feature(type_alias_impl_trait)]
#![feature(impl_trait_in_assoc_type)]
#![feature(inline_const_pat)]
#![no_main]
#![no_std]

pub mod pac {
    pub use embassy_stm32::pac::Interrupt as interrupt;
    pub use embassy_stm32::pac::*;
}

mod boards;

pub(crate) mod types;

use cortex_m_rt::entry;
use embedded_io::*;
// use hex_literal::hex;
use panic_abort as _;

use crate::boards::Board;
use crate::types::ota::*;

#[entry]
fn main() -> ! {
    let board = make_static!(Board, boards::Board::init());
    let mut rx_buf: [u8; 1024] = [0; 1024];

    let rx = unsafe { &mut *board.hardware.rx.get() };
    let tx = unsafe { &mut *board.hardware.tx.get() };

    loop {
        let _n = rx.read(&mut rx_buf);

        if let Ok(cmd) = crate::types::ota::test_packet(&rx_buf) {
            match cmd {
                RequestForm::Handshake => {
                    let _ = tx.write(as_bytes!(&HandshakeForm::response_new()));
                }
                RequestForm::DeviceInfo => {
                    let _ = tx.write(as_bytes!(&DeviceInfoResponseForm::new(board)));
                }
                RequestForm::StartUpdate => {
                    let _ = tx.write(as_bytes!(&StartUpdateResponseForm::new(board)));
                }
                RequestForm::WriteChunk(chunk) => {
                    let _ = tx.write(as_bytes!(&WriteChunkResponseForm::new(
                        chunk.try_flash(board)
                    )));
                }
                RequestForm::UpdateStatus => {
                    let _ = tx.write(as_bytes!(&UpdateStatusResponseForm::new(board)));
                }
                RequestForm::Reset => {
                    let _ = tx.write(as_bytes!(&ResetForm::response_new()));
                    cortex_m::peripheral::SCB::sys_reset();
                }
            }
        }
    }
}
