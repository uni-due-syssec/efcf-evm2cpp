// Copyright 2021 Michael Rodler
// This file is part of evm2cpp.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

use anyhow::Context;
use std::rc::Rc;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum JumpType {
    Call,
    Return,
    Jump,
    //Not,
}

impl JumpType {
    fn parse(c: char) -> Option<JumpType> {
        match c {
            'i' => Some(JumpType::Call),
            'o' => Some(JumpType::Return),
            '-' => Some(JumpType::Jump),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SourceMapEntry {
    byte_offset: usize,
    length: usize,
    file_index: i32,
    pub jump_type: JumpType,
    pub modifier_depth: usize,
    pub line: Rc<String>,
    pub line_number: usize,
}

pub type SourceMap = Vec<SourceMapEntry>;

pub fn parse_source_map_file(
    source_map_path: &str,
    source_files: &[&str],
) -> anyhow::Result<SourceMap> {
    // read input files
    let source_map_string = std::fs::read_to_string(source_map_path)
        .with_context(|| format!("failed to read source map file: {}", source_map_path))?;
    parse_source_map(&source_map_string, source_files)
}

pub fn parse_source_map(
    source_map_string: &str,
    source_files: &[&str],
) -> anyhow::Result<SourceMap> {
    let mut file_contents: Vec<String> = Vec::new();
    for sf in source_files.iter() {
        file_contents.push(
            std::fs::read_to_string(sf)
                .with_context(|| format!("failed to read solidity source file: {}", sf))?,
        );
    }
    let mut entries: Vec<SourceMapEntry> = Vec::new();

    // the sourcemap format is described here:
    // https://docs.soliditylang.org/en/v0.8.0/internals/source_mappings.html?highlight=source%20map#source-mappings
    // and for older versions:
    // https://docs.soliditylang.org/en/v0.5.3/miscellaneous.html?highlight=source%20map#source-mappings

    for (idx, entry) in source_map_string.split(';').into_iter().enumerate() {
        if entry.is_empty() {
            entries.push(entries[entries.len() - 1].clone());
        } else {
            let fields: Vec<&str> = entry.split(':').collect();
            if entries.is_empty() && fields.len() < 4 {
                bail!("Invalid sourcemap - First entry of sourcemap must contain all fields (got {})!", fields.len());
            }
            let byte_offset: usize = if !fields.is_empty() && !fields[0].is_empty() {
                fields[0].parse().with_context(|| {
                    format!(
                        "Failed to parse entry {} unparsable byte_offset = {:?}",
                        idx, fields[0]
                    )
                })?
            } else {
                entries[idx - 2].byte_offset
            };
            let length: usize = if fields.len() > 1 && fields[1].len() > 0 {
                fields[1].parse().with_context(|| {
                    format!(
                        "Failed to parse entry {} unparsable length = {:?}",
                        idx, fields[1]
                    )
                })?
            } else {
                entries[idx - 2].length
            };
            let file_index: i32 = if fields.len() > 2 && fields[2].len() > 0 {
                fields[2].parse().with_context(|| {
                    format!(
                        "Failed to parse entry {} unparsable file_index = {:?}",
                        idx, fields[2]
                    )
                })?
            } else {
                entries[idx - 2].file_index
            };
            let jump_type = if fields.len() > 3 && fields[3].len() > 0 {
                JumpType::parse(fields[3].chars().next().unwrap()).with_context(|| {
                    format!(
                        "Failed to parse entry {} unparsable jump_type = {:?}",
                        idx, fields[3]
                    )
                })?
            } else {
                entries[idx - 2].jump_type
            };
            let modifier_depth: usize = if fields.len() > 4 && fields[4].len() > 0 {
                fields[4].parse().with_context(|| {
                    format!(
                        "Failed to parse entry {} unparsable modifier_depth = {:?}",
                        idx, fields[4]
                    )
                })?
            } else {
                // NOTE: in older solidity versions, there is no modifier depth... so our normal
                // parsing routine fails here. We just return 0 for the first entry and all other
                // entries will then copy the value from the first field, which is 0...
                if !entries.is_empty() {
                    entries[idx - 2].modifier_depth
                } else {
                    0
                }
            };

            let u_file_index = if file_index < 0 {
                (file_contents.len() as i32 + file_index) as usize
            } else {
                file_index as usize
            };
            let mut fi = file_contents[u_file_index].as_bytes().iter();
            // count newlines up to byte offset
            let lineno = 1 + (&mut fi).take(byte_offset).filter(|&&c| c == b'\n').count();
            // take length bytes
            let line_bytes = fi.take(length).cloned().collect();

            let line = String::from_utf8(line_bytes)?;

            let sm_entry = SourceMapEntry {
                byte_offset,
                length,
                file_index,
                jump_type,
                modifier_depth,
                line: Rc::new(line),
                line_number: lineno,
            };

            //println!("{:?}", sm_entry);

            entries.push(sm_entry);
        }
    }

    Ok(entries)
}
