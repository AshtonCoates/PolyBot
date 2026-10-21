//! Actions: the units of work that commands queue and the engine runs (last-in-first-out).
//! Only actions whose rules have been read from the decompiled game are present.

/// Mirrors the game's `ActionBase` subclasses (docs/rules/move-and-endturn.md for the execution model).
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// `StartMatchAction`: every player learns their tribe's starting techs, then the game is "Started".
    StartMatch { player: u8 },
    /// `EnableTaskAction`: give the player the task (started) unless they already have it.
    EnableTask { player: u8, kind: u16 },
    /// `IncreaseScoreAction`: `ActionUtils.AddScore`.
    IncreaseScore { player: u8, amount: i32 },
    /// `IncreaseCurrencyAction`: add stars, then re-check the Wealth task.
    IncreaseCurrency { player: u8, amount: i32 },
    /// `RunDeferredAction`: queued first by every command, so it runs last; handles work that was flagged while the
    /// command ran (currently: route and city-connection updates).
    RunDeferred { player: u8 },
    /// `UpdateRoutesAction`: recompute which tiles carry a route.
    UpdateRoutes,
    /// `UpdateCityConnectionsAction`: recompute which of the player's cities are connected to their capital.
    UpdateCityConnections { players: Vec<u8> },
    /// `EndTurnAction`: end the player's turn and hand over to the next living player.
    EndTurn { player: u8 },
    /// `StartTurnAction`: income, level-ups and diplomacy upkeep for the player whose turn begins.
    StartTurn { player: u8 },
    /// `HealAction`: heal the unit on `at` by `fixed` (0 = the standard amount); cures poison instead if poisoned.
    Heal { player: u8, at: crate::state::Coords, fixed: u16 },
    /// `ReselectAction`: UI only (its `Execute` is empty); kept so the action order matches the game.
    Reselect { player: u8, unit_id: u32 },
    /// `MoveAction`: `path[0]` is the destination and the last element the start (the game stores it reversed).
    Move { player: u8, unit_id: u32, path: Vec<crate::state::Coords> },
    /// `TrainAction`: pay `cost` stars and create a unit of type `kind` on the tile `at`.
    Research { player: u8, tech: u16, cost: i32 },
    Train { player: u8, kind: u16, at: crate::state::Coords, cost: i32 },
    /// `BuildAction.ExecuteDefault`: build improvement `kind` on `at`, paying its cost when `deduct_cost`.
    Build { player: u8, kind: u16, at: crate::state::Coords, deduct_cost: bool },
    /// `DestroyResourceAction`: remove the tile's resource (after re-checking neighbouring improvement levels).
    DestroyResource { player: u8, at: crate::state::Coords },
    /// `BuildRoadAction`: mark the tile as having a road.
    BuildRoad { at: crate::state::Coords },
    /// `IncreasePopulationAction`: add one population to the city on `at` (and score / level-up consequences).
    IncreasePopulation { player: u8, at: crate::state::Coords },
    /// `CityLevelUpAction`: level the city on `at` up if its population allows it.
    CityLevelUp { player: u8, at: crate::state::Coords },
    /// `CityRewardAction`: apply the chosen level-up reward to the city on `at`.
    CityReward { player: u8, reward: u16, at: crate::state::Coords },
    /// `MeetAction`: `player` sees `other` for the first time and is paid a reward in stars. Only valid while they have not met.
    Meet { player: u8, other: u8, at: crate::state::Coords },
    /// `ImprovementLevelUpAction`: raise the improvement's level when its computed level exceeds the stored one.
    ImprovementLevelUp { player: u8, at: crate::state::Coords },
    /// `ImprovementLevelDownAction`: lower the improvement's level by `amount` (reversing its population growth).
    ImprovementLevelDown { player: u8, at: crate::state::Coords, amount: u16 },
    /// `CaptureCityAction`: `player` takes the city on `at` from `old_owner`.
    CaptureCity { player: u8, at: crate::state::Coords, old_owner: u8 },
    /// `DecreaseScoreAction`: `ActionUtils.RemoveScore`.
    DecreaseScore { player: u8, amount: i32 },
    /// `DestroyEmbassyAction`: clears the embassy `owner` has with `player` (the relation of `owner` towards `player`).
    DestroyEmbassy { player: u8, owner: u8 },
    /// `ConnectCityAction`: the city on `at` is now connected to `player`'s capital; both gain a population point.
    ConnectCity { player: u8, at: crate::state::Coords },
    /// `ChangeCityConnectionAction`: the city on `at` is now connected to `player`'s capital instead of another player's.
    ChangeCityConnection { player: u8, at: crate::state::Coords },
    /// `DisconnectCityAction`: the connection is lost; both lose a population point.
    DisconnectCity { player: u8, at: crate::state::Coords },
    /// `DecreasePopulationAction`: remove one population point from the city on `at`.
    DecreasePopulation { player: u8, at: crate::state::Coords },
    /// `AttackAction`: `PerformAttack` from `origin` onto `target` for `damage`.
    Attack { player: u8, origin: crate::state::Coords, target: crate::state::Coords, damage: i32 },
    /// `KillUnitAction`: remove the (dead) unit on `at`.
    KillUnit { player: u8, at: crate::state::Coords },
    /// `ExpandCityAction`: claim the tiles of the city's (enlarged) border area and explore it.
    ExpandCity { player: u8, at: crate::state::Coords },
    /// `ScoutMoveAction`: one step of an explorer's search for fog (Explorer city reward); queues the next step.
    ScoutMove { player: u8, unit_id: u32, remaining: u32, seed: u32, from: crate::state::Coords, path: Vec<crate::state::Coords> },
    /// `ModifyProductionAction`: add `amount` to the city's production.
    ModifyProduction { at: crate::state::Coords, amount: i16 },
    /// `ModifyScoreAction`: add `amount` to the city's base score.
    ModifyScore { at: crate::state::Coords, amount: i16 },
    /// `ExploreAction`: the player explores one tile.
    Explore { player: u8, at: crate::state::Coords },
    /// `RecoverAction`: the unit on `at` has used its turn (moved and attacked); queued before its `Heal`.
    Recover { player: u8, at: crate::state::Coords },
    /// `PromoteAction`: the unit gains a promotion level and is fully healed.
    Promote { player: u8, at: crate::state::Coords },
    /// `ExamineRuinsAction`: the unit on `at` searches the ruins and gets `reward`.
    ExamineRuins { player: u8, reward: u16, at: crate::state::Coords },
    /// `UpgradeAction`: pay `cost` and replace the unit on `at` by a unit of type `kind`, keeping its damage.
    Upgrade { player: u8, kind: u16, at: crate::state::Coords, cost: i32 },
    /// `EmbarkAction`: the land unit on `at` boards the boat that its port turns it into.
    Embark { player: u8, at: crate::state::Coords },
    /// `DisembarkAction`: the boat on `at` lands its passenger.
    Disembark { player: u8, at: crate::state::Coords },
    /// `DisbandUnitAction`: remove the unit on `at` (with its follower), refunding half its cost in stars when `refund`.
    Disband { player: u8, at: crate::state::Coords, refund: bool },
    /// `WipePlayerAction`: `wiped` has lost their last city to `player`; their units are disbanded and embassies dissolved.
    WipePlayer { player: u8, wiped: u8 },
    /// `GameOverAction`: the game ends with `winner`.
    GameOver { player: u8, winner: u8 },
    /// `TaskCompletedAction`: queued when a task becomes complete. Its effects are not implemented.
    TaskCompleted { player: u8, kind: u16 },
}

