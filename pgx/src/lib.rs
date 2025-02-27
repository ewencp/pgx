/*
Portions Copyright 2019-2021 ZomboDB, LLC.
Portions Copyright 2021-2022 Technology Concepts & Design, Inc. <support@tcdi.com>

All rights reserved.

Use of this source code is governed by the MIT license that can be found in the LICENSE file.
*/
//! `pgx` is a framework for creating Postgres extensions in 100% Rust
//!
//! ## Example
//!
//! ```rust
//! use pgx::prelude::*;
//!
//! // Convert the input string to lowercase and return
//! #[pg_extern]
//! fn my_to_lowercase(input: &'static str) -> String {
//!     input.to_lowercase()
//! }
//!
//! ```
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::cast_ptr_alignment)]

#[macro_use]
extern crate bitflags;
extern crate alloc;
extern crate core;

// expose our various derive macros
pub use pgx_macros;
pub use pgx_macros::*;

/// The PGX prelude includes necessary imports to make extensions work.
pub mod prelude;

pub mod aggregate;
pub mod array;
pub mod atomics;
pub mod bgworkers;
pub mod callbacks;
pub mod datum;
pub mod enum_helper;
pub mod fcinfo;
pub mod ffi;
pub mod guc;
pub mod heap_tuple;
#[cfg(feature = "cshim")]
pub mod hooks;
pub mod htup;
pub mod inoutfuncs;
pub mod itemptr;
pub mod iter;
#[cfg(feature = "cshim")]
pub mod list;
pub mod lwlock;
pub mod memcxt;
pub mod misc;
#[cfg(feature = "cshim")]
pub mod namespace;
pub mod nodes;
pub mod pgbox;
pub mod rel;
pub mod shmem;
pub mod spi;
#[cfg(feature = "cshim")]
pub mod spinlock;
pub mod srf;
pub mod stringinfo;
pub mod trigger_support;
pub mod tupdesc;
pub mod varlena;
pub mod wrappers;
pub mod xid;

#[doc(hidden)]
pub use once_cell;

/// Not ready for public exposure.
mod layout;
mod slice;

pub use aggregate::*;
pub use atomics::*;
pub use callbacks::*;
pub use datum::*;
pub use enum_helper::*;
pub use fcinfo::*;
pub use guc::*;
#[cfg(feature = "cshim")]
pub use hooks::*;
pub use htup::*;
pub use inoutfuncs::*;
pub use itemptr::*;
#[cfg(feature = "cshim")]
pub use list::*;
pub use lwlock::*;
pub use memcxt::*;
#[cfg(feature = "cshim")]
pub use namespace::*;
pub use nodes::*;
pub use pgbox::*;
pub use rel::*;
pub use shmem::*;
pub use spi::Spi; // only Spi.  We don't want the top-level namespace polluted with spi::Result and spi::Error
pub use stringinfo::*;
pub use trigger_support::*;
pub use tupdesc::*;
pub use varlena::*;
pub use wrappers::*;
pub use xid::*;

pub use pgx_pg_sys as pg_sys; // the module only, not its contents

// and re-export these
pub use pg_sys::elog::PgLogLevel;
pub use pg_sys::errcodes::PgSqlErrorCode;
pub use pg_sys::oids::PgOid;
pub use pg_sys::panic::pgx_extern_c_guard;
pub use pg_sys::pg_try::PgTryBuilder;
pub use pg_sys::utils::name_data_to_str;
pub use pg_sys::PgBuiltInOids;
pub use pg_sys::{
    check_for_interrupts, debug1, debug2, debug3, debug4, debug5, ereport, error, function_name,
    info, log, notice, warning, FATAL, PANIC,
};
#[doc(hidden)]
pub use pgx_sql_entity_graph;

