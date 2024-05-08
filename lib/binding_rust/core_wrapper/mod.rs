#[cfg(feature = "capi")]
pub(crate) mod capi;

pub(crate) mod alloc;
pub(crate) mod util;

pub mod core {
    #![allow(dead_code)]
    #![allow(mutable_transmutes)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(non_upper_case_globals)]
    #![allow(unused_assignments)]
    #![allow(unused_mut)]
    #![allow(ambiguous_glob_reexports)]
    #![allow(invalid_reference_casting)]

    pub use c2rust_bitfields::*;

    #[allow(unused, clippy::all)]
    pub(crate) mod api_raw;
    #[allow(unused, clippy::all)]
    pub(crate) mod get_changed_ranges;
    #[allow(unused, clippy::all)]
    pub(crate) mod imported_parser;
    #[allow(unused, clippy::all)]
    pub(crate) mod language;
    #[allow(unused, clippy::all)]
    pub(crate) mod lexer;
    #[allow(unused, clippy::all)]
    pub(crate) mod node;
    #[allow(unused, clippy::all)]
    pub(crate) mod parser;
    #[allow(unused, clippy::all)]
    pub(crate) mod query;
    #[allow(unused, clippy::all)]
    pub(crate) mod stack;
    #[allow(unused, clippy::all)]
    pub(crate) mod subtree;
    #[allow(unused, clippy::all)]
    pub(crate) mod tree;
    #[allow(unused, clippy::all)]
    pub(crate) mod tree_cursor;
    #[allow(unused, clippy::all)]
    pub(crate) mod wasm_store;

    pub(crate) use crate::core_wrapper::{alloc, util};

    #[no_mangle]
    pub static ts_current_free: unsafe extern "C" fn(*mut std::ffi::c_void) = ts_free;

    #[no_mangle]
    pub static ts_current_malloc: unsafe extern "C" fn(usize) -> *mut std::ffi::c_void = ts_malloc;

    #[no_mangle]
    pub static ts_current_calloc: unsafe extern "C" fn(usize, usize) -> *mut std::ffi::c_void =
        ts_calloc;

    #[no_mangle]
    pub static ts_current_realloc: unsafe extern "C" fn(
        *mut std::ffi::c_void,
        usize,
    ) -> *mut std::ffi::c_void = ts_realloc;

    #[cfg(feature = "capi")]
    #[cfg(unix)]
    #[no_mangle]
    pub unsafe extern "C" fn _ts_dup(mut file_descriptor: util::libc::c_int) -> util::libc::c_int {
        return util::libc::dup(file_descriptor);
    }

    #[cfg(feature = "capi")]
    #[cfg(windows)]
    #[no_mangle]
    pub unsafe extern "C" fn _ts_dup(
        mut file_descriptor: std::os::windows::raw::HANDLE,
    ) -> util::libc::c_int {
        panic!("Not implemented for Windows");
    }

    #[cfg(feature = "capi")]
    #[cfg(unix)]
    #[no_mangle]
    pub unsafe extern "C" fn ts_tree_print_dot_graph(
        mut self_0: *const TSTree,
        mut file_descriptor: util::libc::c_int,
    ) {
        let mut file: *mut util::libc::FILE = util::libc::fdopen(
            _ts_dup(file_descriptor),
            b"a\0" as *const u8 as *const util::libc::c_char,
        );
        ts_subtree_print_dot_graph((*self_0).root, (*self_0).language, file);
        util::libc::fclose(file);
    }

    #[cfg(feature = "capi")]
    #[cfg(not(unix))]
    #[no_mangle]
    pub unsafe extern "C" fn ts_tree_print_dot_graph(
        mut _self_0: *const TSTree,
        mut _fd: util::libc::c_int,
    ) {
        panic!("Not implemented for Windows");
    }

    pub use alloc::*;

    pub use api_raw::*;
    pub use get_changed_ranges::*;
    pub use imported_parser::*;
    pub use language::*;
    pub use lexer::*;
    pub use node::*;
    pub use parser::*;
    pub use query::*;
    pub use stack::*;
    pub use subtree::*;
    pub use tree::*;
    pub use tree_cursor::*;
    pub use wasm_store::*;

    #[cfg(feature = "capi")]
    pub use crate::core_wrapper::capi::*;

    pub(crate) mod defines;
    pub use defines::*;

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct TSLookaheadIterator {
        _unused: [u8; 0],
    }

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct TSWasmStore {}
}
