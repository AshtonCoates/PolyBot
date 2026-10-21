//! Record layouts for game version 122 (older versions are rejected where the layout differs and has
//! not been decoded). Each `read_*` has a mirror `write_*`; the round-trip test checks they agree.

use super::io::{Reader, Writer};
use super::LoadError;
use crate::state::*;

type R<T> = Result<T, LoadError>;

fn unsupported<T>(msg: String) -> R<T> {
    Err(LoadError::Unsupported(msg))
}

fn read_list<T>(r: &mut Reader, mut f: impl FnMut(&mut Reader) -> R<T>) -> R<Vec<T>> {
    let n = r.u16()?;
    (0..n).map(|_| f(r)).collect()
}

fn write_list<T>(w: &mut Writer, items: &[T], mut f: impl FnMut(&mut Writer, &T)) {
    w.u16(items.len() as u16);
    for item in items {
        f(w, item);
    }
}

// ---- GameSettings -------------------------------------------------------------------------

fn read_rules(r: &mut Reader) -> R<GameRules> {
    Ok(GameRules {
        turn_limit: r.i32()?,
        score_limit: r.i32()?,
        win_by_capital: r.bool()?,
        allow_mirror_pick: r.bool()?,
        allow_special_tribes: r.bool()?,
        win_by_extermination: r.bool()?,
        allow_tech_sharing: r.bool()?,
        player_death_condition: r.u16()?,
    })
}

fn write_rules(w: &mut Writer, g: &GameRules) {
    w.i32(g.turn_limit);
    w.i32(g.score_limit);
    w.bool(g.win_by_capital);
    w.bool(g.allow_mirror_pick);
    w.bool(g.allow_special_tribes);
    w.bool(g.win_by_extermination);
    w.bool(g.allow_tech_sharing);
    w.u16(g.player_death_condition);
}

fn read_settings(r: &mut Reader, version: i32) -> R<GameSettings> {
    if version < 70 {
        return unsupported(format!("GameSettings layout before version 70 (got {version})"));
    }
    let rules = read_rules(r)?;
    let base_game_mode = r.u8()?;
    let rules_game_mode = r.u8()?;
    let game_name = r.string()?;
    let map_size = r.i32()?;
    let disabled_tribes = read_list(r, |r| r.u16())?;
    let unlocked_tribes = read_list(r, |r| r.u16())?;
    let difficulty = r.u16()?;
    let opponent_count = r.i32()?;
    let game_type = r.u16()?;
    let map_preset = r.u8()?;
    let time_limit = r.i32()?;
    let time_bonus_per_city = r.f32()?;
    let time_bonus_per_population = r.f32()?;
    let base_time_seconds = r.f32()?;
    let use_dynamic_timers = r.bool()?;
    let use_time_banks = r.bool()?;
    let live_game_preset = r.bool()?;
    let is_auto_skip_enabled = r.bool()?;
    let mut selected_skins = Vec::new();
    if version > 85 {
        for _ in 0..r.i32()? {
            let tribe = r.u16()?;
            let skin = r.u16()?;
            selected_skins.push((tribe, skin));
        }
    }
    let west_map_placement_user_id = if version > 121 { r.string()? } else { String::new() };
    Ok(GameSettings {
        rules,
        base_game_mode,
        rules_game_mode,
        game_name,
        map_size,
        disabled_tribes,
        unlocked_tribes,
        difficulty,
        opponent_count,
        game_type,
        map_preset,
        time_limit,
        time_bonus_per_city,
        time_bonus_per_population,
        base_time_seconds,
        use_dynamic_timers,
        use_time_banks,
        live_game_preset,
        is_auto_skip_enabled,
        selected_skins,
        west_map_placement_user_id,
    })
}

