/// Simple XML serialization test binary.
///
/// `test_xml.rs` acts as a scratchpad binary to verify that `quick-xml` and `serde`
/// are correctly translating the `Settings` struct into the legacy `RomVault3cfg.xml` schema.
use rv_core::settings::Settings;

fn main() {
    let settings = Settings::default();
    let xml = quick_xml::se::to_string(&settings).unwrap();
    println!("{}", xml);
}
