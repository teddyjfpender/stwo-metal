use ark_std::One;
use stwo::core::fields::qm31::SecureField;
use stwo_metal_sys::metal::U32Buffer;

#[derive(Debug)]
pub struct SecureFieldVec {
    pub(crate) buffer: U32Buffer,
    size: usize,
}

unsafe impl Send for SecureFieldVec {}
unsafe impl Sync for SecureFieldVec {}

impl SecureFieldVec {
    pub(crate) fn from_buffer(buffer: U32Buffer) -> Self {
        let size = buffer.len() / 4;
        Self { buffer, size }
    }

    pub fn gkr_generate_eq_evals(y: &[SecureField], v: SecureField) -> Self {
        if y.is_empty() {
            return Self::from_vec(vec![v]);
        }

        let factors = y
            .iter()
            .flat_map(|y_i| {
                let lhs = SecureField::one() - *y_i;
                [lhs, *y_i]
            })
            .flat_map(|value| value.to_m31_array().map(|limb| limb.0))
            .collect::<Vec<_>>();
        let factors = U32Buffer::from_slice(&factors)
            .expect("Metal GKR eq-eval factor upload should initialize");
        let buffer = U32Buffer::gkr_gen_eq_evals_from_factors(
            &factors,
            y.len(),
            v.to_m31_array().map(|limb| limb.0),
        )
        .expect("Metal GKR eq-eval generation should succeed");
        Self::from_buffer(buffer)
    }

    pub fn from_vec(host_array: Vec<SecureField>) -> Self {
        let raw: Vec<u32> = host_array
            .into_iter()
            .flat_map(|value| value.to_m31_array().map(|limb| limb.0))
            .collect();
        let size = raw.len() / 4;
        let buffer =
            U32Buffer::from_slice(&raw).expect("Metal SecureFieldVec upload should initialize");
        Self { buffer, size }
    }

    pub fn new_uninitialized(size: usize) -> Self {
        let buffer = U32Buffer::uninitialized(size * 4)
            .expect("Metal SecureFieldVec allocation should initialize");
        Self { buffer, size }
    }

