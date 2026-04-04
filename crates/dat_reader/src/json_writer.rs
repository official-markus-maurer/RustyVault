use crate::dat_store::{DatDir, DatHeader, TRRNTZIP_DOS_DATETIME};
use crate::enums::ZipStructure;
use serde_json::{Map, Value};
use std::io::{self, Write};

pub struct DatJsonWriter;

impl DatJsonWriter {
    pub fn write_dat<W: Write>(
        writer: &mut W,
        dat_header: &DatHeader,
        new_style: bool,
    ) -> io::Result<()> {
        let mut root = Map::<String, Value>::new();

        let mut header = Map::<String, Value>::new();
        Self::write_header(&mut header, dat_header);
        root.insert("Header".to_string(), Value::Object(header));

        let mut root_array = Vec::<Value>::new();
        Self::write_base(
            &mut root_array,
            &dat_header.base_dir,
            new_style,
            ZipStructure::None,
        );
        root.insert("root".to_string(), Value::Array(root_array));

        let json_text =
            serde_json::to_string_pretty(&Value::Object(root)).map_err(io::Error::other)?;
        writer.write_all(json_text.as_bytes())?;
        Ok(())
    }

    fn write_header(out: &mut Map<String, Value>, dat_header: &DatHeader) {
        out.insert(
            "name".to_string(),
            Value::String(dat_header.name.clone().unwrap_or_default()),
        );
        out.insert(
            "rootdir".to_string(),
            Value::String(dat_header.root_dir.clone().unwrap_or_default()),
        );
        out.insert(
            "header".to_string(),
            Value::String(dat_header.header.clone().unwrap_or_default()),
        );
        out.insert(
            "type".to_string(),
            Value::String(dat_header.type_.clone().unwrap_or_default()),
        );
        out.insert(
            "description".to_string(),
            Value::String(dat_header.description.clone().unwrap_or_default()),
        );
        out.insert(
            "category".to_string(),
            Value::String(dat_header.category.clone().unwrap_or_default()),
        );
        out.insert(
            "version".to_string(),
            Value::String(dat_header.version.clone().unwrap_or_default()),
        );
        out.insert(
            "date".to_string(),
            Value::String(dat_header.date.clone().unwrap_or_default()),
        );
        out.insert(
            "author".to_string(),
            Value::String(dat_header.author.clone().unwrap_or_default()),
        );
        out.insert(
            "email".to_string(),
            Value::String(dat_header.email.clone().unwrap_or_default()),
        );
        out.insert(
            "homepage".to_string(),
            Value::String(dat_header.homepage.clone().unwrap_or_default()),
        );
        out.insert(
            "url".to_string(),
            Value::String(dat_header.url.clone().unwrap_or_default()),
        );
        out.insert(
            "comment".to_string(),
            Value::String(dat_header.comment.clone().unwrap_or_default()),
        );
        out.insert(
            "forcepacking".to_string(),
            Value::String(dat_header.compression.clone().unwrap_or_default()),
        );
    }

