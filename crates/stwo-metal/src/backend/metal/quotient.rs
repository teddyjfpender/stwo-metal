use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo_metal_sys::metal::{MetalError, U32Buffer};

use crate::stwo_metal::secure_field_vec::SecureFieldVec;

#[derive(Clone, Copy, Debug)]
pub struct MetalWideFibonacciQuotientRequest<'a> {
    pub trace_evaluations: &'a [&'a [BaseField]],
    pub random_coeff_powers: &'a [SecureField],
    pub denominator_inverses: &'a [BaseField],
    pub domain_log_size: u32,
    pub eval_domain_log_size: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetalWideFibonacciQuotientError {
    NotEnoughTraceColumns {
        trace_columns: usize,
    },
    InconsistentTraceColumnLength {
        expected_len: usize,
        actual_len: usize,
        column_index: usize,
    },
    InvalidDomainLogSizes {
        domain_log_size: u32,
        eval_domain_log_size: u32,
    },
    RandomCoeffCountMismatch {
        expected_len: usize,
        actual_len: usize,
    },
    DenominatorCountMismatch {
        expected_len: usize,
        actual_len: usize,
    },
    Runtime {
        message: String,
    },
}

impl core::fmt::Display for MetalWideFibonacciQuotientError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotEnoughTraceColumns { trace_columns } => write!(
                f,
                "wide-fibonacci quotient accumulation requires at least three trace columns, got {trace_columns}"
            ),
            Self::InconsistentTraceColumnLength {
                expected_len,
                actual_len,
                column_index,
            } => write!(
                f,
                "wide-fibonacci quotient accumulation requires equal trace column lengths of {expected_len}, but column {column_index} had length {actual_len}"
            ),
            Self::InvalidDomainLogSizes {
                domain_log_size,
                eval_domain_log_size,
            } => write!(
                f,
                "wide-fibonacci quotient accumulation requires eval_domain_log_size >= domain_log_size, got {eval_domain_log_size} < {domain_log_size}"
            ),
            Self::RandomCoeffCountMismatch {
                expected_len,
                actual_len,
            } => write!(
                f,
                "wide-fibonacci quotient accumulation requires {expected_len} random coefficients, got {actual_len}"
            ),
            Self::DenominatorCountMismatch {
                expected_len,
                actual_len,
            } => write!(
                f,
                "wide-fibonacci quotient accumulation requires {expected_len} denominator inverses, got {actual_len}"
            ),
            Self::Runtime { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for MetalWideFibonacciQuotientError {}

impl From<MetalError> for MetalWideFibonacciQuotientError {
    fn from(value: MetalError) -> Self {
        Self::Runtime {
            message: value.message().to_string(),
        }
    }
}

#[derive(Debug)]
pub struct MetalWideFibonacciQuotients {
    values: SecureFieldVec,
    eval_domain_size: usize,
}

impl MetalWideFibonacciQuotients {
    pub fn len(&self) -> usize {
        self.eval_domain_size
    }

    pub fn is_empty(&self) -> bool {
        self.eval_domain_size == 0
    }

    pub fn value(&self, row_index: usize) -> SecureField {
        self.values.get_data(row_index)
    }

    pub fn values(&self) -> &SecureFieldVec {
        &self.values
    }

    pub fn to_vec(&self) -> Vec<SecureField> {
        self.values.to_vec()
    }

    pub fn to_coordinate_columns(&self) -> [Vec<BaseField>; 4] {
        let mut columns = std::array::from_fn(|_| Vec::with_capacity(self.eval_domain_size));
        for value in self.values.to_vec() {
            for (column, limb) in columns.iter_mut().zip(value.to_m31_array()) {
                column.push(limb);
            }
        }
        columns
    }
}

pub fn accumulate_wide_fibonacci_quotients(
    request: MetalWideFibonacciQuotientRequest<'_>,
) -> Result<MetalWideFibonacciQuotients, MetalWideFibonacciQuotientError> {
    if request.trace_evaluations.len() < 3 {
        return Err(MetalWideFibonacciQuotientError::NotEnoughTraceColumns {
            trace_columns: request.trace_evaluations.len(),
        });
    }
    if request.eval_domain_log_size < request.domain_log_size {
        return Err(MetalWideFibonacciQuotientError::InvalidDomainLogSizes {
            domain_log_size: request.domain_log_size,
            eval_domain_log_size: request.eval_domain_log_size,
        });
    }

    let eval_domain_size = 1usize << request.eval_domain_log_size;
    for (column_index, column) in request.trace_evaluations.iter().enumerate() {
        if column.len() != eval_domain_size {
            return Err(
                MetalWideFibonacciQuotientError::InconsistentTraceColumnLength {
                    expected_len: eval_domain_size,
                    actual_len: column.len(),
                    column_index,
                },
            );
        }
    }

    let expected_coeff_len = request.trace_evaluations.len() - 2;
    if request.random_coeff_powers.len() != expected_coeff_len {
        return Err(MetalWideFibonacciQuotientError::RandomCoeffCountMismatch {
            expected_len: expected_coeff_len,
            actual_len: request.random_coeff_powers.len(),
        });
    }

    let expected_denominator_len =
        1usize << (request.eval_domain_log_size - request.domain_log_size);
    if request.denominator_inverses.len() != expected_denominator_len {
        return Err(MetalWideFibonacciQuotientError::DenominatorCountMismatch {
            expected_len: expected_denominator_len,
            actual_len: request.denominator_inverses.len(),
        });
    }

    let mut trace_values = Vec::with_capacity(
        request
            .trace_evaluations
            .len()
            .checked_mul(eval_domain_size)
            .unwrap(),
    );
    for column in request.trace_evaluations {
        trace_values.extend(column.iter().map(|value| value.0));
    }
    let random_coeff_powers = request
        .random_coeff_powers
        .iter()
        .flat_map(|value| value.to_m31_array().map(|limb| limb.0))
        .collect::<Vec<_>>();
    let denominator_inverses = request
        .denominator_inverses
        .iter()
        .map(|value| value.0)
        .collect::<Vec<_>>();

    let trace_values = U32Buffer::from_slice(&trace_values)?;
    let random_coeff_powers = U32Buffer::from_slice(&random_coeff_powers)?;
    let denominator_inverses = U32Buffer::from_slice(&denominator_inverses)?;
    let values = U32Buffer::accumulate_wide_fibonacci_quotients(
        &trace_values,
        &random_coeff_powers,
        &denominator_inverses,
        request.domain_log_size,
        request.eval_domain_log_size,
        expected_coeff_len as u32,
    )?;

    Ok(MetalWideFibonacciQuotients {
        values: SecureFieldVec::from_buffer(values),
        eval_domain_size,
    })
}
