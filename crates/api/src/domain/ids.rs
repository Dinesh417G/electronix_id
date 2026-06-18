//! Strongly-typed UUID newtypes. A `UserId` is not interchangeable with a
//! `MachineId` even though both wrap a `Uuid` — the compiler enforces it.
//!
//! All app-created ids are UUID **v7** (time-sortable). Stored as `CHAR(36)`.

use uuid::Uuid;

macro_rules! id_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub Uuid);

        impl $name {
            /// Generate a fresh time-sortable v7 id.
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }

        impl From<Uuid> for $name {
            fn from(u: Uuid) -> Self {
                Self(u)
            }
        }
    };
}

id_newtype!(OrgId);
id_newtype!(UserId);
id_newtype!(MachineId);
id_newtype!(DocumentId);
id_newtype!(VersionId);
id_newtype!(PlanId);
id_newtype!(SubscriptionId);
id_newtype!(RefreshTokenId);
