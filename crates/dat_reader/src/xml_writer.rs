use crate::dat_store::DatHeader;
use std::io::{self, Write};

/// Logic for serializing a DAT AST back into an XML file.
/// 
/// `DatXmlWriter` takes the `DatHeader` and its underlying `DatNode` tree and 
/// formats it into a standard Logiqx XML DAT format. This is heavily utilized
/// by the `dir2dat` tool and the `fix_dat_report` exporter.
/// 
/// Differences from C#:
/// - The C# `DatClean` logic and `FixDat` writers contain highly specialized XML writers
///   with deep formatting rules for different DAT engines (e.g. MAME vs ClrMamePro).
/// - The Rust version is currently a simplified generic XML emitter that covers the 
///   standard Logiqx fields but does not yet support arbitrary DOCTYPE emulation.
pub struct DatXmlWriter;

impl DatXmlWriter {
    pub fn write_dat<W: Write>(writer: &mut W, dat_header: &DatHeader) -> io::Result<()> {
        writeln!(writer, "<?xml version=\"1.0\"?>")?;
        
        let is_mame_style = dat_header.name.as_deref().unwrap_or("").to_lowercase().contains("mame");

        if is_mame_style {
            writeln!(writer, r#"<!DOCTYPE mame [
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
"#)?;
            writeln!(writer, "<mame build=\"{}\">", dat_header.name.as_deref().unwrap_or("MAME"))?;
        } else {
            writeln!(writer, "<datafile>")?;
            Self::write_header(writer, dat_header)?;
        }
        
        Self::write_base(writer, &dat_header.base_dir, 1, is_mame_style)?;
        
        if is_mame_style {
            writeln!(writer, "</mame>")?;
        } else {
            writeln!(writer, "</datafile>")?;
        }
        Ok(())
    }

    fn write_header<W: Write>(writer: &mut W, dat_header: &DatHeader) -> io::Result<()> {
        writeln!(writer, "  <header>")?;
        
        if let Some(ref name) = dat_header.name {
            writeln!(writer, "    <name>{}</name>", name)?;
        }
        if let Some(ref desc) = dat_header.description {
            writeln!(writer, "    <description>{}</description>", desc)?;
        }
        if let Some(ref cat) = dat_header.category {
            writeln!(writer, "    <category>{}</category>", cat)?;
        }
        if let Some(ref ver) = dat_header.version {
            writeln!(writer, "    <version>{}</version>", ver)?;
        }
        if let Some(ref date) = dat_header.date {
            writeln!(writer, "    <date>{}</date>", date)?;
        }
        if let Some(ref author) = dat_header.author {
            writeln!(writer, "    <author>{}</author>", author)?;
        }
        
        if dat_header.header.is_some() || dat_header.compression.is_some() {
            write!(writer, "    <romvault")?;
            if let Some(ref h) = dat_header.header {
                write!(writer, " header=\"{}\"", h)?;
            }
            if let Some(ref c) = dat_header.compression {
                write!(writer, " forcepacking=\"{}\"", c)?;
            }
            writeln!(writer, "/>")?;
        }
        
        writeln!(writer, "  </header>")?;
        Ok(())
    }

    fn write_base<W: Write>(writer: &mut W, dir: &crate::dat_store::DatDir, indent: usize, is_mame: bool) -> io::Result<()> {
        let pad = "  ".repeat(indent);
        
        for child in &dir.children {
            if child.is_dir() {
                let d = child.dir().unwrap();
                if d.d_game.is_none() {
                    writeln!(writer, "{}<dir name=\"{}\">", pad, Self::etxt(&child.name))?;
                    Self::write_base(writer, d, indent + 1, is_mame)?;
                    writeln!(writer, "{}</dir>", pad)?;
                } else {
                    let tag_name = if is_mame { "machine" } else { "game" };
                    write!(writer, "{}<{} name=\"{}\"", pad, tag_name, Self::etxt(&child.name))?;
                    
                    let g = d.d_game.as_ref().unwrap();
                    if let Some(ref cloneof) = g.clone_of {
                        write!(writer, " cloneof=\"{}\"", Self::etxt(cloneof))?;
                    }
                    if let Some(ref romof) = g.rom_of {
                        write!(writer, " romof=\"{}\"", Self::etxt(romof))?;
                    }
                    if let Some(ref is_bios) = g.is_bios {
                        if is_bios != "no" {
                            write!(writer, " isbios=\"{}\"", Self::etxt(is_bios))?;
                        }
                    }
                    writeln!(writer, ">")?;

                    if let Some(ref desc) = g.description {
                        writeln!(writer, "{}  <description>{}</description>", pad, Self::etxt(desc))?;
                    }
                    if let Some(ref year) = g.year {
                        writeln!(writer, "{}  <year>{}</year>", pad, Self::etxt(year))?;
                    }
                    if let Some(ref man) = g.manufacturer {
                        writeln!(writer, "{}  <manufacturer>{}</manufacturer>", pad, Self::etxt(man))?;
                    }
                    
                    Self::write_base(writer, d, indent + 1, is_mame)?;
                    writeln!(writer, "{}</{}>", pad, tag_name)?;
                }
            } else {
                let f = child.file().unwrap();
                if f.is_disk {
                    write!(writer, "{}<disk name=\"{}\"", pad, Self::etxt(child.name.trim_end_matches(".chd")))?;
                } else {
                    write!(writer, "{}<rom name=\"{}\"", pad, Self::etxt(&child.name))?;
                    if let Some(s) = f.size {
                        write!(writer, " size=\"{}\"", s)?;
                    }
                    if let Some(ref c) = f.crc {
                        write!(writer, " crc=\"{}\"", hex::encode(c))?;
                    }
                }
                
                if let Some(ref md5) = f.md5 {
                    write!(writer, " md5=\"{}\"", hex::encode(md5))?;
                }
                if let Some(ref sha1) = f.sha1 {
                    write!(writer, " sha1=\"{}\"", hex::encode(sha1))?;
                }
                if let Some(ref sha256) = f.sha256 {
                    write!(writer, " sha256=\"{}\"", hex::encode(sha256))?;
                }
                if let Some(ref status) = f.status {
                    if status.to_lowercase() != "good" {
                        write!(writer, " status=\"{}\"", Self::etxt(status))?;
                    }
                }
                
                writeln!(writer, "/>")?;
            }
        }
        
        Ok(())
    }

    fn etxt(s: &str) -> String {
        let mut ret = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '&' => ret.push_str("&amp;"),
                '\"' => ret.push_str("&quot;"),
                '\'' => ret.push_str("&apos;"),
                '<' => ret.push_str("&lt;"),
                '>' => ret.push_str("&gt;"),
                _ if c < ' ' => {
                    ret.push_str(&format!("&#{:02X};", c as u32));
                }
                _ => ret.push(c),
            }
        }
        ret
    }
}
