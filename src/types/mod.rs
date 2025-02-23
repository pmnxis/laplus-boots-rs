/*
 * SPDX-FileCopyrightText: © 2023 Jinwoo Park (pmnxis@gmail.com)
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

pub const CRC_POLY_INIT: u32 = 0xA097;
pub const BOOTLOADER_KEY: u32 = 0xB00710AD; // BOOTLOAD

pub mod const_convert;
pub mod ota;
pub mod section_mark;

// #[cfg(any(not(feature = "no_std"), feature = "std", test))]
pub(crate) mod std_crc;

#[inline]
#[allow(unused)]
unsafe fn __jump_to_bootloader(param: u32) -> ! {
    core::arch::asm!("mov r0, {0}", in(reg) param); // must be insure r0 is keep until bootstrap

    cortex_m::asm::bootload(section_mark::BOOTLOADER_ORIGIN as *const u32)
}

#[allow(unused)]
pub unsafe fn jump_to_bootloader() -> ! {
    __jump_to_bootloader(BOOTLOADER_KEY)
}

pub unsafe fn read_bootloader_param() -> u32 {
    let param: u32;
    // must be insure r0 is keep after bootstrap
    core::arch::asm!(
        "mov {0}, r0",
        out(reg) param
    );
    param
}

pub unsafe fn jump_to_app() -> ! {
    #[allow(unused_mut)]
    let mut p = cortex_m::Peripherals::steal();
    // #[cfg(not(armv6m))]
    // p.SCB.invalidate_icache();
    p.SCB.vtor.write(section_mark::REMAIN_OFFSET as u32);

    cortex_m::asm::bootload(section_mark::REMAIN_OFFSET as *const u32)
}
