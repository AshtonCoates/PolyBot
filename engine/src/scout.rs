//! The explorer city reward: `ScoutMoveAction` and the `FastPath` fog search that picks each step
//! (`GetNextTargetDefault`, `PopulateLookupDefault`, `PopulateFogDistanceMapDefault`,
//! `ApplyBacktrackingPenaltyDefault`, `GetScoreFromSurroundingFogTiles`), plus the game's `XXHash`.
//! Read from the 2.17.3 binary; see docs/rules/explorer.md.

use crate::engine::EngineError;
use crate::rules::Rules;
use crate::state::{Coords, GameState};

const P1: u32 = 0x9E37_79B1;
const P2: u32 = 0x85EB_CA77;
const P3: u32 = 0xC2B2_AE3D;
const P4: u32 = 0x27D4_EB2F;
const P5: u32 = 0x1656_67B1;

/// `XXHash.GetHash(uint[])`: XXH32 over little-endian 32-bit words with the game's seed.
pub fn xxh32_words(seed: u32, words: &[u32]) -> u32 {
    let len = words.len();
    let mut i = 0;
    let mut h;
    if len >= 4 {
        let mut v = [seed.wrapping_add(P1).wrapping_add(P2), seed.wrapping_add(P2), seed, seed.wrapping_sub(P1)];
        while i + 4 <= len {
            for (k, acc) in v.iter_mut().enumerate() {
                *acc = acc.wrapping_add(words[i + k].wrapping_mul(P2)).rotate_left(13).wrapping_mul(P1);
            }
            i += 4;
        }
        h = v[0].rotate_left(1).wrapping_add(v[1].rotate_left(7)).wrapping_add(v[2].rotate_left(12)).wrapping_add(v[3].rotate_left(18));
    } else {
        h = seed.wrapping_add(P5);
    }
    h = h.wrapping_add((len as u32).wrapping_mul(4));
    while i < len {
        h = h.wrapping_add(words[i].wrapping_mul(P3)).rotate_left(17).wrapping_mul(P4);
        i += 1;
    }
    h ^= h >> 15;
    h = h.wrapping_mul(P2);
    h ^= h >> 13;
    h = h.wrapping_mul(P3);
    h ^ (h >> 16)
}

/// `GameState.RandomHash.GetHash(x, y)`.
pub fn hash_xy(seed: i32, x: i32, y: i32) -> u32 {
    xxh32_words(seed as u32, &[x as u32, y as u32])
}

const RADIUS: i32 = 5;
const SIZE: i32 = 2 * RADIUS + 1;
const LEN: usize = (SIZE * SIZE) as usize;

const VALID: i32 = 1;
const CAN_MOVE_OVER: i32 = 2;
const CAN_END_MOVE_ON: i32 = 4;
const FOG: i32 = 8;
const LIGHTHOUSE: i32 = 32;
const MOUNTAIN: i32 = 64;
const READ_ONLY: i32 = 128;

const IMPROVEMENT_CITY: u16 = 1;
const IMPROVEMENT_LIGHTHOUSE: u16 = 47;
const TERRAIN_MOUNTAIN: u16 = 4;

/// `FastPath.PopulateLookupDefault`: a flag word for each tile of the 11x11 window around `from` (x fastest).
fn populate_lookup(state: &GameState, rules: &Rules, player: u8, from: Coords, allowed: &[u16]) -> Result<[i32; LEN], EngineError> {
    let mut lookup = [0i32; LEN];
    for row in 0..SIZE {
        for col in 0..SIZE {
            let c = Coords { x: from.x - RADIUS + col, y: from.y - RADIUS + row };
            let Some(tile) = state.map.tile(c) else { continue };
            let explored = tile.explorers.contains(&player);
            let has_city = tile.improvement.as_ref().map(|i| i.kind == IMPROVEMENT_CITY).unwrap_or(false);
            let lighthouse = tile.improvement.as_ref().map(|i| i.kind == IMPROVEMENT_LIGHTHOUSE).unwrap_or(false);
            let bridge = tile
                .improvement
                .as_ref()
                .and_then(|i| rules.improvement_by_game_idx(i.kind as u32))
                .map(|d| d.abilities.contains(crate::rules::ImprovementAbility::Bridge))
                .unwrap_or(false);
            if bridge {
                return Err(EngineError::Unimplemented { command: "explorer search across a bridge (CanMoveFromTo)" });
            }
            let movable = has_city || allowed.contains(&tile.terrain);
            let mut flags = VALID;
            flags |= (!explored as i32) * FOG;
            flags |= if movable { CAN_MOVE_OVER | CAN_END_MOVE_ON } else { 0 };
            flags |= lighthouse as i32 * LIGHTHOUSE;
            flags |= (tile.terrain == TERRAIN_MOUNTAIN && explored) as i32 * MOUNTAIN;
            lookup[(row * SIZE + col) as usize] = flags;
        }
    }
    Ok(lookup)
}

