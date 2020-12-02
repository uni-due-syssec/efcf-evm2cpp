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
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

// TODO: a combined parser for the combined json and the sourcemap?
//use crate::sourcemap::{parse_source_map, SourceMap};
//pub fn parse_combined_file(
//    path: &str,
//) -> Result<(Combined, Option<SourceMap>), Box<dyn std::error::Error + 'static>> {
//}

/// solc version 0.8 and above embed the ABI definition as part of the combined.json format. Earlier
/// solc versions will store the ABI as a string in the combined.json, which then again contains the
/// JSON-encoded ABI definition... So with this little helper, we can make sure that the ABI field
/// is indeed a string.
fn ensures_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StrOrJson<'a> {
        Str(&'a str),
        Json(serde_json::Value),
    }

    Ok(match StrOrJson::deserialize(deserializer)? {
        StrOrJson::Str(v) => v.to_string(),
        StrOrJson::Json(v) => match v {
            serde_json::Value::String(s) => s,
            _ => serde_json::to_string(&v).unwrap(),
        },
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Contract {
    #[serde(deserialize_with = "ensures_string")]
    pub abi: String,
    #[serde(rename(deserialize = "bin", deserialize = "bytecode",))]
    pub bin: String,
    #[serde(rename(
        deserialize = "bin-runtime",
        deserialize = "deployedBytecode",
        serialize = "bin-runtime"
    ))]
    pub bin_runtime: String,
    #[serde(default)]
    pub srcmap: String,
    #[serde(
        rename(deserialize = "srcmap-runtime", serialize = "srcmap-runtime"),
        default
    )]
    pub srcmap_runtime: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Combined {
    pub contracts: BTreeMap<String, Contract>,

    #[serde(default)]
    #[serde(rename(deserialize = "sourceList", serialize = "sourceList"))]
    pub source_list: Vec<String>,

    #[serde(default)]
    pub version: String,
}

pub fn read_combined_from_file(path: &str) -> anyhow::Result<Combined> {
    let s = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read combined.json file from: {}", path))?;
    match serde_json::from_str(&s) {
        serde_json::Result::Ok(r) => anyhow::Result::Ok(r),
        serde_json::Result::Err(e) => anyhow::Result::Err(anyhow!(
            "Failed to deserialize file {} due to error {:?}",
            path,
            e
        )),
    }
}

pub fn read_single_contract_combined_from_file(path: &str) -> anyhow::Result<Contract> {
    let s = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read combined.json file from: {}", path))?;
    match serde_json::from_str(&s) {
        serde_json::Result::Ok(r) => anyhow::Result::Ok(r),
        serde_json::Result::Err(e) => anyhow::Result::Err(anyhow!(
            "Failed to deserialize file {} due to error {:?}",
            path,
            e
        )),
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_deserialize() {
        let s = "
{
  \"contracts\": {
    \"grid.sol:Grid\": {
      \"abi\": \"asdf\",
      \"bin\": \"00010203040506070809\",
      \"bin-runtime\": \"00010203040506070809\",
      \"srcmap\": \"93:6800:0:-;;;2057:209;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;2155:10;2147:5;;:18;;;;;;;;;;;;;;;;;;2186:13;2171:12;:28;;;;2216:9;2205:8;:20;;;;2247:14;2231:13;:30;;;;2057:209;;;;93:6800;;;;;;;\",
      \"srcmap-runtime\": \"93:6800:0:-;;;;;;;;;::o\"
    },
    \"grid.sol:SafeMath\": {
      \"abi\": \"[]\",
      \"bin\": \"60606040523415600\",
      \"bin-runtime\": \"00010203040506070809\",
      \"srcmap\": \"6895:662:0:-;;;;;;;;;;;;;;;;\",
      \"srcmap-runtime\": \"6895:662:0:-;;;;\"
    }
  },
  \"sourceList\": [
    \"grid.sol\"
  ],
  \"version\": \"0.4.11+commit.68ef5810.Linux.g++\"
}
        ";

        let c: Combined = serde_json::from_str(s).unwrap();

        assert_eq!(c.source_list[0], "grid.sol");
        assert_eq!(&c.version[0..6], "0.4.11");
        assert_eq!(c.contracts.len(), 2);

        let contract = c.contracts.get("grid.sol:Grid");
        assert!(contract.is_some());
        let contract = contract.unwrap();
        assert_eq!(contract.abi, "asdf");
    }

    #[test]
    fn test_deserialize_missing_fields() {
        let s = "
{
  \"contracts\": {
    \"grid.sol:Grid\": {
      \"abi\": \"asdf\",
      \"bin\": \"00010203040506070809\",
      \"bin-runtime\": \"00010203040506070809\",
      \"srcmap\": \"93:6800:0:-;;;2057:209;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;2155:10;2147:5;;:18;;;;;;;;;;;;;;;;;;2186:13;2171:12;:28;;;;2216:9;2205:8;:20;;;;2247:14;2231:13;:30;;;;2057:209;;;;93:6800;;;;;;;\",
      \"srcmap-runtime\": \"93:6800:0:-;;;;;;;;;::o\"
    },
    \"grid.sol:SafeMath\": {
      \"abi\": \"[]\",
      \"bin\": \"60606040523415600\",
      \"bin-runtime\": \"00010203040506070809\",
      \"srcmap\": \"6895:662:0:-;;;;;;;;;;;;;;;;\",
      \"srcmap-runtime\": \"6895:662:0:-;;;;\"
    }
  },
  \"version\": \"0.4.11+commit.68ef5810.Linux.g++\"
}
        ";

        let c: Combined = serde_json::from_str(s).unwrap();

        assert_eq!(c.source_list.len(), 0);
        assert_eq!(&c.version[0..6], "0.4.11");
        assert_eq!(c.contracts.len(), 2);

        let contract = c.contracts.get("grid.sol:Grid");
        assert!(contract.is_some());
        let contract = contract.unwrap();
        assert_eq!(contract.abi, "asdf");
    }

    #[test]
    fn test_contract_deserialize() {
        let s = "
{
      \"abi\": \"asdf\",
      \"bin\": \"00010203040506070809\",
      \"bin-runtime\": \"00010203040506070809\",
      \"srcmap\": \"93:6800:0:-;;;2057:209;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;2155:10;2147:5;;:18;;;;;;;;;;;;;;;;;;2186:13;2171:12;:28;;;;2216:9;2205:8;:20;;;;2247:14;2231:13;:30;;;;2057:209;;;;93:6800;;;;;;;\",
      \"srcmap-runtime\": \"93:6800:0:-;;;;;;;;;::o\"
}
        ";

        let contract: Contract = serde_json::from_str(s).unwrap();

        assert_eq!(contract.abi, "asdf");
        assert_eq!(contract.bin, "00010203040506070809");
    }

    #[test]
    fn test_contract_deserialize_altnames() {
        let s = "
{
      \"abi\": \"asdf\",
      \"bytecode\": \"00010203040506070809\",
      \"deployedBytecode\": \"00010203040506070809\",
      \"srcmap\": \"93:6800:0:-;;;2057:209;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;2155:10;2147:5;;:18;;;;;;;;;;;;;;;;;;2186:13;2171:12;:28;;;;2216:9;2205:8;:20;;;;2247:14;2231:13;:30;;;;2057:209;;;;93:6800;;;;;;;\",
      \"srcmap-runtime\": \"93:6800:0:-;;;;;;;;;::o\"
}
        ";

        let contract: Contract = serde_json::from_str(s).unwrap();

        assert_eq!(contract.abi, "asdf");
        assert_eq!(contract.bin, "00010203040506070809");
    }
}
