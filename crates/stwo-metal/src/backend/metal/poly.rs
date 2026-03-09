use crate::stwo_metal::base_field_vec::BaseFieldVec;

pub fn permute_coset_to_circle_domain_bit_reversed(values: &BaseFieldVec) -> BaseFieldVec {
    values.coset_to_circle_domain_bit_reversed()
}
