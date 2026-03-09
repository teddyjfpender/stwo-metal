use stwo::core::fields::m31::BaseField;
use stwo_metal_sys::metal::U32Buffer;

#[derive(Debug)]
pub struct BaseFieldVec {
    buffer: U32Buffer,
    size: usize,
}

unsafe impl Send for BaseFieldVec {}
unsafe impl Sync for BaseFieldVec {}

impl BaseFieldVec {
    pub fn from_vec(host_array: Vec<BaseField>) -> Self {
        let raw: Vec<u32> = host_array.into_iter().map(|value| value.0).collect();
        let size = raw.len();
        let buffer =
            U32Buffer::from_slice(&raw).expect("Metal BaseFieldVec upload should initialize");
        Self { buffer, size }
    }

    pub fn new_uninitialized(size: usize) -> Self {
        let buffer = U32Buffer::uninitialized(size)
            .expect("Metal BaseFieldVec allocation should initialize");
        Self { buffer, size }
    }

    pub fn new_zeroes(size: usize) -> Self {
        let buffer =
            U32Buffer::zeroed(size).expect("Metal BaseFieldVec zero allocation should initialize");
        Self { buffer, size }
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn get_data(&self, index: usize) -> BaseField {
        BaseField::from_u32_unchecked(self.buffer.get(index))
    }

    pub fn set_data(&mut self, index: usize, value: BaseField) {
        self.buffer.set(index, value.0);
    }

    pub fn copy_from(&mut self, other: &Self) {
        self.buffer
            .copy_from(&other.buffer)
            .expect("Metal BaseFieldVec copy should succeed");
    }

    pub fn copy_from_offset(&mut self, other: &Self, offset: usize) {
        self.buffer
            .copy_from_offset(&other.buffer, offset)
            .expect("Metal BaseFieldVec offset copy should succeed");
    }

    pub fn to_vec(&self) -> Vec<BaseField> {
        self.buffer
            .to_vec()
            .expect("Metal BaseFieldVec readback should succeed")
            .into_iter()
            .map(BaseField::from_u32_unchecked)
            .collect()
    }

    pub fn bit_reverse(&mut self) {
        self.buffer
            .bit_reverse()
            .expect("Metal BaseFieldVec bit reverse should succeed");
    }

    pub fn coset_to_circle_domain_bit_reversed(&self) -> Self {
        let buffer = self
            .buffer
            .permute_coset_to_circle_domain_bit_reversed()
            .expect("Metal BaseFieldVec permutation should succeed");
        Self {
            buffer,
            size: self.size,
        }
    }

    pub fn extend(&mut self, other: &Self) {
        let new_size = self.size + other.size;
        let mut new_vec = Self::new_uninitialized(new_size);
        new_vec.copy_from(self);
        new_vec.copy_from_offset(other, self.size);
        *self = new_vec;
    }

    pub fn pad_to_size(&mut self, target_size: usize) {
        if self.size >= target_size {
            return;
        }
        let mut new_vec = Self::new_zeroes(target_size);
        new_vec.copy_from(self);
        *self = new_vec;
    }
}

impl Clone for BaseFieldVec {
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
            size: self.size,
        }
    }
}
