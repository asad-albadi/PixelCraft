use super::{Document, layer_operations::layer_owner};

#[derive(Clone, Copy, Debug)]
pub enum LayerDrop {
    Above(usize),
    Below(usize),
    Into(usize),
}

type RestackedLayers = (Document, Vec<usize>);

impl Document {
    pub fn can_restack_layers(&self, selected: &[usize], drop: LayerDrop) -> bool {
        self.layer_drop_parent(selected, drop).is_ok()
    }

    fn layer_drop_parent(
        &self,
        selected: &[usize],
        drop: LayerDrop,
    ) -> Result<Option<usize>, String> {
        let target = match drop {
            LayerDrop::Above(layer) | LayerDrop::Below(layer) | LayerDrop::Into(layer) => layer,
        };
        let dest = self
            .layers
            .get(target)
            .ok_or("Choose a destination layer")?;
        if selected.is_empty() || selected.iter().any(|&layer| layer >= self.layers.len()) {
            return Err("Choose layers to move".into());
        }
        if selected.iter().any(|&layer| self.layers[layer].background) {
            return Err("A background layer stays at the bottom of the stack".into());
        }
        let parent = match drop {
            LayerDrop::Into(_) if dest.group => Some(target),
            LayerDrop::Into(_) => return Err("Drop onto a group to move layers inside it".into()),
            LayerDrop::Below(_) if dest.background => {
                return Err("Layers stay above the background".into());
            }
            _ => dest.parent,
        };
        let mut current = Some(target);
        for _ in 0..=128 {
            let Some(index) = current else {
                return Ok(parent);
            };
            if selected.contains(&index) {
                return Err(
                    "Choose a destination outside the selected layers and their children".into(),
                );
            }
            current = self
                .layers
                .get(index)
                .ok_or("Invalid layer group hierarchy")?
                .parent;
        }
        Err("Invalid layer group hierarchy".into())
    }

    /// Move selected branches together in their current stacking order.
    /// The position map turns old layer indices into their indices in the result.
    pub fn with_layers_restacked(
        &self,
        selected: &[usize],
        drop: LayerDrop,
    ) -> Result<Option<RestackedLayers>, String> {
        let parent = self.layer_drop_parent(selected, drop)?;
        self.validate()?;
        let root = self.layers.len();
        let mut children = vec![Vec::new(); root + 1];
        for (index, layer) in self.layers.iter().enumerate() {
            children[layer.parent.unwrap_or(root)].push(index);
        }
        let previous = flatten_layer_tree(&children, root);
        let mut picked = vec![false; root];
        for &layer in selected {
            picked[layer] = true;
        }
        let mut moving = vec![false; root];
        for &layer in selected {
            let mut ancestor = self.layers[layer].parent;
            while let Some(index) = ancestor.filter(|&index| !picked[index]) {
                ancestor = self.layers[index].parent;
            }
            moving[layer] = ancestor.is_none();
        }
        let ordered = previous.iter().copied().filter(|&layer| moving[layer]);
        for siblings in &mut children {
            siblings.retain(|&layer| !moving[layer]);
        }
        let siblings = &mut children[parent.unwrap_or(root)];
        let insertion = match drop {
            LayerDrop::Above(layer) | LayerDrop::Below(layer) => {
                let index = siblings
                    .iter()
                    .position(|&index| index == layer)
                    .ok_or("Invalid destination layer")?;
                index + usize::from(matches!(drop, LayerDrop::Above(_)))
            }
            LayerDrop::Into(_) => siblings.len(),
        };
        siblings.splice(insertion..insertion, ordered);
        let order = flatten_layer_tree(&children, root);
        if order == previous
            && selected
                .iter()
                .all(|&layer| !moving[layer] || self.layers[layer].parent == parent)
        {
            return Ok(None);
        }

        let mut positions = vec![0; order.len()];
        for (index, &old) in order.iter().enumerate() {
            positions[old] = index;
        }
        let mut doc = self.clone();
        let mut layers = std::mem::take(&mut doc.layers)
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>();
        doc.layers = order
            .into_iter()
            .map(|index| {
                let mut layer = layers[index]
                    .take()
                    .expect("each layer has one place in the tree");
                if moving[index] {
                    layer.parent = parent;
                }
                layer.parent = layer.parent.map(|parent| positions[parent]);
                layer
            })
            .collect();
        for chunk in &mut doc.metadata {
            if let Some((prefix, index)) = layer_owner(&chunk.owner)
                && let Some(reordered) = positions.get(index)
                && *reordered != index
            {
                chunk.owner = format!("{prefix}{reordered}");
            }
        }
        doc.validate()?;
        Ok(Some((doc, positions)))
    }
}