fn write_settings(w: &mut Writer, s: &GameSettings, version: i32) {
    write_rules(w, &s.rules);
    w.u8(s.base_game_mode);
    w.u8(s.rules_game_mode);
    w.string(&s.game_name);
    w.i32(s.map_size);
    write_list(w, &s.disabled_tribes, |w, v| w.u16(*v));
    write_list(w, &s.unlocked_tribes, |w, v| w.u16(*v));
    w.u16(s.difficulty);
    w.i32(s.opponent_count);
    w.u16(s.game_type);
    w.u8(s.map_preset);
    w.i32(s.time_limit);
    w.f32(s.time_bonus_per_city);
    w.f32(s.time_bonus_per_population);
    w.f32(s.base_time_seconds);
    w.bool(s.use_dynamic_timers);
    w.bool(s.use_time_banks);
    w.bool(s.live_game_preset);
    w.bool(s.is_auto_skip_enabled);
    if version > 85 {
        w.i32(s.selected_skins.len() as i32);
        for (tribe, skin) in &s.selected_skins {
            w.u16(*tribe);
            w.u16(*skin);
        }
    }
    if version > 121 {
        w.string(&s.west_map_placement_user_id);
    }
}

// ---- Map ----------------------------------------------------------------------------------

fn read_improvement(r: &mut Reader) -> R<ImprovementState> {
    let kind = r.u16()?;
    let level = r.u16()?;
    let founded = r.u16()?;
    let xp = r.i16()?;
    let population = r.i16()?;
    let production = r.u16()?;
    let base_score = r.u16()?;
    let border_size = r.u16()?;
    let upgrade = r.u16()?;
    let connected_to_capital_of_player = r.u8()?;
    let name = if r.bool()? { r.string()? } else { String::new() };
    let founder = r.u8()?;
    let rewards = read_list(r, |r| r.u16())?;
    let effects = read_list(r, |r| r.u16())?;
    Ok(ImprovementState {
        kind,
        level,
        founded,
        xp,
        population,
        production,
        base_score,
        border_size,
        upgrade,
        connected_to_capital_of_player,
        name,
        founder,
        rewards,
        effects,
    })
}

fn write_improvement(w: &mut Writer, i: &ImprovementState) {
    w.u16(i.kind);
    w.u16(i.level);
    w.u16(i.founded);
    w.i16(i.xp);
    w.i16(i.population);
    w.u16(i.production);
    w.u16(i.base_score);
    w.u16(i.border_size);
    w.u16(i.upgrade);
    w.u8(i.connected_to_capital_of_player);
    w.bool(!i.name.is_empty());
    if !i.name.is_empty() {
        w.string(&i.name);
    }
    w.u8(i.founder);
    write_list(w, &i.rewards, |w, v| w.u16(*v));
    write_list(w, &i.effects, |w, v| w.u16(*v));
}

fn read_unit(r: &mut Reader, version: i32) -> R<UnitState> {
    let id = r.u32()?;
    let owner = r.u8()?;
    let kind = r.u16()?;
    let follower = r.u32()?;
    let leader = r.u32()?;
    let coordinates = r.coords()?;
    let home = r.coords()?;
    let health = r.u16()?;
    let promotion_level = r.u16()?;
    let xp = r.u16()?;
    let moved = r.bool()?;
    let attacked = r.bool()?;
    let flipped = r.bool()?;
    let created_turn = r.u16()?;
    let passenger = if r.bool()? { Some(Box::new(read_unit(r, version)?)) } else { None };
    let effects = read_list(r, |r| r.u16())?;
    let birth_climate = r.i16()?;
    let direction = r.u8()?;
    let birth_climate_skin = if version >= 87 { r.u16()? } else { 0 };
    Ok(UnitState {
        id,
        owner,
        kind,
        follower,
        leader,
        coordinates,
        home,
        health,
        promotion_level,
        xp,
        moved,
        attacked,
        flipped,
        created_turn,
        passenger,
        effects,
        birth_climate,
        direction,
        birth_climate_skin,
    })
}

