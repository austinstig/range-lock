#[macro_use]
extern crate derive_new;

#[macro_use]
extern crate derive_is_enum_variant;

extern crate num;

// import modules
mod utils;
mod lock;

// types to use
pub use utils::{IntervalForest, RangeType};
pub use lock::{RangeLockResult, RangeLockGuard, RangeLock, LockGuard};