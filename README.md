<!--
SPDX-FileCopyrightText: © 2025 Jinwoo Park (pmnxis@gmail.com)

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# `laplus-boots-rs`

 <span style="font-size: 60px;"><img src="https://hololive.hololivepro.com/wp-content/uploads/2020/07/La-Darknesss_pr-img_04.png" alt="darkness (image's copyright is under  COVER company)" width="200">🐦‍⬛👢🦀</span>

### **Rust Embedded Firmware Bootloader Proof of Concept**
- Receives ChaCha20-encrypted binary data over UART for firmware updates.  
- Targets **STM32G030C8**<sup>[1](#footnote_1)</sup> (64KiB Flash), utilizing only **8KiB** for the bootloader, maximizing the remaining **56KiB** for firmware storage.
- Due to size constraints, it is a bare-metal Rust embedded implementation, leveraging **Embassy-rs**<sup>[2](#footnote_2)</sup>' STM32 HAL. Relies on panic_abort (defmt and RTT are cannot be utilized).

### State Diagram

```mermaid
---
title: laplus-boots-rs
---
stateDiagram-v2
    [*] --> Bootloader

    Bootloader: 0x0800_0000 Bootloader
    Bootloader --> Init : Backup r0 as boot parm
    Init --> OtaCheck
    OtaCheck --> BootPinCheck
    BootPinCheck --> OtaProc: Yes
    BootPinCheck --> Application: No

    SoftReset --> Init
    OtaCheck: Check boot parm
    
    OtaProc: OTA Procedure
    state OtaProc {
        [*] --> Handshake
        [*] --> DeviceInfo
        [*] --> StartUpdate
        [*] --> WriteChunk
        [*] --> UpdateStatus
        [*] --> JumpApp
        [*] --> SoftReset
    }
    JumpApp --> Application

    Application: 0x0800_2000 App
    SoftReset: Soft Reset
```

## Footnote
<a name="footnote_1">1</a> `STM32G030C8` is STMicroelectronics' MCU with ARM-Cortex M0+ , 64KiB Flash and 8KiB SRAM. <br>
( https://www.st.com/en/microcontrollers-microprocessors/stm32g030c8.html ) <br><br>

<a name="footnote_2">2</a> `embassy-rs` is rust embedded framework<br>
( https://github.com/embassy-rs/embassy )<br><br>
