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

pub enum Key {
    /// Only transmit thorugh UART, usize is length to send
    Tx(usize),
    /// Reset after transmit thorugh UART, usize is length to send
    TxAndReset(usize),
    /// Jump to app region after transmit thorugh UART, usize is length to send
    TxAndJump(usize),
}

#[entry]
fn main() -> ! {
    let raw_boot_parm = unsafe { types::read_bootloader_param() };
    let mut board = boards::Board::init();
    let mut rx_buf: [u8; 1024] = [0; 1024];
    let mut tx_buf: [u8; REASONABLE_TX_BUF] = [0; REASONABLE_TX_BUF];

    let mut stacked: StackedBufferRxIndex = 0;
    let mut last_rx = Instant::now();

    // if there's any condition to settle on bootloader
    // otherwise jump to application
    if raw_boot_parm != types::BOOTLOADER_KEY {
        // Check Gpio
        for _ in 0..50 {
            if board.hardware.force_bootloader.is_high() {
                unsafe { types::jump_to_app() }
            }
            board.hardware.delay.delay_ms(1);
        }
    }

    loop {
        let rx_len = match board.hardware.rx.read(&mut rx_buf) {
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
                board.hardware.delay.delay_ms(1);
                last_rx = Instant::now();
                stacked = 0;

                continue;
            }
        };

        match crate::types::ota::test_packet(&rx_buf[..stacked + rx_len]) {
            Ok(cmd) => {
                let key = match cmd {
                    RequestForm::Handshake => Key::Tx(on_tx_buffer!(
                        tx_buf,
                        HandshakeForm,
                        HandshakeForm::response_new()
                    )),
                    RequestForm::DeviceInfo => Key::Tx(on_tx_buffer!(
                        tx_buf,
                        DeviceInfoResponseForm,
                        DeviceInfoResponseForm::new(&mut board)
                    )),
                    RequestForm::StartUpdate => Key::Tx(on_tx_buffer!(
                        tx_buf,
                        StartUpdateResponseForm,
                        StartUpdateResponseForm::new(&mut board)
                    )),
                    RequestForm::WriteChunk(chunk) => Key::Tx(on_tx_buffer!(
                        tx_buf,
                        WriteChunkResponseForm,
                        WriteChunkResponseForm::new(chunk.try_flash(&mut board))
                    )),
                    RequestForm::UpdateStatus => Key::Tx(on_tx_buffer!(
                        tx_buf,
                        UpdateStatusResponseForm,
                        UpdateStatusResponseForm::new(&mut board)
                    )),
                    RequestForm::Reset => {
                        Key::TxAndReset(on_tx_buffer!(tx_buf, ResetForm, ResetForm::response_new()))
                    }
                    RequestForm::JumpToApplication => Key::TxAndJump(on_tx_buffer!(
                        tx_buf,
                        JumpToApplicationForm,
                        JumpToApplicationForm::response_new()
                    )),
                };

                match key {
                    Key::Tx(x) => {
                        let _ = board.hardware.tx.write(&tx_buf[..x]);
                    }
                    Key::TxAndReset(x) => {
                        let _ = board.hardware.tx.write(&tx_buf[..x]);

                        cortex_m::peripheral::SCB::sys_reset();
                    }
                    Key::TxAndJump(x) => {
                        let _ = board.hardware.tx.write(&tx_buf[..x]);

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
