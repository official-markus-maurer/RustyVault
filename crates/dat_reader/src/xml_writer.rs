use crate::dat_store::DatHeader;
use std::io::{self, Write};

/// Logic for serializing a DAT AST back into an XML file.
///
/// `DatXmlWriter` takes the `DatHeader` and its underlying `DatNode` tree and
/// formats it into a standard Logiqx XML DAT format. This is heavily utilized
/// by the `dir2dat` tool and the `fix_dat_report` exporter.
///
/// Implementation notes:
/// - Emits a generic Logiqx/MAME-style XML representation based on the header flags.
pub struct DatXmlWriter;

#[derive(Default)]
pub struct DatXmlWriterOptions {
    pub emit_doctype: bool,
}

impl DatXmlWriter {
    pub fn write_dat<W: Write>(writer: &mut W, dat_header: &DatHeader) -> io::Result<()> {
        Self::write_dat_with_options(writer, dat_header, DatXmlWriterOptions::default())
    }

    pub fn write_dat_with_options<W: Write>(
        writer: &mut W,
        dat_header: &DatHeader,
        options: DatXmlWriterOptions,
    ) -> io::Result<()> {
        if dat_header.mame_xml {
            return Self::write_mame_xml(writer, dat_header);
        }
        writeln!(writer, "<?xml version=\"1.0\"?>")?;
        if options.emit_doctype {
            writeln!(
                writer,
                r#"<!DOCTYPE datafile SYSTEM "http://www.logiqx.com/Dats/datafile.dtd">"#
            )?;
        }
        writeln!(writer, "<datafile>")?;
        Self::write_header(writer, dat_header)?;
        Self::write_base(writer, &dat_header.base_dir, 1)?;
        writeln!(writer, "</datafile>")?;
        Ok(())
    }

    pub fn write_dat_newstyle<W: Write>(writer: &mut W, dat_header: &DatHeader) -> io::Result<()> {
        writeln!(writer, "<?xml version=\"1.0\"?>")?;
        writeln!(writer, "<RVDatFile>")?;
        Self::write_header(writer, dat_header)?;
        Self::write_base_newstyle(writer, &dat_header.base_dir, 1)?;
        writeln!(writer, "</RVDatFile>")?;
        Ok(())
    }

