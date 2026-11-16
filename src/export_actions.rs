use super::*;
use pixelcraft::workflows::SheetOptions;
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[derive(Clone)]
pub(super) struct ExportRecipe {
    pub(super) path: PathBuf,
    pub(super) kind: ExportKind,
}

#[derive(Clone)]
pub(super) enum ExportKind {
    Raster {
        frame_id: u32,
        scale: usize,
        sheet: bool,
    },
    Animation {
        format: &'static str,
        scale: usize,
    },
    Sequence {
        scale: usize,
    },
    BakedAse,
    Sheet {
        options: Box<SheetOptions>,
        slice: Option<String>,
        layers: Option<Vec<String>>,
        tag: Option<String>,
    },
    Tileset {
        columns: usize,
        layers: Option<Vec<String>>,
    },
    Tiled {
        frame_id: u32,
    },
    Tags {
        gif: bool,
        options: Box<SheetOptions>,
    },
    MultiAtlas {
        inputs: Vec<PathBuf>,
        options: pixelcraft::atlas::AtlasOptions,
    },
}

pub(super) struct ExportJob {
    origin: String,
    recipe: ExportRecipe,
    receiver: Receiver<Result<(), String>>,
}

impl ExportJob {
    #[cfg(test)]
    pub(super) fn recv_timeout(
        self,
        timeout: std::time::Duration,
    ) -> Result<(PathBuf, Result<(), String>), mpsc::RecvTimeoutError> {
        self.receiver
            .recv_timeout(timeout)
            .map(|result| (self.recipe.path, result))
    }
}

impl ExportRecipe {
    fn write(&self, doc: &Document, repeat: bool) -> Result<(), String> {
        use pixelcraft::{automation, interchange};
        let path = &self.path;
        let prepare = |args: serde_json::Value| {
            automation::prepare_live_export(doc, 0, &args, "")
                .and_then(|export| export.write())
                .map(|_| ())
        };
        let frame = |id| {
            (0..doc.durations.len())
                .find(|&frame| doc.frame_export_id(frame) == id)
                .ok_or_else(|| "The exported frame was deleted".to_owned())
        };
        match &self.kind {
            ExportKind::Raster {
                frame_id,
                scale,
                sheet,
            } => formats::export_raster_scaled(path, doc, frame(*frame_id)?, *sheet, *scale),
            ExportKind::Animation { format, scale } => {
                automation::write_animation_export(format, path, doc, *scale)
            }
            ExportKind::Sequence { scale } => prepare(serde_json::json!({
                "output":path, "format":"png-sequence", "scale":scale, "overwrite":repeat,
            })),
            ExportKind::BakedAse => formats::export_baked_ase(path, doc),
            ExportKind::Sheet {
                options,
                slice,
                layers,
                tag,
            } => {
                let mut options = options.as_ref().clone();
                options.layers = resolve_layer_ids(doc, layers.as_deref())?;
                if let Some(name) = tag
                    && options
                        .tag
                        .and_then(|index| doc.tags.get(index))
                        .is_none_or(|tag| &tag.name != name)
                {
                    let mut matches = doc
                        .tags
                        .iter()
                        .enumerate()
                        .filter(|(_, tag)| &tag.name == name);
                    let (index, _) = matches
                        .next()
                        .ok_or("The exported tag was removed or renamed")?;
                    if matches.next().is_some() {
                        return Err("More than one tag has the saved export name".into());
                    }
                    options.tag = Some(index);
                }
                automation::write_editor_sheet_export(path, doc, &options, slice.as_deref())
            }
            ExportKind::Tileset { columns, layers } => {
                let mut args =
                    serde_json::json!({"output":path,"format":"tileset-atlas","overwrite":true});
                if let Some(layers) = resolve_layer_ids(doc, layers.as_deref())? {
                    args["layers"] = serde_json::json!(layers);
                }
                if *columns != 0 {
                    args["columns"] = serde_json::json!(columns);
                }
                prepare(args)
            }
            ExportKind::Tiled { frame_id } => {
                interchange::export_tiled(path, doc, frame(*frame_id)?)
            }
            ExportKind::Tags { gif, options } => {
                pixelcraft::productivity::batch_tags(path, doc, *gif, options).map(|_| ())
            }
            ExportKind::MultiAtlas { inputs, options } => {
                use pixelcraft::atlas::{pack_atlas, save_packed_atlas_with_metadata};
                let sources = load_atlas_inputs(inputs)?;
                let packed = pack_atlas(&sources, options)?;
                save_packed_atlas_with_metadata(path, &packed, options.metadata)
            }
        }
    }
}

pub(super) fn load_atlas_inputs(inputs: &[PathBuf]) -> Result<Vec<(String, Document)>, String> {
    inputs
        .iter()
        .map(|path| {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            Ok((name, formats::load(path)?))
        })
        .collect()
}

pub(super) fn export_layer_ids(doc: &Document, indices: &[usize]) -> Vec<String> {
    indices
        .iter()
        .filter_map(|&index| doc.layers.get(index))
        .map(|layer| layer.editor_id.to_string())
        .collect()
}

fn resolve_layer_ids(doc: &Document, ids: Option<&[String]>) -> Result<Option<Vec<usize>>, String> {
    ids.map(|ids| {
        let indices: std::collections::HashMap<_, _> = doc
            .layers
            .iter()
            .enumerate()
            .map(|(index, layer)| (layer.editor_id.to_string(), index))
            .collect();
        ids.iter()
            .map(|id| {
                indices
                    .get(id)
                    .copied()
                    .ok_or_else(|| "An exported layer was deleted".to_owned())
            })
            .collect()
    })
    .transpose()
}

impl PixelCraft {
    pub(super) fn can_repeat_export(&self) -> bool {
        self.export_job.is_none() && self.last_export.is_some()
    }

    pub(super) fn repeat_export(&mut self) {
        if self.export_job.is_some() {
            self.status = "An export is already running".into();
        } else if let Some(recipe) = self.last_export.clone() {
            self.start_export_recipe(recipe, true);
        } else {
            self.status = "Export this sprite first".into();
        }
    }

    pub(super) fn start_export_recipe(&mut self, recipe: ExportRecipe, repeat: bool) {
        if self.export_job.is_some() {
            self.status = "An export is already running".into();
            return;
        }
        let doc = self.doc.clone();
        let (tx, receiver) = mpsc::channel();
        self.status = format!(
            "Exporting {}",
            recipe
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );
        self.export_job = Some(ExportJob {
            origin: doc.editor_id.to_string(),
            recipe: recipe.clone(),
            receiver,
        });
        std::thread::spawn(move || {
            let _ = tx.send(recipe.write(&doc, repeat));
        });
    }

    pub(super) fn poll_export(&mut self) {
        let Some(job) = &self.export_job else { return };
        let result = job.receiver.try_recv();
        if matches!(result, Err(TryRecvError::Empty)) {
            return;
        }
        let job = self.export_job.take().unwrap();
        self.status = match result {
            Ok(Ok(())) => {
                let label = format!(
                    "Exported {}",
                    job.recipe
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                );
                if self.doc.editor_id.to_string() == job.origin {
                    self.last_export = Some(job.recipe);
                } else if let Some(tab) = self
                    .tabs
                    .iter_mut()
                    .flatten()
                    .find(|tab| tab.doc.editor_id.to_string() == job.origin)
                {
                    tab.last_export = Some(job.recipe);
                }
                label
            }
            Ok(Err(error)) => format!("Export failed: {error}"),
            Err(_) => "Export worker stopped unexpectedly".into(),
        };
    }
}
