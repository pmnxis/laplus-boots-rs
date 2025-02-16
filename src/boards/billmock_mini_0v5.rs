/*
 * SPDX-FileCopyrightText: © 2023 Jinwoo Park (pmnxis@gmail.com)
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

//! Hardware initialization code for BillMock Hardware Version mini 0.4
//! The code follows on version mini 0.4 schematic
//! https://github.com/pmnxis/BillMock-HW-RELEASE/blob/master/sch/BillMock-Mini-HW-0v5.pdf

use core::cell::UnsafeCell;

use embassy_stm32::crc::{self, Crc};
use embassy_stm32::flash::Flash;
// use embassy_stm32::flash::FlashLayout;
// use embassy_stm32::gpio::{Input, Level, Output, Pin, Pull, Speed};
// use embassy_stm32::time::Hertz;
use embassy_stm32::usart::Uart;
use embassy_stm32::{bind_interrupts, peripherals};

use super::{Hardware, SharedResource};

bind_interrupts!(struct Irqs {
    USART2 => embassy_stm32::usart::InterruptHandler<peripherals::USART2>;
});

// static mut USART2_RX_BUF: [u8; 256] = [0u8; 256];

pub fn hardware_specific_init<'s>(
    p: embassy_stm32::Peripherals,
    _shared_resource: &'static SharedResource,
) -> Hardware<'s> {
    // USART2 initialization for CardReaderDevice
    // let usart2_rx_buf = unsafe { &mut *core::ptr::addr_of_mut!(USART2_RX_BUF) };

    let crc_config = crc::Config::new(
        crc::InputReverseConfig::Word,
        false,
        0xA097, // 0xFFFF_FFFF (init)
    )
    .unwrap_or_else(|_| panic!());

    let usart2_config = {
        let mut ret = embassy_stm32::usart::Config::default();
        ret.baudrate = 115200;
        ret.assume_noise_free = false;
        ret.detect_previous_overrun = true;
        ret
    };

    let (tx, rx) = Uart::new(
        p.USART2,
        p.PA3,
        p.PA2,
        Irqs,
        embassy_stm32::dma::NoDma,
        embassy_stm32::dma::NoDma,
        usart2_config,
    )
    .unwrap_or_else(|_| panic!())
    .split();

    Hardware {
        crc: UnsafeCell::new(Crc::new(p.CRC, crc_config)),
        flash: UnsafeCell::new(Flash::new_blocking(p.FLASH).into_blocking_regions()),
        rx: UnsafeCell::new(rx),
        tx: UnsafeCell::new(tx),
    }
}
