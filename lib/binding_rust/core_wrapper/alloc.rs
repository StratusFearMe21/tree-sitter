use std::{alloc, alloc::Layout, ffi, mem::align_of, ptr};

pub unsafe fn ts_set_allocator(
    _new_malloc: Option<unsafe extern "C" fn(usize) -> *mut std::ffi::c_void>,
    _new_calloc: Option<unsafe extern "C" fn(usize, usize) -> *mut std::ffi::c_void>,
    _new_realloc: Option<
        unsafe extern "C" fn(*mut std::ffi::c_void, usize) -> *mut std::ffi::c_void,
    >,
    _new_free: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
) {
}
// Private
#[inline]
pub(crate) unsafe extern "C" fn ts_malloc(size: usize) -> *mut ffi::c_void {
    if size == 0 {
        return ptr::null_mut();
    }

    let (layout, offset_data) = Layout::new::<Layout>()
        .extend(Layout::from_size_align(size, align_of::<*const u8>() * 2).unwrap())
        .unwrap();
    let result = alloc::alloc(layout);
    if result.is_null() {
        panic!("tree-sitter failed to allocate {} bytes", size);
    }

    *(result as *mut Layout) = layout;
    (result as *mut u8).offset(offset_data as isize) as *mut ffi::c_void
}

#[inline]
pub(crate) unsafe extern "C" fn ts_calloc(count: usize, size: usize) -> *mut ffi::c_void {
    if count == 0 || size == 0 {
        return ptr::null_mut();
    }

    let (layout, offset_data) = Layout::new::<Layout>()
        .extend(Layout::from_size_align(size * count, align_of::<*const u8>() * 2).unwrap())
        .unwrap();
    let result = alloc::alloc_zeroed(layout);
    if result.is_null() {
        panic!("tree-sitter failed to allocate {} bytes", size);
    }

    *(result as *mut Layout) = layout;
    (result as *mut u8).offset(offset_data as isize) as *mut ffi::c_void
}

#[inline]
pub(crate) unsafe extern "C" fn ts_realloc(
    buffer: *mut ffi::c_void,
    size: usize,
) -> *mut ffi::c_void {
    if buffer.is_null() {
        ts_malloc(size)
    } else if size == 0 {
        ts_free(buffer);
        ptr::null_mut()
    } else {
        let (_, layout_offset) = Layout::new::<Layout>()
            .extend(Layout::from_size_align(0, align_of::<*const u8>() * 2).unwrap())
            .unwrap();

        let buffer = (buffer as *mut u8).offset(-(layout_offset as isize));
        let layout = *(buffer as *mut Layout);

        let (new_layout, offset_data) = Layout::new::<Layout>()
            .extend(Layout::from_size_align(size, layout.align()).unwrap())
            .unwrap();

        let result = alloc::realloc(buffer, layout, new_layout.size());
        if result.is_null() {
            panic!("tree-sitter failed to reallocate {} bytes", size);
        }

        *(result as *mut Layout) = new_layout;
        (result as *mut u8).offset(offset_data as isize) as *mut ffi::c_void
    }
}

#[inline]
pub(crate) unsafe extern "C" fn ts_free(buffer: *mut ffi::c_void) {
    if buffer.is_null() {
        return;
    }

    let (_, layout_offset) = Layout::new::<Layout>()
        .extend(Layout::from_size_align(0, align_of::<*const u8>() * 2).unwrap())
        .unwrap();

    let buffer = (buffer as *mut u8).offset(-(layout_offset as isize));
    let layout = *(buffer as *mut Layout);

    alloc::dealloc(buffer, layout);
}
