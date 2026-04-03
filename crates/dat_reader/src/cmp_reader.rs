use crate::dat_store::{DatDir, DatGame, DatHeader, DatNode};
use crate::enums::FileType;
use crate::var_fix;

/// ClrMamePro (CMP) DAT parser.
/// 
/// `cmp_reader.rs` handles the legacy non-XML ClrMamePro DAT format (which uses curly braces `{ }` 
/// and space-separated key-value pairs).
/// 
/// Differences from C#:
/// - The C# `CmpReader` operates directly on a `StreamReader`, buffering line by line.
/// - The Rust version reads the entire text buffer into memory and uses a custom `Chars` iterator
///   to tokenize strings (`gn`, `gn_rest`) with zero/low allocations, providing extremely fast 
///   parsing of legacy text DATs.
pub struct DatFileLoader<'a> {
    _input: &'a str,
    chars: std::str::Chars<'a>,
    current_line: usize,
    buffer: String,
    pub next_token: String,
}

impl<'a> DatFileLoader<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            _input: input,
            chars: input.chars(),
            current_line: 1,
            buffer: String::new(),
            next_token: String::new(),
        }
    }

    pub fn end_of_stream(&self) -> bool {
        self.buffer.trim().is_empty() && self.chars.as_str().is_empty()
    }

    pub fn gn_rest(&mut self) -> String {
        self.next_token.clear();
        self.buffer.clear();
        
        let mut in_comment = false;

        while let Some(c) = self.chars.clone().next() {
            if in_comment {
                if c == '\n' {
                    self.current_line += 1;
                    self.chars.next();
                    break;
                }
                self.chars.next();
                continue;
            }

            if c == '\n' {
                self.current_line += 1;
                self.chars.next();
                break;
            }

            if c == '\r' {
                self.chars.next();
                continue;
            }
            
            // Check for comment
            if c == '/' {
                let mut lookahead = self.chars.clone();
                lookahead.next(); // Consume current '/'
                if let Some(next_c) = lookahead.next() {
                    if next_c == '/' {
                        in_comment = true;
                        if !self.buffer.is_empty() {
                            self.next_token = self.buffer.clone();
                            return self.next_token.clone();
                        }
                        self.chars.next(); // Consume '/'
                        self.chars.next(); // Consume '/'
                        continue;
                    }
                }
            }

            self.buffer.push(c);
            self.chars.next();
        }

        if !self.buffer.is_empty() {
            self.next_token = self.buffer.trim().to_string();
        }
        self.next_token.clone()
    }

    pub fn gn(&mut self) -> String {
        self.next_token.clear();
        self.buffer.clear();

        let mut in_quotes = false;
        let mut in_comment = false;

        while let Some(c) = self.chars.clone().next() {
            if in_comment {
                if c == '\n' {
                    self.current_line += 1;
                    in_comment = false;
                }
                self.chars.next();
                continue;
            }

            if c == '\n' {
                self.current_line += 1;
                if in_quotes {
                    self.buffer.push(c);
                    self.chars.next();
                    continue;
                }
            }

            if c == '"' {
                in_quotes = !in_quotes;
                self.chars.next();
                if !in_quotes && !self.buffer.is_empty() {
                    self.next_token = self.buffer.clone();
                    return self.next_token.clone();
                }
                continue;
            }

            if !in_quotes && (c == ' ' || c == '\t' || c == '\n' || c == '\r') {
                if !self.buffer.is_empty() {
                    self.next_token = self.buffer.clone();
                    return self.next_token.clone();
                }
                self.chars.next();
                continue;
            }

            if !in_quotes && (c == '(' || c == ')') {
                if !self.buffer.is_empty() {
                    self.next_token = self.buffer.clone();
                    return self.next_token.clone();
                }
                self.next_token = c.to_string();
                self.chars.next();
                return self.next_token.clone();
            }
            
            // Check for comment
            if !in_quotes && c == '/' {
                let mut lookahead = self.chars.clone();
                lookahead.next(); // Consume current '/'
                if let Some(next_c) = lookahead.next() {
                    if next_c == '/' {
                        in_comment = true;
                        if !self.buffer.is_empty() {
                            self.next_token = self.buffer.clone();
                            return self.next_token.clone();
                        }
                        self.chars.next(); // Consume '/'
                        self.chars.next(); // Consume '/'
                        continue;
                    }
                }
            }

            self.buffer.push(c);
            self.chars.next();
        }

        if !self.buffer.is_empty() {
            self.next_token = self.buffer.clone();
        }
        self.next_token.clone()
    }
}

pub fn read_cmp_dat(input: &str, filename: &str) -> Result<DatHeader, String> {
    let mut dfl = DatFileLoader::new(input);
    let mut dat_header = DatHeader {
        base_dir: DatDir::new(FileType::Dir),
        filename: Some(filename.to_string()),
        ..Default::default()
    };

    dfl.gn();
    if dfl.end_of_stream() {
        return Err("Empty file".to_string());
    }

    let token_lower = dfl.next_token.to_lowercase();
    if token_lower == "clrmamepro" || token_lower == "clrmame" || token_lower == "romvault" {
        load_header_from_dat(&mut dfl, &mut dat_header)?;
        dfl.gn();
    }

    while !dfl.end_of_stream() {
        let t = dfl.next_token.to_lowercase();
        if t == "dir" {
            load_dir_from_dat(&mut dfl, &mut dat_header.base_dir)?;
        } else if t == "game" || t == "machine" || t == "resource" {
            load_game_from_dat(&mut dfl, &mut dat_header.base_dir)?;
        } else if t == "#" {
            dfl.gn_rest();
        } else if !t.is_empty() {
            // unknown block
            skip_block(&mut dfl)?;
        } else {
            dfl.gn();
        }
    }

    Ok(dat_header)
}

