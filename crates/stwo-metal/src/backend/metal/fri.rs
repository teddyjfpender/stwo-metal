use stwo::core::fields::qm31::SecureField;
use stwo::core::fri::CIRCLE_TO_LINE_FOLD_STEP;
use stwo::core::poly::circle::CircleDomain;
use stwo::core::poly::line::LineDomain;
use stwo::core::utils::bit_reverse_index;

use crate::stwo_metal::secure_field_vec::SecureFieldVec;

pub fn fold_circle_into_line_first_layer(
    src: &SecureFieldVec,
    domain: CircleDomain,
    alpha: SecureField,
) -> SecureFieldVec {
    assert_eq!(
        src.len(),
        domain.size(),
        "FRI first-layer fold requires one secure-field value per domain point"
    );

    let inverse_y_factors = (0..(src.len() >> CIRCLE_TO_LINE_FOLD_STEP))
        .map(|i| {
            domain
                .at(bit_reverse_index(
                    i << CIRCLE_TO_LINE_FOLD_STEP,
                    domain.log_size(),
                ))
                .y
                .inverse()
                .0
        })
        .collect::<Vec<_>>();

    src.fold_circle_into_line_first_layer(&inverse_y_factors, alpha)
}

pub fn fold_line(
    src: &SecureFieldVec,
    mut domain: LineDomain,
    alpha: SecureField,
    fold_step: u32,
) -> SecureFieldVec {
    assert!(
        fold_step >= 1,
        "FRI line fold requires a positive fold_step"
    );
    assert_eq!(
        src.len(),
        domain.size(),
        "FRI line fold requires one secure-field value per domain point"
    );
    assert!(
        domain.log_size() >= fold_step,
        "FRI line fold cannot remove more layers than the domain contains"
    );

    let mut current = src.clone();
    let mut current_alpha = alpha;
    for _ in 0..fold_step {
        let inverse_x_factors = (0..(current.len() >> 1))
            .map(|i| {
                domain
                    .at(bit_reverse_index(i << 1, domain.log_size()))
                    .inverse()
                    .0
            })
            .collect::<Vec<_>>();
        current = current.fold_line_step(&inverse_x_factors, current_alpha);
        domain = domain.double();
        current_alpha = current_alpha * current_alpha;
    }

    current
}
