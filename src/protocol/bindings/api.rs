// Automatically generated rust module for 'api.proto' file

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(unknown_lints)]
#![allow(clippy::all)]
#![cfg_attr(rustfmt, rustfmt_skip)]


use std::borrow::Cow;
use quick_protobuf::{MessageRead, MessageWrite, BytesReader, Writer, WriterBackend, Result};
use quick_protobuf::sizeofs::*;
use super::*;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum KeyKind {
    UNKNOWN = 0,
    BACKSPACE = 1,
    ENTER = 2,
    LEFT = 3,
    RIGHT = 4,
    UP = 5,
    DOWN = 6,
    HOME = 7,
    END = 8,
    PAGEUP = 9,
    PAGEDOWN = 10,
    TAB = 11,
    DELETE = 13,
    INSERT = 14,
    FUNCTION = 15,
    CHAR = 16,
    ESC = 17,
    SHIFT = 18,
    META = 19,
}

impl Default for KeyKind {
    fn default() -> Self {
        KeyKind::UNKNOWN
    }
}

impl From<i32> for KeyKind {
    fn from(i: i32) -> Self {
        match i {
            0 => KeyKind::UNKNOWN,
            1 => KeyKind::BACKSPACE,
            2 => KeyKind::ENTER,
            3 => KeyKind::LEFT,
            4 => KeyKind::RIGHT,
            5 => KeyKind::UP,
            6 => KeyKind::DOWN,
            7 => KeyKind::HOME,
            8 => KeyKind::END,
            9 => KeyKind::PAGEUP,
            10 => KeyKind::PAGEDOWN,
            11 => KeyKind::TAB,
            13 => KeyKind::DELETE,
            14 => KeyKind::INSERT,
            15 => KeyKind::FUNCTION,
            16 => KeyKind::CHAR,
            17 => KeyKind::ESC,
            18 => KeyKind::SHIFT,
            19 => KeyKind::META,
            _ => Self::default(),
        }
    }
}

impl<'a> From<&'a str> for KeyKind {
    fn from(s: &'a str) -> Self {
        match s {
            "UNKNOWN" => KeyKind::UNKNOWN,
            "BACKSPACE" => KeyKind::BACKSPACE,
            "ENTER" => KeyKind::ENTER,
            "LEFT" => KeyKind::LEFT,
            "RIGHT" => KeyKind::RIGHT,
            "UP" => KeyKind::UP,
            "DOWN" => KeyKind::DOWN,
            "HOME" => KeyKind::HOME,
            "END" => KeyKind::END,
            "PAGEUP" => KeyKind::PAGEUP,
            "PAGEDOWN" => KeyKind::PAGEDOWN,
            "TAB" => KeyKind::TAB,
            "DELETE" => KeyKind::DELETE,
            "INSERT" => KeyKind::INSERT,
            "FUNCTION" => KeyKind::FUNCTION,
            "CHAR" => KeyKind::CHAR,
            "ESC" => KeyKind::ESC,
            "SHIFT" => KeyKind::SHIFT,
            "META" => KeyKind::META,
            _ => Self::default(),
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct HandshakeRequest<'a> {
    pub hostname: Cow<'a, str>,
    pub version: u32,
    pub token: Cow<'a, [u8]>,
    pub pkey: Cow<'a, [u8]>,
}

impl<'a> MessageRead<'a> for HandshakeRequest<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.hostname = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(21) => msg.version = r.read_fixed32(bytes)?,
                Ok(26) => msg.token = r.read_bytes(bytes).map(Cow::Borrowed)?,
                Ok(34) => msg.pkey = r.read_bytes(bytes).map(Cow::Borrowed)?,
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for HandshakeRequest<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.hostname == "" { 0 } else { 1 + sizeof_len((&self.hostname).len()) }
        + if self.version == 0u32 { 0 } else { 1 + 4 }
        + if self.token == Cow::Borrowed(b"") { 0 } else { 1 + sizeof_len((&self.token).len()) }
        + if self.pkey == Cow::Borrowed(b"") { 0 } else { 1 + sizeof_len((&self.pkey).len()) }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.hostname != "" { w.write_with_tag(10, |w| w.write_string(&**&self.hostname))?; }
        if self.version != 0u32 { w.write_with_tag(21, |w| w.write_fixed32(*&self.version))?; }
        if self.token != Cow::Borrowed(b"") { w.write_with_tag(26, |w| w.write_bytes(&**&self.token))?; }
        if self.pkey != Cow::Borrowed(b"") { w.write_with_tag(34, |w| w.write_bytes(&**&self.pkey))?; }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct HandshakeResponse<'a> {
    pub hostname: Cow<'a, str>,
    pub version: u32,
    pub pkey: Cow<'a, [u8]>,
}

impl<'a> MessageRead<'a> for HandshakeResponse<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.hostname = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(21) => msg.version = r.read_fixed32(bytes)?,
                Ok(26) => msg.pkey = r.read_bytes(bytes).map(Cow::Borrowed)?,
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for HandshakeResponse<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.hostname == "" { 0 } else { 1 + sizeof_len((&self.hostname).len()) }
        + if self.version == 0u32 { 0 } else { 1 + 4 }
        + if self.pkey == Cow::Borrowed(b"") { 0 } else { 1 + sizeof_len((&self.pkey).len()) }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.hostname != "" { w.write_with_tag(10, |w| w.write_string(&**&self.hostname))?; }
        if self.version != 0u32 { w.write_with_tag(21, |w| w.write_fixed32(*&self.version))?; }
        if self.pkey != Cow::Borrowed(b"") { w.write_with_tag(26, |w| w.write_bytes(&**&self.pkey))?; }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct KeyEvent {
    pub kind: KeyKind,
    pub key: u32,
    pub modifiers: u32,
}

impl<'a> MessageRead<'a> for KeyEvent {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(8) => msg.kind = r.read_enum(bytes)?,
                Ok(16) => msg.key = r.read_uint32(bytes)?,
                Ok(24) => msg.modifiers = r.read_uint32(bytes)?,
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl MessageWrite for KeyEvent {
    fn get_size(&self) -> usize {
        0
        + if self.kind == api::KeyKind::UNKNOWN { 0 } else { 1 + sizeof_varint(*(&self.kind) as u64) }
        + if self.key == 0u32 { 0 } else { 1 + sizeof_varint(*(&self.key) as u64) }
        + if self.modifiers == 0u32 { 0 } else { 1 + sizeof_varint(*(&self.modifiers) as u64) }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.kind != api::KeyKind::UNKNOWN { w.write_with_tag(8, |w| w.write_enum(*&self.kind as i32))?; }
        if self.key != 0u32 { w.write_with_tag(16, |w| w.write_uint32(*&self.key))?; }
        if self.modifiers != 0u32 { w.write_with_tag(24, |w| w.write_uint32(*&self.modifiers))?; }
        Ok(())
    }
}