    pub fn new_zeroes(size: usize) -> Self {
        let buffer = U32Buffer::zeroed(size * 4)
            .expect("Metal SecureFieldVec zero allocation should initialize");
        Self { buffer, size }
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn get_data(&self, index: usize) -> SecureField {
        assert!(
            index < self.size,
            "secure-field index {index} out of bounds for len {}",
            self.size
        );
        let base = index * 4;
        SecureField::from_u32_unchecked(
            self.buffer.get(base),
            self.buffer.get(base + 1),
            self.buffer.get(base + 2),
            self.buffer.get(base + 3),
        )
    }

    pub fn set_data(&mut self, index: usize, value: SecureField) {
        assert!(
            index < self.size,
            "secure-field index {index} out of bounds for len {}",
            self.size
        );
        let base = index * 4;
        for (offset, limb) in value.to_m31_array().into_iter().enumerate() {
            self.buffer.set(base + offset, limb.0);
        }
    }

    pub fn copy_from(&mut self, other: &Self) {
        assert!(
            self.size >= other.size,
            "destination secure-field len {} is smaller than source len {}",
            self.size,
            other.size
        );
        self.buffer
            .copy_from(&other.buffer)
            .expect("Metal SecureFieldVec copy should succeed");
    }

    pub fn to_vec(&self) -> Vec<SecureField> {
        let raw = self
            .buffer
            .to_vec()
            .expect("Metal SecureFieldVec readback should succeed");
        raw.chunks_exact(4)
            .map(|limbs| SecureField::from_u32_unchecked(limbs[0], limbs[1], limbs[2], limbs[3]))
            .collect()
    }

    pub fn bit_reverse(&mut self) {
        self.buffer
            .bit_reverse_u32x4(self.size)
            .expect("Metal SecureFieldVec bit reverse should succeed");
    }

    pub fn fold_circle_into_line_first_layer(
        &self,
        inverse_y_factors: &[u32],
        alpha: SecureField,
    ) -> Self {
        let factors = U32Buffer::from_slice(inverse_y_factors)
            .expect("Metal inverse-y factor upload should initialize");
        let alpha_limbs = alpha.to_m31_array().map(|limb| limb.0);
        let buffer = self
            .buffer
            .fri_fold_circle_into_line_first_layer_u32x4(&factors, alpha_limbs)
            .expect("Metal FRI first-layer fold should succeed");
        Self {
            buffer,
            size: self.size / 2,
        }
    }

    pub fn fold_line_step(&self, inverse_x_factors: &[u32], alpha: SecureField) -> Self {
        let factors = U32Buffer::from_slice(inverse_x_factors)
            .expect("Metal inverse-x factor upload should initialize");
        let alpha_limbs = alpha.to_m31_array().map(|limb| limb.0);
        let buffer = self
            .buffer
            .fri_fold_line_step_u32x4(&factors, alpha_limbs)
            .expect("Metal FRI line-fold step should succeed");
        Self {
            buffer,
            size: self.size / 2,
        }
    }

    pub fn fix_first_variable(&self, assignment: SecureField) -> Self {
        assert!(
            self.size >= 2,
            "Metal SecureFieldVec MLE fix-first-variable requires at least two evaluations"
        );
        let buffer = self
            .buffer
            .fix_first_variable_secure_field(assignment.to_m31_array().map(|limb| limb.0))
            .expect("Metal SecureFieldVec MLE fix-first-variable should succeed");
        Self::from_buffer(buffer)
    }

    pub fn gkr_next_grand_product_layer(&self) -> Self {
        let buffer = self
            .buffer
            .gkr_next_grand_product_layer()
            .expect("Metal GKR next grand-product layer should succeed");
        Self::from_buffer(buffer)
    }

    pub fn gkr_next_logup_generic_layer(numerators: &Self, denominators: &Self) -> (Self, Self) {
        let (next_numerators, next_denominators) =
            U32Buffer::gkr_next_logup_generic_layer(&numerators.buffer, &denominators.buffer)
                .expect("Metal GKR next generic layer should succeed");
        (
            Self::from_buffer(next_numerators),
            Self::from_buffer(next_denominators),
        )
    }

    pub fn gkr_next_logup_singles_layer(denominators: &Self) -> (Self, Self) {
        let (next_numerators, next_denominators) =
            U32Buffer::gkr_next_logup_singles_layer(&denominators.buffer)
                .expect("Metal GKR next singles layer should succeed");
        (
            Self::from_buffer(next_numerators),
            Self::from_buffer(next_denominators),
        )
    }

    pub fn gkr_sum_grand_product(
        eq_evals: &Self,
        input_layer: &Self,
    ) -> (SecureField, SecureField) {
        let (eval_at_0, eval_at_2) =
            U32Buffer::gkr_sum_grand_product(&eq_evals.buffer, &input_layer.buffer)
                .expect("Metal GKR grand-product sum should succeed");
        (
            SecureField::from_u32_unchecked(eval_at_0[0], eval_at_0[1], eval_at_0[2], eval_at_0[3]),
            SecureField::from_u32_unchecked(eval_at_2[0], eval_at_2[1], eval_at_2[2], eval_at_2[3]),
        )
    }

    pub fn gkr_sum_logup_generic(
        eq_evals: &Self,
        numerators: &Self,
        denominators: &Self,
        lambda: SecureField,
    ) -> (SecureField, SecureField) {
        let (eval_at_0, eval_at_2) = U32Buffer::gkr_sum_logup_generic(
            &eq_evals.buffer,
            &numerators.buffer,
            &denominators.buffer,
            lambda.to_m31_array().map(|limb| limb.0),
        )
        .expect("Metal GKR generic sum should succeed");
        (
            SecureField::from_u32_unchecked(eval_at_0[0], eval_at_0[1], eval_at_0[2], eval_at_0[3]),
            SecureField::from_u32_unchecked(eval_at_2[0], eval_at_2[1], eval_at_2[2], eval_at_2[3]),
        )
    }

    pub fn gkr_sum_logup_singles(
        eq_evals: &Self,
        denominators: &Self,
        lambda: SecureField,
    ) -> (SecureField, SecureField) {
        let (eval_at_0, eval_at_2) = U32Buffer::gkr_sum_logup_singles(
            &eq_evals.buffer,
            &denominators.buffer,
            lambda.to_m31_array().map(|limb| limb.0),
        )
        .expect("Metal GKR singles sum should succeed");
        (
            SecureField::from_u32_unchecked(eval_at_0[0], eval_at_0[1], eval_at_0[2], eval_at_0[3]),
            SecureField::from_u32_unchecked(eval_at_2[0], eval_at_2[1], eval_at_2[2], eval_at_2[3]),
        )
    }
}

impl Clone for SecureFieldVec {
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
            size: self.size,
        }
    }
}