fn write_unit(w: &mut Writer, u: &UnitState, version: i32) {
    w.u32(u.id);
    w.u8(u.owner);
    w.u16(u.kind);
    w.u32(u.follower);
    w.u32(u.leader);
    w.coords(u.coordinates);
    w.coords(u.home);
    w.u16(u.health);
    w.u16(u.promotion_level);
    w.u16(u.xp);
    w.bool(u.moved);
    w.bool(u.attacked);
    w.bool(u.flipped);
    w.u16(u.created_turn);
    w.bool(u.passenger.is_some());
    if let Some(p) = &u.passenger {
        write_unit(w, p, version);
    }
    write_list(w, &u.effects, |w, v| w.u16(*v));
    w.i16(u.birth_climate);
    w.u8(u.direction);
    if version >= 87 {
        w.u16(u.birth_climate_skin);
    }
}

fn read_tile(r: &mut Reader, version: i32) -> R<TileData> {
    let coordinates = r.coords()?;
    let terrain = r.u16()?;
    let climate = r.i16()?;
    let altitude = r.i16()?;
    let owner = r.u8()?;
    let capital_of = r.u8()?;
    let ruling_city = r.coords()?;
    let resource = if r.bool()? { Some(r.u16()?) } else { None };
    let improvement = if r.bool()? { Some(read_improvement(r)?) } else { None };
    let unit = if r.bool()? { Some(read_unit(r, version)?) } else { None };
    let explorers = {
        let n = r.u8()?;
        (0..n).map(|_| r.u8()).collect::<R<Vec<u8>>>()?
    };
    let has_road = r.bool()?;
    let has_route = r.bool()?;
    let skin = if version >= 86 { r.i32()? } else { 0 };
    let effects = if version >= 105 {
        let n = r.u8()?;
        (0..n).map(|_| r.i32()).collect::<R<Vec<i32>>>()?
    } else {
        Vec::new()
    };
    Ok(TileData {
        coordinates,
        terrain,
        climate,
        altitude,
        owner,
        capital_of,
        ruling_city,
        resource,
        improvement,
        unit,
        explorers,
        has_road,
        has_route,
        skin,
        effects,
    })
}

fn write_tile(w: &mut Writer, t: &TileData, version: i32) {
    w.coords(t.coordinates);
    w.u16(t.terrain);
    w.i16(t.climate);
    w.i16(t.altitude);
    w.u8(t.owner);
    w.u8(t.capital_of);
    w.coords(t.ruling_city);
    w.bool(t.resource.is_some());
    if let Some(res) = t.resource {
        w.u16(res);
    }
    w.bool(t.improvement.is_some());
    if let Some(i) = &t.improvement {
        write_improvement(w, i);
    }
    w.bool(t.unit.is_some());
    if let Some(u) = &t.unit {
        write_unit(w, u, version);
    }
    w.u8(t.explorers.len() as u8);
    for e in &t.explorers {
        w.u8(*e);
    }
    w.bool(t.has_road);
    w.bool(t.has_route);
    if version >= 86 {
        w.i32(t.skin);
    }
    if version >= 105 {
        w.u8(t.effects.len() as u8);
        for e in &t.effects {
            w.i32(*e);
        }
    }
}

fn read_map(r: &mut Reader, version: i32) -> R<MapData> {
    let width = r.u16()?;
    let height = r.u16()?;
    let tiles = (0..width as usize * height as usize)
        .map(|_| read_tile(r, version))
        .collect::<R<Vec<_>>>()?;
    Ok(MapData { width, height, tiles })
}

fn write_map(w: &mut Writer, m: &MapData, version: i32) {
    w.u16(m.width);
    w.u16(m.height);
    for t in &m.tiles {
        write_tile(w, t, version);
    }
}

// ---- Players ------------------------------------------------------------------------------

fn read_task(r: &mut Reader) -> R<Task> {
    let kind = r.u16()?;
    let is_started = r.bool()?;
    let is_completed = r.bool()?;
    let counter = if crate::state::task_has_counter(kind) { Some(r.i32()?) } else { None };
    Ok(Task { kind, is_started, is_completed, counter })
}