// Postgres v15 has the concept of an ABI "name".  The default is `b"PostgreSQL\0"` and this is the
// ABI that pgx extensions expect to be running under.  We will refuse to compile if it is detected
// that we're trying to be built against some other kind of "postgres" that has its own ABI name.
//
// Unless the compiling user explicitly told us that they're aware of this via `--features unsafe-postgres`.
#[cfg(all(feature = "pg15", not(feature = "unsafe-postgres")))]
const _: () = {
    // to appease `const`
    const fn same_slice(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let mut i = 0;
        while i < a.len() {
            if a[i] != b[i] {
                return false;
            }
            i += 1;
        }
        true
    }
    assert!(
        same_slice(pg_sys::FMGR_ABI_EXTRA, b"PostgreSQL\0"),
        "Unsupported Postgres ABI. Perhaps you need `--features unsafe-postgres`?",
    );
};

/// A macro for marking a library compatible with [`pgx`][crate].
///
/// <div class="example-wrap" style="display:inline-block">
/// <pre class="ignore" style="white-space:normal;font:inherit;">
///
/// **Note**: Every [`pgx`][crate] extension **must** have this macro called at top level (usually `src/lib.rs`) to be valid.
///
/// </pre></div>
///
/// This calls both [`pg_magic_func!()`](pg_magic_func) and [`pg_sql_graph_magic!()`](pg_sql_graph_magic).
#[macro_export]
macro_rules! pg_module_magic {
    () => {
        $crate::pg_magic_func!();
        $crate::pg_sql_graph_magic!();
    };
}

/// Create the `Pg_magic_func` required by PGX in extensions.
///
/// <div class="example-wrap" style="display:inline-block">
/// <pre class="ignore" style="white-space:normal;font:inherit;">
///
/// **Note**: Generally [`pg_module_magic`] is preferred, and results in this macro being called.
/// This macro should only be directly called in advanced use cases.
///
/// </pre></div>
///
/// Creates a “magic block” that describes the capabilities of the extension to
/// Postgres at runtime. From the [Dynamic Loading] section of the upstream documentation:
///
/// > To ensure that a dynamically loaded object file is not loaded into an incompatible
/// > server, PostgreSQL checks that the file contains a “magic block” with the appropriate
/// > contents. This allows the server to detect obvious incompatibilities, such as code
/// > compiled for a different major version of PostgreSQL. To include a magic block,
/// > write this in one (and only one) of the module source files, after having included
/// > the header `fmgr.h`:
/// >
/// > ```c
/// > PG_MODULE_MAGIC;
/// > ```
///
/// ## Acknowledgements
///
/// This macro was initially inspired from the `pg_module` macro by [Daniel Fagnan]
/// and expanded by [Benjamin Fry].
///
/// [Benjamin Fry]: https://github.com/bluejekyll/pg-extend-rs
/// [Daniel Fagnan]: https://github.com/thehydroimpulse/postgres-extension.rs
/// [Dynamic Loading]: https://www.postgresql.org/docs/current/xfunc-c.html#XFUNC-C-DYNLOAD
#[macro_export]
macro_rules! pg_magic_func {
    () => {
        #[no_mangle]
        #[allow(non_snake_case)]
        #[allow(unused)]
        #[link_name = "Pg_magic_func"]
        #[doc(hidden)]
        pub extern "C" fn Pg_magic_func() -> &'static pgx::pg_sys::Pg_magic_struct {
            use core::mem::size_of;
            use pgx;

            #[cfg(any(feature = "pg11", feature = "pg12"))]
            const MY_MAGIC: pgx::pg_sys::Pg_magic_struct = pgx::pg_sys::Pg_magic_struct {
                len: size_of::<pgx::pg_sys::Pg_magic_struct>() as i32,
                version: pgx::pg_sys::PG_VERSION_NUM as i32 / 100,
                funcmaxargs: pgx::pg_sys::FUNC_MAX_ARGS as i32,
                indexmaxkeys: pgx::pg_sys::INDEX_MAX_KEYS as i32,
                namedatalen: pgx::pg_sys::NAMEDATALEN as i32,
                float4byval: pgx::pg_sys::USE_FLOAT4_BYVAL as i32,
                float8byval: cfg!(target_pointer_width = "64") as i32,
            };

            #[cfg(any(feature = "pg13", feature = "pg14"))]
            const MY_MAGIC: pgx::pg_sys::Pg_magic_struct = pgx::pg_sys::Pg_magic_struct {
                len: size_of::<pgx::pg_sys::Pg_magic_struct>() as i32,
                version: pgx::pg_sys::PG_VERSION_NUM as i32 / 100,
                funcmaxargs: pgx::pg_sys::FUNC_MAX_ARGS as i32,
                indexmaxkeys: pgx::pg_sys::INDEX_MAX_KEYS as i32,
                namedatalen: pgx::pg_sys::NAMEDATALEN as i32,
                float8byval: cfg!(target_pointer_width = "64") as i32,
            };

            #[cfg(any(feature = "pg15"))]
            const MY_MAGIC: pgx::pg_sys::Pg_magic_struct = pgx::pg_sys::Pg_magic_struct {
                len: size_of::<pgx::pg_sys::Pg_magic_struct>() as i32,
                version: pgx::pg_sys::PG_VERSION_NUM as i32 / 100,
                funcmaxargs: pgx::pg_sys::FUNC_MAX_ARGS as i32,
                indexmaxkeys: pgx::pg_sys::INDEX_MAX_KEYS as i32,
                namedatalen: pgx::pg_sys::NAMEDATALEN as i32,
                float8byval: cfg!(target_pointer_width = "64") as i32,
                abi_extra: {
                    // we'll use what the bindings tell us, but if it ain't "PostgreSQL" then we'll
                    // raise a compilation error unless the `unsafe-postgres` feature is set
                    let magic = pgx::pg_sys::FMGR_ABI_EXTRA;
                    let mut abi = [0 as ::pgx::ffi::c_char; 32];
                    let mut i = 0;
                    while i < magic.len() {
                        abi[i] = magic[i] as _;
                        i += 1;
                    }
                    abi
                },
            };

            // go ahead and register our panic handler since Postgres
            // calls this function first
            pgx::initialize();

            // return the magic
            &MY_MAGIC
        }
    };
}

