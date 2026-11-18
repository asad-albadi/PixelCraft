//! Shared editor selection snapshots. Wire data stays a plain boolean array.
use serde::{Deserialize, Serialize};
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub(super) struct SelectionMask(Arc<Vec<bool>>);

impl PartialEq for SelectionMask {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
    }
}
impl Eq for SelectionMask {}
impl From<Vec<bool>> for SelectionMask {
    fn from(mask: Vec<bool>) -> Self {
        Self(Arc::new(mask))
    }
}
impl FromIterator<bool> for SelectionMask {
    fn from_iter<T: IntoIterator<Item = bool>>(values: T) -> Self {
        Vec::from_iter(values).into()
    }
}
impl Deref for SelectionMask {
    type Target = [bool];
    fn deref(&self) -> &Self::Target {
        self.0.as_slice()
    }
}
impl DerefMut for SelectionMask {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0).as_mut_slice()
    }
}
impl IntoIterator for SelectionMask {
    type Item = bool;
    type IntoIter = std::vec::IntoIter<bool>;
    fn into_iter(self) -> Self::IntoIter {
        Arc::unwrap_or_clone(self.0).into_iter()
    }
}
impl<'a> IntoIterator for &'a SelectionMask {
    type Item = &'a bool;
    type IntoIter = std::slice::Iter<'a, bool>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl PartialEq<Vec<bool>> for SelectionMask {
    fn eq(&self, other: &Vec<bool>) -> bool {
        self.as_ref() == other.as_slice()
    }
}
impl AsRef<[bool]> for SelectionMask {
    fn as_ref(&self) -> &[bool] {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_wire_remains_a_boolean_array_and_detached_edits_do_not_change_it() {
        let mut mask: SelectionMask = serde_json::from_str("[true,false,true]").unwrap();
        let snapshot = mask.clone();
        mask[0] = false;
        assert_eq!(
            serde_json::to_string(&snapshot).unwrap(),
            "[true,false,true]"
        );
        assert_eq!(serde_json::to_string(&mask).unwrap(), "[false,false,true]");
        assert_eq!(
            snapshot.into_iter().collect::<Vec<_>>(),
            vec![true, false, true]
        );
    }
}
