fn main() {
    let settings = rv_core::settings::Settings::default();
    let xml = quick_xml::se::to_string(&settings).unwrap();
    println!("{}", xml);
}