impl Action {
    pub fn name(&self) -> &'static str {
        match self {
            Action::StartMatch { .. } => "StartMatch",
            Action::EnableTask { .. } => "EnableTask",
            Action::IncreaseScore { .. } => "IncreaseScore",
            Action::IncreaseCurrency { .. } => "IncreaseCurrency",
            Action::RunDeferred { .. } => "RunDeferred",
            Action::UpdateRoutes => "UpdateRoutes",
            Action::UpdateCityConnections { .. } => "UpdateCityConnections",
            Action::EndTurn { .. } => "EndTurn",
            Action::StartTurn { .. } => "StartTurn",
            Action::Heal { .. } => "Heal",
            Action::Reselect { .. } => "Reselect",
            Action::Move { .. } => "Move",
            Action::Research { .. } => "Research",
            Action::Train { .. } => "Train",
            Action::Build { .. } => "Build",
            Action::DestroyResource { .. } => "DestroyResource",
            Action::BuildRoad { .. } => "BuildRoad",
            Action::IncreasePopulation { .. } => "IncreasePopulation",
            Action::CityLevelUp { .. } => "CityLevelUp",
            Action::CityReward { .. } => "CityReward",
            Action::Meet { .. } => "Meet",
            Action::ImprovementLevelUp { .. } => "ImprovementLevelUp",
            Action::ImprovementLevelDown { .. } => "ImprovementLevelDown",
            Action::CaptureCity { .. } => "CaptureCity",
            Action::DecreaseScore { .. } => "DecreaseScore",
            Action::DestroyEmbassy { .. } => "DestroyEmbassy",
            Action::ConnectCity { .. } => "ConnectCity",
            Action::ChangeCityConnection { .. } => "ChangeCityConnection",
            Action::DisconnectCity { .. } => "DisconnectCity",
            Action::DecreasePopulation { .. } => "DecreasePopulation",
            Action::Attack { .. } => "Attack",
            Action::KillUnit { .. } => "KillUnit",
            Action::ExpandCity { .. } => "ExpandCity",
            Action::ScoutMove { .. } => "ScoutMove",
            Action::ModifyProduction { .. } => "ModifyProduction",
            Action::ModifyScore { .. } => "ModifyScore",
            Action::Explore { .. } => "Explore",
            Action::Recover { .. } => "Recover",
            Action::Promote { .. } => "Promote",
            Action::ExamineRuins { .. } => "ExamineRuins",
            Action::Upgrade { .. } => "Upgrade",
            Action::Embark { .. } => "Embark",
            Action::Disembark { .. } => "Disembark",
            Action::Disband { .. } => "Disband",
            Action::WipePlayer { .. } => "WipePlayer",
            Action::GameOver { .. } => "GameOver",
            Action::TaskCompleted { .. } => "TaskCompleted",
        }
    }
}