fn write_task(w: &mut Writer, t: &Task) {
    w.u16(t.kind);
    w.bool(t.is_started);
    w.bool(t.is_completed);
    if let Some(c) = t.counter {
        w.i32(c);
    }
}

fn read_relation(r: &mut Reader, version: i32) -> R<DiplomacyRelation> {
    let state = r.u8()?;
    let last_attack_turn = r.i32()?;
    let embassy_level = r.u8()?;
    let last_peace_broken_turn = r.i32()?;
    let first_meet = r.i32()?;
    let embassy_build_turn = r.i32()?;
    let previous_attack_turn = if version > 80 { r.i32()? } else { 0 };
    Ok(DiplomacyRelation {
        state,
        last_attack_turn,
        embassy_level,
        last_peace_broken_turn,
        first_meet,
        embassy_build_turn,
        previous_attack_turn,
    })
}

fn write_relation(w: &mut Writer, d: &DiplomacyRelation, version: i32) {
    w.u8(d.state);
    w.i32(d.last_attack_turn);
    w.u8(d.embassy_level);
    w.i32(d.last_peace_broken_turn);
    w.i32(d.first_meet);
    w.i32(d.embassy_build_turn);
    if version > 80 {
        w.i32(d.previous_attack_turn);
    }
}

fn read_player(r: &mut Reader, version: i32) -> R<PlayerState> {
    if version < 113 {
        return unsupported(format!("PlayerState layout before version 113 (got {version})"));
    }
    let id = r.u8()?;
    let user_name = r.string()?;
    let account_id = r.string()?;
    let auto_play = r.bool()?;
    let start_tile = r.coords()?;
    let tribe = r.u16()?;
    let has_chosen_tribe = r.bool()?;
    let handicap = r.i32()?;
    let currency = r.i32()?;
    let score = r.u32()?;
    let end_score = r.u32()?;
    let cities = r.u16()?;
    let available_tech = read_list(r, |r| r.u16())?;
    let known_players = read_list(r, |r| r.u8())?;
    let tasks = read_list(r, read_task)?;
    let kills = r.u32()?;
    let casualities = r.u32()?;
    let wipe_outs = r.u32()?;
    let color = r.i32()?;
    let tribe_mix = r.u8()?;
    let built_unique_improvements = read_list(r, |r| r.i16())?;
    let mut relations = Vec::new();
    let mut messages = Vec::new();
    if version >= 60 {
        relations = read_list(r, |r| {
            let key = r.u8()?;
            Ok((key, read_relation(r, version)?))
        })?;
        messages = read_list(r, |r| {
            let kind = r.u8()?;
            let sender = r.u8()?;
            Ok(DiplomacyMessage { kind, sender })
        })?;
    }
    let killer_id = r.u8()?;
    let killed_turn = r.u32()?;
    let (mut resigned_at_command_index, mut wiped_at_command_index) = (0, 0);
    if version >= 70 {
        resigned_at_command_index = r.i32()?;
        wiped_at_command_index = r.i32()?;
    }
    let skin = if version >= 86 { r.u16()? } else { 0xffff };
    let resigned_turn = if version >= 93 { r.i32()? } else { 0 };
    let climate = if version >= 121 { r.u8()? } else { tribe as u8 };
    Ok(PlayerState {
        id,
        user_name,
        account_id,
        auto_play,
        start_tile,
        tribe,
        has_chosen_tribe,
        handicap,
        currency,
        score,
        end_score,
        cities,
        available_tech,
        known_players,
        tasks,
        kills,
        casualities,
        wipe_outs,
        color,
        tribe_mix,
        built_unique_improvements,
        relations,
        messages,
        killer_id,
        killed_turn,
        resigned_at_command_index,
        wiped_at_command_index,
        skin,
        resigned_turn,
        climate,
    })
}

