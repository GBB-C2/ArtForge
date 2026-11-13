//! Immutable snapshots share large assets; the first write detaches the buffer.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

static REVISION: AtomicU64 = AtomicU64::new(1);
fn next_revision() -> u64 {
    REVISION.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone)]
pub struct SharedBuffer<T> {
    data: Arc<Vec<T>>,
    revision: u64,
}
impl<T> SharedBuffer<T> {
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn shares_storage(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }
}
impl<T> From<Vec<T>> for SharedBuffer<T> {
    fn from(data: Vec<T>) -> Self {
        Self {
            data: Arc::new(data),
            revision: next_revision(),
        }
    }
}
impl<T> Default for SharedBuffer<T> {
    fn default() -> Self {
        Vec::new().into()
    }
}
impl<T> Deref for SharedBuffer<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.data
    }
}
impl<T: Clone> DerefMut for SharedBuffer<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        self.revision = next_revision();
        Arc::make_mut(&mut self.data)
    }
}
impl<T: PartialEq> PartialEq for SharedBuffer<T> {
    fn eq(&self, rhs: &Self) -> bool {
        Arc::ptr_eq(&self.data, &rhs.data) || self.data == rhs.data
    }
}
impl<T: PartialEq> PartialEq<Vec<T>> for SharedBuffer<T> {
    fn eq(&self, rhs: &Vec<T>) -> bool {
        **self == *rhs
    }
}
impl<T: PartialEq, const N: usize> PartialEq<[T; N]> for SharedBuffer<T> {
    fn eq(&self, rhs: &[T; N]) -> bool {
        self.as_slice() == rhs
    }
}
impl<T: Eq> Eq for SharedBuffer<T> {}
impl<T: Serialize> Serialize for SharedBuffer<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.data.serialize(s)
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for SharedBuffer<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Vec::<T>::deserialize(d)?.into())
    }
}
impl<T> FromIterator<T> for SharedBuffer<T> {
    fn from_iter<I: IntoIterator<Item = T>>(i: I) -> Self {
        i.into_iter().collect::<Vec<_>>().into()
    }
}
impl<T: Clone> IntoIterator for SharedBuffer<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        Arc::unwrap_or_clone(self.data).into_iter()
    }
}
impl<'a, T> IntoIterator for &'a SharedBuffer<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T: Clone> IntoIterator for &'a mut SharedBuffer<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
impl<T> AsRef<[T]> for SharedBuffer<T> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_share_storage_and_mutation_preserves_undo_and_json() {
        let original: SharedBuffer<u8> = vec![1, 2, 3].into();
        let mut snapshot = original.clone();
        assert!(original.shares_storage(&snapshot));
        let revision = snapshot.revision();
        snapshot[0] = 9;
        assert_ne!(revision, snapshot.revision());
        assert!(!original.shares_storage(&snapshot));
        assert_eq!(original, vec![1, 2, 3]);
        assert_eq!(serde_json::to_string(&snapshot).unwrap(), "[9,2,3]");
        let loaded: SharedBuffer<u8> = serde_json::from_str("[1,2,3]").unwrap();
        assert_eq!(loaded, original);
    }
}
