//! Byte counters of the CPU → GPU uploads (`queue.write_buffer`, `queue.write_texture`) and GPU → CPU readbacks
//! (`map_async` of a read buffer) the renderers of this crate make, split into ParticleSystem data and the rest.
//!
//! Every upload and readback site adds the bytes it hands to wgpu, so the counts are what was actually transferred,
//! not an estimate. A counter is one thread-local addition: the cost is outside any measurable frame time, and tests
//! that render on several threads at once count their own transfers only.

use std::cell::Cell;

/// What a transfer carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// ParticleSystem instance records, their shadow, pointing and collision draws, wide LINE inputs and triangles.
    Particle,
    Other,
}

/// Bytes since the last [`take`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Transfers {
    pub particle_upload: u64,
    pub other_upload: u64,
    pub particle_readback: u64,
    pub other_readback: u64,
}

impl Transfers {
    pub fn upload(&self) -> u64 {
        self.particle_upload + self.other_upload
    }

    pub fn readback(&self) -> u64 {
        self.particle_readback + self.other_readback
    }
}

thread_local! {
    static COUNTS: Cell<Transfers> = const {
        Cell::new(Transfers {
            particle_upload: 0,
            other_upload: 0,
            particle_readback: 0,
            other_readback: 0,
        })
    };
}

fn add(f: impl FnOnce(&mut Transfers)) {
    COUNTS.with(|c| {
        let mut t = c.get();
        f(&mut t);
        c.set(t);
    });
}

/// `bytes` written to a GPU buffer or texture.
pub fn upload(kind: Kind, bytes: usize) {
    add(|t| match kind {
        Kind::Particle => t.particle_upload += bytes as u64,
        Kind::Other => t.other_upload += bytes as u64,
    });
}

/// `bytes` of a buffer mapped for reading.
pub fn readback(kind: Kind, bytes: u64) {
    add(|t| match kind {
        Kind::Particle => t.particle_readback += bytes,
        Kind::Other => t.other_readback += bytes,
    });
}

/// `bytes` uploaded in one transfer of which `particle` are ParticleSystem data.
pub fn split_upload(bytes: usize, particle: usize) {
    let particle = particle.min(bytes);

    upload(Kind::Particle, particle);
    upload(Kind::Other, bytes - particle);
}

/// `Particle` when `particle`.
pub fn kind(particle: bool) -> Kind {
    if particle {
        Kind::Particle
    } else {
        Kind::Other
    }
}

/// The counts of this thread since the last call, and zero them.
pub fn take() -> Transfers {
    COUNTS.with(|c| c.replace(Transfers::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_by_kind_and_take_zeroes() {
        take();
        upload(Kind::Particle, 192);
        upload(Kind::Other, 64);
        readback(Kind::Other, 1024);

        let t = take();

        assert_eq!(
            (t.particle_upload, t.other_upload, t.readback()),
            (192, 64, 1024)
        );
        assert_eq!(take(), Transfers::default());
    }
}
