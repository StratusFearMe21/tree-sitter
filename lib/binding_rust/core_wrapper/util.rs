use std::{
    cmp::Ordering,
    fmt,
    io::{self, ErrorKind, Write},
    os::raw::{c_char, c_int, c_uint},
    sync::atomic::{AtomicU32, AtomicUsize},
};

use instant::Instant;

use crate::core::parser::TSClock;

// the default transpilation uses c_ulong, but we want a proper u64
#[allow(non_camel_case_types)]
pub type uint64_t = u64;

#[allow(non_camel_case_types)]
pub type size_t = usize;

pub mod libc {
    #[cfg(feature = "capi")]
    pub use libc::{dup, fclose, fdopen, fputc, fputs, FILE};

    #[cfg(not(feature = "capi"))]
    pub fn fdopen(_fd: c_int, _name: *const c_char) -> *mut FILE {
        panic!("fdopen requires the `capi` feature");
    }

    #[cfg(not(feature = "capi"))]
    pub fn dup(_fd: c_int) -> c_int {
        panic!("dup requires the `capi` feature");
    }

    #[cfg(not(feature = "capi"))]
    pub fn fclose(_fd: *mut FILE) {
        panic!("fclose requires the `capi` feature");
    }

    #[cfg(not(feature = "capi"))]
    pub fn fputc(_c: c_int, _stream: *mut FILE) -> c_int {
        panic!("fputc requires the `capi` feature");
    }

    #[cfg(not(feature = "capi"))]
    pub fn fputs(_s: *const c_char, _stream: *mut FILE) -> c_int {
        panic!("fputs requires the `capi` feature");
    }

    pub use std::os::raw::*;

    // there is some confustion between long long and long
    // in the c2rust output, so we need to fix it
    #[allow(non_camel_case_types)]
    pub type c_ulong = usize;