/// `FastPath.GetScoreFromSurroundingFogTiles`: `-10 * min(fog tiles in the window, 4)`, plus 73, or 40 when a fogged lighthouse
/// is in the window; 9999 for an empty radius-2 window.
fn fog_score(lookup: &[i32; LEN], index: i32, radius: i32) -> i32 {
    let (x, y) = (index % SIZE, index / SIZE);
    let (x0, x1) = ((x - radius).max(0), (x + radius).min(SIZE - 1));
    let (y0, y1) = ((y - radius).max(0), (y + radius).min(SIZE - 1));
    let (mut fog, mut lighthouse) = (0, false);
    for yy in y0..=y1 {
        for xx in x0..=x1 {
            let f = lookup[(yy * SIZE + xx) as usize];
            fog += (f & FOG != 0) as i32;
            lighthouse |= f & (LIGHTHOUSE | FOG) == (LIGHTHOUSE | FOG);
        }
    }
    let fog = fog.min(4);
    if radius == 2 && fog == 0 && !lighthouse {
        return 9999;
    }
    -10 * fog + if lighthouse { 40 } else { 73 }
}

/// `FastPath.PopulateFogDistanceMapDefault` (checkMountains on, as for game version 123): distance in steps of 100 to the
/// nearest fogged tile, through tiles the player can move over.
fn fog_distance_map(lookup: &mut [i32; LEN]) -> [i32; LEN] {
    let mut dist = [0i32; LEN];
    for i in 0..LEN {
        dist[i] = if lookup[i] & CAN_MOVE_OVER != 0 { 9999 } else { 1000 };
    }
    let mut queue: Vec<usize> = Vec::new();
    for i in 0..LEN {
        if lookup[i] & FOG != 0 {
            dist[i] = 0;
            queue.push(i);
        }
    }
    // Explored, climbable mountains next to fog get a starting distance of their own and are then read-only.
    for i in 0..LEN {
        if lookup[i] & (MOUNTAIN | FOG | CAN_MOVE_OVER) == (MOUNTAIN | CAN_MOVE_OVER) {
            let score = fog_score(lookup, i as i32, 2);
            if score != 9999 {
                dist[i] = score + 100;
                queue.push(i);
                lookup[i] |= READ_ONLY;
            }
        }
    }
    let mut k = 0;
    while k < queue.len() {
        let p = queue[k] as i32;
        let (x, y) = (p % SIZE, p / SIZE);
        let step = dist[p as usize] + 100;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= SIZE || ny >= SIZE {
                    continue;
                }
                let n = (ny * SIZE + nx) as usize;
                let flags = lookup[n];
                if dist[n] <= step || flags & READ_ONLY != 0 || flags & (VALID | CAN_MOVE_OVER) != (VALID | CAN_MOVE_OVER) {
                    continue;
                }
                let mut s = step;
                if s == 100 && flags & FOG == 0 {
                    s = 100 + fog_score(lookup, n as i32, 1);
                }
                dist[n] = s;
                queue.push(n);
            }
        }
        // The game stops after queue entry 498.
        if k == 0x1f2 {
            break;
        }
        k += 1;
    }
    dist
}

