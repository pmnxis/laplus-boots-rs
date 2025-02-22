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
use embassy_time::{Duration, Instant};
use embedded_io::*;
// use hex_literal::hex;
use panic_abort as _;

use crate::boards::Board;
use crate::types::ota::*;

type StackedBufferRxIndex = usize;

const WAIT_DURATION_RX: Duration = Duration::from_millis(200); // heuristic value

pub enum Key<'d> {
    Tx(&'d [u8]),
    TxAndReset(&'d [u8]),
    TxAndJump(&'d [u8]),
}

#[entry]
fn main() -> ! {
    let raw_boot_parm = unsafe { types::read_bootloader_param() };
    let board = make_static!(Board, boards::Board::init());
    let mut rx_buf: [u8; 1024] = [0; 1024];

    let delay = unsafe { &mut *board.hardware.delay.get() };
    let rx = unsafe { &mut *board.hardware.rx.get() };
    let tx = unsafe { &mut *board.hardware.tx.get() };
    let force_pin = unsafe { &mut *board.hardware.force_bootloader.get() };

    let mut stacked: StackedBufferRxIndex = 0;
    let mut last_rx = Instant::now();

    // if there's any condition to settle on bootloader
    // otherwise jump to application
    if raw_boot_parm != types::BOOTLOADER_KEY {
        unsafe { types::jump_to_app() }
    } else {
        for _ in 0..50 {
            if force_pin.is_high() {
                unsafe { types::jump_to_app() }
            }
            delay.delay_ms(1);
        }
    }

    loop {
        let rx_len = match rx.read(&mut rx_buf) {
            Ok(0) => {
                let now = Instant::now();
                // when hang too much, just reset uart stacking
                if (now - last_rx) > WAIT_DURATION_RX {
                    last_rx = now;
                    stacked = 0;
                }
                continue;
            }
            Ok(n) => n,
            Err(_) => {
                last_rx = Instant::now();
                stacked = 0;
                continue;
            }
        };

        match crate::types::ota::test_packet(&rx_buf[..stacked + rx_len]) {
            Ok(cmd) => {
                let key = match cmd {
                    RequestForm::Handshake => Key::Tx(as_bytes!(&HandshakeForm::response_new())),
                    RequestForm::DeviceInfo => {
                        Key::Tx(as_bytes!(&DeviceInfoResponseForm::new(board)))
                    }
                    RequestForm::StartUpdate => {
                        Key::Tx(as_bytes!(&StartUpdateResponseForm::new(board)))
                    }
                    RequestForm::WriteChunk(chunk) => Key::Tx(as_bytes!(
                        &WriteChunkResponseForm::new(chunk.try_flash(board))
                    )),
                    RequestForm::UpdateStatus => {
                        Key::Tx(as_bytes!(&UpdateStatusResponseForm::new(board)))
                    }
                    RequestForm::Reset => Key::TxAndReset(as_bytes!(&ResetForm::response_new())),
                    RequestForm::JumpToApplication => {
                        Key::TxAndJump(as_bytes!(&JumpToApplicationForm::response_new()))
                    }
                };

                match key {
                    Key::Tx(x) => {
                        let _ = tx.write(x);
                    }
                    Key::TxAndReset(x) => {
                        let _ = tx.write(x);
                        cortex_m::peripheral::SCB::sys_reset();
                    }
                    Key::TxAndJump(x) => {
                        let _ = tx.write(x);
                        unsafe { types::jump_to_app() }
                    }
                }
            }
            Err(e) => {
                if e == OtaError::OutOfRange {
                    stacked += rx_len;
                } else {
                    stacked = 0;
                }
            }
        }

        last_rx = Instant::now();
    }
}
