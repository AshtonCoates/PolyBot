//! Dense ids into the compiled rule tables.
//!
//! Ids are positions in the table, ordered by the game's own `idx`. They are
//! NOT the game's `idx` (which has gaps); that is kept as `game_idx` on each
//! entry for matching against the savestate.

macro_rules! id_type {
    ($($(#[$meta:meta])* $name:ident),+ $(,)?) => {
        $(
            $(#[$meta])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $name(pub u8);

            impl $name {
                pub fn index(self) -> usize {
                    self.0 as usize
                }
            }
        )+
    };
}

id_type! {
    TribeId,
    TechId,
    UnitId,
    ImprovementId,
    TaskId,
}