    #[cfg(not(feature = "capi"))]
    pub type FILE = u8;
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct StackElement<T> {
    pub contents: T,
    pub size: u32,
    pub capacity: u32,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union LongShortData {
    pub long_data: *mut std::os::raw::c_char,
    pub short_data: [std::os::raw::c_char; 24],
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct ParseActionEntry {
    pub count: u8,
    pub reusable: bool,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct ParseActionReduce {
    pub type_: u8,
    pub child_count: u8,
    pub symbol: u16,
    pub dynamic_precedence: i16,
    pub production_id: u16,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct ParseActionShift {
    pub type_: u8,
    pub state: u16,
    pub extra: bool,
    pub repetition: bool,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct ExternalScanner {
    pub states: *const bool,
    pub symbol_map: *const u16,
    pub create: Option<unsafe extern "C" fn() -> *mut libc::c_void>,
    pub destroy: Option<unsafe extern "C" fn(*mut libc::c_void) -> ()>,
    pub scan: Option<
        unsafe extern "C" fn(*mut libc::c_void, *mut crate::core::TSLexer, *const bool) -> bool,
    >,
    pub serialize:
        Option<unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_char) -> libc::c_uint>,
    pub deserialize:
        Option<unsafe extern "C" fn(*mut libc::c_void, *const libc::c_char, libc::c_uint) -> ()>,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union ScannerStateWithLookahead {
    pub c2rust_unnamed: ScannerStateLookaheadMeta,
    pub external_scanner_state: crate::core::ExternalScannerState,
    pub lookahead_char: std::os::raw::c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct ScannerStateLookaheadMeta {
    pub visible_child_count: u32,
    pub named_child_count: u32,
    pub visible_descendant_count: u32,
    pub dynamic_precedence: std::os::raw::c_int,
    pub repeat_depth: u16,
    pub production_id: u16,
    pub first_leaf: ScannerStateLookaheadFirstLeaf,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct ScannerStateLookaheadFirstLeaf {
    pub symbol: std::os::raw::c_ushort,
    pub parse_state: std::os::raw::c_ushort,
}

/// Simple replacement for libc::strncmp
///
/// Safety: both `a` and `b` must point to valid C strings, and no alive unique reference must
/// point to the underlying memory. Neither `a + n` or `b + n` can overflow.
pub(crate) unsafe fn strncmp(mut s1: *const c_char, mut s2: *const c_char, mut n: usize) -> c_int {
    while n > 0 {
        let a = *s1;
        let b = *s2;

        match a.cmp(&b) {
            Ordering::Less => return -1,
            Ordering::Greater => return 1,
            Ordering::Equal => {}
        }

        if a == 0 {
            return 0;
        }

        s1 = s1.add(1);
        s2 = s2.add(1);
        n -= 1;
    }

    0
}

#[derive(Debug)]
pub struct WriteCounter<W> {
    inner: W,
    count: usize,
}

impl<W: Write> WriteCounter<W> {
    pub(crate) fn new(write: W) -> Self {
        Self {
            inner: write,
            count: 0,
        }
    }
}

impl<W> WriteCounter<W> {
    pub fn count(&self) -> usize {
        self.count
    }
}

impl<W> Write for WriteCounter<W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self.inner.write(buf) {
            Ok(count) => {
                self.count += count;
                Ok(count)
            }
            err => err,
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

struct VoidWriter;

impl Write for VoidWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

macro_rules! try_fmt_write_zero {
    ($counter:expr, $args:expr) => {
        match $counter.write_fmt($args) {
            Ok(()) => Ok(()),
            Err(err) if matches!(err.kind(), ErrorKind::WriteZero) => return Ok($counter.count()),
            Err(err) => Err(err),
        }
    };
}

pub(crate) unsafe fn snwrite_runtime(
    string: *mut c_char,
    size: usize,
    args: fmt::Arguments,
) -> io::Result<usize> {
    let raw = snwrite_inner(
        WriteCounter::new(std::slice::from_raw_parts_mut(string as *mut u8, size)),
        args,
    )?;

    // TODO(shadaj): more precise semantics
    if size == 1 {
        snwrite_inner(WriteCounter::new(VoidWriter), args)
    } else {
        Ok(raw)
    }
}

fn snwrite_inner<W>(mut counter: WriteCounter<W>, args: fmt::Arguments) -> io::Result<usize>
where
    W: Write,
{
    try_fmt_write_zero!(counter, args)?;
    try_fmt_write_zero!(counter, format_args!("\0"))?;

    Ok(counter.count() - 1)
}

macro_rules! snwrite {
    ($string:expr, $size:expr, $($arg:tt)*) => {
        crate::core::util::snwrite_runtime($string, $size, ::std::format_args!($($arg)*))
    }
}

pub(crate) use snwrite;

#[cfg(feature = "capi")]
macro_rules! fwrite {
    ($to:expr, $($arg:tt)*) => {
        {
            let formatted = ::std::format!($($arg)*);
            std::io::Write::write(&mut crate::core_wrapper::capi::CFile::new($to), formatted.as_bytes())
                .map(|_| formatted.as_bytes().len())
        }
    }
}

#[cfg(not(feature = "capi"))]
macro_rules! fwrite {
    ($to:expr, $($arg:tt)*) => {{
        panic!("fwrite is only available with the `capi` feature");
        Result::<usize, ()>::Ok(0)
    }};
}

pub(crate) use fwrite;

#[cfg(feature = "capi")]
#[repr(transparent)]
pub struct File(#[cfg(feature = "capi")] crate::core_wrapper::capi::CFile);

#[cfg(feature = "capi")]
impl Write for File {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[cfg(feature = "capi")]
impl<T> From<T> for File
where
    T: Into<crate::core_wrapper::capi::CFile>,
{
    fn from(file: T) -> Self {
        Self(file.into())
    }
}

// Credits: (musl libc library)[https://www.musl-libc.org/]
pub(crate) fn iswspace(wc: c_int) -> c_int {
    const SPACES: [c_int; 21] = [
        b' ' as _, b'\t' as _, b'\n' as _, b'\r' as _, 11, 12, 0x0085, 0x2000, 0x2001, 0x2002,
        0x2003, 0x2004, 0x2005, 0x2006, 0x2008, 0x2009, 0x200a, 0x2028, 0x2029, 0x205f, 0x3000,
    ];

    (wc != 0 && SPACES.contains(&wc)) as c_int
}

// Credits: (musl libc library)[https://www.musl-libc.org/]
pub(crate) fn iswdigit(wc: c_int) -> c_int {
    (wc.wrapping_sub(b'0' as c_int) < 10) as c_int
}

// Credits: (musl libc library)[https://www.musl-libc.org/]
pub(crate) fn iswalpha(wc: c_int) -> c_int {
    const TABLE: [c_int; 3904] = include!("./alpha.data");

    if wc < 0x20000 {
        ((TABLE[(TABLE[(wc >> 8) as usize].wrapping_mul(32) + ((wc & 255) >> 3)) as usize]
            >> (wc & 7))
            & 1) as c_int
    } else if wc < 0x2fffe {
        1
    } else {
        0
    }
}

// Credits: (musl libc library)[https://www.musl-libc.org/]
pub(crate) fn iswalnum(wc: c_int) -> c_int {
    (iswdigit(wc) != 0 || iswalpha(wc) != 0) as c_int
}

// Credits: (musl libc library)[https://www.musl-libc.org/]
pub(crate) fn isprint(c: c_int) -> c_int {
    return ((c as c_uint) - 0x20 < 0x5f) as c_int;
}

#[inline]
pub(crate) unsafe extern "C" fn atomic_inc(p: *const u32) -> u32 {
    (*(p as *const AtomicU32))
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        .wrapping_add(1)
}

#[inline]
pub(crate) unsafe extern "C" fn atomic_dec(p: *mut u32) -> u32 {
    (*(p as *const AtomicU32))
        .fetch_sub(1, std::sync::atomic::Ordering::SeqCst)
        .wrapping_sub(1)
}

static EPOCH: once_cell::sync::Lazy<Instant> = once_cell::sync::Lazy::new(Instant::now);

#[inline]
pub(crate) unsafe extern "C" fn clock_now() -> TSClock {
    let now = EPOCH.elapsed();
    TSClock {
        // clock_is_null thinks zero seconds is null
        tv_sec: (now.as_secs() + 1) as libc::c_longlong,
        tv_nsec: now.subsec_nanos() as libc::c_long,
    }
}

#[inline]
pub(crate) unsafe extern "C" fn atomic_load(p: *const usize) -> usize {
    (*(p as *const AtomicUsize)).load(std::sync::atomic::Ordering::SeqCst)
}
