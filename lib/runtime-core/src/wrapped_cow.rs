//! Provides WrappedCow, a newtype built on std::borrow:Cow in order to implement
//! the rkyv::Archive trait.

use std::borrow::Cow;
use core::borrow::Borrow;
use core::ops::Deref;
use std::hash::Hash;
use std::cmp::PartialEq;

use rkyv::{Archive, Serialize as RkyvSerialize, Deserialize as RkyvDeserialize};

/// A newtype that wraps borrow::Cow.
#[derive(Hash, Archive, RkyvSerialize, RkyvDeserialize, Serialize, Deserialize)]
pub struct WrappedCow<'a, B: ?Sized + ToOwned +  'a>(Cow<'a, B>);

impl<'a, B: ?Sized> Borrow<B> for WrappedCow<'a, B>
where
    B: ToOwned + Clone,
    <B as ToOwned>::Owned: 'a,
{
    fn borrow(&self) -> &B {
        self.0.borrow()
    }
}

impl<B: ?Sized + Clone> Eq for WrappedCow<'_, B> where B: Eq + ToOwned {}

impl<'a, 'b, B: ?Sized, C: ?Sized> PartialEq<WrappedCow<'b, C>> for WrappedCow<'a, B>
where
    B: PartialEq<C> + ToOwned + Clone,
    C: ToOwned + Clone,
{
    #[inline]
    fn eq(&self, other: &WrappedCow<'b, C>) -> bool {
        self.0.eq(&other.0)
    }
}

impl<B: ?Sized + ToOwned + Clone> Clone for WrappedCow<'_, B> {
    /// Passthrough method.
    fn clone(&self) -> Self {
        WrappedCow(self.0.clone())
    }

    /// Passthrough method.
    fn clone_from(&mut self, source: &Self) {
        self.0.clone_from(&source.0);
    }
}

impl<B: ?Sized + ToOwned + Clone> WrappedCow<'_, B> {
    /// Passthrough method.
    pub fn to_mut(&mut self) -> &mut <B as ToOwned>::Owned {
        self.0.to_mut()
    }

    /// Passthrough method.
    pub fn into_owned(self) -> <B as ToOwned>::Owned {
        self.0.into_owned()
    }
}

impl<B: ?Sized + ToOwned + Clone> Deref for WrappedCow<'_, B> {
    type Target = B;

    /// Passthrough method.
    fn deref(&self) -> &B {
        self.0.deref()
    }
}

impl<B: ?Sized + ToOwned + Clone> AsRef<B> for WrappedCow<'_, B> {
    /// Passthrough method.
    fn as_ref(&self) -> &B {
        self.0.as_ref()
    }
}

impl<'a, B: Clone> From<Vec<B>> for WrappedCow<'a, [B]> {
    /// Newtype requirement.
    fn from(v: Vec<B>) -> WrappedCow<'a, [B]> {
        WrappedCow(Cow::Owned(v))
    }
}

impl<'a, B: Clone> From<&'a Vec<B>> for WrappedCow<'a, [B]> {
    /// Newtype requirement.
    fn from(v: &'a Vec<B>) -> WrappedCow<'a, [B]> {
        WrappedCow(Cow::Borrowed(v.as_slice()))
    }
}

impl<'a, B> From<&'a [B]> for WrappedCow<'a, [B]>
where
    [B]: ToOwned
{
    /// Newtype requirement
    fn from(s: &'a [B]) -> WrappedCow<'a, [B]> {
        WrappedCow(Cow::Borrowed(s))
    }
}
