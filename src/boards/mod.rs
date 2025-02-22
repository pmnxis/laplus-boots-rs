/*
 * SPDX-FileCopyrightText: © 2023 Jinwoo Park (pmnxis@gmail.com)
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

use core::cell::UnsafeCell;

use chacha20::{cipher::KeyIvInit, ChaCha20};
use embassy_stm32::crc::Crc;
use embassy_stm32::flash::FlashLayout;
use embassy_stm32::gpio::{AnyPin, Input};
use embassy_stm32::peripherals;
use embassy_stm32::usart::{BufferedUartRx, BufferedUartTx};

// #[cfg(feature = "hw_0v2")]
// use self::billmock_0v2::hardware_init_0v2;
#[cfg(feature = "hw_billmock_mini_0v5")]
use self::billmock_mini_0v5::*;
use crate::types::section_mark::SectionMark;

// #[cfg(feature = "hw_0v2")]
// mod billmock_0v2;
#[cfg(feature = "hw_billmock_mini_0v5")]
mod billmock_mini_0v5;

#[allow(dead_code)]
pub mod const_str;

#[allow(dead_code)]
pub struct Hardware<'s> {
    pub delay: UnsafeCell<cortex_m::delay::Delay>,
    pub crc: UnsafeCell<Crc<'static>>,
    pub flash: UnsafeCell<FlashLayout<'s, embassy_stm32::flash::Blocking>>,
    pub tx: UnsafeCell<BufferedUartTx<'static, peripherals::USART2>>,
    pub rx: UnsafeCell<BufferedUartRx<'static, peripherals::USART2>>,
    pub force_bootloader: UnsafeCell<Input<'static, AnyPin>>,
}

impl Hardware<'static> {
    /// Initialize MCU PLL and CPU on init hardware
    pub fn mcu_pre_init() -> embassy_stm32::Peripherals {
        embassy_stm32::init(Default::default())
    }

    /// Initialize MCU peripherals and nearby components
    #[inline]
    fn hardware_init<'s>(peripherals: embassy_stm32::Peripherals) -> Hardware<'s> {
        hardware_specific_init(peripherals)
    }
}

pub struct SharedResource {
    pub cipher: ChaCha20,
    pub section_mark: SectionMark,
}

impl SharedResource {
    /// Initialize necessary shared resource
    fn init() -> Self {
        let key = [0x42; 32]; // fill any key.
        let nonce = crypto_nonce();

        Self {
            cipher: ChaCha20::new(&key.into(), &nonce.into()),
            section_mark: SectionMark::new(),
        }
    }
}

#[allow(dead_code)]
pub struct Board<'s> {
    pub hardware: Hardware<'s>,
    pub shared_resource: SharedResource,
}

impl Board<'static> {
    pub fn init() -> Self {
        let peripherals = Hardware::mcu_pre_init();

        let hardware: Hardware = Hardware::hardware_init(peripherals);
        let shared_resource = SharedResource::init();

        Self {
            hardware,
            shared_resource,
        }
    }

    pub fn get_nonce() -> [u8; 12] {
        crypto_nonce()
    }

    pub fn get_serial_number() -> [u8; 12] {
        serial_number()
    }
}
