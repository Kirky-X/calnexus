//! CalNexus 计算域集合。
pub mod arithmetic;
pub mod complex;
pub mod scientific;
pub use arithmetic::ArithmeticDomain;
pub use complex::ComplexDomain;
pub use scientific::ScientificDomain;
