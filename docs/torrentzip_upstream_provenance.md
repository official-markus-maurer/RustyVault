## TorrentZip Upstream Provenance

- The strongest publicly visible match for the old developer's description is the DotNetZip / Ionic.Zlib deflate code family.
- DotNetZip's current README states its DeflateStream and GZipStream are based on a .NET port of zlib and are distributed as a standalone compression implementation.
- Public mirrors of DotNetZip-derived source contain the header text:
  - "This code is derived from the jzlib implementation of zlib."
  - "This program is based on zlib-1.1.3"
- Public jzlib sources also state they are based on zlib-1.1.3.

## Candidate Upstreams

- DotNetZip README:
  - https://github.com/DinoChiesa/dotnetzip-2025
- Ionic.Zlib source tree:
  - https://github.com/jstedfast/Ionic.Zlib
- jzlib reference tree:
  - https://github.com/ymnk/jzlib
- Public managed-code mirror showing the DotNetZip / jzlib / zlib-1.1.3 lineage in source headers:
  - http://fangmiaokeji.cn:30000/linweizhu/emergencysystem/-/raw/515faba137383f0503ca4ff487e55d2756798dae/Assets/Plugins/Best%20HTTP/Source/Decompression/ZTree.cs

## Meaning For RustyRoms

- The current Rust implementation is closest when it:
  - reuses existing raw deflate streams whenever possible
  - uses native zlib for newly compressed TorrentZip entries
- Full byte-for-byte historical parity for newly compressed entries likely requires porting or embedding the exact DotNetZip / Ionic.Zlib / jzlib-era compressor path rather than relying on a generic modern deflater.
