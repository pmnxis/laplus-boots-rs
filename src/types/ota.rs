/*
 * SPDX-FileCopyrightText: © 2025 Jinwoo Park (pmnxis@gmail.com)
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

use chacha20::cipher::StreamCipher;
use embassy_stm32::flash::WRITE_SIZE;

use super::section_mark::{SectionMark, CHUNK_BIT_IDX, WRITE_CHUNK_SIZE};
use crate::Board;

pub const EOF_SIGNATURE: u8 = 0xFF;
pub const PROTOCOL_VERSION_BYTE: u8 = 0x01;

#[macro_export]
macro_rules! as_bytes {
    ($val:expr) => {
        unsafe {
            core::slice::from_raw_parts(
                ($val as *const _) as *const u8,
                core::mem::size_of_val($val),
            )
        }
    };
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
    }
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

    pub fn new(board: &Board) -> Self {
        let crc = unsafe { &mut *board.hardware.crc.get() };

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
        let checksum = crc.feed_bytes(&ret.checksum_source());

        ret.checksum = (checksum as u16).to_le_bytes();

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
    pub fn new(board: &Board) -> Self {
        let crc = unsafe { &mut *board.hardware.crc.get() };

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
    pub length: u8,
    pub checksum: [u8; 2], // little endian
    pub offset: [u8; 4],   // little endian
    pub payload: [u8; WRITE_CHUNK_SIZE],
    pub eof: u8,
}

impl WriteChunkRequestForm {
    #[allow(unused)]
    pub fn new(offset: u32, bytes: &[u8]) -> Result<Self, OtaError> {
        // if offset + bytes.len() as u32 > size {
        //     return Err(Error::Size);
        // }
        if offset % WRITE_SIZE as u32 != 0 || bytes.len() % WRITE_SIZE != 0 {
            return Err(OtaError::FlashUnaligned);
        }

        unimplemented!()
    }

    pub(crate) fn try_flash(&self, board: &mut Board) -> Result<(), OtaError> {
        let flash = unsafe { &mut *board.hardware.flash.get() };
        let crc = unsafe { &mut *board.hardware.crc.get() };
        // let cipher = unsafe { &mut *board.shared_resource.cipher.get() };
        // let mut cipher = board.shared_resource;

        let mut data = self.payload;

        crc.reset();
        let _ = crc.feed_bytes(&self.offset);
        let actual = crc.feed_bytes(&data) as u16;
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
    pub fn new(board: &Board) -> Self {
        let crc = unsafe { &mut *board.hardware.crc.get() };

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
