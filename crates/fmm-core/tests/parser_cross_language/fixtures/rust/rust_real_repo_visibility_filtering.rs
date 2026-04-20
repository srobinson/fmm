
use crate::error::AppError;

pub fn public_api() -> String {
    internal_helper()
}

pub(crate) fn internal_helper() -> String {
    "internal".to_string()
}

pub(super) fn parent_only() -> bool {
    true
}

fn totally_private() -> i32 {
    42
}

pub struct PublicType {
    pub(crate) internal_field: String,
}
