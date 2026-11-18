//! Copy-on-write placement words with the existing JSON array representation.
use super::{BudgetedVec, charge_project_bytes};
use serde::{Deserialize, Serialize};
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, OnceLock},
};

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct TileCells(Arc<Vec<u32>>);
impl Default for TileCells {
    fn default() -> Self {
        static EMPTY: OnceLock<Arc<Vec<u32>>> = OnceLock::new();
        Self(Arc::clone(EMPTY.get_or_init(|| Arc::new(Vec::new()))))
    }
}
impl From<Vec<u32>> for TileCells {
    fn from(value: Vec<u32>) -> Self {
        if value.is_empty() {
            Self::default()
        } else {
            Self(Arc::new(value))
        }
    }
}
impl FromIterator<u32> for TileCells {
    fn from_iter<I: IntoIterator<Item = u32>>(iter: I) -> Self {
        Vec::from_iter(iter).into()
    }
}
impl Deref for TileCells {
    type Target = Vec<u32>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for TileCells {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}
impl AsRef<[u32]> for TileCells {
    fn as_ref(&self) -> &[u32] {
        self.0.as_slice()
    }
}
impl<T: AsRef<[u32]> + ?Sized> PartialEq<T> for TileCells {
    fn eq(&self, other: &T) -> bool {
        {
            let left = self.0.as_slice();
            let right = other.as_ref();
            std::ptr::eq(left, right) || left == right
        }
    }
}
impl Eq for TileCells {}
impl PartialEq<TileCells> for Vec<u32> {
    fn eq(&self, other: &TileCells) -> bool {
        self.as_slice() == other.as_ref()
    }
}
impl<const N: usize> PartialEq<TileCells> for [u32; N] {
    fn eq(&self, other: &TileCells) -> bool {
        self.as_slice() == other.as_ref()
    }
}
impl<'de> Deserialize<'de> for TileCells {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let values = BudgetedVec::<u32, 3>::deserialize(decoder)?.0;
        if !values.is_empty() {
            charge_project_bytes::<D::Error>(
                std::mem::size_of::<Vec<u32>>() + 2 * std::mem::size_of::<usize>(),
            )?;
        }
        Ok(values.into())
    }
}
impl TileCells {
    pub fn backing_id(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
    pub fn shared(&self) -> Arc<Vec<u32>> {
        Arc::clone(&self.0)
    }
    pub fn set(&mut self, index: usize, value: u32) {
        if self.0[index] != value {
            Arc::make_mut(&mut self.0)[index] = value;
        }
    }
}
impl<'a> IntoIterator for &'a TileCells {
    type Item = &'a u32;
    type IntoIter = std::slice::Iter<'a, u32>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
impl<'a> IntoIterator for &'a mut TileCells {
    type Item = &'a mut u32;
    type IntoIter = std::slice::IterMut<'a, u32>;
    fn into_iter(self) -> Self::IntoIter {
        Arc::make_mut(&mut self.0).iter_mut()
    }
}
pub struct IntoIter {
    words: Arc<Vec<u32>>,
    range: std::ops::Range<usize>,
}
impl Iterator for IntoIter {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        self.range.next().map(|i| self.words[i])
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}
impl DoubleEndedIterator for IntoIter {
    fn next_back(&mut self) -> Option<u32> {
        self.range.next_back().map(|i| self.words[i])
    }
}
impl ExactSizeIterator for IntoIter {}
impl std::iter::FusedIterator for IntoIter {}
impl IntoIterator for TileCells {
    type Item = u32;
    type IntoIter = IntoIter;
    fn into_iter(self) -> IntoIter {
        let range = 0..self.0.len();
        IntoIter {
            words: self.0,
            range,
        }
    }
}
