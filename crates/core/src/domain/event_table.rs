//! The tables an event collection is written in, one per protocol and event kind.

use crate::domain::Protocol;

/// One table a collection is written in; declared by `event_tables!`, never by hand.
pub trait EventTable: Copy + Send + Sync + 'static {
    /// Every table of the collection.
    const ALL: &'static [Self];

    /// Its name in the database, the logs and a live cursor: unique across collections.
    fn name(self) -> &'static str;

    fn protocol(self) -> Protocol;
}

/// A collection's tables, their names and protocols from one list.
/// ⚠️ One list: a table declared outside `ALL` would compile and never be read live.
macro_rules! event_tables {
    ($(#[$meta:meta])* $enum:ident { $($table:ident => ($protocol:ident, $name:literal)),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $enum {
            $($table),+
        }

        impl $crate::domain::EventTable for $enum {
            const ALL: &'static [Self] = &[$(Self::$table),+];

            fn name(self) -> &'static str {
                match self {
                    $(Self::$table => $name),+
                }
            }

            fn protocol(self) -> $crate::domain::Protocol {
                match self {
                    $(Self::$table => $crate::domain::Protocol::$protocol),+
                }
            }
        }
    };
}
pub(crate) use event_tables;
