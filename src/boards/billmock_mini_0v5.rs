/*
 * SPDX-FileCopyrightText: © 2023 Jinwoo Park (pmnxis@gmail.com)
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

//! Hardware initialization code for BillMock Hardware Version mini 0.4
//! The code follows on version mini 0.4 schematic
//! https://github.com/pmnxis/BillMock-HW-RELEASE/blob/master/sch/BillMock-Mini-HW-0v5.pdf

use embassy_stm32::crc::{self, Crc};
use embassy_stm32::flash::Flash;
use embassy_stm32::gpio::{Input, Pin, Pull};
use embassy_stm32::usart::BufferedUart;
use embassy_stm32::{bind_interrupts, peripherals};

use super::Hardware;

bind_interrupts!(struct Irqs {
    USART2 => embassy_stm32::usart::BufferedInterruptHandler<peripherals::USART2>; // InterruptHandler
});

static mut UART_RX_BUF: [u8; 1024] = [0u8; 1024];
static mut UART_TX_BUF: [u8; 512] = [0u8; 512];

pub fn hardware_specific_init<'s>(p: embassy_stm32::Peripherals) -> Hardware<'s> {
    let delay = cortex_m::delay::Delay::new(
        unsafe { cortex_m::Peripherals::steal().SYST },
        embassy_stm32::rcc::HSI_FREQ.0,
    );

    // USART2 initialization for CardReaderDevice
    let usart_rx_buf = unsafe { &mut *core::ptr::addr_of_mut!(UART_RX_BUF) };
    let usart_tx_buf = unsafe { &mut *core::ptr::addr_of_mut!(UART_TX_BUF) };

    let crc_config = crc::Config::new(
        crc::InputReverseConfig::Word,
        false,
        crate::types::CRC_POLY_INIT,
    )
    .unwrap_or_else(|_| panic!());

    let usart2_config = {
        let mut ret = embassy_stm32::usart::Config::default();
        ret.baudrate = 115200;
        ret.assume_noise_free = false;
        ret.detect_previous_overrun = true;
        ret
    };

    let (tx, rx) = BufferedUart::new(
        p.USART2,
        Irqs,
        p.PA3,
        p.PA2,
        usart_tx_buf,
        usart_rx_buf,
        usart2_config,
    )
    .unwrap_or_else(|_| panic!())
    .split();

    let force_bootloader = Input::new(p.PC6.degrade(), Pull::Up);

    Hardware {
        delay,
        crc: Crc::new(p.CRC, crc_config),
        flash: Flash::new_blocking(p.FLASH).into_blocking_regions(),
        rx: rx,
        tx: tx,
        force_bootloader,
    }
}

pub(crate) fn serial_number() -> [u8; 12] {
    billmock_otp_dev_info::OtpDeviceInfo::from_stm32g0().dev_sn
}

pub(crate) fn crypto_nonce() -> [u8; 12] {
    // Use RNG when HW support it
    serial_number()
}
