use crate::dat_store::{DatDir, DatFile, DatHeader, DatNode};
use std::io::{self, Write};

pub struct DatDosWriter;

impl DatDosWriter {
    pub fn write_dat<W: Write>(writer: &mut W, dat_header: &DatHeader) -> io::Result<()> {
        writeln!(writer, "DOSCenter (")?;
        Self::write_header(writer, dat_header)?;
        writeln!(writer, ")")?;
        Self::write_base(writer, &dat_header.base_dir, 0)?;
        Ok(())
    }

    fn write_header<W: Write>(writer: &mut W, dat_header: &DatHeader) -> io::Result<()> {
        if let Some(ref name) = dat_header.name {
            writeln!(writer, "\tname: {}", name)?;
        }
        if let Some(ref desc) = dat_header.description {
            writeln!(writer, "\tdescription: {}", desc)?;
        }
        if let Some(ref version) = dat_header.version {
            writeln!(writer, "\tversion: {}", version)?;
        }
        if let Some(ref date) = dat_header.date {
            writeln!(writer, "\tdate: {}", date)?;
        }
        if let Some(ref author) = dat_header.author {
            writeln!(writer, "\tauthor: {}", author)?;
        }
        if let Some(ref homepage) = dat_header.homepage {
            writeln!(writer, "\thomepage: {}", homepage)?;
        }
        if let Some(ref comment) = dat_header.comment {
            writeln!(writer, "\tcomment: {}", comment)?;
        }
        Ok(())
    }

    fn write_base<W: Write>(writer: &mut W, dir: &DatDir, indent: usize) -> io::Result<()> {
        for child in &dir.children {
            if child.is_dir() {
                let d = child.dir().unwrap();
                if d.d_game.is_some() {
                    writeln!(writer)?;
                    writeln!(writer, "{}game (", "\t".repeat(indent))?;
                    Self::write_game_name(writer, &child.name, indent + 1)?;
                    Self::write_base(writer, d, indent + 1)?;
                    writeln!(writer, "{})", "\t".repeat(indent))?;
                }
                continue;
            }

            let f = child.file().unwrap();
            Self::write_file(writer, child, f, indent)?;
        }
        Ok(())
    }

    fn write_game_name<W: Write>(writer: &mut W, name: &str, indent: usize) -> io::Result<()> {
        writeln!(writer, "{}name \"{}.zip\"", "\t".repeat(indent), name)?;
        Ok(())
    }

    fn write_file<W: Write>(writer: &mut W, node: &DatNode, file: &DatFile, indent: usize) -> io::Result<()> {
        write!(writer, "{}file (", "\t".repeat(indent))?;
        write!(writer, " name {}", node.name)?;
        if let Some(size) = file.size {
            write!(writer, " size {}", size)?;
        }
        if let Some(ref crc) = file.crc {
            write!(writer, " crc {}", hex::encode(crc))?;
        }
        if let Some(ref sha1) = file.sha1 {
            write!(writer, " sha1 {}", hex::encode(sha1))?;
        }
        if let Some(ref sha256) = file.sha256 {
            write!(writer, " sha256 {}", hex::encode(sha256))?;
        }
        if let Some(ref md5) = file.md5 {
            write!(writer, " md5 {}", hex::encode(md5))?;
        }
        writeln!(writer, " )")?;
        Ok(())
    }
}