/// Create necessary extension-local internal marker for use with SQL generation.
///
/// <div class="example-wrap" style="display:inline-block">
/// <pre class="ignore" style="white-space:normal;font:inherit;">
///
/// **Note**: Generally [`pg_module_magic`] is preferred, and results in this macro being called.
/// This macro should only be directly called in advanced use cases.
///
/// </pre></div>
#[macro_export]
macro_rules! pg_sql_graph_magic {
    () => {
        // A marker which must exist in the root of the extension.
#[no_mangle]
        #[doc(hidden)]
        #[rustfmt::skip] // explicit extern "Rust" is more clear here
        pub extern "Rust" fn __pgx_marker(
            _: (),
        ) -> $crate::pgx_sql_entity_graph::ControlFile {
            use ::core::convert::TryFrom;
            let package_version = env!("CARGO_PKG_VERSION");
            let context = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/",
                env!("CARGO_CRATE_NAME"),
                ".control"
            ))
            .replace("@CARGO_VERSION@", package_version);

            let control_file =
                $crate::pgx_sql_entity_graph::ControlFile::try_from(context.as_str())
                    .expect("Could not parse control file, is it valid?");
            control_file
        }
    };
}

/// Initialize the extension with Postgres
///
/// Sets up panic handling with [`register_pg_guard_panic_hook()`] to ensure that a crash within
/// the extension does not adversely affect the entire server process.
///
/// ## Note
///
/// This is called automatically by the [`pg_module_magic!()`] macro and need not be called
/// directly.
#[allow(unused)]
pub fn initialize() {
    pg_sys::panic::register_pg_guard_panic_hook();
}
