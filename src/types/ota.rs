/*
 * SPDX-FileCopyrightText: © 2025 Jinwoo Park (pmnxis@gmail.com)
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

use core::borrow::BorrowMut;

use chacha20::cipher::StreamCipher;
use embassy_stm32::flash::WRITE_SIZE;

use super::section_mark::{SectionMark, CHUNK_BIT_IDX, WRITE_CHUNK_SIZE};
use crate::Board;

pub const EOF_SIGNATURE: u8 = 0xFF;
pub const PROTOCOL_VERSION_BYTE: u8 = 0x01;
pub const REASONABLE_TX_BUF: usize = (response_packet_max_size() + 15) / 8; // 8bytes padding

#[macro_export]
macro_rules! on_tx_buffer {
    ($tx_buf:expr, $type:ty, $val:expr) => {{
        #[allow(clippy::macro_metavars_in_unsafe)]
        unsafe {
            ($tx_buf.as_mut_ptr() as *mut $type).write($val);
        }
        core::mem::size_of::<$type>()
    }};
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum Command {
    Handshake = 0x01,
    DeviceInfo = 0x02,
    StartUpdate = 0x30,
    WriteChunk = 0x40,
    UpdateStatus = 0xE0,
    Reset = 0xF0,
    JumpToApplication = 0xF1,
}

impl TryFrom<u8> for Command {
    type Error = OtaError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        // #![feature(inline_const_pat)]
        match value {
            const { Self::Handshake as u8 } => Ok(Self::Handshake),
            const { Self::DeviceInfo as u8 } => Ok(Self::DeviceInfo),
            const { Self::StartUpdate as u8 } => Ok(Self::StartUpdate),
            const { Self::WriteChunk as u8 } => Ok(Self::WriteChunk),
            const { Self::UpdateStatus as u8 } => Ok(Self::UpdateStatus),
            const { Self::Reset as u8 } => Ok(Self::Reset),
            _ => Err(OtaError::UnknownCommand),
        }
    }
}

pub enum RequestForm<'a> {
    Handshake,
    DeviceInfo,
    StartUpdate,
    WriteChunk(&'a WriteChunkRequestForm),
    UpdateStatus,
    Reset,
    JumpToApplication,
}

impl RequestForm<'_> {
    unsafe fn transmute(cmd: Command, arr: &'_ [u8]) -> Self {
        match cmd {
            Command::Handshake => Self::Handshake,
            Command::DeviceInfo => Self::DeviceInfo,
            Command::StartUpdate => Self::StartUpdate,
            Command::WriteChunk => Self::WriteChunk(&*(arr.as_ptr() as *const _)),
            Command::UpdateStatus => Self::UpdateStatus,
            Command::Reset => Self::Reset,
            Command::JumpToApplication => Self::JumpToApplication,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OtaError {
    Nothing = 0,
    ChecksumError = 0x80,
    UnknownCommand = 0x81,
    OutOfRange = 0x82,
    MissingEof = 0x83,
    MissingSof = 0x84,
    FlashProg = 0x90,
    FlashSize = 0x91,
    FlashMiss = 0x92,
    FlashSeq = 0x93,
    FlashProtected = 0x94,
    FlashUnaligned = 0x95,
    FlashParallelism = 0x96,
    #[allow(unused)]
    UnknownError = 0xFF,
}

impl From<embassy_stm32::flash::Error> for OtaError {
    fn from(value: embassy_stm32::flash::Error) -> Self {
        match value {
            embassy_stm32::flash::Error::Prog => Self::FlashProg,
            embassy_stm32::flash::Error::Size => Self::FlashSize,
            embassy_stm32::flash::Error::Miss => Self::FlashMiss,
            embassy_stm32::flash::Error::Seq => Self::FlashSeq,
            embassy_stm32::flash::Error::Protected => Self::FlashProtected,
            embassy_stm32::flash::Error::Unaligned => Self::FlashUnaligned,
            embassy_stm32::flash::Error::Parallelism => Self::FlashParallelism,
        }
    }
}

const fn request_packet_size(command: Command) -> usize {
    match command {
        Command::Handshake => core::mem::size_of::<HandshakeForm>(),
        Command::DeviceInfo => core::mem::size_of::<DeviceInfoRequestForm>(),
        Command::StartUpdate => core::mem::size_of::<StartUpdateRequestForm>(),
        Command::WriteChunk => core::mem::size_of::<WriteChunkRequestForm>(),
        Command::UpdateStatus => core::mem::size_of::<UpdateStatusRequestForm>(),
        Command::Reset => core::mem::size_of::<ResetForm>(),
        Command::JumpToApplication => core::mem::size_of::<JumpToApplicationForm>(),
    }
}

#[allow(unused)]
const fn response_packet_size(command: Command) -> usize {
    match command {
        Command::Handshake => core::mem::size_of::<HandshakeForm>(),
        Command::DeviceInfo => core::mem::size_of::<DeviceInfoResponseForm>(),
        Command::StartUpdate => core::mem::size_of::<StartUpdateResponseForm>(),
        Command::WriteChunk => core::mem::size_of::<WriteChunkResponseForm>(),
        Command::UpdateStatus => core::mem::size_of::<UpdateStatusResponseForm>(),
        Command::Reset => core::mem::size_of::<ResetForm>(),
        Command::JumpToApplication => core::mem::size_of::<JumpToApplicationForm>(),
    }
}

const fn response_packet_max_size() -> usize {
    const fn max(a: usize, b: usize) -> usize {
        if a > b {
            a
        } else {
            b
        }
    }
    let mut ret = response_packet_size(Command::Handshake);
    ret = max(ret, response_packet_size(Command::DeviceInfo));
    ret = max(ret, response_packet_size(Command::StartUpdate));
    ret = max(ret, response_packet_size(Command::WriteChunk));
    ret = max(ret, response_packet_size(Command::UpdateStatus));
    max(ret, response_packet_size(Command::Reset))
}

#[allow(unused)]
const fn packet_size(sof: Sof, command: Command) -> usize {
    match sof {
        Sof::Request => request_packet_size(command),
        Sof::Response => response_packet_size(command),
    }
}

pub(crate) fn test_packet<'a>(packet: &[u8]) -> Result<RequestForm<'a>, OtaError> {
    if packet[0] != Sof::Request as u8 {
        return Err(OtaError::MissingSof);
    }

    let cmd = Command::try_from(packet[1])?;

    let estimated_packet_size = request_packet_size(cmd);

    if packet.len() < estimated_packet_size {
        return Err(OtaError::OutOfRange);
    } else if packet[estimated_packet_size - 1] != EOF_SIGNATURE {
        return Err(OtaError::MissingEof);
    }

    Ok(unsafe { RequestForm::transmute(cmd, packet) })
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sof {
    /// Host to slave
    Request = 0xAA,
    /// Slave to host
    Response = 0xBB,
}

#[repr(C)]
pub struct HandshakeForm {
    pub sof: Sof,
    pub command: Command,
    pub eof: u8,
}

impl HandshakeForm {
    #[allow(unused)]
    pub const fn request_new() -> Self {
        Self {
            sof: Sof::Request,
            command: Command::Handshake,
            eof: EOF_SIGNATURE,
        }
    }

    pub const fn response_new() -> Self {
        Self {
            sof: Sof::Response,
            command: Command::Handshake,
            eof: EOF_SIGNATURE,
        }
    }
}

#[repr(C)]
pub struct DeviceInfoRequestForm {
    pub sof: Sof,
    pub command: Command,
    pub eof: u8,
}

impl DeviceInfoRequestForm {
    #[allow(unused)]
    pub const fn new() -> Self {
        Self {
            sof: Sof::Request,
            command: Command::DeviceInfo,
            eof: EOF_SIGNATURE,
        }
    }
}

#[repr(C)]
pub struct DeviceInfoResponseForm {
    pub sof: Sof,
    pub command: Command,
    pub checksum: [u8; 2],
    pub protocol_version: u8,
    pub payload_exponent: u8,
    pub serial_number: [u8; 12],
    pub eof: u8,
}

impl DeviceInfoResponseForm {
    pub fn checksum_source(&self) -> &[u8] {
        unsafe {
            let start_ptr = &self.protocol_version as *const u8;
            let end_ptr = &self.eof as *const u8;

            core::slice::from_raw_parts(start_ptr, end_ptr as usize - start_ptr as usize)
        }
    }

    pub fn new(board: &mut Board) -> Self {
        let crc = board.hardware.crc.borrow_mut();

        let mut ret = Self {
            sof: Sof::Response,
            command: Command::DeviceInfo,
            checksum: [0; 2],
            protocol_version: PROTOCOL_VERSION_BYTE,
            payload_exponent: CHUNK_BIT_IDX as u8,
            serial_number: Board::get_serial_number(),
            eof: EOF_SIGNATURE,
        };

        crc.reset();
        ret.checksum = (crc.feed_bytes(ret.checksum_source()) as u16).to_le_bytes();

        ret
    }
}

#[repr(C)]
pub struct StartUpdateRequestForm {
    pub sof: Sof,
    pub command: Command,
    pub eof: u8,
}

impl StartUpdateRequestForm {
    #[allow(unused)]
    pub const fn new() -> Self {
        Self {
            sof: Sof::Request,
            command: Command::StartUpdate,
            eof: EOF_SIGNATURE,
        }
    }
}

#[repr(C)]
pub struct StartUpdateResponseForm {
    pub sof: Sof,
    pub command: Command,
    pub checksum: [u8; 2],
    pub nonce: [u8; 12],
    pub eof: u8,
}

impl StartUpdateResponseForm {
    pub fn new(board: &mut Board) -> Self {
        let crc = board.hardware.crc.borrow_mut();

        let mut ret = Self {
            sof: Sof::Response,
            command: Command::StartUpdate,
            checksum: [0; 2],
            nonce: Board::get_nonce(),
            eof: EOF_SIGNATURE,
        };

        crc.reset();
        let checksum = crc.feed_bytes(&ret.nonce);

        ret.checksum = (checksum as u16).to_le_bytes();

        ret
    }
}

#[repr(C)]
pub struct WriteChunkRequestForm {
    pub sof: Sof,
    pub command: Command,
    pub checksum: [u8; 2], // little endian
    pub offset: [u8; 4],   // little endian
    pub payload: [u8; WRITE_CHUNK_SIZE],
    pub eof: u8,
}

impl WriteChunkRequestForm {
    pub fn checksum_source(&self) -> &[u8] {
        unsafe {
            let start_ptr = &self.offset as *const u8;
            let end_ptr = &self.eof as *const u8;

            core::slice::from_raw_parts(start_ptr, end_ptr as usize - start_ptr as usize)
        }
    }

    // #[cfg(any(not(feature = "no_std"), feature = "std", test))]
    #[allow(unused)]
    pub fn new_std(offset: u32, bytes: &[u8; WRITE_CHUNK_SIZE]) -> Result<Self, OtaError> {
        // if offset + bytes.len() as u32 > size {
        //     return Err(Error::Size);
        // }
        if offset % WRITE_SIZE as u32 != 0 || bytes.len() % WRITE_SIZE != 0 {
            return Err(OtaError::FlashUnaligned);
        }
        let mut ret = Self {
            sof: Sof::Request,
            command: Command::WriteChunk,
            checksum: [0; 2],
            offset: offset.to_le_bytes(),
            payload: *bytes,
            eof: EOF_SIGNATURE,
        };

        ret.checksum = (crate::types::std_crc::std_crc(ret.checksum_source()) as u16).to_le_bytes();

        Ok(ret)
    }

    pub(crate) fn try_flash(&self, board: &mut Board) -> Result<(), OtaError> {
        let flash = board.hardware.flash.borrow_mut();
        let crc = board.hardware.crc.borrow_mut();
        // let cipher = unsafe { &mut *board.shared_resource.cipher.get() };
        // let mut cipher = board.shared_resource;

        let mut data = self.payload;

        crc.reset();
        let actual = crc.feed_bytes(self.checksum_source()) as u16;

        let expected = u16::from_le_bytes(self.checksum);
        if actual != expected {
            return Err(OtaError::ChecksumError);
        }

        let address = u32::from_le_bytes(self.offset);

        board.shared_resource.cipher.apply_keystream(&mut data); // decrypt

        flash.bank1_region.blocking_write(address, &data)?;

        board.shared_resource.section_mark.mark_offset(address);

        Ok(())
    }
}

#[repr(C)]
pub struct WriteChunkResponseForm {
    pub sof: Sof,
    pub command: Command,
    pub result: OtaError,
    pub eof: u8,
}

impl WriteChunkResponseForm {
    pub fn new(result: Result<(), OtaError>) -> Self {
        Self {
            sof: Sof::Response,
            command: Command::WriteChunk,
            result: result.map_or_else(|e| e, |_| OtaError::Nothing),
            eof: EOF_SIGNATURE,
        }
    }
}

#[repr(C)]
pub struct UpdateStatusRequestForm {
    pub sof: Sof,
    pub command: Command,
    pub eof: u8,
}

impl UpdateStatusRequestForm {
    #[allow(unused)]
    pub const fn new() -> Self {
        Self {
            sof: Sof::Response,
            command: Command::UpdateStatus,
            eof: EOF_SIGNATURE,
        }
    }
}

// acutal MCU will use above form
#[repr(C)]
pub struct UpdateStatusResponseForm {
    pub sof: Sof,
    pub command: Command,
    pub checksum: [u8; 2],
    pub chunk_mark: SectionMark,
    pub eof: u8,
}

impl UpdateStatusResponseForm {
    pub fn new(board: &mut Board) -> Self {
        let crc = board.hardware.crc.borrow_mut();

        let mut ret: Self = Self {
            sof: Sof::Response,
            command: Command::UpdateStatus,
            checksum: [0; 2],
            chunk_mark: board.shared_resource.section_mark.clone(),
            eof: EOF_SIGNATURE,
        };

        crc.reset();
        let checksum = crc.feed_bytes(&ret.chunk_mark.bitmap);

        ret.checksum = (checksum as u16).to_le_bytes();

        ret
    }
}

#[repr(C)]
pub struct ResetForm {
    pub sof: Sof,
    pub command: Command,
    pub eof: u8,
}

impl ResetForm {
    #[allow(unused)]
    pub const fn request_new() -> Self {
        Self {
            sof: Sof::Request,
            command: Command::Reset,
            eof: EOF_SIGNATURE,
        }
    }

    pub const fn response_new() -> Self {
        Self {
            sof: Sof::Response,
            command: Command::Reset,
            eof: EOF_SIGNATURE,
        }
    }
}

#[repr(C)]
pub struct JumpToApplicationForm {
    pub sof: Sof,
    pub command: Command,
    pub eof: u8,
}

impl JumpToApplicationForm {
    #[allow(unused)]
    pub const fn request_new() -> Self {
        Self {
            sof: Sof::Request,
            command: Command::JumpToApplication,
            eof: EOF_SIGNATURE,
        }
    }

    pub const fn response_new() -> Self {
        Self {
            sof: Sof::Response,
            command: Command::JumpToApplication,
            eof: EOF_SIGNATURE,
        }
    }
}
