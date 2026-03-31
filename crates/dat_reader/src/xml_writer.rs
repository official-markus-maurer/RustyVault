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
        writeln!(writer, "<datafile>")?;
        
        Self::write_header(writer, dat_header)?;
        Self::write_base(writer, &dat_header.base_dir, 1)?;
        
        writeln!(writer, "</datafile>")?;
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

    fn write_base<W: Write>(writer: &mut W, dir: &crate::dat_store::DatDir, indent: usize) -> io::Result<()> {
        let pad = "  ".repeat(indent);
        
        for child in &dir.children {
            if child.is_dir() {
                let d = child.dir().unwrap();
                if d.d_game.is_none() {
                    writeln!(writer, "{}<dir name=\"{}\">", pad, child.name)?;
                    Self::write_base(writer, d, indent + 1)?;
                    writeln!(writer, "{}</dir>", pad)?;
                } else {
                    writeln!(writer, "{}<game name=\"{}\">", pad, child.name)?;
                    let g = d.d_game.as_ref().unwrap();
                    if let Some(ref desc) = g.description {
                        writeln!(writer, "{}  <description>{}</description>", pad, desc)?;
                    }
                    Self::write_base(writer, d, indent + 1)?;
                    writeln!(writer, "{}</game>", pad)?;
                }
            } else {
                let f = child.file().unwrap();
                if f.is_disk {
                    write!(writer, "{}<disk name=\"{}\"", pad, child.name.trim_end_matches(".chd"))?;
                } else {
                    write!(writer, "{}<rom name=\"{}\"", pad, child.name)?;
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
                
                writeln!(writer, "/>")?;
            }
        }
        
        Ok(())
    }
}