    fn write_mame_xml<W: Write>(writer: &mut W, dat_header: &DatHeader) -> io::Result<()> {
        writeln!(writer, "<?xml version=\"1.0\"?>")?;
        writeln!(
            writer,
            r#"<!DOCTYPE mame [
<!ELEMENT mame (machine+)>
	<!ATTLIST mame build CDATA #IMPLIED>
	<!ATTLIST mame debug (yes|no) "no">
	<!ELEMENT machine (description, year?, manufacturer?, biosset*, rom*, disk*, device_ref*, sample*, chip*, display*, sound?, input?, dipswitch*, configuration*, port*, adjuster*, driver?, feature*, device*, slot*, softwarelist*, ramoption*)>
		<!ATTLIST machine name CDATA #REQUIRED>
		<!ATTLIST machine isbios (yes|no) "no">
		<!ATTLIST machine isdevice (yes|no) "no">
		<!ATTLIST machine runnable (yes|no) "yes">
		<!ATTLIST machine cloneof CDATA #IMPLIED>
		<!ATTLIST machine romof CDATA #IMPLIED>
		<!ELEMENT description (#PCDATA)>
		<!ELEMENT year (#PCDATA)>
		<!ELEMENT manufacturer (#PCDATA)>
		<!ELEMENT rom EMPTY>
			<!ATTLIST rom name CDATA #REQUIRED>
			<!ATTLIST rom bios CDATA #IMPLIED>
			<!ATTLIST rom size CDATA #REQUIRED>
			<!ATTLIST rom crc CDATA #IMPLIED>
			<!ATTLIST rom sha1 CDATA #IMPLIED>
			<!ATTLIST rom merge CDATA #IMPLIED>
			<!ATTLIST rom region CDATA #IMPLIED>
			<!ATTLIST rom offset CDATA #IMPLIED>
			<!ATTLIST rom status (baddump|nodump|good) "good">
			<!ATTLIST rom optional (yes|no) "no">
		<!ELEMENT disk EMPTY>
			<!ATTLIST disk name CDATA #REQUIRED>
			<!ATTLIST disk sha1 CDATA #IMPLIED>
			<!ATTLIST disk merge CDATA #IMPLIED>
			<!ATTLIST disk region CDATA #IMPLIED>
			<!ATTLIST disk index CDATA #IMPLIED>
			<!ATTLIST disk writable (yes|no) "no">
			<!ATTLIST disk status (baddump|nodump|good) "good">
			<!ATTLIST disk optional (yes|no) "no">
		<!ELEMENT device_ref EMPTY>
			<!ATTLIST device_ref name CDATA #REQUIRED>
]>

"#
        )?;
        writeln!(
            writer,
            "<mame build=\"{}\">",
            Self::etxt(dat_header.name.as_deref().unwrap_or(""), true)
        )?;
        Self::write_base_mame(writer, &dat_header.base_dir, 1)?;
        writeln!(writer, "</mame>")?;
        Ok(())
    }

    fn write_header<W: Write>(writer: &mut W, dat_header: &DatHeader) -> io::Result<()> {
        writeln!(writer, "\t<header>")?;

        fn write_node<W: Write>(
            writer: &mut W,
            name: &str,
            val: &Option<String>,
            mame: bool,
        ) -> io::Result<()> {
            let Some(v) = val.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
                return Ok(());
            };
            writeln!(
                writer,
                "\t\t<{}>{}</{}>",
                name,
                DatXmlWriter::etxt(v, mame),
                name
            )
        }

        write_node(writer, "id", &dat_header.id, false)?;
        write_node(writer, "name", &dat_header.name, false)?;
        write_node(writer, "type", &dat_header.type_, false)?;
        write_node(writer, "rootdir", &dat_header.root_dir, false)?;
        write_node(writer, "description", &dat_header.description, false)?;
        write_node(writer, "category", &dat_header.category, false)?;
        write_node(writer, "version", &dat_header.version, false)?;
        write_node(writer, "date", &dat_header.date, false)?;
        write_node(writer, "author", &dat_header.author, false)?;
        write_node(writer, "email", &dat_header.email, false)?;
        write_node(writer, "homepage", &dat_header.homepage, false)?;
        write_node(writer, "url", &dat_header.url, false)?;
        write_node(writer, "comment", &dat_header.comment, false)?;

        write!(writer, "\t\t<romvault")?;
        if let Some(h) = dat_header
            .header
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            write!(writer, " header=\"{}\"", Self::etxt(h, false))?;
        }
        if let Some(c) = dat_header
            .compression
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            write!(writer, " forcepacking=\"{}\"", Self::etxt(c, false))?;
        }
        writeln!(writer, "/>")?;