fn write_player(w: &mut Writer, p: &PlayerState, version: i32) {
    w.u8(p.id);
    w.string(&p.user_name);
    w.string(&p.account_id);
    w.bool(p.auto_play);
    w.coords(p.start_tile);
    w.u16(p.tribe);
    w.bool(p.has_chosen_tribe);
    w.i32(p.handicap);
    w.i32(p.currency);
    w.u32(p.score);
    w.u32(p.end_score);
    w.u16(p.cities);
    write_list(w, &p.available_tech, |w, v| w.u16(*v));
    write_list(w, &p.known_players, |w, v| w.u8(*v));
    write_list(w, &p.tasks, write_task);
    w.u32(p.kills);
    w.u32(p.casualities);
    w.u32(p.wipe_outs);
    w.i32(p.color);
    w.u8(p.tribe_mix);
    write_list(w, &p.built_unique_improvements, |w, v| w.i16(*v));
    if version >= 60 {
        write_list(w, &p.relations, |w, (key, rel)| {
            w.u8(*key);
            write_relation(w, rel, version);
        });
        write_list(w, &p.messages, |w, m| {
            w.u8(m.kind);
            w.u8(m.sender);
        });
    }
    w.u8(p.killer_id);
    w.u32(p.killed_turn);
    if version >= 70 {
        w.i32(p.resigned_at_command_index);
        w.i32(p.wiped_at_command_index);
    }
    if version >= 86 {
        w.u16(p.skin);
    }
    if version >= 93 {
        w.i32(p.resigned_turn);
    }
    if version >= 121 {
        w.u8(p.climate);
    }
}

fn read_trigger(r: &mut Reader, version: i32) -> R<CommandTrigger> {
    let player_id = r.u8()?;
    let kind = r.u16()?;
    let coordinates = r.coords()?;
    let opponent_id = if version > 59 { r.u8()? } else { 0 };
    Ok(CommandTrigger { player_id, kind, coordinates, opponent_id })
}

fn write_trigger(w: &mut Writer, t: &CommandTrigger, version: i32) {
    w.u8(t.player_id);
    w.u16(t.kind);
    w.coords(t.coordinates);
    if version > 59 {
        w.u8(t.opponent_id);
    }
}

// ---- Commands -----------------------------------------------------------------------------

