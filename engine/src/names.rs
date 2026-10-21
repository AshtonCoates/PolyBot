//! City names (`MapDataExtensions.GenerateCityName` -> `PolyLanguage.MakeWord`) and the .NET `System.Random` they use.
//!
//! The name of a city is built from the tribe's syllables with a seeded `System.Random` (the classic subtractive
//! generator), seeded by the game's village-name hash of the city's coordinates. Verified against the capital names of
//! the fixtures (`names_match_the_capitals_of_the_fixtures`).

use crate::scout::xxh32_words;

/// `System.Random(seed)`: Knuth's subtractive generator, as in the .NET Framework / Unity.
pub struct NetRandom {
    seed_array: [i32; 56],
    inext: usize,
    inextp: usize,
}

impl NetRandom {
    pub fn new(seed: i32) -> Self {
        const MBIG: i32 = i32::MAX;
        const MSEED: i32 = 161_803_398;
        let subtraction = if seed == i32::MIN { i32::MAX } else { seed.abs() };
        let mut a = [0i32; 56];
        let mut mj = MSEED - subtraction;
        a[55] = mj;
        let mut mk = 1;
        for i in 1..55 {
            let ii = (21 * i) % 55;
            a[ii] = mk;
            mk = mj - mk;
            if mk < 0 {
                mk += MBIG;
            }
            mj = a[ii];
        }
        for _ in 1..5 {
            for i in 1..56 {
                a[i] = a[i].wrapping_sub(a[1 + (i + 30) % 55]);
                if a[i] < 0 {
                    a[i] += MBIG;
                }
            }
        }
        NetRandom { seed_array: a, inext: 0, inextp: 21 }
    }

    fn internal_sample(&mut self) -> i32 {
        self.inext += 1;
        if self.inext >= 56 {
            self.inext = 1;
        }
        self.inextp += 1;
        if self.inextp >= 56 {
            self.inextp = 1;
        }
        let mut r = self.seed_array[self.inext].wrapping_sub(self.seed_array[self.inextp]);
        if r == i32::MAX {
            r -= 1;
        }
        if r < 0 {
            r += i32::MAX;
        }
        self.seed_array[self.inext] = r;
        r
    }

    /// `NextDouble()`.
    pub fn sample(&mut self) -> f64 {
        self.internal_sample() as f64 * (1.0 / i32::MAX as f64)
    }

    /// `Next(min, max)` for a range that fits an int.
    pub fn next_range(&mut self, min: i32, max: i32) -> i32 {
        (self.sample() * (max - min) as f64) as i32 + min
    }
}

/// `GenerateCityName(state, coordinates, tribe, skin)` with the default length. `language` is the tribe's comma-separated
/// syllable list; `village_name_seed` is `GameState.VillageNameSeed`. Returns `None` for the (non-deterministic) cases the
/// game solves with an unseeded `Random`.
pub fn city_name(village_name_seed: i32, x: i32, y: i32, language: &str, capitalize: bool) -> String {
    let seed = (xxh32_words(village_name_seed as u32, &[x as u32, y as u32]) % 0x7fff_ffff) as i32;
    let mut rng = NetRandom::new(seed);
    let syllables: Vec<&str> = language.split(',').collect();
    // length = Value(rng) * 3 + 3 (in floats)
    let length = (rng.sample() as f32) * 3.0f32 + 3.0f32;
    let mut word = String::new();
    for _ in 0..100 {
        if length <= word.chars().count() as f32 {
            break;
        }
        let next = format!("{word}{}", syllables[rng.next_range(0, syllables.len() as i32) as usize]);
        // A word may not start with a space or a hyphen.
        if next.starts_with(' ') || next.starts_with('-') {
            word.clear();
            continue;
        }
        word = next;
    }
    if !capitalize {
        return word;
    }
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => word,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Rules;

    #[test]
    fn names_match_the_capitals_of_the_fixtures() {
        let rules = Rules::bundled();
        // (village name seed, x, y, tribe idx, name) from start_game_1.state
        for (x, y, tribe, name) in [(6, 2, 4, "Flarø"), (13, 6, 10, "Silim"), (10, 11, 8, "Anlu"), (2, 12, 2, "Po dee"), (15, 13, 13, "Szubu")] {
            let language = &rules.tribe_by_game_idx(tribe).unwrap().language;
            assert_eq!(city_name(635_881_111, x, y, language, tribe != 5), name);
        }
    }
}