fn skip_block(dfl: &mut DatFileLoader) -> Result<(), String> {
    dfl.gn();
    if dfl.next_token != "(" {
        return Err("Expected ( for unknown block".to_string());
    }
    let mut depth = 1;
    while depth > 0 && !dfl.end_of_stream() {
        dfl.gn();
        if dfl.next_token == "(" {
            depth += 1;
        } else if dfl.next_token == ")" {
            depth -= 1;
        }
    }
    dfl.gn();
    Ok(())
}

fn load_header_from_dat(dfl: &mut DatFileLoader, dat_header: &mut DatHeader) -> Result<(), String> {
    dfl.gn();
    if dfl.next_token != "(" {
        return Err("Expected ( after clrmamepro".to_string());
    }
    dfl.gn();
    while dfl.next_token != ")" && !dfl.end_of_stream() {
        let key = dfl.next_token.to_lowercase();
        let val = dfl.gn();

        match key.as_str() {
            "name" => dat_header.name = Some(val),
            "description" => dat_header.description = Some(val),
            "category" => dat_header.category = Some(val),
            "version" => dat_header.version = Some(val),
            "author" => dat_header.author = Some(val),
            "homepage" => dat_header.homepage = Some(val),
            "url" => dat_header.url = Some(val),
            "comment" => dat_header.comment = Some(val),
            "header" => dat_header.header = Some(val),
            "forcepacking" => dat_header.compression = Some(val.to_lowercase()),
            "forcemerging" => dat_header.merge_type = Some(val.to_lowercase()),
            "dir" => dat_header.dir = Some(val.to_lowercase()),
            _ => {}
        }
        dfl.gn();
    }
    Ok(())
}

fn load_dir_from_dat(dfl: &mut DatFileLoader, parent_dir: &mut DatDir) -> Result<(), String> {
    dfl.gn();
    if dfl.next_token != "(" {
        return Err("Expected ( after dir".to_string());
    }
    
    let mut name = String::new();
    
    dfl.gn();
    while dfl.next_token != ")" && !dfl.end_of_stream() {
        let key = dfl.next_token.to_lowercase();
        let val = dfl.gn();
        if key == "name" {
            name = val;
        }
        dfl.gn();
    }
    
    let dir = DatNode::new_dir(name, FileType::UnSet);
    parent_dir.add_child(dir);
    dfl.gn();
    Ok(())
}

fn load_game_from_dat(dfl: &mut DatFileLoader, parent_dir: &mut DatDir) -> Result<(), String> {
    dfl.gn();
    if dfl.next_token != "(" {
        return Err("Expected ( after game".to_string());
    }

    let mut d_game = DatGame::default();
    let mut d_dir_node = DatNode::new_dir(String::new(), FileType::UnSet);

    dfl.gn();
    while dfl.next_token != ")" && !dfl.end_of_stream() {
        let key = dfl.next_token.to_lowercase();
        if key == "rom" || key == "disk" || key == "sample" || key == "archive" {
            load_rom_from_dat(dfl, d_dir_node.dir_mut().unwrap(), &key)?;
        } else {
            let val = dfl.gn();
            match key.as_str() {
                "name" => d_dir_node.name = val.clone(),
                "description" => d_game.description = Some(val),
                "year" => d_game.year = Some(val),
                "manufacturer" => d_game.manufacturer = Some(val),
                "cloneof" => d_game.clone_of = Some(val),
                "romof" => d_game.rom_of = Some(val),
                _ => {}
            }
            dfl.gn();
        }
    }

    if let Some(d) = d_dir_node.dir_mut() {
        d.d_game = Some(Box::new(d_game));
    }
    parent_dir.add_child(d_dir_node);
    dfl.gn();
    Ok(())
}

fn load_rom_from_dat(dfl: &mut DatFileLoader, parent_dir: &mut DatDir, node_type: &str) -> Result<(), String> {
    dfl.gn();
    if dfl.next_token != "(" {
        return Err(format!("Expected ( after {}", node_type));
    }

    let mut file = DatNode::new_file(String::new(), FileType::UnSet);

    if node_type == "disk" {
        if let Some(f) = file.file_mut() {
            f.is_disk = true;
        }
    }

    dfl.gn();
    while dfl.next_token != ")" && !dfl.end_of_stream() {
        let key = dfl.next_token.to_lowercase();
        let val = dfl.gn();
        
        match key.as_str() {
            "name" => {
                if node_type == "disk" {
                    file.name = var_fix::clean_chd(&val);
                } else {
                    file.name = val;
                }
            }
            "size" => { if let Some(f) = file.file_mut() { f.size = var_fix::u64_opt(&val); } }
            "crc" => { if let Some(f) = file.file_mut() { f.crc = var_fix::clean_md5_sha1(&val, 8); } }
            "sha1" => { if let Some(f) = file.file_mut() { f.sha1 = var_fix::clean_md5_sha1(&val, 40); } }
            "sha256" => { if let Some(f) = file.file_mut() { f.sha256 = var_fix::clean_md5_sha1(&val, 64); } }
            "md5" => { if let Some(f) = file.file_mut() { f.md5 = var_fix::clean_md5_sha1(&val, 32); } }
            "merge" => {
                if let Some(f) = file.file_mut() {
                    f.merge = Some(if node_type == "disk" {
                        var_fix::clean_chd(&val)
                    } else {
                        val
                    });
                }
            }
            "status" => { if let Some(f) = file.file_mut() { f.status = Some(var_fix::to_lower(&val)); } }
            _ => {}
        }
        dfl.gn();
    }
    
    parent_dir.add_child(file);
    dfl.gn();
    Ok(())
}

#[cfg(test)]
#[path = "tests/cmp_reader_tests.rs"]
mod tests;
