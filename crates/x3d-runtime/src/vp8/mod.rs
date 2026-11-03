//! A VP8 decoder (RFC 6386) for MovieTexture WebM files.
//!
//! `decoder`, `bool_decoder`, `predict`, `loop_filter` and `tables` are adapted from the `zvidlib-vp8` 0.5.0 crate
//! (MIT, Copyright (c) 2026 Loren Segal), which follows the reference decoder of RFC 6386 and matches libvpx on its
//! output. Their changes here: the framework types are replaced by the small [`Error`] and [`Limits`] below and the
//! vector (SIMD) kernels are left out, so the scalar paths run. The VP8 tables come from RFC 6386 section 20 and are
//! under the license of that RFC:
//!
//! > Copyright (c) 2010, 2011, Google Inc. All rights reserved.
//! >
//! > Redistribution and use in source and binary forms, with or without modification, are permitted provided that the
//! > following conditions are met: Redistributions of source code must retain the above copyright notice, this list of
//! > conditions and the following disclaimer. Redistributions in binary form must reproduce the above copyright
//! > notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided
//! > with the distribution. Neither the name of Google nor the names of its contributors may be used to endorse or
//! > promote products derived from this software without specific prior written permission.
//! >
//! > THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED
//! > WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
//! > PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
//! > INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
//! > SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
//! > THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
//! > IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//!
//! The admission record is `docs/decoder-admission.md`.

#![allow(clippy::all, dead_code)]

mod bool_decoder;
mod decoder;
mod loop_filter;
mod predict;
mod simd;
mod tables;

pub use decoder::Picture;

/// What the decoder refuses to do, with a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    MalformedMedia,
    Unsupported,
    ResourceLimit,
}

#[derive(Debug, Clone)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// The size the decoder may allocate for.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_width: u32,
    pub max_height: u32,
    pub max_allocation_bytes: u64,
}

/// The VP8 decoder: one compressed frame at a time.
pub struct Vp8Decoder(decoder::Decoder);

impl Vp8Decoder {
    pub fn new(limits: Limits) -> Self {
        Self(decoder::Decoder::new(limits))
    }

    /// Decodes one frame. `Ok(None)` is a frame that is not shown (it only updates the references).
    pub fn decode(&mut self, data: &[u8]) -> Result<Option<Picture>> {
        self.0.decode(data)
    }
}

/// The width and height a VP8 key frame codes (the 14-bit values; the two scaling bits above them are dropped), or
/// `None` for an inter frame or a frame too short to say.
pub fn key_frame_size(frame: &[u8]) -> Option<(u32, u32)> {
    if frame.len() < 10 || frame[0] & 1 != 0 || frame[3..6] != [0x9d, 0x01, 0x2a] {
        return None;
    }

    let w = u32::from(u16::from_le_bytes([frame[6], frame[7]]) & 0x3fff);
    let h = u32::from(u16::from_le_bytes([frame[8], frame[9]]) & 0x3fff);

    Some((w, h))
}
