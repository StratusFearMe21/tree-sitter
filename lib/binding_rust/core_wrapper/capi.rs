use std::{
    convert::From,
    ffi::c_void,
    io::{self, Write},
};

use errno::errno;

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct CFile(*mut libc::FILE);

impl CFile {
    pub(crate) fn new(file: *mut libc::FILE) -> Self {
        Self(file)
    }
}

impl Write for CFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        assert!(!self.0.is_null());
        let written = unsafe { libc::fwrite(buf.as_ptr() as *const c_void, buf.len(), 1, self.0) };

        if buf.len() == written {
            Ok(written)
        } else if unsafe { libc::feof(self.0) } != 0 {
            Err(io::ErrorKind::UnexpectedEof.into())
        } else {
            let err = unsafe { libc::ferror(self.0) };

            if err == 0 {
                Ok(written)
            } else {
                Err(io::Error::from_raw_os_error(err))
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        assert!(!self.0.is_null());
        match unsafe { libc::fflush(self.0) } {
            0 => Ok(()),
            libc::EOF => {
                let err = errno();
                Err(io::Error::from_raw_os_error(err.0))
            }
            _ => unreachable!(),
        }
    }
}

impl From<*mut libc::FILE> for CFile {
    fn from(file: *mut libc::FILE) -> Self {
        Self(file)
    }
}
