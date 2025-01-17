use git_hash::ObjectId;
use git_traverse::commit::{ancestors, Ancestors};

pub trait Sealed {}

/// An extension trait to add functionality to [`ObjectId`]s.
pub trait ObjectIdExt: Sealed {
    /// Create an iterator over the ancestry of the commits reachable from this id, which must be a commit.
    fn ancestors<Find, E>(self, find: Find) -> Ancestors<Find, fn(&git_hash::oid) -> bool, ancestors::State>
    where
        Find: for<'a> FnMut(&git_hash::oid, &'a mut Vec<u8>) -> Result<git_object::CommitRefIter<'a>, E>,
        E: std::error::Error + Send + Sync + 'static;

    /// Infuse this object id `repo` access.
    fn attach(self, repo: &crate::Repository) -> crate::Id<'_>;
}

impl Sealed for ObjectId {}

impl ObjectIdExt for ObjectId {
    fn ancestors<Find, E>(self, find: Find) -> Ancestors<Find, fn(&git_hash::oid) -> bool, ancestors::State>
    where
        Find: for<'a> FnMut(&git_hash::oid, &'a mut Vec<u8>) -> Result<git_object::CommitRefIter<'a>, E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        Ancestors::new(Some(self), ancestors::State::default(), find)
    }

    fn attach(self, repo: &crate::Repository) -> crate::Id<'_> {
        crate::Id::from_id(self, repo)
    }
}
