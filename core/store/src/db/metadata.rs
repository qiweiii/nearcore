/// Database version type.
pub type DbVersion = u32;

/// Current version of the database.
pub const DB_VERSION: DbVersion = 45;

/// Database version at which point DbKind was introduced.
const DB_VERSION_WITH_KIND: DbVersion = 34;

/// Key for the version entry in DBCol::DbVersion.
///
/// The key holds [`DbVersion`] value serialized to a string.
///
/// The version is strictly increasing.  We bump it each time
/// a backwards-incompatible change to the database is required.  Increasing the
/// version involves performing a migration since node can only open databases
/// with version they was built for (see [`DB_VERSION`]).
pub(crate) const VERSION_KEY: &[u8; 7] = b"VERSION";

/// Key for the database kind entry in DBCol::DbVersion.
///
/// The key holds a [`DbKind`] value serialized to a string.
pub(crate) const KIND_KEY: &[u8; 4] = b"KIND";

/// Describes what kind the storage is.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, strum::Display, strum::EnumString, strum::IntoStaticStr,
)]
pub enum DbKind {
    /// The database is an RPC database meaning that it is garbage collected and
    /// stores only a handful of epochs worth of data.
    RPC,
    /// The database is an archive database meaning that it is not garbage
    /// collected and stores all chain data.
    Archive,
    /// The database is Hot meaning that the node runs in archival mode with
    /// a paired Cold database.
    Hot,
    /// The database is Cold meaning that the node runs in archival mode with
    /// a paired Hot database.
    Cold,
}

/// Metadata about a database.
#[derive(Clone, Copy)]
pub(crate) struct DbMetadata {
    /// Version of the database.
    pub version: DbVersion,

    /// Kind of the database.
    ///
    /// This is always set if version is ≥ [`DB_VERSION_WITH_KIND`] and always
    /// `None` otherwise.
    pub kind: Option<DbKind>,
}

impl DbMetadata {
    /// Reads metadata from the database. This method enforces the invariant
    /// that version and kind must always be set.
    ///
    /// If the database version is not present, returns an error.  Similarly, if
    /// database version is ≥ [`DB_VERSION_WITH_KIND`] but the kind is not
    /// specified, returns an error.
    pub(crate) fn read(db: &dyn crate::Database) -> std::io::Result<Self> {
        let version = read("DbVersion", db, VERSION_KEY)?;
        let kind = if version < DB_VERSION_WITH_KIND {
            // If database is at version less than DB_VERSION_WITH_KIND then it
            // doesn’t have kind.  Return None as kind.
            None
        } else {
            Some(read("DbKind", db, KIND_KEY)?)
        };
        Ok(Self { version, kind })
    }

    /// Reads the version from the db. If version is not set returns None. This
    /// method doesn't enforce the invariant that version must always be set so
    /// it should only be used when setting the version for the first time.
    pub(crate) fn maybe_read_version(
        db: &dyn crate::Database,
    ) -> std::io::Result<Option<DbVersion>> {
        maybe_read("DbVersion", db, VERSION_KEY)
    }

    /// Reads the kind from the db. If kind is not set returns None. This method
    /// doesn't enforce the invariant that kind must always be set so it should
    /// only be used when setting the kind for the first time.
    pub(crate) fn maybe_read_kind(db: &dyn crate::Database) -> std::io::Result<Option<DbKind>> {
        maybe_read("DbKind", db, KIND_KEY)
    }
}

/// Reads value from DbVersion column and parses it using `FromStr`.
///
/// Same as maybe_read but this method returns an error if the value is not set.
fn read<T: std::str::FromStr>(
    what: &str,
    db: &dyn crate::Database,
    key: &[u8],
) -> std::io::Result<T> {
    let msg = "it’s not a neard database or database is corrupted";
    let result = maybe_read::<T>(what, db, key)?;

    match result {
        Some(value) => Ok(value),
        None => Err(std::io::Error::other(format!("missing {what}; {msg}"))),
    }
}

/// Reads value from DbVersion column and parses it using `FromStr`.
///
/// Reads raw bytes for given `key` from [`DBCol::DbVersion`], verifies that
/// they’re valid UTF-8 and then converts into `T` using `from_str`.  If the
/// value is missing or parsing fails returns None.
fn maybe_read<T: std::str::FromStr>(
    what: &str,
    db: &dyn crate::Database,
    key: &[u8],
) -> std::io::Result<Option<T>> {
    let msg = "it’s not a neard database or database is corrupted";
    db.get_raw_bytes(crate::DBCol::DbVersion, key)?
        .map(|bytes| {
            let value = std::str::from_utf8(&bytes)
                .map_err(|_err| format!("invalid {what}: {bytes:?}; {msg}"))?;
            T::from_str(value).map_err(|_err| format!("invalid {what}: ‘{value}’; {msg}"))
        })
        .transpose()
        .map_err(std::io::Error::other)
}