fn flatten_layer_tree(children: &[Vec<usize>], root: usize) -> Vec<usize> {
    fn append(children: &[Vec<usize>], parent: usize, order: &mut Vec<usize>) {
        for &layer in &children[parent] {
            append(children, layer, order);
            order.push(layer);
        }
    }
    // The document stores bottom-to-top stacking. Put children before their
    // group row so reversing the stack shows the parent above its children.
    let mut order = Vec::with_capacity(root);
    append(children, root, &mut order);
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MetadataChunk;

    #[test]
    fn moving_nonadjacent_roots_reparents_groups_and_keeps_metadata_with_children() {
        let mut doc = Document::new(2, 1);
        doc.layers[0].name = "Child".into();
        let group = doc.add_group("Group").unwrap();
        doc.layers[0].parent = Some(group);
        doc.add_layer();
        doc.layers[2].name = "Unselected".into();
        doc.add_layer();
        doc.layers[3].name = "Selected".into();
        let target = doc.add_group("Destination").unwrap();
        doc.set_pixel(0, 0, 0, [12, 98, 156, 255]);
        doc.metadata.push(MetadataChunk {
            frame: 0,
            kind: 0x2020,
            data: vec![1, 2],
            owner: "cel:0".into(),
        });
        let child_id = doc.layers[0].cel_meta[0].editor_id.to_string();

        let (next, remap) = doc
            .with_layers_restacked(&[0, group, 3], LayerDrop::Into(target))
            .unwrap()
            .unwrap();
        assert_eq!(remap, [1, 2, 0, 3, 4]);
        assert_eq!(next.layers[1].parent, Some(2));
        assert_eq!(next.layers[2].parent, Some(4));
        assert_eq!(next.layers[3].parent, Some(4));
        assert_eq!(next.layers[0].parent, None);
        assert_eq!(next.layers[1].cel_meta[0].editor_id.to_string(), child_id);
        assert_eq!(next.metadata[0].owner, "cel:1");
        assert_eq!(next.composite(0), doc.composite(0));
        next.validate().unwrap();
        let reopened =
            crate::formats::decode_project(&crate::formats::encode_project(&next).unwrap())
                .unwrap();
        assert_eq!(reopened, next);

        let (extracted, _) = reopened
            .with_layers_restacked(&[1], LayerDrop::Above(4))
            .unwrap()
            .unwrap();
        assert_eq!(extracted.layers[4].name, "Child");
        assert_eq!(extracted.layers[4].parent, None);
        assert_eq!(extracted.metadata[0].owner, "cel:4");
        assert_eq!(extracted.composite(0), doc.composite(0));
    }

    #[test]
    fn restacking_preserves_background_and_rejects_cycles() {
        let mut doc = Document::new(1, 1);
        doc.layers[0].background = true;
        let group = doc.add_group("Parent").unwrap();
        let child = doc.add_group("Child").unwrap();
        doc.layers[child].parent = Some(group);
        doc.add_layer();
        let before = doc.clone();
        assert!(
            doc.with_layers_restacked(&[group], LayerDrop::Into(child))
                .is_err()
        );
        assert!(
            doc.with_layers_restacked(&[group], LayerDrop::Below(child))
                .is_err()
        );
        assert!(
            doc.with_layers_restacked(&[0], LayerDrop::Above(group))
                .is_err()
        );
        assert!(
            doc.with_layers_restacked(&[3], LayerDrop::Below(0))
                .is_err()
        );
        assert!(
            doc.with_layers_restacked(&[3], LayerDrop::Above(group))
                .unwrap()
                .is_none()
        );
        assert_eq!(doc, before);
    }
}