fn read_command(r: &mut Reader, version: i32) -> R<Command> {
    let start = r.pos;
    let kind = r.u16()?;
    Ok(match kind {
        1 => {
            let player = r.u8()?;
            let kind = r.u16()?;
            let at = r.coords()?;
            Command::Build { player, kind, at }
        }
        2 => {
            let player = r.u8()?;
            let unit_id = r.u32()?;
            let origin = r.coords()?;
            let target = r.coords()?;
            Command::Attack { player, unit_id, origin, target }
        }
        3 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::Recover { player, at }
        }
        4 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::HealOthers { player, at }
        }
        5 => {
            let player = r.u8()?;
            let kind = r.u16()?;
            let at = r.coords()?;
            Command::Train { player, kind, at }
        }
        6 => {
            let player = r.u8()?;
            let from = r.coords()?;
            let to = r.coords()?;
            let unit_id = r.u32()?;
            Command::Move { player, from, to, unit_id }
        }
        7 => {
            let player = r.u8()?;
            let unit_id = r.u32()?;
            let at = r.coords()?;
            Command::Capture { player, unit_id, at }
        }
        8 => {
            let player = r.u8()?;
            let tech = r.u16()?;
            Command::Research { player, tech }
        }
        9 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::Destroy { player, at }
        }
        10 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::Disband { player, at }
        }
        11 => {
            let player = r.u8()?;
            let at = r.coords()?;
            let reward = r.u16()?;
            Command::CityReward { player, at, reward }
        }
        13 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::Promote { player, at }
        }
        14 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::ExamineRuins { player, at }
        }
        15 => {
            let player = r.u8()?;
            Command::EndTurn { player }
        }
        16 => {
            let player = r.u8()?;
            let kind = r.u16()?;
            let at = r.coords()?;
            Command::Upgrade { player, kind, at }
        }
        17 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::FreezeArea { player, at }
        }
        18 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::BreakIce { player, at }
        }
        20 => {
            let player = r.u8()?;
            Command::StartMatch { player }
        }
        21 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::Stay { player, at }
        }
        22 => {
            let player = r.u8()?;
            Command::EndMatch { player }
        }
        24 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::Explode { player, at }
        }
        25 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::Boost { player, at }
        }
        26 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::Decompose { player, at }
        }
        27 => {
            let player = r.u8()?;
            let at = r.coords()?;
            let opponent = r.u8()?;
            Command::PeaceTreaty { player, at, opponent }
        }
        28 => {
            let player = r.u8()?;
            let opponent = r.u8()?;
            let accepted = r.bool()?;
            Command::PeaceRequestResponse { player, opponent, accepted }
        }
        29 => {
            let player = r.u8()?;
            let at = r.coords()?;
            let opponent = r.u8()?;
            Command::BreakPeace { player, at, opponent }
        }
        30 => {
            let player = r.u8()?;
            let at = r.coords()?;
            let opponent = r.u8()?;
            Command::EstablishEmbassy { player, at, opponent }
        }
        32 => {
            let player = r.u8()?;
            let at = r.coords()?;
            let opponent = r.u8()?;
            Command::DestroyEmbassy { player, at, opponent }
        }
        33 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::Hide { player, at }
        }
        35 => {
            let player = r.u8()?;
            let resigned = r.u8()?;
            let kicker = if version > 92 { r.u8()? } else { 0 };
            let was_kicked = if version > 92 { r.bool()? } else { false };
            Command::Resign { player, resigned, kicker, was_kicked }
        }
        36 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::Disembark { player, at }
        }
        37 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::Flood { player, at }
        }
        38 => {
            let player = r.u8()?;
            let at = r.coords()?;
            Command::Swarm { player, at }
        }
        39 => {
            let player = r.u8()?;
            let effect = r.u16()?;
            let at = r.coords()?;
            Command::ClearTileEffect { player, effect, at }
        }
        other => return unsupported(format!("command type {other} at offset {start} is not decoded yet")),
    })
}

