fn main() {
    let base = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let zlib_dir = base.join("../rv_core/vendor/zlib-1.2.3");

    let tracked = [
        "adler32.c",
        "compress.c",
        "crc32.c",
        "deflate.c",
        "trees.c",
        "zutil.c",
        "zlib.h",
        "zconf.h",
        "deflate.h",
        "trees.h",
        "zutil.h",
    ];

    for file in tracked {
        println!("cargo:rerun-if-changed={}", zlib_dir.join(file).display());
    }

    cc::Build::new()
        .include(&zlib_dir)
        .file(zlib_dir.join("adler32.c"))
        .file(zlib_dir.join("compress.c"))
        .file(zlib_dir.join("crc32.c"))
        .file(zlib_dir.join("deflate.c"))
        .file(zlib_dir.join("trees.c"))
        .file(zlib_dir.join("zutil.c"))
        .warnings(false)
        .compile("z123");
}
