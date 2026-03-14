use stwo_metal_sys::metal::U32Buffer;

use super::BaseFieldVec;

#[derive(Debug)]
pub struct BaseFieldColumnBatch {
    buffer: U32Buffer,
    column_len: usize,
    n_columns: usize,
}

impl BaseFieldColumnBatch {
    pub(crate) fn from_buffer(buffer: U32Buffer, column_len: usize, n_columns: usize) -> Self {
        assert_eq!(
            buffer.len(),
            column_len
                .checked_mul(n_columns)
                .expect("column batch length should fit in usize"),
            "column batch buffer length must match column_len * n_columns"
        );
        Self {
            buffer,
            column_len,
            n_columns,
        }
    }

    pub(crate) fn buffer(&self) -> &U32Buffer {
        &self.buffer
    }

    pub fn column_len(&self) -> usize {
        self.column_len
    }

    pub fn n_columns(&self) -> usize {
        self.n_columns
    }

    /// Consume the batch and return the underlying contiguous GPU buffer.
    /// The buffer layout is column-major: column 0 data, column 1 data, ...
    pub fn into_buffer(self) -> U32Buffer {
        self.buffer
    }

    pub fn to_columns(&self) -> Vec<BaseFieldVec> {
        (0..self.n_columns)
            .map(|column_index| {
                let start = column_index
                    .checked_mul(self.column_len)
                    .expect("column batch offset should fit in usize");
                let buffer = self
                    .buffer
                    .clone_range(start, self.column_len)
                    .expect("column batch slicing should succeed");
                BaseFieldVec::from_buffer(buffer)
            })
            .collect()
    }
}
