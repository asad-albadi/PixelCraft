//! Edits and transfers ASE user property maps while retaining untouched entries.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_DEPTH: usize = 128;

#[derive(Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
enum Property {
    Bool(bool),
    Integer(i64),
    Unsigned(String),
    Number(f64),
    String(String),
    Point([i32; 2]),
    Size([i32; 2]),
    Rectangle([i32; 4]),
    Vector(Vec<Property>),
    Map(Vec<NamedProperty>),
    Uuid([u8; 16]),
    Opaque,
}

#[derive(Deserialize, Serialize, PartialEq)]
struct NamedProperty {
    key: String,
    value: Property,
}

struct Entry<'a> {
    property: NamedProperty,
    bytes: &'a [u8],
}

struct Map<'a> {
    id: u32,
    entries: Vec<Entry<'a>>,
    bytes: &'a [u8],
}

struct Chunk<'a> {
    flags: u32,
    prefix: &'a [u8],
    maps: Vec<Map<'a>>,
    suffix: &'a [u8],
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or("Invalid user data size")?;
        let data = self
            .bytes
            .get(self.offset..end)
            .ok_or("Truncated ASE user data")?;
        self.offset = end;
        Ok(data)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().expect("exact byte count"))
    }

    fn word(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn dword(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn int(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    fn text(&mut self) -> Result<String, String> {
        let len = self.word()? as usize;
        String::from_utf8(self.take(len)?.to_vec())
            .map_err(|_| "Invalid UTF-8 in ASE property".into())
    }

    fn count(&mut self) -> Result<usize, String> {
        let count = self.dword()? as usize;
        if count > 65536 {
            return Err("ASE property collection exceeds the entry limit".into());
        }
        Ok(count)
    }

    fn entries(&mut self, depth: usize) -> Result<Vec<Entry<'a>>, String> {
        let count = self.count()?;
        (0..count)
            .map(|_| {
                let start = self.offset;
                let key = self.text()?;
                let kind = self.word()?;
                let value = self.property(kind, depth)?;
                Ok(Entry {
                    property: NamedProperty { key, value },
                    bytes: &self.bytes[start..self.offset],
                })
            })
            .collect()
    }

    fn property(&mut self, kind: u16, depth: usize) -> Result<Property, String> {
        if depth > MAX_DEPTH {
            return Err("ASE property nesting exceeds the limit".into());
        }
        let number = |n: f64| {
            if n.is_finite() {
                Property::Number(n)
            } else {
                Property::Opaque
            }
        };
        Ok(match kind {
            1 => Property::Bool(self.take(1)?[0] != 0),
            2 => Property::Integer(i8::from_le_bytes(self.array()?) as i64),
            3 => Property::Integer(self.take(1)?[0] as i64),
            4 => Property::Integer(i16::from_le_bytes(self.array()?) as i64),
            5 => Property::Integer(self.word()? as i64),
            6 => Property::Integer(self.int()? as i64),
            7 => Property::Integer(self.dword()? as i64),
            8 => Property::Integer(i64::from_le_bytes(self.array()?)),
            9 => {
                let value = u64::from_le_bytes(self.array()?);
                i64::try_from(value)
                    .map_or_else(|_| Property::Unsigned(value.to_string()), Property::Integer)
            }
            10 => number(f64::from(self.int()?) / 65536.),
            11 => number(f64::from(f32::from_le_bytes(self.array()?))),
            12 => number(f64::from_le_bytes(self.array()?)),
            13 => Property::String(self.text()?),
            14 => Property::Point([self.int()?, self.int()?]),
            15 => Property::Size([self.int()?, self.int()?]),
            16 => Property::Rectangle([self.int()?, self.int()?, self.int()?, self.int()?]),
            17 => {
                let count = self.count()?;
                let element = self.word()?;
                let mut values = Vec::new();
                for _ in 0..count {
                    let element = if element == 0 { self.word()? } else { element };
                    values.push(self.property(element, depth + 1)?);
                }
                Property::Vector(values)
            }
            18 => Property::Map(
                self.entries(depth + 1)?
                    .into_iter()
                    .map(|entry| entry.property)
                    .collect(),
            ),
            19 => Property::Uuid(self.array()?),
            _ => return Err(format!("Unsupported ASE property type {kind:#06x}")),
        })
    }
}

impl<'a> Chunk<'a> {
    fn decode(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err("ASE user data exceeds the script limit".into());
        }
        let mut input = Reader { bytes, offset: 0 };
        let flags = input.dword()?;
        if flags & 1 != 0 {
            let len = input.word()?;
            input.take(len as usize)?;
        }
        if flags & 2 != 0 {
            input.take(4)?;
        }
        let prefix = &bytes[4..input.offset];
        let mut maps = Vec::new();
        if flags & 4 != 0 {
            let size = (input.dword()? as usize)
                .checked_sub(4)
                .ok_or("Invalid ASE property size")?;
            let mut section = Reader {
                bytes: input.take(size)?,
                offset: 0,
            };
            let count = section.count()?;
            for _ in 0..count {
                let start = section.offset;
                let id = section.dword()?;
                let entries = section.entries(0)?;
                maps.push(Map {
                    id,
                    entries,
                    bytes: &section.bytes[start..section.offset],
                });
            }
            if section.offset != section.bytes.len() {
                return Err("Unexpected bytes in ASE property section".into());
            }
        }
        Ok(Self {
            flags,
            prefix,
            maps,
            suffix: &bytes[input.offset..],
        })
    }

    fn property_map(&self, id: u32) -> Option<&Map<'a>> {
        self.maps.iter().find(|map| map.id == id)
    }

    fn with_entries(&self, id: u32, entries: Vec<Vec<u8>>) -> Result<Vec<u8>, String> {
        let mut maps = Vec::new();
        let count =
            self.maps.iter().filter(|map| map.id != id).count() + usize::from(!entries.is_empty());
        if count != 0 {
            dword(&mut maps, count)?;
            let mut written = false;
            for map in &self.maps {
                if map.id == id {
                    if !written && !entries.is_empty() {
                        encode_map(&mut maps, id, &entries)?;
                        written = true;
                    }
                } else {
                    maps.extend_from_slice(map.bytes);
                }
            }
            if !written && !entries.is_empty() {
                encode_map(&mut maps, id, &entries)?;
            }
        }
        self.encode(maps)
    }

    fn encode(&self, maps: Vec<u8>) -> Result<Vec<u8>, String> {
        let flags = (self.flags & !4) | if maps.is_empty() { 0 } else { 4 };
        let mut output = flags.to_le_bytes().to_vec();
        output.extend_from_slice(self.prefix);
        if !maps.is_empty() {
            dword(&mut output, maps.len() + 4)?;
            output.extend(maps);
        }
        output.extend_from_slice(self.suffix);
        if output.len() > MAX_BYTES {
            return Err("ASE user data exceeds the script limit".into());
        }
        Ok(output)
    }
}

