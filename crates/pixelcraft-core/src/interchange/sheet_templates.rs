use crate::{
    model::{Document, FrameTag},
    workflows::SheetOptions,
};
use serde_json::Value;
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashSet},
    path::Path,
};

pub(super) fn apply(
    doc: &Document,
    options: &SheetOptions,
    path: &Path,
    metadata: &mut Value,
) -> Result<(), String> {
    let filenames = Template::parse(&options.filename_template)?;
    let tags = Template::parse(&options.tag_template)?;
    if filenames.is_none() && tags.is_none() {
        return Ok(());
    }
    let entries = metadata["frames"].as_array().unwrap();
    let slice = metadata["meta"]["sourceSlice"].as_str().unwrap_or_default();
    let tag_ranges = frame_tags(doc);
    let mut remaining = 32 * 1024 * 1024;
    let render = |template: &Template<'_>, index, tag: Option<&Value>| {
        let entry: &Value = &entries[index];
        let source = entry["sourceFrame"].as_u64().unwrap() as usize;
        let (inner, outer) = tag_ranges[source];
        let layer = entry["sourceLayer"].as_u64().map(|layer| layer as usize);
        let group = layer
            .and_then(|layer| doc.layers[layer].parent)
            .map(|parent| doc.layers[parent].name.as_str())
            .unwrap_or_default();
        let default_name = entry["filename"].as_str().unwrap_or_default();
        let authored_tag = tag.and_then(|tag| tag["name"].as_str()).map(|name| {
            if let Some((label, _)) = default_name.rsplit_once("_frame_")
                && let Some(bare) = name
                    .strip_prefix(label)
                    .and_then(|rest| rest.strip_prefix("__"))
            {
                return bare;
            }
            name
        });
        let inner_name = authored_tag
            .or_else(|| inner.map(|tag| tag.name.as_str()))
            .unwrap_or_default();
        let from = tag.and_then(|tag| tag["from"].as_u64());
        let to = tag.and_then(|tag| tag["to"].as_u64());
        template.render(|field| {
            let number = match field {
                Field::Frame => Some(source as u64),
                Field::FrameId => entry["sourceFrameId"]
                    .as_u64()
                    .or_else(|| entry["frameId"].as_u64()),
                Field::TagFrame => {
                    Some(inner.map_or(0, |tag| source.saturating_sub(tag.from)) as u64)
                }
                Field::Duration => entry["duration"].as_u64(),
                Field::LayerIndex => entry["sourceLayer"].as_u64(),
                Field::GridX => entry["gridCell"]["x"].as_u64(),
                Field::GridY => entry["gridCell"]["y"].as_u64(),
                Field::From => from,
                Field::To => to,
                _ => None,
            };
            if field.is_number() {
                return Substitution::Number(number);
            }
            let value = match field {
                Field::Title => path.file_stem().and_then(|s| s.to_str()),
                Field::Name => path.file_name().and_then(|s| s.to_str()),
                Field::Extension => path.extension().and_then(|s| s.to_str()),
                Field::FrameName => entry["frameName"].as_str(),
                Field::Tag => Some(inner_name),
                Field::OuterTag => authored_tag.or_else(|| outer.map(|tag| tag.name.as_str())),
                Field::Layer => entry["sourceLayerName"].as_str(),
                Field::Group => Some(group),
                Field::Slice => Some(slice),
                Field::Sample => Some(default_name),
                _ => None,
            };
            Substitution::Text(value.unwrap_or_default())
        })
    };

    let frame_names = filenames
        .as_ref()
        .map(|template| {
            checked_names(
                (0..entries.len()).map(|index| render(template, index, None)),
                &mut remaining,
                "frame",
            )
        })
        .transpose()?;
    let tag_names = tags
        .as_ref()
        .map(|template| {
            let tags = metadata["meta"]["frameTags"].as_array().unwrap();
            checked_names(
                tags.iter()
                    .map(|tag| render(template, tag["from"].as_u64().unwrap() as usize, Some(tag))),
                &mut remaining,
                "tag",
            )
        })
        .transpose()?;
    if let Some(names) = frame_names {
        for (entry, name) in metadata["frames"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(names)
        {
            entry["filename"] = Value::String(name);
        }
    }
    if let Some(names) = tag_names {
        for (tag, name) in metadata["meta"]["frameTags"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(names)
        {
            tag["name"] = Value::String(name);
        }
    }
    Ok(())
}

fn checked_names(
    values: impl Iterator<Item = Result<String, String>>,
    remaining: &mut usize,
    kind: &str,
) -> Result<Vec<String>, String> {
    let names: Vec<String> = values
        .map(|name| {
            let name = name?;
            *remaining = remaining
                .checked_sub(name.len())
                .ok_or("Sheet template names exceed the metadata size limit")?;
            Ok::<_, String>(name)
        })
        .collect::<Result<_, _>>()?;
    let mut seen = HashSet::with_capacity(names.len());
    for name in &names {
        if !seen.insert(name) {
            return Err(format!("Duplicate exported {kind} name: {name:?}"));
        }
    }
    Ok(names)
}

fn frame_tags(doc: &Document) -> Vec<(Option<&FrameTag>, Option<&FrameTag>)> {
    let mut tags: Vec<_> = doc.tags.iter().enumerate().collect();
    tags.sort_unstable_by_key(|&(index, tag)| (tag.from, index));
    let mut next = tags.into_iter().peekable();
    let mut inner = BinaryHeap::new();
    let mut outer = BinaryHeap::new();
    (0..doc.durations.len())
        .map(|frame| {
            while next.peek().is_some_and(|(_, tag)| tag.from <= frame) {
                let (index, tag) = next.next().unwrap();
                let span = tag.to.saturating_sub(tag.from);
                inner.push((Reverse(span), Reverse(index), tag.to));
                outer.push((span, Reverse(index), tag.to));
            }
            while inner.peek().is_some_and(|entry| entry.2 < frame) {
                inner.pop();
            }
            while outer.peek().is_some_and(|entry| entry.2 < frame) {
                outer.pop();
            }
            (
                inner.peek().map(|entry| &doc.tags[entry.1.0]),
                outer.peek().map(|entry| &doc.tags[entry.1.0]),
            )
        })
        .collect()
}

#[derive(Clone, Copy)]
enum Field {
    Title,
    Name,
    Extension,
    Frame,
    FrameId,
    FrameName,
    Tag,
    OuterTag,
    TagFrame,
    Duration,
    Layer,
    LayerIndex,
    Group,
    Slice,
    GridX,
    GridY,
    From,
    To,
    Sample,
}

impl Field {
    fn is_number(self) -> bool {
        matches!(
            self,
            Self::Frame
                | Self::FrameId
                | Self::TagFrame
                | Self::Duration
                | Self::LayerIndex
                | Self::GridX
                | Self::GridY
                | Self::From
                | Self::To
        )
    }
}

enum Substitution<'a> {
    Text(&'a str),
    Number(Option<u64>),
}

enum Part<'a> {
    Text(&'a str),
    Field(Field, usize, u64),
}

struct Template<'a>(Vec<Part<'a>>);

