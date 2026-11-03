//! The vector kernels of the original crate are not used here: every function reports that it did nothing, so the
//! scalar code in `predict` and `loop_filter` runs.

pub(super) struct EdgeLimits {
    pub edge: i32,
    pub interior: i32,
    pub hev_threshold: i32,
    pub macroblock_edge: bool,
    pub simple: bool,
}

pub(super) fn filter_vertical_edge(
    _data: &mut [u8],
    _at: usize,
    _advance: usize,
    _count: usize,
    _vector: EdgeLimits,
) -> bool {
    false
}

pub(super) fn filter_horizontal_edge(
    _data: &mut [u8],
    _at: usize,
    _step: usize,
    _count: usize,
    _vector: EdgeLimits,
) -> bool {
    false
}

pub(super) fn idct_add(_c: &[i16; 16], _plane: &mut [u8], _offset: usize, _stride: usize) -> bool {
    false
}

pub(super) fn idct_dc_add_row(
    _dcs: &[i16],
    _plane: &mut [u8],
    _offset: usize,
    _stride: usize,
) -> bool {
    false
}

pub(super) fn inverse_walsh(_input: &[i16; 16]) -> Option<[i16; 16]> {
    None
}

#[allow(clippy::too_many_arguments)]
pub(super) fn tm_predict(
    _above: &[u8],
    _left: &[u8],
    _corner: i32,
    _n: usize,
    _plane: &mut [u8],
    _offset: usize,
    _stride: usize,
) -> bool {
    false
}

pub(super) fn subblock(
    _mode: u8,
    _above: &[u8; 9],
    _left: &[u8; 4],
    _plane: &mut [u8],
    _offset: usize,
    _stride: usize,
) -> bool {
    false
}

#[allow(clippy::too_many_arguments)]
pub(super) fn sixtap(
    _window: &[u8],
    _width: usize,
    _height: usize,
    _horizontal: &[i32],
    _vertical: &[i32],
    _output: &mut [u8],
    _destination: usize,
    _stride: usize,
) -> bool {
    false
}
