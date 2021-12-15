//! Provides WrappedCow, a newtype built on std::borrow:Cow in order to implement
//! the rkyv::Archive trait.

use std::borrow::Cow;
use core::ops::Deref;

use rkyv::{Archive, Serialize as RkyvSerialize, Deserialize as RkyvDeserialize};

/// A newtype that wraps borrow::Cow.
#[derive(Archive, RkyvSerialize, RkyvDeserialize)]
pub struct WrappedCow<'a, B: ?Sized + ToOwned + 'a>(Cow<'a, B>);

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

impl<'a, T: Clone> From<Vec<T>> for Cow<'a, [T]> {
    /// Newtype requirement.
    fn from(v: Vec<T>) -> Cow<'a, [T]> {
        Cow::Owned(v)
    }
}

impl<'a, T: Clone> From<&'a Vec<T>> for Cow<'a, [T]> {
    /// Newtype requirement.
    fn from(v: &'a Vec<T>) -> Cow<'a, [T]> {
        Cow::Borrowed(v.as_slice())
    }
}
