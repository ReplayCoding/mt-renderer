mod read_struct;
mod hexdump;

#[macro_export]
macro_rules! get_enum_value {
    ($object:expr, $variant:path) => {{
        if let $variant(value_inner) = $object {
            value_inner
        } else {
            panic!("Expected type {}", stringify!($variant))
        }
    }};
}

pub use get_enum_value;
pub use read_struct::*;
pub use hexdump::*;
