use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo_metal_sys::metal::U32Buffer;

use super::SecureFieldVec;

#[derive(Debug)]
pub struct BaseFieldVec {
    pub(crate) buffer: U32Buffer,
    size: usize,
}

unsafe impl Send for BaseFieldVec {}
unsafe impl Sync for BaseFieldVec {}

impl BaseFieldVec {
    pub(crate) fn from_buffer(buffer: U32Buffer) -> Self {
        let size = buffer.len();
        Self { buffer, size }
    }

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

    pub fn fix_first_variable(&self, assignment: SecureField) -> SecureFieldVec {
        assert!(
            self.size >= 2,
            "Metal BaseFieldVec MLE fix-first-variable requires at least two evaluations"
        );
        let buffer = self
            .buffer
            .fix_first_variable_base_field(assignment.to_m31_array().map(|limb| limb.0))
            .expect("Metal BaseFieldVec MLE fix-first-variable should succeed");
        SecureFieldVec::from_buffer(buffer)
    }

    pub fn gkr_next_logup_multiplicities_layer(
        &self,
        denominators: &SecureFieldVec,
    ) -> (SecureFieldVec, SecureFieldVec) {
        let (next_numerators, next_denominators) =
            U32Buffer::gkr_next_logup_multiplicities_layer(&self.buffer, &denominators.buffer)
                .expect("Metal base-field multiplicities next-layer generation should succeed");
        (
            SecureFieldVec::from_buffer(next_numerators),
            SecureFieldVec::from_buffer(next_denominators),
        )
    }

    pub fn inclusive_prefix_sum_bit_rev_circle_domain(&mut self) {
        self.buffer
            .inclusive_prefix_sum_bit_rev_circle_domain_in_place()
            .expect("Metal BaseFieldVec prefix sum should succeed");
    }

    pub fn gkr_sum_logup_multiplicities(
        &self,
        eq_evals: &SecureFieldVec,
        denominators: &SecureFieldVec,
        lambda: SecureField,
    ) -> (SecureField, SecureField) {
        let (eval_at_0, eval_at_2) = U32Buffer::gkr_sum_logup_multiplicities(
            &eq_evals.buffer,
            &self.buffer,
            &denominators.buffer,
            lambda.to_m31_array().map(|limb| limb.0),
        )
        .expect("Metal GKR multiplicities sum should succeed");
        (
            SecureField::from_u32_unchecked(eval_at_0[0], eval_at_0[1], eval_at_0[2], eval_at_0[3]),
            SecureField::from_u32_unchecked(eval_at_2[0], eval_at_2[1], eval_at_2[2], eval_at_2[3]),
        )
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