fn encode_map(output: &mut Vec<u8>, id: u32, entries: &[Vec<u8>]) -> Result<(), String> {
    output.extend_from_slice(&id.to_le_bytes());
    dword(output, entries.len())?;
    for entry in entries {
        output.extend_from_slice(entry);
    }
    Ok(())
}

fn dword(output: &mut Vec<u8>, value: usize) -> Result<(), String> {
    let value = u32::try_from(value).map_err(|_| "ASE property length exceeds its binary limit")?;
    output.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn text(output: &mut Vec<u8>, value: &str) -> Result<(), String> {
    let length = u16::try_from(value.len()).map_err(|_| "ASE property text exceeds 65535 bytes")?;
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn encode_value(output: &mut Vec<u8>, value: &Property, depth: usize) -> Result<(), String> {
    if depth > MAX_DEPTH || output.len() > MAX_BYTES {
        return Err("ASE property exceeds size or nesting limits".into());
    }
    let kind: u16 = match value {
        Property::Bool(_) => 1,
        Property::Integer(_) => 8,
        Property::Unsigned(_) => 9,
        Property::Number(_) => 12,
        Property::String(_) => 13,
        Property::Point(_) => 14,
        Property::Size(_) => 15,
        Property::Rectangle(_) => 16,
        Property::Vector(_) => 17,
        Property::Map(_) => 18,
        Property::Uuid(_) => 19,
        Property::Opaque => return Err("Opaque ASE property cannot be assigned".into()),
    };
    output.extend_from_slice(&kind.to_le_bytes());
    match value {
        Property::Bool(value) => output.push(u8::from(*value)),
        Property::Integer(value) => output.extend_from_slice(&value.to_le_bytes()),
        Property::Unsigned(value) => output.extend_from_slice(
            &value
                .parse::<u64>()
                .map_err(|_| "Invalid unsigned property")?
                .to_le_bytes(),
        ),
        Property::Number(value) => output.extend_from_slice(&value.to_le_bytes()),
        Property::String(value) => text(output, value)?,
        Property::Point(values) | Property::Size(values) => {
            output.extend(values.iter().flat_map(|n| n.to_le_bytes()))
        }
        Property::Rectangle(values) => output.extend(values.iter().flat_map(|n| n.to_le_bytes())),
        Property::Vector(values) => {
            dword(output, values.len())?;
            output.extend_from_slice(&0_u16.to_le_bytes());
            for value in values {
                encode_value(output, value, depth + 1)?;
            }
        }
        Property::Map(entries) => {
            dword(output, entries.len())?;
            for entry in entries {
                text(output, &entry.key)?;
                encode_value(output, &entry.value, depth + 1)?;
            }
        }
        Property::Uuid(bytes) => output.extend_from_slice(bytes),
        Property::Opaque => unreachable!("rejected above"),
    }
    Ok(())
}

fn encoded_entry(key: &str, value: &Property) -> Result<Vec<u8>, String> {
    let mut entry = Vec::new();
    text(&mut entry, key)?;
    encode_value(&mut entry, value, 0)?;
    Ok(entry)
}

struct ExternalFiles<'a> {
    chunks: &'a [Vec<u8>],
    used_ids: HashSet<u32>,
    names: HashMap<String, u32>,
    ids: HashMap<u32, String>,
    first_end: usize,
}

impl<'a> ExternalFiles<'a> {
    fn scan(chunks: &'a [Vec<u8>]) -> Result<Self, String> {
        let mut files = Self {
            chunks,
            used_ids: Default::default(),
            names: Default::default(),
            ids: Default::default(),
            first_end: 0,
        };
        for (index, bytes) in chunks.iter().enumerate() {
            if bytes.len() > MAX_BYTES {
                return Err("ASE external files chunk exceeds the script limit".into());
            }
            let mut reader = Reader { bytes, offset: 0 };
            let count = reader.count()?;
            reader.take(8)?;
            for _ in 0..count {
                let id = reader.dword()?;
                let kind = reader.take(8)?[0];
                let name = reader.text()?;
                files.used_ids.insert(id);
                if id != 0 && kind == 2 {
                    files.names.entry(name.clone()).or_insert(id);
                    files.ids.entry(id).or_insert(name);
                }
            }
            if index == 0 {
                files.first_end = reader.offset;
            }
        }
        Ok(files)
    }

    fn add(&self, entries: &[(u32, String)]) -> Result<Vec<u8>, String> {
        let mut data = self
            .chunks
            .first()
            .map_or_else(|| vec![0; 12], |first| first[..self.first_end].to_vec());
        let count = u32::from_le_bytes(data[..4].try_into().expect("external file count"));
        let count = (count as usize).saturating_add(entries.len());
        if count > 65536 {
            return Err("ASE external files chunk exceeds the entry limit".into());
        }
        data[..4].copy_from_slice(&(count as u32).to_le_bytes());
        for (id, namespace) in entries {
            data.extend_from_slice(&id.to_le_bytes());
            data.extend_from_slice(&[2, 0, 0, 0, 0, 0, 0, 0]);
            text(&mut data, namespace)?;
        }
        if let Some(first) = self.chunks.first() {
            data.extend_from_slice(&first[self.first_end..]);
        }
        if data.len() > MAX_BYTES {
            return Err("ASE external files chunk exceeds the script limit".into());
        }
        Ok(data)
    }
}

#[derive(Deserialize)]
struct TilesetPropertyImport {
    data: Vec<Vec<u8>>,
    source: Vec<Vec<u8>>,
    external: Vec<Vec<u8>>,
}

fn import_tileset(value: Value) -> Result<Value, String> {
    let request: TilesetPropertyImport =
        serde_json::from_value(value).map_err(|e| e.to_string())?;
    if request
        .data
        .iter()
        .chain(&request.source)
        .chain(&request.external)
        .map(Vec::len)
        .sum::<usize>()
        > crate::extensions::MAX_TILESET_METADATA_BYTES
    {
        return Err("Tileset metadata exceeds the clipboard limit".into());
    }
    let source = ExternalFiles::scan(&request.source)?;
    let mut destination = ExternalFiles::scan(&request.external)?;
    let mut additions = Vec::new();
    let mut assigned = HashMap::new();
    let mut candidates = 1..=u32::MAX;
    let mut result = Vec::with_capacity(request.data.len());
    for bytes in &request.data {
        let chunk = Chunk::decode(bytes)?;
        if chunk.maps.is_empty() {
            result.push(bytes.clone());
            continue;
        }
        let mut maps = Vec::new();
        dword(&mut maps, chunk.maps.len())?;
        for map in &chunk.maps {
            let id = if map.id == 0 {
                0
            } else if let Some(&id) = assigned.get(&map.id) {
                id
            } else {
                let name = source
                    .ids
                    .get(&map.id)
                    .ok_or("Missing tileset property namespace")?;
                let id = if let Some(&id) = destination.names.get(name) {
                    id
                } else {
                    let id = candidates
                        .find(|id| !destination.used_ids.contains(id))
                        .ok_or("No ASE extension ID available")?;
                    destination.used_ids.insert(id);
                    destination.names.insert(name.clone(), id);
                    additions.push((id, name.clone()));
                    id
                };
                assigned.insert(map.id, id);
                id
            };
            maps.extend_from_slice(&id.to_le_bytes());
            maps.extend_from_slice(&map.bytes[4..]);
        }
        result.push(chunk.encode(maps)?);
    }
    if additions.is_empty() {
        Ok(json!({"data":result}))
    } else {
        Ok(json!({"data":result,"external":destination.add(&additions)?}))
    }
}

pub(crate) fn request(value: Value) -> Result<Value, String> {
    if value["action"] == "importTileset" {
        return import_tileset(value);
    }
    let bytes: Vec<u8> =
        serde_json::from_value(value["data"].clone()).map_err(|e| e.to_string())?;
    let chunk = Chunk::decode(&bytes)?;
    let namespace = value["namespace"].as_str().unwrap_or_default();
    let namespaced = !namespace.is_empty();
    let external: Vec<Vec<u8>> = if namespaced {
        serde_json::from_value(value["external"].clone()).map_err(|e| e.to_string())?
    } else {
        vec![]
    };
    let files = ExternalFiles::scan(&external)?;
    let map_id = if namespaced {
        files.names.get(namespace).copied()
    } else {
        Some(0)
    };
    let entries = map_id
        .and_then(|id| chunk.property_map(id))
        .map_or(&[][..], |map| &map.entries);
    let new_entries = match value["action"].as_str() {
        Some("read") => {
            if let Some(key) = value["key"].as_str() {
                let property = entries
                    .iter()
                    .find(|entry| entry.property.key == key)
                    .map(|entry| &entry.property.value);
                return Ok(json!({"property":property}));
            }
            let properties: Vec<_> = entries.iter().map(|entry| &entry.property).collect();
            return Ok(json!({"properties":properties}));
        }
        Some("set") => {
            let key = value["key"].as_str().ok_or("Expected a property name")?;
            let property: Option<Property> =
                serde_json::from_value(value["property"].clone()).map_err(|e| e.to_string())?;
            let previous = entries.iter().find(|entry| entry.property.key == key);
            if previous.map(|entry| &entry.property.value) == property.as_ref() {
                return Ok(json!({"data":bytes}));
            }
            let mut data: Vec<_> = entries
                .iter()
                .filter(|entry| entry.property.key != key)
                .map(|entry| entry.bytes.to_vec())
                .collect();
            if let Some(property) = property {
                data.push(encoded_entry(key, &property)?);
            }
            data
        }
        Some("replace") => {
            let properties: Vec<NamedProperty> =
                serde_json::from_value(value["properties"].clone()).map_err(|e| e.to_string())?;
            properties
                .iter()
                .map(|entry| encoded_entry(&entry.key, &entry.value))
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => return Err("Unknown ASE property action".into()),
    };
    if let Some(id) = map_id {
        return Ok(json!({"data":chunk.with_entries(id, new_entries)?}));
    }
    if new_entries.is_empty() {
        return Ok(json!({"data":bytes}));
    }
    let id = (1..=u32::MAX)
        .find(|id| !files.used_ids.contains(id) && chunk.property_map(*id).is_none())
        .ok_or("No ASE extension ID available")?;
    Ok(
        json!({"data":chunk.with_entries(id, new_entries)?,"external":files.add(&[(id, namespace.to_owned())])?}),
    )
}