fn write_command(w: &mut Writer, c: &Command, version: i32) {
    w.u16(c.type_id());
    match c {
        Command::Build { player, kind, at } => {
            w.u8(*player);
            w.u16(*kind);
            w.coords(*at);
        }
        Command::Attack { player, unit_id, origin, target } => {
            w.u8(*player);
            w.u32(*unit_id);
            w.coords(*origin);
            w.coords(*target);
        }
        Command::Recover { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::HealOthers { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::Train { player, kind, at } => {
            w.u8(*player);
            w.u16(*kind);
            w.coords(*at);
        }
        Command::Move { player, from, to, unit_id } => {
            w.u8(*player);
            w.coords(*from);
            w.coords(*to);
            w.u32(*unit_id);
        }
        Command::Capture { player, unit_id, at } => {
            w.u8(*player);
            w.u32(*unit_id);
            w.coords(*at);
        }
        Command::Research { player, tech } => {
            w.u8(*player);
            w.u16(*tech);
        }
        Command::Destroy { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::Disband { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::CityReward { player, at, reward } => {
            w.u8(*player);
            w.coords(*at);
            w.u16(*reward);
        }
        Command::Promote { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::ExamineRuins { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::EndTurn { player } => {
            w.u8(*player);
        }
        Command::Upgrade { player, kind, at } => {
            w.u8(*player);
            w.u16(*kind);
            w.coords(*at);
        }
        Command::FreezeArea { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::BreakIce { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::StartMatch { player } => {
            w.u8(*player);
        }
        Command::Stay { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::EndMatch { player } => {
            w.u8(*player);
        }
        Command::Explode { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::Boost { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::Decompose { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::PeaceTreaty { player, at, opponent } => {
            w.u8(*player);
            w.coords(*at);
            w.u8(*opponent);
        }
        Command::PeaceRequestResponse { player, opponent, accepted } => {
            w.u8(*player);
            w.u8(*opponent);
            w.bool(*accepted);
        }
        Command::BreakPeace { player, at, opponent } => {
            w.u8(*player);
            w.coords(*at);
            w.u8(*opponent);
        }
        Command::EstablishEmbassy { player, at, opponent } => {
            w.u8(*player);
            w.coords(*at);
            w.u8(*opponent);
        }
        Command::DestroyEmbassy { player, at, opponent } => {
            w.u8(*player);
            w.coords(*at);
            w.u8(*opponent);
        }
        Command::Hide { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::Resign { player, resigned, kicker, was_kicked } => {
            w.u8(*player);
            w.u8(*resigned);
            if version > 92 { w.u8(*kicker); }
            if version > 92 { w.bool(*was_kicked); }
        }
        Command::Disembark { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::Flood { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::Swarm { player, at } => {
            w.u8(*player);
            w.coords(*at);
        }
        Command::ClearTileEffect { player, effect, at } => {
            w.u8(*player);
            w.u16(*effect);
            w.coords(*at);
        }
    }
}

// ---- GameState / LocalGameData ------------------------------------------------------------

fn read_game_state(r: &mut Reader) -> R<GameState> {
    let version = r.i32()?;
    let current_command = r.u16()?;
    let current_turn = r.u32()?;
    let current_player_index = r.u8()?;
    let current_unit_id = r.u32()?;
    let current_state = r.u8()?;
    let seed = r.i32()?;
    let settings = read_settings(r, version)?;
    let map = read_map(r, version)?;
    let players = read_list(r, |r| read_player(r, version))?;
    let triggers = read_list(r, |r| read_trigger(r, version))?;
    let commands = read_list(r, |r| read_command(r, version))?;
    let action_count = r.u16()?;
    if action_count != 0 {
        return unsupported(format!("{action_count} actions in the action stack (action records not decoded)"));
    }
    let has_flagged_need_for_update_routes = if version > 97 { r.bool()? } else { false };
    let village_name_seed = if version > 121 { r.i32()? } else { 0 };
    Ok(GameState {
        version,
        current_command,
        current_turn,
        current_player_index,
        current_unit_id,
        current_state,
        seed,
        settings,
        map,
        players,
        triggers,
        commands,
        has_flagged_need_for_update_routes,
        village_name_seed,
    })
}

fn write_game_state(w: &mut Writer, g: &GameState) {
    let version = g.version;
    w.i32(version);
    w.u16(g.current_command);
    w.u32(g.current_turn);
    w.u8(g.current_player_index);
    w.u32(g.current_unit_id);
    w.u8(g.current_state);
    w.i32(g.seed);
    write_settings(w, &g.settings, version);
    write_map(w, &g.map, version);
    write_list(w, &g.players, |w, p| write_player(w, p, version));
    write_list(w, &g.triggers, |w, t| write_trigger(w, t, version));
    write_list(w, &g.commands, |w, c| write_command(w, c, version));
    w.u16(0); // action stack
    if version > 97 {
        w.bool(g.has_flagged_need_for_update_routes);
    }
    if version > 121 {
        w.i32(g.village_name_seed);
    }
}

/// Parse the decompressed payload of a `.state` file.
pub fn read_local_game_data(payload: &[u8]) -> R<LocalGameData> {
    let mut r = Reader::new(payload, 0);
    let version = r.i32()?;
    let initial = read_game_state(&mut r)?;
    let current = read_game_state(&mut r)?;
    let last_seen_command = r.u16()?;
    if r.pos != r.len() {
        return Err(LoadError::Trailing { pos: r.pos, len: r.len() });
    }
    Ok(LocalGameData { version, initial, current, last_seen_command })
}

/// Serialize back to the payload layout (not LZ4-compressed). Used to test the reader.
pub fn write_local_game_data(d: &LocalGameData) -> Vec<u8> {
    let mut w = Writer::default();
    w.i32(d.version);
    write_game_state(&mut w, &d.initial);
    write_game_state(&mut w, &d.current);
    w.u16(d.last_seen_command);
    w.buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_command_type_is_reported() {
        // type 99 is not decoded; must be an Unsupported error naming the type, never a mis-parse.
        let bytes = [99u8, 0, 1, 2, 3];
        match read_command(&mut Reader::new(&bytes, 0), 123) {
            Err(LoadError::Unsupported(m)) => assert!(m.contains("99"), "{m}"),
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    #[test]
    fn command_round_trips() {
        let cmds = [
            Command::Move { player: 3, from: Coords { x: 1, y: 2 }, to: Coords { x: 3, y: 4 }, unit_id: 77 },
            Command::Build { player: 1, kind: 15, at: Coords { x: 0, y: 9 } },
            Command::EndTurn { player: 5 },
        ];
        for c in cmds {
            let mut w = Writer::default();
            write_command(&mut w, &c, 123);
            assert_eq!(read_command(&mut Reader::new(&w.buf, 0), 123).unwrap(), c);
        }
    }

    /// For every command type id: whatever the reader accepts, the writer reproduces byte for byte.
    #[test]
    fn every_supported_command_type_round_trips() {
        let mut supported = Vec::new();
        for type_id in 0u16..=64 {
            let mut bytes = type_id.to_le_bytes().to_vec();
            bytes.extend((1u8..=60).map(|b| b.wrapping_mul(7))); // arbitrary payload, 0 and 1 bools included
            let mut r = Reader::new(&bytes, 0);
            let Ok(cmd) = read_command(&mut r, 123) else { continue };
            let used = r.pos;
            assert_eq!(cmd.type_id(), type_id);
            let mut w = Writer::default();
            write_command(&mut w, &cmd, 123);
            // Same length, and reading the output gives the same command (a bool byte like 21 is written back as 1).
            assert_eq!(w.buf.len(), used, "{}", cmd.name());
            assert_eq!(read_command(&mut Reader::new(&w.buf, 0), 123).unwrap(), cmd, "{}", cmd.name());
            supported.push(type_id);
        }
        // The game defines SelectTribe (19) and Harvest (23) as ids but has no classes for them.
        let expected: Vec<u16> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 16, 17, 18, 20, 21, 22, 24, 25, 26, 27, 28, 29, 30, 32, 33, 35, 36, 37, 38, 39];
        assert_eq!(supported, expected);
    }

    #[test]
    fn resign_reads_the_kick_fields_only_from_version_93() {
        let bytes = [35u8, 0, 1, 2, 3, 1]; // type 35, player 1, resigned 2, kicker 3, was_kicked
        let new = read_command(&mut Reader::new(&bytes, 0), 123).unwrap();
        assert_eq!(new, Command::Resign { player: 1, resigned: 2, kicker: 3, was_kicked: true });
        let mut r = Reader::new(&bytes, 0);
        let old = read_command(&mut r, 90).unwrap();
        assert_eq!(old, Command::Resign { player: 1, resigned: 2, kicker: 0, was_kicked: false });
        assert_eq!(r.pos, 4, "old versions stop after the resigned player");
    }

    #[test]
    fn strings_use_dotnet_length_prefix() {
        let mut w = Writer::default();
        w.string("Pond of Piusum");
        assert_eq!(w.buf[0], 14);
        let long = "x".repeat(200); // length needs two 7-bit groups
        let mut w2 = Writer::default();
        w2.string(&long);
        assert_eq!(&w2.buf[..2], &[0xc8, 0x01]);
        assert_eq!(Reader::new(&w2.buf, 0).string().unwrap(), long);
    }
}