impl<'a> Template<'a> {
    fn parse(mut source: &'a str) -> Result<Option<Self>, String> {
        if source.trim().is_empty() {
            return Ok(None);
        }
        if source.len() > 4096 {
            return Err("Sheet templates are limited to 4096 bytes".into());
        }
        let mut parts = Vec::new();
        while let Some(brace) = source.find(['{', '}']) {
            parts.push(Part::Text(&source[..brace]));
            source = &source[brace..];
            if source.starts_with("{{") || source.starts_with("}}") {
                parts.push(Part::Text(&source[..1]));
                source = &source[2..];
                continue;
            }
            let (token, rest) = source
                .strip_prefix('{')
                .and_then(|s| s.split_once('}'))
                .ok_or("Unmatched brace in sheet template")?;
            parts.push(Self::field(token)?);
            source = rest;
        }
        parts.push(Part::Text(source));
        Ok(Some(Self(parts)))
    }

    fn field(token: &str) -> Result<Part<'a>, String> {
        let (name, padding) = token
            .split_once(':')
            .map_or((token, None), |(name, width)| (name, Some(width)));
        let (field, width, offset) = match name {
            "title" => (Field::Title, 0, 0),
            "name" => (Field::Name, 0, 0),
            "extension" => (Field::Extension, 0, 0),
            "frame_id" => (Field::FrameId, 0, 0),
            "frame_name" => (Field::FrameName, 0, 0),
            "tag" | "innertag" => (Field::Tag, 0, 0),
            "outertag" => (Field::OuterTag, 0, 0),
            "duration" => (Field::Duration, 0, 0),
            "layer" => (Field::Layer, 0, 0),
            "layer_index" => (Field::LayerIndex, 0, 0),
            "group" => (Field::Group, 0, 0),
            "slice" => (Field::Slice, 0, 0),
            "grid_x" => (Field::GridX, 0, 0),
            "grid_y" => (Field::GridY, 0, 0),
            "from" => (Field::From, 0, 0),
            "to" => (Field::To, 0, 0),
            "sample" => (Field::Sample, 0, 0),
            _ => {
                let (field, suffix) = if let Some(suffix) = name.strip_prefix("frame") {
                    (Field::Frame, suffix)
                } else if let Some(suffix) = name.strip_prefix("tagframe") {
                    (Field::TagFrame, suffix)
                } else {
                    return Err(format!("Unknown sheet template field: {token:?}"));
                };
                let offset = if suffix.is_empty() {
                    0
                } else {
                    suffix
                        .parse::<u64>()
                        .ok()
                        .filter(|_| suffix.bytes().all(|b| b.is_ascii_digit()))
                        .ok_or_else(|| format!("Unknown sheet template field: {token:?}"))?
                };
                (field, suffix.len(), offset)
            }
        };
        let width = if let Some(padding) = padding {
            if !field.is_number() || width != 0 {
                return Err(format!(
                    "Invalid padding in sheet template field: {token:?}"
                ));
            }
            padding
                .parse::<usize>()
                .map_err(|_| format!("Invalid sheet template padding: {token:?}"))?
        } else {
            width
        };
        if width > 32 {
            return Err("Sheet template padding is limited to 32 digits".into());
        }
        Ok(Part::Field(field, width, offset))
    }

    fn render<'b>(&self, substitute: impl Fn(Field) -> Substitution<'b>) -> Result<String, String> {
        use std::fmt::Write;
        let mut result = String::new();
        for part in &self.0 {
            match *part {
                Part::Text(text) => result.push_str(text),
                Part::Field(field, width, offset) => match substitute(field) {
                    Substitution::Text(text) => result.push_str(text),
                    Substitution::Number(Some(number)) => {
                        let number = number
                            .checked_add(offset)
                            .ok_or("Sheet template number overflow")?;
                        write!(result, "{number:0width$}").unwrap();
                    }
                    Substitution::Number(None) => {}
                },
            }
            if result.len() > 4096 {
                return Err("Exported sheet name exceeds 4096 bytes".into());
            }
        }
        if result.trim().is_empty() || result.chars().any(char::is_control) {
            return Err("Exported sheet names must contain text and no control characters".into());
        }
        Ok(result)
    }
}
