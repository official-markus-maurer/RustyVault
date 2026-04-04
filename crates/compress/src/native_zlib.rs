use std::ffi::c_char;
use std::ffi::c_int;
use std::ffi::c_uint;
use std::ffi::c_void;

#[repr(C)]
struct ZStream {
    next_in: *mut u8,
    avail_in: c_uint,
    total_in: c_uint,
    next_out: *mut u8,
    avail_out: c_uint,
    total_out: c_uint,
    msg: *mut c_char,
    state: *mut c_void,
    zalloc: Option<unsafe extern "C" fn(*mut c_void, c_uint, c_uint) -> *mut c_void>,
    zfree: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    opaque: *mut c_void,
    data_type: c_int,
    adler: c_uint,
    reserved: c_uint,
}

unsafe extern "C" {
    fn deflateInit2_(
        strm: *mut ZStream,
        level: c_int,
        method: c_int,
        window_bits: c_int,
        mem_level: c_int,
        strategy: c_int,
        version: *const c_char,
        stream_size: c_int,
    ) -> c_int;
    fn deflate(strm: *mut ZStream, flush: c_int) -> c_int;
    fn deflateEnd(strm: *mut ZStream) -> c_int;
}

pub fn deflate_raw_best(data: &[u8]) -> Option<Vec<u8>> {
    const Z_OK: c_int = 0;
    const Z_FINISH: c_int = 4;
    const Z_STREAM_END: c_int = 1;
    const Z_DEFLATED: c_int = 8;
    const Z_DEFAULT_STRATEGY: c_int = 0;
    const Z_BEST_COMPRESSION: c_int = 9;
    const ZLIB_123_VERSION: &[u8] = b"1.2.3\0";

    unsafe extern "C" fn zlib_alloc(
        _opaque: *mut c_void,
        items: c_uint,
        size: c_uint,
    ) -> *mut c_void {
        unsafe { libc::malloc(items as usize * size as usize) }
    }

    unsafe extern "C" fn zlib_free(_opaque: *mut c_void, address: *mut c_void) {
        unsafe { libc::free(address) }
    }

    unsafe {
        let mut stream = ZStream {
            next_in: data.as_ptr() as *mut u8,
            avail_in: data.len().try_into().ok()?,
            total_in: 0,
            next_out: std::ptr::null_mut(),
            avail_out: 0,
            total_out: 0,
            msg: std::ptr::null_mut(),
            state: std::ptr::null_mut(),
            zalloc: Some(zlib_alloc),
            zfree: Some(zlib_free),
            opaque: std::ptr::null_mut(),
            data_type: 0,
            adler: 0,
            reserved: 0,
        };

        let init_result = deflateInit2_(
            &mut stream,
            Z_BEST_COMPRESSION,
            Z_DEFLATED,
            -15,
            8,
            Z_DEFAULT_STRATEGY,
            ZLIB_123_VERSION.as_ptr() as *const c_char,
            std::mem::size_of::<ZStream>() as c_int,
        );
        if init_result != Z_OK {
            return None;
        }

        let mut output = vec![
            0u8;
            data.len()
                .saturating_add(data.len() / 10)
                .saturating_add(64)
        ];
        let mut success = false;
        loop {
            if stream.total_out as usize == output.len() {
                output.resize(output.len().saturating_mul(2).max(64), 0);
            }

            stream.next_out = output[stream.total_out as usize..].as_mut_ptr();
            stream.avail_out = (output.len() - stream.total_out as usize).try_into().ok()?;

            let result = deflate(&mut stream, Z_FINISH);
            if result == Z_STREAM_END {
                success = true;
                break;
            }
            if result != Z_OK {
                break;
            }
        }

        let _ = deflateEnd(&mut stream);
        if !success {
            return None;
        }

        output.truncate(stream.total_out as usize);
        Some(output)
    }
}
