/*
 * SPDX-FileCopyrightText: © 2025 Jinwoo Park (pmnxis@gmail.com)
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherCoreWrapper, StreamCipherSeek};

// use num_enum::{IntoPrimitive, TryFromPrimitive};
use crate::Board;

pub const EOF_SIGNATURE: u8 = 0xFF;
const CHECKSUM_OFFSET: usize = 2;
const CHECKSUM_END: usize = CHECKSUM_OFFSET + 2;

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
    UnknownError = 0xFF,
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

impl Command {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::Handshake),
            0x02 => Some(Self::StartUpdate),
            0x03 => Some(Self::WriteChunk),
            0x04 => Some(Self::UpdateStatus),
            0x05 => Some(Self::Reset),
            _ => None,
        }
    }
}

#[repr(C)]
pub struct HandshakeForm {
    pub sof: Sof,
    pub command: Command,
    pub eof: u8, // last 256'th byte is firmware eof (end)
}

#[repr(C)]
pub struct DeviceInfoRequestForm {
    pub sof: Sof,
    pub command: Command,
    pub eof: u8, // last 256'th byte is firmware eof (end)
}

#[repr(C)]
pub struct DeviceInfoResponseForm {
    pub sof: Sof,
    pub command: Command,
    pub checksum: [u8; 2],
    pub protocol_version: u8,
    pub nonce: [u8; 12],
    pub payload_exponent: u8,
    pub flash_page_size: u8,
    pub serial_number: [u8; 12],
    pub eof: u8, // last 256'th byte is firmware eof (end)
}

#[repr(C)]
pub struct StartUpdateRequestForm {
    pub sof: Sof,
    pub command: Command,
    pub eof: u8, // last 256'th byte is firmware eof (end)
}

#[repr(C)]
pub struct StartUpdateResponseForm {
    pub sof: Sof,
    pub command: Command,
    pub checksum: [u8; 2],
    pub nonce: [u8; 12],
    pub eof: u8, // last 256'th byte is firmware eof (end)
}

#[repr(C)]
pub struct WriteChunkRequestForm {
    pub sof: Sof,
    pub command: Command,
    pub length: u8,
    pub checksum: [u8; 2], // little endian
    pub offset: [u8; 4],   // little endian
    pub payload: [u8; 256],
    pub eof: u8, // last 256'th byte is firmware eof (end)
}

impl WriteChunkRequestForm {
    pub(crate) fn try_flash(&self, board: &Board) -> bool {
        let flash = unsafe { &mut *board.hardware.flash.get() };
        let crc = unsafe { &mut *board.hardware.crc.get() };
        let cipher = unsafe { &mut *board.shared_resource.cipher.get() };

        let mut data = self.payload;
        // let actual = crc.feed_words(unsafe {
        //     core::mem::transmute_copy::<&[u8], &[u32]>(&data)
        //     // core::slice::from_raw_parts((&data as *const _) as *const u32, 128 / core::mem::size_of::<u32>())
        // });

        let actual = crc.feed_bytes(&data) as u16;
        let expected = u16::from_le_bytes(self.checksum);
        if actual != expected {
            return false;
        }

        let address = u32::from_le_bytes(self.offset);

        cipher.apply_keystream(&mut data); // decrypt

        let _ = flash.bank1_region.blocking_write(address, &data);

        true
    }
}

#[repr(C)]
pub struct WriteChunkResponseForm {
    pub sof: Sof,
    pub command: Command,
    pub checksum: [u8; 2],
    pub result: OtaError,
    /// associated slice number in single page
    pub result_tail: u8,
    pub eof: u8, // last 256'th byte is firmware eof (end)
}

#[repr(C)]
pub struct UpdateStatusRequestForm {
    pub sof: Sof,
    pub command: Command,
    pub eof: u8,
}

#[repr(C)]
pub struct UpdateStatusResponseForm {
    pub sof: Sof,
    pub command: Command,
    pub checksum: [u8; 2],
    pub start_addr: u32,
    pub end_addr: u32,
    pub count_of_slice: u32,
    pub eof: u8,
}

// acutal MCU will use above form
#[repr(C)]
pub struct UpdateStatusResponseRawForm {
    pub sof: Sof,
    pub command: Command,
    pub checksum: [u8; 2],
    pub start_addr: [u8; core::mem::size_of::<u32>()],
    pub end_addr: [u8; core::mem::size_of::<u32>()],
    pub count_of_slice: [u8; core::mem::size_of::<u32>()],
    pub eof: u8,
}

#[repr(C)]
pub struct ResetForm {
    pub sof: Sof,
    pub command: Command,
    pub eof: u8,
}