        writeln!(writer, "\t</header>")?;
        Ok(())
    }

    fn write_base<W: Write>(
        writer: &mut W,
        dir: &crate::dat_store::DatDir,
        indent: usize,
    ) -> io::Result<()> {
        let pad = "\t".repeat(indent);
        let child_pad = "\t".repeat(indent + 1);

        for child in &dir.children {
            if child.is_dir() {
                let d = child.dir().unwrap();
                if let Some(g) = d.d_game.as_ref() {
                    write!(writer, "{}<game", pad)?;
                    write!(writer, " name=\"{}\"", Self::etxt(&child.name, false))?;
                    if !g.is_emu_arc {
                        if let Some(id) = g.id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                            write!(writer, " id=\"{}\"", Self::etxt(id, false))?;
                        }
                        if let Some(cloneof) = g
                            .clone_of
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            write!(writer, " cloneof=\"{}\"", Self::etxt(cloneof, false))?;
                        }
                        if let Some(cloneid) = g
                            .clone_of_id
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            write!(writer, " cloneofid=\"{}\"", Self::etxt(cloneid, false))?;
                        }
                        if let Some(romof) =
                            g.rom_of.as_deref().map(str::trim).filter(|s| !s.is_empty())
                        {
                            write!(writer, " romof=\"{}\"", Self::etxt(romof, false))?;
                        }
                    }
                    if let Some(is_bios) = g
                        .is_bios
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        if !is_bios.eq_ignore_ascii_case("no") {
                            write!(writer, " isbios=\"{}\"", Self::etxt(is_bios, false))?;
                        }
                    }
                    if let Some(is_device) = g
                        .is_device
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        if !is_device.eq_ignore_ascii_case("no") {
                            write!(writer, " isdevice=\"{}\"", Self::etxt(is_device, false))?;
                        }
                    }
                    if let Some(runnable) = g
                        .runnable
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        if !runnable.eq_ignore_ascii_case("yes") {
                            write!(writer, " runnable=\"{}\"", Self::etxt(runnable, false))?;
                        }
                    }
                    writeln!(writer, ">")?;

                    for cat in &g.category {
                        let c = cat.trim();
                        if !c.is_empty() {
                            writeln!(
                                writer,
                                "{}<category>{}</category>",
                                child_pad,
                                Self::etxt(c, false)
                            )?;
                        }
                    }
                    if let Some(desc) = g
                        .description
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        writeln!(
                            writer,
                            "{}<description>{}</description>",
                            child_pad,
                            Self::etxt(desc, false)
                        )?;
                    }
                    if g.is_emu_arc {
                        writeln!(writer, "{}<trurip>", child_pad)?;
                        fn trurip_node<W: Write>(
                            writer: &mut W,
                            pad: &str,
                            name: &str,
                            val: &Option<String>,
                        ) -> io::Result<()> {
                            let Some(v) = val.as_deref().map(str::trim).filter(|s| !s.is_empty())
                            else {
                                return Ok(());
                            };
                            writeln!(
                                writer,
                                "{}\t<{}>{}</{}>",
                                pad,
                                name,
                                DatXmlWriter::etxt(v, false),
                                name
                            )
                        }
                        trurip_node(writer, &child_pad, "titleid", &g.id)?;
                        trurip_node(writer, &child_pad, "source", &g.source)?;
                        trurip_node(writer, &child_pad, "publisher", &g.publisher)?;
                        trurip_node(writer, &child_pad, "developer", &g.developer)?;
                        trurip_node(writer, &child_pad, "year", &g.year)?;
                        trurip_node(writer, &child_pad, "genre", &g.genre)?;
                        trurip_node(writer, &child_pad, "subgenre", &g.sub_genre)?;
                        trurip_node(writer, &child_pad, "ratings", &g.ratings)?;
                        trurip_node(writer, &child_pad, "score", &g.score)?;
                        trurip_node(writer, &child_pad, "players", &g.players)?;
                        trurip_node(writer, &child_pad, "enabled", &g.enabled)?;
                        trurip_node(writer, &child_pad, "crc", &g.crc)?;
                        trurip_node(writer, &child_pad, "cloneof", &g.clone_of)?;
                        trurip_node(writer, &child_pad, "relatedto", &g.related_to)?;
                        writeln!(writer, "{}</trurip>", child_pad)?;
                    } else {
                        if let Some(year) =
                            g.year.as_deref().map(str::trim).filter(|s| !s.is_empty())
                        {
                            writeln!(
                                writer,
                                "{}<year>{}</year>",
                                child_pad,
                                Self::etxt(year, false)
                            )?;
                        }
                        if let Some(man) = g
                            .manufacturer
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            writeln!(
                                writer,
                                "{}<manufacturer>{}</manufacturer>",
                                child_pad,
                                Self::etxt(man, false)
                            )?;
                        }
                    }

                    Self::write_base(writer, d, indent + 1)?;

                    for dev in &g.device_ref {
                        let d = dev.trim();
                        if !d.is_empty() {
                            writeln!(
                                writer,
                                "{}<device_ref name=\"{}\"/>",
                                child_pad,
                                Self::etxt(d, false)
                            )?;
                        }
                    }

                    writeln!(writer, "{}</game>", pad)?;
                } else {
                    write!(writer, "{}<dir", pad)?;
                    write!(writer, " name=\"{}\"", Self::etxt(&child.name, false))?;
                    writeln!(writer, ">")?;
                    Self::write_base(writer, d, indent + 1)?;
                    writeln!(writer, "{}</dir>", pad)?;
                }
            } else {
                let f = child.file().unwrap();
                let tag = if f.is_disk { "disk" } else { "rom" };
                write!(writer, "{}<{}", pad, tag)?;
                write!(writer, " name=\"{}\"", Self::etxt(&child.name, false))?;
                if let Some(s) = f.size {
                    write!(writer, " size=\"{}\"", s)?;
                }
                if let Some(ref c) = f.crc {
                    write!(writer, " crc=\"{}\"", hex::encode(c))?;
                }
                if let Some(ref sha1) = f.sha1 {
                    write!(writer, " sha1=\"{}\"", hex::encode(sha1))?;
                }
                if let Some(ref sha256) = f.sha256 {
                    write!(writer, " sha256=\"{}\"", hex::encode(sha256))?;
                }
                if let Some(ref md5) = f.md5 {
                    write!(writer, " md5=\"{}\"", hex::encode(md5))?;
                }
                if let Some(dt) = child.date_modified {
                    if dt != crate::dat_store::TRRNTZIP_DOS_DATETIME {
                        let dt_str = Self::zip_date_time_to_string(dt);
                        if !dt_str.is_empty() {
                            write!(writer, " date=\"{}\"", dt_str)?;
                        }
                    }
                }
                if let Some(status) = f.status.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    if !status.eq_ignore_ascii_case("good") {
                        write!(writer, " status=\"{}\"", Self::etxt(status, false))?;
                    }
                }
                if f.mia.as_deref() == Some("yes") {
                    write!(writer, " mia=\"yes\"")?;
                }
                writeln!(writer, "/>")?;
            }
        }
        Ok(())
    }

    fn write_base_newstyle<W: Write>(
        writer: &mut W,
        dir: &crate::dat_store::DatDir,
        indent: usize,
    ) -> io::Result<()> {
        use crate::enums::FileType;
        let pad = "\t".repeat(indent);
        let child_pad = "\t".repeat(indent + 1);

        for child in &dir.children {
            if child.is_dir() {
                let d = child.dir().unwrap();
                if d.d_game.is_none() {
                    write!(writer, "{}<dir", pad)?;
                    write!(writer, " name=\"{}\"", Self::etxt(&child.name, false))?;
                    let tval = match child.file_type {
                        FileType::Zip => "trrntzip",
                        FileType::SevenZip => "7zip",
                        FileType::Dir => "dir",
                        _ => "",
                    };
                    if !tval.is_empty() {
                        write!(writer, " type=\"{}\"", tval)?;
                    }
                    if let Some(dt) = child.date_modified {
                        write!(writer, " date=\"{}\"", dt)?;
                    }
                    writeln!(writer, ">")?;
                    Self::write_base_newstyle(writer, d, indent + 1)?;
                    writeln!(writer, "{}</dir>", pad)?;
                } else {
                    write!(writer, "{}<set", pad)?;
                    write!(writer, " name=\"{}\"", Self::etxt(&child.name, false))?;
                    let tval = match child.file_type {
                        FileType::Zip => "trrntzip",
                        FileType::SevenZip => "7zip",
                        FileType::Dir => "dir",
                        _ => "",
                    };
                    if !tval.is_empty() {
                        write!(writer, " type=\"{}\"", tval)?;
                    }
                    if let Some(dt) = child.date_modified {
                        write!(writer, " date=\"{}\"", dt)?;
                    }
                    if let Some(g) = d.d_game.as_ref() {
                        if !g.is_emu_arc {
                            if let Some(id) =
                                g.id.as_deref().map(str::trim).filter(|s| !s.is_empty())
                            {
                                write!(writer, " id=\"{}\"", Self::etxt(id, false))?;
                            }
                            if let Some(cloneof) = g
                                .clone_of
                                .as_deref()
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                            {
                                write!(writer, " cloneof=\"{}\"", Self::etxt(cloneof, false))?;
                            }
                            if let Some(cloneid) = g
                                .clone_of_id
                                .as_deref()
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                            {
                                write!(writer, " cloneofid=\"{}\"", Self::etxt(cloneid, false))?;
                            }
                            if let Some(romof) =
                                g.rom_of.as_deref().map(str::trim).filter(|s| !s.is_empty())
                            {
                                write!(writer, " romof=\"{}\"", Self::etxt(romof, false))?;
                            }
                        }
                        if let Some(is_bios) = g
                            .is_bios
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            if !is_bios.eq_ignore_ascii_case("no") {
                                write!(writer, " isbios=\"{}\"", Self::etxt(is_bios, false))?;
                            }
                        }
                        if let Some(is_device) = g
                            .is_device
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            if !is_device.eq_ignore_ascii_case("no") {
                                write!(writer, " isdevice=\"{}\"", Self::etxt(is_device, false))?;
                            }
                        }
                        if let Some(runnable) = g
                            .runnable
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            if !runnable.eq_ignore_ascii_case("yes") {
                                write!(writer, " runnable=\"{}\"", Self::etxt(runnable, false))?;
                            }
                        }
                    }
                    writeln!(writer, ">")?;

                    if let Some(g) = d.d_game.as_ref() {
                        for cat in &g.category {
                            let c = cat.trim();
                            if !c.is_empty() {
                                writeln!(
                                    writer,
                                    "{}<category>{}</category>",
                                    child_pad,
                                    Self::etxt(c, false)
                                )?;
                            }
                        }
                        if let Some(desc) = g
                            .description
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            writeln!(
                                writer,
                                "{}<description>{}</description>",
                                child_pad,
                                Self::etxt(desc, false)
                            )?;
                        }
                        if g.is_emu_arc {
                            writeln!(writer, "{}<trurip>", child_pad)?;
                            fn trurip_node<W: Write>(
                                writer: &mut W,
                                pad: &str,
                                name: &str,
                                val: &Option<String>,
                            ) -> io::Result<()> {
                                let Some(v) =
                                    val.as_deref().map(str::trim).filter(|s| !s.is_empty())
                                else {
                                    return Ok(());
                                };
                                writeln!(
                                    writer,
                                    "{}\t<{}>{}</{}>",
                                    pad,
                                    name,
                                    DatXmlWriter::etxt(v, false),
                                    name
                                )
                            }
                            trurip_node(writer, &child_pad, "titleid", &g.id)?;
                            trurip_node(writer, &child_pad, "source", &g.source)?;
                            trurip_node(writer, &child_pad, "publisher", &g.publisher)?;
                            trurip_node(writer, &child_pad, "developer", &g.developer)?;
                            trurip_node(writer, &child_pad, "year", &g.year)?;
                            trurip_node(writer, &child_pad, "genre", &g.genre)?;
                            trurip_node(writer, &child_pad, "subgenre", &g.sub_genre)?;
                            trurip_node(writer, &child_pad, "ratings", &g.ratings)?;
                            trurip_node(writer, &child_pad, "score", &g.score)?;
                            trurip_node(writer, &child_pad, "players", &g.players)?;
                            trurip_node(writer, &child_pad, "enabled", &g.enabled)?;
                            trurip_node(writer, &child_pad, "crc", &g.crc)?;
                            trurip_node(writer, &child_pad, "cloneof", &g.clone_of)?;
                            trurip_node(writer, &child_pad, "relatedto", &g.related_to)?;
                            writeln!(writer, "{}</trurip>", child_pad)?;
                        } else {
                            if let Some(year) =
                                g.year.as_deref().map(str::trim).filter(|s| !s.is_empty())
                            {
                                writeln!(
                                    writer,
                                    "{}<year>{}</year>",
                                    child_pad,
                                    Self::etxt(year, false)
                                )?;
                            }
                            if let Some(man) = g
                                .manufacturer
                                .as_deref()
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                            {
                                writeln!(
                                    writer,
                                    "{}<manufacturer>{}</manufacturer>",
                                    child_pad,
                                    Self::etxt(man, false)
                                )?;
                            }
                        }
                    }

                    Self::write_base_newstyle(writer, d, indent + 1)?;

                    if let Some(g) = d.d_game.as_ref() {
                        for dev in &g.device_ref {
                            let d = dev.trim();
                            if d.is_empty() {
                                continue;
                            }
                            writeln!(
                                writer,
                                "{}<device_ref name=\"{}\"/>",
                                child_pad,
                                Self::etxt(d, false)
                            )?;
                        }
                    }
                    writeln!(writer, "{}</set>", pad)?;
                }
            } else {
                let f = child.file().unwrap();
                if child.name.ends_with('/') {
                    write!(
                        writer,
                        "{}<dir name=\"{}\"",
                        pad,
                        Self::etxt(child.name.trim_end_matches('/'), false)
                    )?;
                    if let Some(dt) = child.date_modified {
                        if dt != crate::dat_store::TRRNTZIP_DOS_DATETIME {
                            let dt_str = Self::zip_date_time_to_string(dt);
                            if !dt_str.is_empty() {
                                write!(writer, " date=\"{}\"", dt_str)?;
                            }
                        }
                    }
                    writeln!(writer, "/>")?;
                } else {
                    let tag = if f.is_disk { "disk" } else { "file" };
                    write!(
                        writer,
                        "{}<{} name=\"{}\"",
                        pad,
                        tag,
                        Self::etxt(&child.name, false)
                    )?;
                    if let Some(s) = f.size {
                        write!(writer, " size=\"{}\"", s)?;
                    }
                    if let Some(ref c) = f.crc {
                        write!(writer, " crc=\"{}\"", hex::encode(c))?;
                    }
                    if let Some(ref sha1) = f.sha1 {
                        write!(writer, " sha1=\"{}\"", hex::encode(sha1))?;
                    }
                    if let Some(ref sha256) = f.sha256 {
                        write!(writer, " sha256=\"{}\"", hex::encode(sha256))?;
                    }
                    if let Some(ref md5) = f.md5 {
                        write!(writer, " md5=\"{}\"", hex::encode(md5))?;
                    }
                    if let Some(dt) = child.date_modified {
                        if dt != crate::dat_store::TRRNTZIP_DOS_DATETIME {
                            let dt_str = Self::zip_date_time_to_string(dt);
                            if !dt_str.is_empty() {
                                write!(writer, " date=\"{}\"", dt_str)?;
                            }
                        }
                    }
                    if let Some(status) =
                        f.status.as_deref().map(str::trim).filter(|s| !s.is_empty())
                    {
                        if !status.eq_ignore_ascii_case("good") {
                            write!(writer, " status=\"{}\"", Self::etxt(status, false))?;
                        }
                    }
                    if f.mia.as_deref() == Some("yes") {
                        write!(writer, " mia=\"yes\"")?;
                    }
                    writeln!(writer, "/>")?;
                }
            }
        }
        Ok(())
    }

    fn zip_date_time_to_string(zip_file_date_time: i64) -> String {
        if zip_file_date_time == 0 || zip_file_date_time == i64::MIN {
            return String::new();
        }

        if zip_file_date_time > 0xffff_ffff {
            let ticks = zip_file_date_time;
            if !(0..=3_155_378_975_999_999_999).contains(&ticks) {
                return String::new();
            }

            const TICKS_PER_SECOND: i64 = 10_000_000;
            const TICKS_PER_MINUTE: i64 = TICKS_PER_SECOND * 60;
            const TICKS_PER_HOUR: i64 = TICKS_PER_MINUTE * 60;
            const TICKS_PER_DAY: i64 = TICKS_PER_HOUR * 24;

            let total_days = ticks / TICKS_PER_DAY;
            let mut rem = ticks - total_days * TICKS_PER_DAY;
            let hour = (rem / TICKS_PER_HOUR) as i32;
            rem -= (hour as i64) * TICKS_PER_HOUR;
            let minute = (rem / TICKS_PER_MINUTE) as i32;
            rem -= (minute as i64) * TICKS_PER_MINUTE;
            let second = (rem / TICKS_PER_SECOND) as i32;

            let mut n = total_days as i32;
            let y400 = n / 146097;
            n -= y400 * 146097;
            let mut y100 = n / 36524;
            if y100 == 4 {
                y100 = 3;
            }
            n -= y100 * 36524;
            let y4 = n / 1461;
            n -= y4 * 1461;
            let mut y1 = n / 365;
            if y1 == 4 {
                y1 = 3;
            }
            n -= y1 * 365;

            let year = y400 * 400 + y100 * 100 + y4 * 4 + y1 + 1;
            let day_of_year = n;
            let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            let days_to_month = if leap {
                [0, 31, 60, 91, 121, 152, 182, 213, 244, 274, 305, 335, 366]
            } else {
                [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334, 365]
            };
            let mut month = 1;
            while month < 13 && day_of_year >= days_to_month[month] {
                month += 1;
            }
            let month = month;
            let day = day_of_year - days_to_month[month - 1] + 1;

            return format!(
                "{:04}/{:02}/{:02} {:02}:{:02}:{:02}",
                year, month, day, hour, minute, second
            );
        }

        let dos_file_date = ((zip_file_date_time >> 16) & 0xffff) as i32;
        let dos_file_time = (zip_file_date_time & 0xffff) as i32;

        let second = (dos_file_time & 0x1f) << 1;
        let minute = (dos_file_time >> 5) & 0x3f;
        let hour = (dos_file_time >> 11) & 0x1f;
        let day = dos_file_date & 0x1f;
        let month = (dos_file_date >> 5) & 0x0f;
        let year = ((dos_file_date >> 9) & 0x7f) + 1980;

        format!(
            "{:04}/{:02}/{:02} {:02}:{:02}:{:02}",
            year, month, day, hour, minute, second
        )
    }

    fn write_base_mame<W: Write>(
        writer: &mut W,
        dir: &crate::dat_store::DatDir,
        indent: usize,
    ) -> io::Result<()> {
        let pad = "\t".repeat(indent);
        let child_pad = "\t".repeat(indent + 1);

        for child in &dir.children {
            if child.is_dir() {
                let d = child.dir().unwrap();
                if let Some(g) = d.d_game.as_ref() {
                    write!(writer, "{}<machine", pad)?;
                    write!(writer, " name=\"{}\"", Self::etxt(&child.name, true))?;
                    if !g.is_emu_arc {
                        if let Some(cloneof) = g
                            .clone_of
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            write!(writer, " cloneof=\"{}\"", Self::etxt(cloneof, true))?;
                        }
                        if let Some(romof) =
                            g.rom_of.as_deref().map(str::trim).filter(|s| !s.is_empty())
                        {
                            write!(writer, " romof=\"{}\"", Self::etxt(romof, true))?;
                        }
                    }
                    if let Some(is_bios) = g
                        .is_bios
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        if !is_bios.eq_ignore_ascii_case("no") {
                            write!(writer, " isbios=\"{}\"", Self::etxt(is_bios, true))?;
                        }
                    }
                    if let Some(is_device) = g
                        .is_device
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        if !is_device.eq_ignore_ascii_case("no") {
                            write!(writer, " isdevice=\"{}\"", Self::etxt(is_device, true))?;
                        }
                    }
                    if let Some(runnable) = g
                        .runnable
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        if !runnable.eq_ignore_ascii_case("yes") {
                            write!(writer, " runnable=\"{}\"", Self::etxt(runnable, true))?;
                        }
                    }
                    writeln!(writer, ">")?;

                    if let Some(desc) = g
                        .description
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        writeln!(
                            writer,
                            "{}<description>{}</description>",
                            child_pad,
                            Self::etxt(desc, true)
                        )?;
                    }
                    if let Some(year) = g.year.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                        writeln!(
                            writer,
                            "{}<year>{}</year>",
                            child_pad,
                            Self::etxt(year, true)
                        )?;
                    }
                    if let Some(man) = g
                        .manufacturer
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        writeln!(
                            writer,
                            "{}<manufacturer>{}</manufacturer>",
                            child_pad,
                            Self::etxt(man, true)
                        )?;
                    }

                    Self::write_base_mame(writer, d, indent + 1)?;

                    for dev in &g.device_ref {
                        let d = dev.trim();
                        if !d.is_empty() {
                            writeln!(
                                writer,
                                "{}<device_ref name=\"{}\"/>",
                                child_pad,
                                Self::etxt(d, true)
                            )?;
                        }
                    }

                    writeln!(writer, "{}</machine>", pad)?;
                } else {
                    writeln!(
                        writer,
                        "{}<dir name=\"{}\">",
                        pad,
                        Self::etxt(&child.name, true)
                    )?;
                    Self::write_base_mame(writer, d, indent + 1)?;
                    writeln!(writer, "{}</dir>", pad)?;
                }
            } else {
                let f = child.file().unwrap();
                let tag = if f.is_disk { "disk" } else { "rom" };
                write!(writer, "{}<{}", pad, tag)?;
                write!(writer, " name=\"{}\"", Self::etxt(&child.name, true))?;
                if let Some(merge) = f.merge.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    write!(writer, " merge=\"{}\"", Self::etxt(merge, true))?;
                }
                if let Some(s) = f.size {
                    write!(writer, " size=\"{}\"", s)?;
                }
                if let Some(ref c) = f.crc {
                    write!(writer, " crc=\"{}\"", hex::encode(c))?;
                }
                if let Some(ref sha1) = f.sha1 {
                    write!(writer, " sha1=\"{}\"", hex::encode(sha1))?;
                }
                if let Some(ref md5) = f.md5 {
                    write!(writer, " md5=\"{}\"", hex::encode(md5))?;
                }
                if let Some(dt) = child.date_modified {
                    if dt != crate::dat_store::TRRNTZIP_DOS_DATETIME {
                        let dt_str = Self::zip_date_time_to_string(dt);
                        if !dt_str.is_empty() {
                            write!(writer, " date=\"{}\"", dt_str)?;
                        }
                    }
                }
                if let Some(status) = f.status.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    if !status.eq_ignore_ascii_case("good") {
                        write!(writer, " status=\"{}\"", Self::etxt(status, true))?;
                    }
                }
                writeln!(writer, "/>")?;
            }
        }
        Ok(())
    }

    fn etxt(s: &str, mame: bool) -> String {
        let mut ret = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '&' => ret.push_str("&amp;"),
                '\"' => ret.push_str("&quot;"),
                '\'' => ret.push_str("&apos;"),
                '<' => ret.push_str("&lt;"),
                '>' => ret.push_str("&gt;"),
                _ if mame && c == '\u{7f}' => ret.push_str("&#7f;"),
                _ if c < ' ' => {
                    ret.push_str(&format!("&#{:02X};", c as u32));
                }
                _ => ret.push(c),
            }
        }
        ret
    }
}

#[cfg(test)]
#[path = "tests/xml_writer_tests.rs"]
mod tests;
