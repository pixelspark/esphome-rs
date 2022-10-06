#![deny(clippy::pedantic)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::default_trait_access)]
#![allow(clippy::module_name_repetitions)]
//#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::struct_excessive_bools)]


mod api;
mod api_options;
pub mod connection;
pub mod device;
pub mod model;
pub use connection::*;
pub use device::*;
pub use model::*;