    fn write_base(out: &mut Vec<Value>, base_dir: &DatDir, new_style: bool, context: ZipStructure) {
        for child in &base_dir.children {
            if child.is_dir() {
                let Some(d) = child.dir() else { continue };
                let node_struct = d.dat_struct();
                if d.d_game.is_some() {
                    let mut game = Map::<String, Value>::new();
                    game.insert("name".to_string(), Value::String(child.name.clone()));
                    game.insert(
                        "type".to_string(),
                        Value::String(Self::json_type_for_struct(node_struct).to_string()),
                    );

                    if let Some(g) = d.d_game.as_ref() {
                        game.insert(
                            "description".to_string(),
                            Value::String(g.description.clone().unwrap_or_default()),
                        );

                        if g.is_emu_arc {
                            let mut tea = Map::<String, Value>::new();
                            if let Some(ref v) = g.id {
                                tea.insert("titleid".to_string(), Value::String(v.clone()));
                            }
                            if let Some(ref v) = g.source {
                                tea.insert("source".to_string(), Value::String(v.clone()));
                            }
                            if let Some(ref v) = g.publisher {
                                tea.insert("publisher".to_string(), Value::String(v.clone()));
                            }
                            if let Some(ref v) = g.developer {
                                tea.insert("developer".to_string(), Value::String(v.clone()));
                            }
                            if let Some(ref v) = g.year {
                                tea.insert("year".to_string(), Value::String(v.clone()));
                            }
                            if let Some(ref v) = g.genre {
                                tea.insert("genre".to_string(), Value::String(v.clone()));
                            }
                            if let Some(ref v) = g.sub_genre {
                                tea.insert("subgenre".to_string(), Value::String(v.clone()));
                            }
                            if let Some(ref v) = g.ratings {
                                tea.insert("ratings".to_string(), Value::String(v.clone()));
                            }
                            if let Some(ref v) = g.score {
                                tea.insert("score".to_string(), Value::String(v.clone()));
                            }
                            if let Some(ref v) = g.players {
                                tea.insert("players".to_string(), Value::String(v.clone()));
                            }
                            if let Some(ref v) = g.enabled {
                                tea.insert("enabled".to_string(), Value::String(v.clone()));
                            }
                            if let Some(ref v) = g.crc {
                                tea.insert("crc".to_string(), Value::String(v.clone()));
                            }
                            if let Some(ref v) = g.clone_of {
                                tea.insert("cloneof".to_string(), Value::String(v.clone()));
                            }
                            if let Some(ref v) = g.related_to {
                                tea.insert("relatedto".to_string(), Value::String(v.clone()));
                            }
                            game.insert("tea".to_string(), Value::Object(tea));
                        } else {
                            if let Some(ref v) = g.year {
                                game.insert("year".to_string(), Value::String(v.clone()));
                            }
                            if let Some(ref v) = g.manufacturer {
                                game.insert("manufacturer".to_string(), Value::String(v.clone()));
                            }
                        }
                    }

                    let mut objects = Vec::<Value>::new();
                    Self::write_base(&mut objects, d, new_style, node_struct);
                    game.insert("objects".to_string(), Value::Array(objects));
                    out.push(Value::Object(game));
                } else {
                    let mut dir_obj = Map::<String, Value>::new();
                    dir_obj.insert("name".to_string(), Value::String(child.name.clone()));
                    dir_obj.insert("type".to_string(), Value::String("dir".to_string()));
                    let mut objects = Vec::<Value>::new();
                    Self::write_base(&mut objects, d, new_style, context);
                    dir_obj.insert("objects".to_string(), Value::Array(objects));
                    out.push(Value::Object(dir_obj));
                }
                continue;
            }

            let Some(f) = child.file() else { continue };

            if new_style && child.name.ends_with('/') {
                let mut d = Map::<String, Value>::new();
                d.insert(
                    "name".to_string(),
                    Value::String(child.name.trim_end_matches('/').to_string()),
                );
                d.insert("type".to_string(), Value::String("dir".to_string()));
                if let Some(dt) = child.date_modified {
                    if dt != TRRNTZIP_DOS_DATETIME {
                        d.insert("date".to_string(), Value::Number(dt.into()));
                    }
                }
                out.push(Value::Object(d));
                continue;
            }

            let mut file_obj = Map::<String, Value>::new();
            file_obj.insert("name".to_string(), Value::String(child.name.clone()));
            if context == ZipStructure::ZipTrrnt {
                file_obj.insert(
                    "type".to_string(),
                    Value::String("filetrrntzip".to_string()),
                );
            }
            if let Some(size) = f.size {
                file_obj.insert("size".to_string(), Value::Number(size.into()));
            }
            file_obj.insert(
                "crc".to_string(),
                Value::String(f.crc.as_deref().map(hex::encode).unwrap_or_default()),
            );
            file_obj.insert(
                "sha1".to_string(),
                Value::String(f.sha1.as_deref().map(hex::encode).unwrap_or_default()),
            );
            if let Some(ref md5) = f.md5 {
                file_obj.insert("md5".to_string(), Value::String(hex::encode(md5)));
            }
            if let Some(dt) = child.date_modified {
                if dt != TRRNTZIP_DOS_DATETIME {
                    file_obj.insert("date".to_string(), Value::Number(dt.into()));
                }
            }
            if let Some(ref status) = f.status {
                if !status.eq_ignore_ascii_case("good") {
                    file_obj.insert("status".to_string(), Value::String(status.clone()));
                }
            }
            if let Some(ref mia) = f.mia {
                if mia == "yes" {
                    file_obj.insert("mia".to_string(), Value::String("yes".to_string()));
                }
            }
            if f.is_disk {
                file_obj.insert("is_disk".to_string(), Value::Bool(true));
            }
            out.push(Value::Object(file_obj));
        }
    }

    fn json_type_for_struct(zip_struct: ZipStructure) -> &'static str {
        match zip_struct {
            ZipStructure::ZipTrrnt => "trrntzip",
            ZipStructure::ZipZSTD => "rvzip",
            ZipStructure::ZipTDC => "rvzip",
            ZipStructure::SevenZipTrrnt
            | ZipStructure::SevenZipSLZMA
            | ZipStructure::SevenZipNLZMA
            | ZipStructure::SevenZipSZSTD
            | ZipStructure::SevenZipNZSTD => "7zip",
            ZipStructure::None => "dir",
        }
    }
}

#[cfg(test)]
#[path = "tests/json_writer_tests.rs"]
mod tests;
