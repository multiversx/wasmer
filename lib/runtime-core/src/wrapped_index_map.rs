//! Provides WrappedIndexMap, a newtype built on indexmap::IndexMap in order to implement
//! the rkyv::Archive trait.

use ::core::hash::Hash;
use indexmap::{IndexMap, Equivalent};
use indexmap::map::{Iter, Values};
use rkyv::{Archive, Serialize as RkyvSerialize, Deserialize as RkyvDeserialize};
use ::core::fmt;
use serde::ser::{Serialize, Serializer};
use serde::de::{ Deserialize, Deserializer};
use ::core::ops::Index;

/// A newtype that wraps indexmap::IndexMap.
#[derive(Archive, RkyvSerialize, RkyvDeserialize)]
pub struct WrappedIndexMap<K, V>(IndexMap<K, V>);

impl<K, V> WrappedIndexMap<K, V>
where
    K: Hash + Eq,
{
    /// Passthrough method.
    pub fn new() -> Self {
        WrappedIndexMap(IndexMap::new())
    }

    /// Passthrough method.
    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        Q: Hash + Eq + Equivalent<K>,
    {
        self.0.get(key)
    }

    /// Passthrough method.
    pub fn iter(&self) -> Iter<'_, K, V> {
        self.0.iter()
    }

    /// Passthrough method.
    pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
    where
        Q: Hash + Eq + Equivalent<K>,
    {
        self.0.contains_key(key)
    }

    /// Passthrough method.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }

    /// Passthrough method.
    pub fn values(&self) -> Values<'_, K, V> {
        self.0.values()
    }
}

impl<K, V> Clone for WrappedIndexMap<K, V>
where
    K: Clone,
    V: Clone,
{
    /// Passthrough method.
    fn clone(&self) -> Self {
        WrappedIndexMap(self.0.clone())
    }

    /// Passthrough method.
    fn clone_from(&mut self, other: &Self) {
        self.0.clone_from(&other.0);
    }
}

impl<K, V> fmt::Debug for WrappedIndexMap<K, V>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    /// Passthrough method.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<K, V> Serialize for WrappedIndexMap<K, V>
where
    K: Serialize + Hash + Eq,
    V: Serialize,
{
    /// Passthrough method.
    fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
    where
        T: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, K, V> Deserialize<'de> for WrappedIndexMap<K, V>
where
    K: Deserialize<'de> + Eq + Hash,
    V: Deserialize<'de>,
{
    /// Passthrough method.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        serde::Deserialize::deserialize(deserializer)
    }
}

impl<K, V, Q: ?Sized> Index<&Q> for WrappedIndexMap<K, V>
where
    Q: Hash + Equivalent<K>,
    K: Hash + Eq,
{
    type Output = V;

    /// Passthrough method.
    fn index(&self, key: &Q) -> &V {
        self.0.index(key)
    }
}

impl<K, V> Default for WrappedIndexMap<K, V>
{
    /// Passthrough method.
    fn default() -> Self {
        WrappedIndexMap(IndexMap::default())
    }
}
