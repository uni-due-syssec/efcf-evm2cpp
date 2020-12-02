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

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate anyhow;

use anyhow::Context;
use clap::{arg, Command};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

mod analysis;
mod codegen;
mod combinedjson;
#[allow(dead_code)]
mod instructions;
mod sourcemap;

use codegen::translate_to_c;
use combinedjson::{read_combined_from_file, read_single_contract_combined_from_file};
use sourcemap::parse_source_map;

//impl std::error::Error for hexutil::ParseHexError {}

fn to_hex(b: &str) -> anyhow::Result<Vec<u8>> {
    match hexutil::read_hex(b) {
        Ok(b) => anyhow::Result::Ok(b),
        Err(e) => anyhow::Result::Err(anyhow!("Failed to convert hex bytes (Error: {:?})", e)),
    }
}

fn write_abi(name: &str, evm_path: &Path, abi: &[u8]) -> anyhow::Result<()> {
    let abi_file = format!("fuzz/abi/{}.abi", name);
    let file_path = evm_path.join(abi_file);
    println!("Writing ABI to {}", file_path.display());
    let mut file = File::create(&file_path)?;
    file.write_all(abi)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let app = Command::new(env!("CARGO_BIN_NAME"))
        .about("EVM bytecode to C++ transpiler targeting the eEVM framework")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(arg!(-a --abi [ABI_FILE] "path to abi definition file").multiple_values(false).multiple_occurrences(false))
        .arg(arg!(-A --"translate-all" "Translate all contracts found in combined.json"))
        .arg(arg!(-c --"combined-json" "force use of combined json as input (auto-detected on filetype)"))
        .arg(arg!(-C --"single-combined-json" "force use of combined json of a single contract (i.e., truffle-style)"))
        .arg(arg!(-e --"evm-path" [EVM_PATH] "path to eEVM project").default_value("./eEVM").multiple_values(false).multiple_occurrences(false))
        .arg(arg!(-s --"emit-sourcemap" "emit source information to generated code for easier codegen debugging"))
        .arg(arg!(-F --"clang-format" "launch clang-format on generated code"))
        .arg(arg!(--"contract-name" [NAME] "contract name to look for in the combined.json input format (defaults to the <name> parameter)").multiple_values(false).multiple_occurrences(false))
        .arg(arg!(<name> "name/identifier of the contract for the generated code"))
        .arg(arg!([input] "path to EVM runtime code (.bin-runtime) or combined-json input"))
        .arg(arg!([constructor_path] "path to EVM constructor code (.bin)"));
    let matches = app.get_matches();

    let evm_path = if let Some(path) = matches.value_of("evm-path") {
        let p = Path::new(path);
        if p.exists() {
            Ok(p)
        } else {
            Err(anyhow!(
                "Provided path to eEVM does not exit! ({})",
                p.display()
            ))
        }
    } else {
        let p = Path::new("./eEVM");
        if p.exists() {
            Ok(p)
        } else {
            let p = Path::new("../eEVM");
            if p.exists() {
                Ok(p)
            } else {
                Err(anyhow!(
                    "failed to find eEVM project directory in './eEVM' or '../eEVM'"
                ))
            }
        }
    }?;

    if !if matches.is_present("translate-all") {
        matches.is_present("name") && !matches.is_present("input")
    } else {
        matches.is_present("name") && matches.is_present("input")
    } {
        bail!("invalid arguments");
    }

    let input = if matches.is_present("translate-all") {
        matches.value_of("name")
    } else {
        matches.value_of("input")
    }
    .ok_or(anyhow!("Missing input"))?;
    println!("reading input {}", input);

    println!("Writing contracts to eEVM at {}", evm_path.display());

    if matches.is_present("single-combined-json") {
        let name = matches
            .value_of("name")
            .ok_or(anyhow!("Missing contract name"))?;

        let contract = read_single_contract_combined_from_file(input)?;

        let sourcemap = if matches.is_present("emit-sourcemap") {
            println!("[WARNING] Cannot emitting source(-map) information to contract without the bigger combined json input!");
            None
        } else {
            None
        };

        let bytecode = contract.bin_runtime.trim();
        let bytecode = to_hex(bytecode).with_context(|| {
            format!(
                "failed to convert bytecode of contract {} in combined.json from hex",
                name
            )
        })?;

        let constructor_bytecode = contract.bin.trim();
        let constructor_bytecode = to_hex(constructor_bytecode).with_context(|| {
            format!(
                "failed to convert constructor bytecode of contract {} in combined.json from hex",
                name
            )
        })?;

        write_abi(name, evm_path, contract.abi.as_bytes())?;

        //(bytecode, Some(constructor_bytecode), sourcemap)

        println!("Translating contract {} to C++...", name);
        println!("Writing contract to eEVM {}", evm_path.display());
        translate_to_c(
            evm_path,
            name,
            bytecode,
            Some(constructor_bytecode),
            sourcemap,
            matches.is_present("clang-format"),
        )?;
    } else if input.ends_with("combined.json") || matches.is_present("combined-json") {
        let combined_path = Path::new(input);
        let combined = read_combined_from_file(input)?;

        let name_best_match = if matches.is_present("translate-all") {
            None
        } else {
            let look_for_name = if let Some(lname) = matches.value_of("contract-name") {
                lname
            } else {
                matches.value_of("name").unwrap()
            }
            .to_string();

            let mut look_for_name_normalized = look_for_name.replace("_", "");
            look_for_name_normalized.make_ascii_lowercase();

            let mut best_match = if let Some(s) = combined.contracts.keys().cloned().last() {
                Some(s)
            } else {
                bail!("invalid input - no contracts");
            };
            let mut best_match_score = 10000;

            for name in combined.contracts.keys().cloned() {
                let cname = if let Some(s) = name.split(":").skip(1).next() {
                    s.to_string()
                } else {
                    name.clone()
                };

                if cname == look_for_name {
                    // complete string equality - we take this one
                    best_match = Some(cname);
                    break;
                } else {
                    let mut cname_normalized = cname.replace("_", "");
                    cname_normalized.make_ascii_lowercase();

                    if look_for_name_normalized == cname_normalized {
                        // normalized equality; we take this one unless we find non-normalized
                        // equality
                        best_match = Some(cname);
                        best_match_score = 0;
                    } else {
                        // otherwise we do some matching. We prefer to look for matches with the
                        // starts_with over ends_with. Also we look for matches with the best fit in
                        // terms of string length difference.
                        let str_len_diff = (cname_normalized.len() as isize
                            - look_for_name_normalized.len() as isize)
                            .abs();
                        if cname_normalized.starts_with(&look_for_name_normalized) {
                            let score = 1 * str_len_diff;
                            if best_match_score > score {
                                best_match_score = score;
                                best_match = Some(cname);
                                continue;
                            }
                        }
                        if look_for_name_normalized.starts_with(&cname_normalized) {
                            let score = 2 * str_len_diff;
                            if best_match_score > score {
                                best_match_score = score;
                                best_match = Some(cname);
                                continue;
                            }
                        }
                        if cname_normalized.ends_with(&look_for_name_normalized) {
                            let score = 3 * str_len_diff;
                            if best_match_score > score {
                                best_match_score = score;
                                best_match = Some(cname);
                                continue;
                            }
                        }
                        if look_for_name_normalized.ends_with(&cname_normalized) {
                            let score = 4 * str_len_diff;
                            if best_match_score > score {
                                best_match_score = score;
                                best_match = Some(cname);
                                continue;
                            }
                        }
                    }
                }
            }

            best_match
        };

        for (name, contract) in combined.contracts.iter() {
            let name = if let Some(s) = name.split(":").skip(1).next() {
                s.to_string()
            } else {
                name.clone()
            };
            let mut identifier = name.clone();

            if !matches.is_present("translate-all") {
                if let Some(bmatch) = name_best_match.as_ref() {
                    if &name == bmatch {
                        identifier = if let Some(lname) = matches.value_of("contract-name") {
                            lname
                        } else {
                            matches.value_of("name").unwrap()
                        }
                        .to_string();

                        println!(
                            "Selecting contract {} from combined.json (identifier is {})",
                            name, identifier
                        );
                    } else {
                        continue;
                    }
                }
            }

            let sourcemap = if matches.is_present("emit-sourcemap") {
                println!("Emitting source(-map) information to contract!");
                let x = if let Some(parent) = combined_path.parent() {
                    let filepaths: Vec<PathBuf> = combined
                        .source_list
                        .iter()
                        .cloned()
                        .map(|s| parent.join(s))
                        .collect();
                    let files: Vec<&str> = filepaths.iter().map(|s| s.to_str().unwrap()).collect();
                    parse_source_map(&contract.srcmap_runtime, &files)
                } else {
                    let files: Vec<&str> = combined.source_list.iter().map(|s| &**s).collect();
                    parse_source_map(&contract.srcmap_runtime, &files)
                }
                .with_context(|| {
                    format!(
                        "failed to parse sourcemap from combined.json at {:?}",
                        combined_path
                    )
                })?;
                Some(x)
            } else {
                None
            };

            let bytecode = contract.bin_runtime.trim();
            let bytecode = to_hex(bytecode).with_context(|| {
                format!(
                    "failed to convert bytecode of contract {} in combined.json from hex",
                    name
                )
            })?;

            let constructor_bytecode = contract.bin.trim();
            let constructor_bytecode = to_hex(constructor_bytecode).with_context(|| {
                format!(
                "failed to convert constructor bytecode of contract {} in combined.json from hex",
                name
            )
            })?;

            write_abi(&identifier, evm_path, contract.abi.as_bytes())?;
            println!(
                "Translating contract with name {} (identifier {}) to C++...",
                name, identifier
            );
            translate_to_c(
                evm_path,
                &identifier,
                bytecode,
                Some(constructor_bytecode),
                sourcemap,
                matches.is_present("clang-format"),
            )?;

            if !matches.is_present("translate-all") {
                break;
            }
        }
    } else {
        let name = matches
            .value_of("name")
            .ok_or(anyhow!("Missing contract name"))?;

        let bytecode = std::fs::read_to_string(input)
            .with_context(|| format!("failed to read bytecode data from {}", input))?;
        let bytecode = bytecode.trim();
        let bytecode = to_hex(bytecode)
            .with_context(|| format!("failed to convert bytecode file {} from hex", input))?;
        let constructor_file = if let Some(constructor_path) = matches.value_of("constructor_path")
        {
            Some(std::path::Path::new(constructor_path))
        } else {
            let suffix = "-runtime";
            if input.ends_with(suffix) {
                let cpath = Path::new(&input[..(input.len() - suffix.len())]);
                if cpath.exists() {
                    println!("[INFO] Reading constructor bytecode from {:?}", cpath);
                    Some(cpath)
                } else {
                    None
                }
            } else {
                None
            }
        };
        let constructor_bytecode = if let Some(constructor_file) = constructor_file {
            let chex = std::fs::read_to_string(constructor_file)?;
            let chex = chex.trim();
            let cbytes = to_hex(&chex).with_context(|| {
                format!(
                    "failed to convert constructor file {:?} from hex",
                    constructor_file
                )
            })?;
            Some(cbytes)
        } else {
            None
        };

        if let Some(abi) = matches.value_of("abi") {
            let abi = std::fs::read_to_string(abi)?;
            write_abi(name, evm_path, abi.as_bytes())?
        } else {
            let suffix = "bin-runtime";
            if input.ends_with(suffix) {
                let cpath = Path::new(&input[..(input.len() - suffix.len())]);
                if cpath.exists() {
                    println!("[INFO] Reading ABI data from {}", cpath.display());
                    let abi = std::fs::read_to_string(cpath)?;
                    write_abi(name, evm_path, abi.as_bytes())?
                }
            }
        }

        //(bytecode, constructor_bytecode, None)
        println!("Translating contract {} to C++...", name);
        println!("Writing contract to eEVM {}", evm_path.display());
        translate_to_c(
            evm_path,
            name,
            bytecode,
            constructor_bytecode,
            None,
            matches.is_present("clang-format"),
        )?;
    };

    println!("Done!");
    Ok(())
}
