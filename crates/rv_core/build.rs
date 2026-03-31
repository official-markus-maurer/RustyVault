fn main() {
    let zlib_dir = "vendor/zlib-1.2.3";

    println!("cargo:rerun-if-changed={zlib_dir}/adler32.c");
    println!("cargo:rerun-if-changed={zlib_dir}/compress.c");
    println!("cargo:rerun-if-changed={zlib_dir}/crc32.c");
    println!("cargo:rerun-if-changed={zlib_dir}/deflate.c");
    println!("cargo:rerun-if-changed={zlib_dir}/trees.c");
    println!("cargo:rerun-if-changed={zlib_dir}/zutil.c");
    println!("cargo:rerun-if-changed={zlib_dir}/zlib.h");
    println!("cargo:rerun-if-changed={zlib_dir}/zconf.h");
    println!("cargo:rerun-if-changed={zlib_dir}/deflate.h");
    println!("cargo:rerun-if-changed={zlib_dir}/trees.h");
    println!("cargo:rerun-if-changed={zlib_dir}/zutil.h");

    cc::Build::new()
        .include(zlib_dir)
        .file(format!("{zlib_dir}/adler32.c"))
        .file(format!("{zlib_dir}/compress.c"))
        .file(format!("{zlib_dir}/crc32.c"))
        .file(format!("{zlib_dir}/deflate.c"))
        .file(format!("{zlib_dir}/trees.c"))
        .file(format!("{zlib_dir}/zutil.c"))
        .warnings(false)
        .compile("z123");
}
