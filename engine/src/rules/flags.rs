//! Small enums for the string-valued tags in the rule data, plus a bitset over them.
//!
//! Terrain and resource discriminants equal the `idx` the game uses in its rule
//! tables; the loader checks this, so a change in the data fails loudly.

use std::marker::PhantomData;

use serde::Deserialize;

/// An enum that can live in a [`FlagSet`].
pub trait Flag: Copy {
    fn bit(self) -> u32;
}

/// Bitset of up to 64 flags of one enum type.
pub struct FlagSet<T: Flag>(u64, PhantomData<T>);

impl<T: Flag> FlagSet<T> {
    pub const fn empty() -> Self {
        FlagSet(0, PhantomData)
    }

    pub fn contains(&self, flag: T) -> bool {
        self.0 & (1 << flag.bit()) != 0
    }

    pub fn insert(&mut self, flag: T) {
        self.0 |= 1 << flag.bit();
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn bits(&self) -> u64 {
        self.0
    }
}

impl<T: Flag> FromIterator<T> for FlagSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut set = Self::empty();
        for flag in iter {
            set.insert(flag);
        }
        set
    }
}

// Manual impls: derives would add an unwanted `T: Clone/Debug/...` bound.
impl<T: Flag> Clone for FlagSet<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Flag> Copy for FlagSet<T> {}
impl<T: Flag> Default for FlagSet<T> {
    fn default() -> Self {
        Self::empty()
    }
}
impl<T: Flag> PartialEq for FlagSet<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T: Flag> Eq for FlagSet<T> {}
impl<T: Flag> std::fmt::Debug for FlagSet<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FlagSet({:#b})", self.0)
    }
}

macro_rules! flag_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $disc:expr),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
        #[serde(rename_all = "lowercase")]
        #[repr(u8)]
        pub enum $name { $($variant = $disc),+ }

        impl Flag for $name {
            fn bit(self) -> u32 {
                self as u8 as u32
            }
        }
    };
}

flag_enum! {
    /// Order and values match `terrainData.*.idx`.
    Terrain {
        None = 0, Water = 1, Ocean = 2, Field = 3, Mountain = 4,
        Forest = 5, Ice = 6, Wetland = 7, Mangrove = 8,
    }
}

flag_enum! {
    /// Order and values match `resourceData.*.idx`.
    Resource {
        None = 0, Game = 1, Crop = 2, Fish = 3, Whale = 4,
        Metal = 5, Fruit = 6, Spores = 7, Starfish = 8, Aquacrop = 9,
    }
}

flag_enum! {
    /// Every value seen in `unitData.*.unitAbilities`. Semantics are not yet
    /// known; they live in code (to be decompiled).
    UnitAbility {
        Algae = 0, Amphibious = 1, Autoflood = 2, Autofreeze = 3, Carry = 4,
        Consumed = 5, Convert = 6, Creep = 7, Dash = 8, Doubleattack = 9,
        Drench = 10, Eat = 11, Escape = 12, Explode = 13, Fly = 14,
        Fortify = 15, Freeze = 16, Freezearea = 17, Grow = 18, Heal = 19,
        Hide = 20, Independent = 21, Infiltrate = 22, Land = 23, Persist = 24,
        Poison = 25, Scout = 26, Skate = 27, Sneak = 28, Splash = 29,
        Static = 30, Stiff = 31, Stomp = 32, Surprise = 33, Swarm = 34,
        Swim = 35, Tentacles = 36, Water = 37,
    }
}

flag_enum! {
    /// Every value seen in `improvementData.*.improvementAbilities`.
    ImprovementAbility {
        Attract = 0, Bridge = 1, Consumed = 2, Discrete = 3, Embark = 4,
        Flood = 5, Freelance = 6, Heal = 7, Limited = 8, Manual = 9,
        Network = 10, Patina = 11, Poison = 12, Simple = 13, Unique = 14,
    }
}

flag_enum! {
    /// Every value seen in `tribeData.*.tribeAbilities`.
    TribeAbility {
        Alienclimate = 0, Flooded = 1, Freeze = 2, Poisonresist = 3,
        Rainbowvision = 4, Spread = 5,
    }
}

flag_enum! {
    /// Every value seen in `improvementData.*.creates[].effect`.
    Effect {
        Algae = 0,
    }
}