/// `FastPath.ApplyBacktrackingPenaltyDefault`: tiles already visited by this explorer cost more, growing with how long ago.
fn backtracking_penalty(dist: &mut [i32; LEN], from: Coords, path: &[Coords]) {
    let (ox, oy) = (from.x - RADIUS, from.y - RADIUS);
    let mut penalty = 1;
    let mut slow = 0;
    for c in path {
        if c.x >= ox && c.x < ox + SIZE && c.y >= oy && c.y < oy + SIZE {
            dist[((c.y - oy) * SIZE + (c.x - ox)) as usize] += penalty;
        }
        if path.len() < 9 {
            penalty += 1;
        } else {
            slow += 1;
            penalty += (!slow) & 1;
        }
    }
}

/// `ScoutMoveAction.GetNextTargetDefault`: the neighbouring tile with the smallest fog distance (ties broken by
/// `HashFunction.Range(0, n, seed)`), or `None` when nothing is reachable.
pub fn next_target(state: &GameState, rules: &Rules, player: u8, from: Coords, path: &[Coords], seed: u32) -> Result<Option<Coords>, EngineError> {
    let p = state.players.iter().find(|p| p.id == player).ok_or_else(|| EngineError::Inconsistent { what: format!("player {player}") })?;
    let mut allowed = Vec::new();
    for tech in &p.available_tech {
        if let Some(t) = rules.tech_by_game_idx(*tech as u32) {
            allowed.extend(t.movement_unlocks.iter().map(|(terrain, _)| *terrain as u16));
        }
    }
    let mut lookup = populate_lookup(state, rules, player, from, &allowed)?;
    let mut dist = fog_distance_map(&mut lookup);
    backtracking_penalty(&mut dist, from, path);
    const DIRS: [(i32, i32); 8] = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)];
    let centre = RADIUS * SIZE + RADIUS;
    let mut best = i32::MAX;
    let mut candidates: Vec<usize> = Vec::new();
    for (i, (dx, dy)) in DIRS.iter().enumerate() {
        let v = dist[(centre + dx + SIZE * dy) as usize];
        if v < best {
            best = v;
            candidates.clear();
            candidates.push(i);
        } else if v == best {
            candidates.push(i);
        }
    }
    if best > 9998 || candidates.is_empty() {
        return Ok(None);
    }
    let pick = if candidates.len() == 1 {
        0
    } else {
        // HashFunction.Range(0, count, seed) = GetHash(seed) % count.
        (xxh32_words(state.seed as u32, &[seed]) % candidates.len() as u32) as usize
    };
    let (dx, dy) = DIRS[candidates[pick]];
    Ok(Some(Coords { x: from.x + dx, y: from.y + dy }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xxh32_matches_the_reference_implementation() {
        // XXH32 of the 4 bytes of one little-endian word == XXHash.GetHash(int), computed independently here.
        let h = |seed: u32, v: u32| {
            let mut h = seed.wrapping_add(P5).wrapping_add(4);
            h = h.wrapping_add(v.wrapping_mul(P3)).rotate_left(17).wrapping_mul(P4);
            h ^= h >> 15;
            h = h.wrapping_mul(P2);
            h ^= h >> 13;
            h = h.wrapping_mul(P3);
            h ^ (h >> 16)
        };
        for (seed, v) in [(0u32, 0u32), (7, 12345), (123456, 0xFFFF_FFFF)] {
            assert_eq!(xxh32_words(seed, &[v]), h(seed, v));
        }
        // Known XXH32 vector: empty input with seed 0 is 0x02CC5D05.
        assert_eq!(xxh32_words(0, &[]), 0x02CC_5D05);
    }

    #[test]
    fn backtracking_penalty_grows_along_the_path() {
        let mut dist = [0i32; LEN];
        let from = Coords { x: 10, y: 10 };
        backtracking_penalty(&mut dist, from, &[Coords { x: 10, y: 10 }, Coords { x: 11, y: 10 }, Coords { x: 12, y: 10 }]);
        let at = |x: i32| dist[(RADIUS * SIZE + RADIUS + (x - 10)) as usize];
        assert_eq!((at(10), at(11), at(12)), (1, 2, 3));
    }
}
