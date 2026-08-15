//! NV2.0 gameplay systems.
//!
//! Pure, testable logic for the release-state mechanics:
//!   - `GameClock`        — day/night cycle (drives spawns + lighting tint)
//!   - `PlayerStats`      — health & hunger, regen, starvation, death
//!   - `EnemyManager`     — simple hostile AI (spawn at night, chase, attack)
//!   - `ToolWear`         — tool durability (wear, break, repair)
//!   - `AchievementTracker` — progression/unlocks
//!   - `GameSession`      — ties it all together for the app loop
//!
//! None of this touches the renderer or input; every subsystem is covered by
//! unit tests below.

use std::collections::HashMap;

use crate::world::biomes::BiomeId;
use crate::world::BlockType;

/// Full day/night cycle length in real seconds (10 minutes = one day).
pub const DAY_LENGTH_SECONDS: f32 = 600.0;
/// Night starts at this clock phase (last 35% of the day).
pub const NIGHT_PHASE_START: f32 = 0.65;

// ------------------------------------------------------------------ GameClock
#[derive(Clone, Debug, PartialEq)]
pub struct GameClock {
    /// Seconds elapsed in the current day, 0..day_length.
    pub time_of_day: f32,
    /// Seconds for a full day.
    pub day_length: f32,
    /// How many full days have passed.
    pub day_count: u32,
}

impl GameClock {
    pub fn new(day_length: f32) -> Self {
        Self {
            time_of_day: 0.0,
            day_length,
            day_count: 0,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.time_of_day += dt.max(0.0);
        while self.time_of_day >= self.day_length {
            self.time_of_day -= self.day_length;
            self.day_count += 1;
        }
    }

    /// Phase of the day in [0, 1).
    pub fn phase(&self) -> f32 {
        (self.time_of_day / self.day_length).clamp(0.0, 1.0)
    }

    pub fn is_night(&self) -> bool {
        self.phase() >= NIGHT_PHASE_START
    }

    /// 0.0 at full day, 1.0 at deepest night (for lighting tint).
    pub fn darkness(&self) -> f32 {
        let p = self.phase();
        if p < NIGHT_PHASE_START {
            0.0
        } else {
            ((p - NIGHT_PHASE_START) / (1.0 - NIGHT_PHASE_START)).clamp(0.0, 1.0)
        }
    }

    /// Clock time as (hour, minute). The day starts at 06:00, so `time_of_day
    /// == 0` reads as 06:00 and 06:00 reads as 06:00 — the game clock is a
    /// normal 24-hour wall clock.
    pub fn hour_minute(&self) -> (u32, u32) {
        let frac = self.phase();
        let total_hours = frac * 24.0 + 6.0;
        let mut hour = (total_hours as u32) % 24;
        let minute_f = total_hours.fract() * 60.0;
        let mut minute = minute_f.round() as u32;
        if minute >= 60 {
            minute = 0;
            hour = (hour + 1) % 24;
        }
        (hour, minute)
    }

    /// Set the time of day directly, in 24-hour wall-clock hours
    /// (e.g. 21.0 = 21:00, which is night in NV2).
    pub fn set_time_hours(&mut self, hours: f32) {
        let h = hours.rem_euclid(24.0);
        let frac = ((h - 6.0).rem_euclid(24.0)) / 24.0;
        self.time_of_day = frac * self.day_length;
    }
}

// ------------------------------------------------------------------ PlayerStats
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerStats {
    pub health: f32,
    pub max_health: f32,
    pub hunger: f32,
    pub max_hunger: f32,
    pub thirst: f32,
    pub max_thirst: f32,
}

impl PlayerStats {
    pub fn new() -> Self {
        Self {
            health: 20.0,
            max_health: 20.0,
            hunger: 100.0,
            max_hunger: 100.0,
            thirst: 100.0,
            max_thirst: 100.0,
        }
    }

    /// Per-second survival tick:
    /// - hunger decays over ~5 game-minutes of play
    /// - thirst decays over ~8 game-minutes of play
    /// - above 60% food AND 60% water the player regenerates health
    /// - at 0 hunger the player starves; at 0 water the player dehydrates
    pub fn tick(&mut self, dt: f32) {
        if self.health <= 0.0 {
            return;
        }
        self.hunger = (self.hunger - dt * 100.0 / 300.0).max(0.0);
        self.thirst = (self.thirst - dt * 100.0 / 500.0).max(0.0);
        let well_fed = self.hunger > self.max_hunger * 0.6;
        let hydrated = self.thirst > self.max_thirst * 0.6;
        if well_fed && hydrated {
            self.health = (self.health + dt * 1.0).min(self.max_health);
        } else if self.hunger <= 0.0 {
            self.health = (self.health - dt * 1.5).max(0.0);
        } else if self.thirst <= 0.0 {
            self.health = (self.health - dt * 1.5).max(0.0);
        }
    }

    pub fn eat(&mut self, food: f32) {
        self.hunger = (self.hunger + food).min(self.max_hunger);
    }

    pub fn drink(&mut self, water: f32) {
        self.thirst = (self.thirst + water).min(self.max_thirst);
    }

    /// Apply damage. Returns true if the hit was fatal.
    pub fn damage(&mut self, amount: f32) -> bool {
        self.health = (self.health - amount.max(0.0)).max(0.0);
        self.health <= 0.0
    }

    pub fn heal(&mut self, amount: f32) {
        self.health = (self.health + amount).min(self.max_health);
    }

    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for PlayerStats {
    fn default() -> Self {
        Self::new()
    }
}

// ------------------------------------------------------------------ Animals
/// Passive wildlife — the reason the world doesn't feel dead. Animals
/// wander, hop and flee from the player, and only spawn in biomes where
/// they belong (deer in forests, rabbits in plains — never in a desert).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimalKind {
    Deer,
    Rabbit,
}

impl AnimalKind {
    /// Fur colour used by the renderer.
    pub fn color(self) -> [f32; 3] {
        match self {
            AnimalKind::Deer => [0.62, 0.44, 0.28],
            AnimalKind::Rabbit => [0.86, 0.84, 0.78],
        }
    }

    pub fn speed(self) -> f32 {
        match self {
            AnimalKind::Deer => 2.4,
            AnimalKind::Rabbit => 3.6,
        }
    }

    /// Whether this animal can live in the given biome.
    pub fn lives_in(self, biome: BiomeId) -> bool {
        match self {
            AnimalKind::Deer => matches!(
                biome,
                BiomeId::Forest
                    | BiomeId::DarkForest
                    | BiomeId::Taiga
                    | BiomeId::Swamp
                    | BiomeId::Mountains
            ),
            AnimalKind::Rabbit => matches!(biome, BiomeId::Plains | BiomeId::Coast),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Animal {
    pub id: u64,
    pub kind: AnimalKind,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub heading: f32,
    pub wander_timer: f32,
    pub flee_timer: f32,
    /// Animation phase (hop/sway) — starts from a per-animal offset.
    pub bob: f32,
}

pub struct AnimalManager {
    pub animals: Vec<Animal>,
    pub next_id: u64,
    pub spawn_timer: f32,
    pub max_animals: usize,
    /// Biome at the player — decides which animals may spawn nearby.
    pub biome: Option<BiomeId>,
}

impl AnimalManager {
    pub fn new() -> Self {
        Self {
            animals: Vec::new(),
            next_id: 1,
            spawn_timer: 1.0,
            max_animals: 10,
            biome: None,
        }
    }

    /// Advance wildlife. `candidates` are pre-computed land positions near
    /// the player (the world knows the terrain; this module does not).
    /// Returns the player-distance of the closest animal (for the HUD).
    pub fn update(
        &mut self,
        dt: f32,
        player: (f32, f32),
        candidates: &[(AnimalKind, (f32, f32, f32))],
    ) -> Option<f32> {
        // Top up the herd with the pre-computed spawn positions.
        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 && self.animals.len() < self.max_animals {
            self.spawn_timer = 1.4;
            if let Some(&(kind, (x, y, z))) = candidates.first() {
                self.animals.push(Animal {
                    id: self.next_id,
                    kind,
                    x,
                    y,
                    z,
                    heading: (self.next_id as f32) * 2.399_963,
                    wander_timer: 1.0,
                    flee_timer: 0.0,
                    bob: (self.next_id as f32) * 1.7,
                });
                self.next_id += 1;
            }
        }

        // Despawn stragglers so the herd follows the player.
        self.animals.retain(|a| {
            let dx = a.x - player.0;
            let dz = a.z - player.1;
            dx * dx + dz * dz < 80.0 * 80.0
        });

        let mut closest: Option<f32> = None;
        for a in &mut self.animals {
            a.bob += dt * 5.0;
            let dx = player.0 - a.x;
            let dz = player.1 - a.z;
            let dist_sq = dx * dx + dz * dz;
            closest = Some(closest.map_or(dist_sq, |c: f32| c.min(dist_sq)));

            if dist_sq < 6.0 * 6.0 {
                // Panic: run away from the player.
                a.flee_timer = 2.5;
            }
            if a.flee_timer > 0.0 {
                a.flee_timer -= dt;
                let dist = dist_sq.sqrt().max(0.001);
                a.x -= dx / dist * a.kind.speed() * 1.7 * dt;
                a.z -= dz / dist * a.kind.speed() * 1.7 * dt;
            } else {
                // Graze: turn gently and stroll.
                a.wander_timer -= dt;
                if a.wander_timer <= 0.0 {
                    a.wander_timer = 2.0 + (a.id % 3) as f32;
                    a.heading += (a.id as f32 * 0.77).sin() * 1.4 + 0.6;
                }
                a.x += a.heading.cos() * a.kind.speed() * 0.30 * dt;
                a.z += a.heading.sin() * a.kind.speed() * 0.30 * dt;
            }
        }
        closest.map(|d| d.sqrt())
    }

    /// The animal kind that may spawn here (None in deserts/tundra/ocean).
    pub fn next_kind(&self) -> Option<AnimalKind> {
        match self.biome {
            Some(BiomeId::Plains) | Some(BiomeId::Coast) => Some(AnimalKind::Rabbit),
            Some(BiomeId::Forest)
            | Some(BiomeId::DarkForest)
            | Some(BiomeId::Taiga)
            | Some(BiomeId::Swamp)
            | Some(BiomeId::Mountains) => Some(AnimalKind::Deer),
            _ => None,
        }
    }
}

impl Default for AnimalManager {
    fn default() -> Self {
        Self::new()
    }
}

// ------------------------------------------------------------------ Enemy
#[derive(Clone, Debug, PartialEq)]
pub struct Enemy {
    pub id: u64,
    pub x: f32,
    pub z: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub attack_cooldown: f32,
    pub attack_damage: f32,
    pub range: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnemyEvent {
    Spawned(u64),
    DamagedPlayer(f32),
    Killed(u64),
}

pub struct EnemyManager {
    pub enemies: Vec<Enemy>,
    pub next_id: u64,
    pub spawn_timer: f32,
    pub kills: u32,
    /// Seconds between spawns while it is night (and below the cap).
    pub spawn_interval: f32,
    /// Maximum simultaneous enemies.
    pub max_enemies: usize,
}

impl EnemyManager {
    pub fn new() -> Self {
        Self {
            enemies: Vec::new(),
            next_id: 1,
            spawn_timer: 0.0,
            kills: 0,
            spawn_interval: 6.0,
            max_enemies: 5,
        }
    }

    /// Advance the simulation: spawn while it's night, chase the player,
    /// attack when in range. Returns events (damage dealt, spawns, deaths).
    pub fn update(
        &mut self,
        dt: f32,
        player: (f32, f32),
        is_night: bool,
    ) -> Vec<EnemyEvent> {
        let mut events = Vec::new();

        if is_night && self.enemies.len() < self.max_enemies {
            self.spawn_timer -= dt;
            if self.spawn_timer <= 0.0 {
                self.spawn_near(player);
                events.push(EnemyEvent::Spawned(self.next_id - 1));
                self.spawn_timer = self.spawn_interval;
            }
        } else if !is_night {
            // daylight clears the hostile swarm
            self.enemies.clear();
            self.spawn_timer = self.spawn_interval;
        }

        for enemy in &mut self.enemies {
            let dx = player.0 - enemy.x;
            let dz = player.1 - enemy.z;
            let dist_sq = dx * dx + dz * dz;
            let range = enemy.range;
            if dist_sq > range * range {
                // chase
                let dist = dist_sq.sqrt().max(0.001);
                let step = enemy.speed * dt;
                enemy.x += dx / dist * step;
                enemy.z += dz / dist * step;
            } else {
                // in range — attack on cooldown
                enemy.attack_cooldown -= dt;
                if enemy.attack_cooldown <= 0.0 {
                    enemy.attack_cooldown = 1.5;
                    events.push(EnemyEvent::DamagedPlayer(enemy.attack_damage));
                }
            }
        }
        events
    }

    pub fn spawn_near(&mut self, player: (f32, f32)) {
        let angle = self.next_id as f32 * 2.399_963; // golden angle
        let radius = 12.0 + (self.next_id % 5) as f32 * 3.0;
        self.enemies.push(Enemy {
            id: self.next_id,
            x: player.0 + angle.cos() * radius,
            z: player.1 + angle.sin() * radius,
            hp: 10.0,
            max_hp: 10.0,
            speed: 2.2,
            attack_cooldown: 1.0,
            attack_damage: 2.0,
            range: 1.6,
        });
        self.next_id += 1;
    }

    /// Player attack: damage the nearest enemy within `reach`.
    /// Returns Some(enemy_id) if a hit landed, None otherwise.
    pub fn attack_nearest(&mut self, player: (f32, f32), reach: f32) -> Option<u64> {
        let mut best: Option<(u64, f32)> = None;
        for enemy in &self.enemies {
            let dx = player.0 - enemy.x;
            let dz = player.1 - enemy.z;
            let dist_sq = dx * dx + dz * dz;
            if dist_sq <= reach * reach {
                if best.map_or(true, |(_, d)| dist_sq < d) {
                    best = Some((enemy.id, dist_sq));
                }
            }
        }
        best.map(|(id, _)| {
            let _ = self.damage_enemy(id, 4.0);
            id
        })
    }

    /// Deal damage to an enemy by id. Returns Killed(id) when it dies.
    pub fn damage_enemy(&mut self, id: u64, amount: f32) -> Option<EnemyEvent> {
        if let Some(idx) = self.enemies.iter().position(|e| e.id == id) {
            self.enemies[idx].hp -= amount;
            if self.enemies[idx].hp <= 0.0 {
                self.enemies.remove(idx);
                self.kills += 1;
                return Some(EnemyEvent::Killed(id));
            }
        }
        None
    }

    pub fn nearest_distance_sq(&self, player: (f32, f32)) -> Option<f32> {
        self.enemies
            .iter()
            .map(|e| {
                let dx = player.0 - e.x;
                let dz = player.1 - e.z;
                dx * dx + dz * dz
            })
            .fold(None, |acc, d| Some(acc.map_or(d, |a: f32| a.min(d))))
    }
}

impl Default for EnemyManager {
    fn default() -> Self {
        Self::new()
    }
}

// ------------------------------------------------------------------ ToolWear
/// Which block types count as tools (by display name).
pub fn is_tool(block: BlockType) -> bool {
    let name = block.display_name().to_ascii_lowercase();
    ["pickaxe", "axe", "sword", "shovel", "hoe"]
        .iter()
        .any(|t| name.contains(t))
}

#[derive(Clone, Debug, Default)]
pub struct ToolWear {
    /// Cumulative wear per tool type.
    pub wear: HashMap<BlockType, u32>,
    /// Uses before a tool breaks.
    pub max_wear: u32,
}

impl ToolWear {
    pub fn new(max_wear: u32) -> Self {
        Self {
            wear: HashMap::new(),
            max_wear,
        }
    }

    /// Use a tool once. Returns Some(remaining_uses) or None if it's broken.
    pub fn use_tool(&mut self, tool: BlockType) -> Option<u32> {
        if !is_tool(tool) {
            return None;
        }
        let entry = self.wear.entry(tool).or_insert(0);
        *entry += 1;
        if *entry >= self.max_wear {
            None
        } else {
            Some(self.max_wear - *entry)
        }
    }

    pub fn remaining(&self, tool: BlockType) -> u32 {
        self.max_wear.saturating_sub(self.wear.get(&tool).copied().unwrap_or(0))
    }

    pub fn broken(&self, tool: BlockType) -> bool {
        self.remaining(tool) == 0
    }

    pub fn repair(&mut self, tool: BlockType) {
        self.wear.remove(&tool);
    }

    pub fn repair_all(&mut self) {
        self.wear.clear();
    }

    pub fn worn_tools(&self) -> Vec<(BlockType, u32)> {
        let mut v: Vec<(BlockType, u32)> = self
            .wear
            .iter()
            .map(|(t, w)| (*t, self.max_wear.saturating_sub(*w)))
            .collect();
        v.sort_by_key(|(_, remaining)| *remaining);
        v
    }
}

// ------------------------------------------------------------------ Achievements
pub const ACHIEVEMENTS: &[&str] = &[
    "first-night",       // survive your first night
    "five-nights",       // survive five nights
    "first-kill",        // defeat your first hostile
    "hunter",            // defeat ten hostiles
    "full-health",       // reach full health
    "master-crafter",    // craft an item from the 3x3 grid
];

#[derive(Clone, Debug)]
pub struct AchievementTracker {
    pub unlocked: Vec<String>,
    pub nights_survived: u32,
    pub kills: u32,
    pub crafts: u32,
}

impl AchievementTracker {
    pub fn new() -> Self {
        Self {
            unlocked: Vec::new(),
            nights_survived: 0,
            kills: 0,
            crafts: 0,
        }
    }

    pub fn unlock(&mut self, name: &str) -> bool {
        if self.unlocked.iter().any(|a| a == name) {
            return false;
        }
        self.unlocked.push(name.to_string());
        true
    }

    pub fn record_night_survived(&mut self) -> bool {
        self.nights_survived += 1;
        let mut any = false;
        if self.nights_survived == 1 {
            any |= self.unlock("first-night");
        }
        if self.nights_survived >= 5 {
            any |= self.unlock("five-nights");
        }
        any
    }

    pub fn record_kill(&mut self) -> bool {
        self.kills += 1;
        let mut any = false;
        if self.kills == 1 {
            any |= self.unlock("first-kill");
        }
        if self.kills >= 10 {
            any |= self.unlock("hunter");
        }
        any
    }

    pub fn record_craft(&mut self) -> bool {
        self.crafts += 1;
        self.unlock("master-crafter")
    }

    pub fn record_full_health(&mut self) -> bool {
        self.unlock("full-health")
    }
}

impl Default for AchievementTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ------------------------------------------------------------------ GameSession
/// Aggregated per-frame player feedback.
#[derive(Clone, Debug, Default)]
pub struct SessionFeedback {
    pub messages: Vec<String>,
    pub died: bool,
    /// Achievement IDs unlocked this tick (for EOS / store reporting).
    pub unlocked_achievements: Vec<String>,
}

pub struct GameSession {
    pub clock: GameClock,
    pub stats: PlayerStats,
    pub enemies: EnemyManager,
    pub animals: AnimalManager,
    pub wear: ToolWear,
    pub achievements: AchievementTracker,
    /// Tracked so night→morning transitions count exactly once.
    prev_night: bool,
}

impl GameSession {
    pub fn new() -> Self {
        Self {
            clock: GameClock::new(DAY_LENGTH_SECONDS),
            stats: PlayerStats::new(),
            enemies: EnemyManager::new(),
            animals: AnimalManager::new(),
            wear: ToolWear::new(64),
            achievements: AchievementTracker::new(),
            prev_night: false,
        }
    }

    /// Advance the whole simulation. `player` is (x, z) in world space.
    pub fn update(&mut self, dt: f32, player: (f32, f32)) -> SessionFeedback {
        let mut fb = SessionFeedback::default();
        self.clock.tick(dt);
        self.stats.tick(dt);

        // night transitions
        let night = self.clock.is_night();
        if self.prev_night && !night {
            if self.achievements.record_night_survived() {
                fb.messages.push(format!(
                    "Achievement unlocked: first-night — survived night #{}!",
                    self.achievements.nights_survived
                ));
                fb.unlocked_achievements.push("first-night".to_string());
            }
            if self.achievements.nights_survived >= 5 {
                fb.unlocked_achievements.push("five-nights".to_string());
            }
        }
        self.prev_night = night;

        // enemies
        let events = self.enemies.update(dt, player, night);
        for event in events {
            match event {
                EnemyEvent::Spawned(id) => {
                    fb.messages.push(format!("☠ Hostile sighted (id {id}) — it's night!"));
                }
                EnemyEvent::DamagedPlayer(amount) => {
                    let fatal = self.stats.damage(amount);
                    fb.messages
                        .push(format!("☠ Took {amount:.0} damage (hp {:.0})", self.stats.health));
                    if fatal {
                        fb.died = true;
                    }
                }
                EnemyEvent::Killed(_) => {
                    if self.achievements.record_kill() {
                        fb.messages.push("Achievement: first-kill!".to_string());
                        fb.unlocked_achievements.push("first-kill".to_string());
                    }
                }
            }
        }

        // full-health achievement
        if self.stats.health >= self.stats.max_health {
            if self.achievements.record_full_health() {
                fb.messages.push("Achievement: full-health!".to_string());
                fb.unlocked_achievements.push("full-health".to_string());
            }
        }
        fb
    }

    pub fn eat(&mut self, food: f32) -> String {
        self.stats.eat(food);
        format!("Food: {:.0}/{:.0}", self.stats.hunger, self.stats.max_hunger)
    }

    pub fn drink(&mut self, water: f32) -> String {
        self.stats.drink(water);
        format!("Water: {:.0}/{:.0}", self.stats.thirst, self.stats.max_thirst)
    }

    pub fn heal_player(&mut self, amount: f32) -> String {
        self.stats.heal(amount);
        format!("Health: {:.0}/{:.0}", self.stats.health, self.stats.max_health)
    }

    pub fn attack_nearest(&mut self, player: (f32, f32)) -> String {
        match self.enemies.attack_nearest(player, 4.0) {
            Some(_) => format!("⚔ hit! hostiles left: {}", self.enemies.enemies.len()),
            None => "⚔ swing — nothing in reach".to_string(),
        }
    }

    pub fn damage_player(&mut self, amount: f32) -> bool {
        self.stats.damage(amount)
    }

    /// Respawn: reset stats, clear hostiles, start a fresh morning.
    pub fn respawn(&mut self) -> String {
        self.stats.reset();
        self.enemies.enemies.clear();
        self.clock.set_time_hours(6.0);
        self.prev_night = false;
        "You died and respawned at dawn.".to_string()
    }

    /// One-line HUD summary.
    pub fn hud_line(&self) -> String {
        let (h, m) = self.clock.hour_minute();
        let icon = if self.clock.is_night() { "🌙" } else { "☀" };
        format!(
            "{} {:02}:{:02} | ♥ {:.0}/{:.0} | 🍗 {:.0}/{:.0} | 💧 {:.0}/{:.0} | ☠ {}",
            icon,
            h,
            m,
            self.stats.health,
            self.stats.max_health,
            self.stats.hunger,
            self.stats.max_hunger,
            self.stats.thirst,
            self.stats.max_thirst,
            self.enemies.enemies.len(),
        )
    }
}

impl Default for GameSession {
    fn default() -> Self {
        Self::new()
    }
}

// ================================================================== tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_advances_and_wraps() {
        let mut clock = GameClock::new(100.0);
        clock.tick(40.0);
        assert_eq!(clock.time_of_day, 40.0);
        assert_eq!(clock.day_count, 0);
        clock.tick(70.0);
        assert_eq!(clock.time_of_day, 10.0);
        assert_eq!(clock.day_count, 1);
    }

    #[test]
    fn clock_night_phase_and_darkness() {
        let mut clock = GameClock::new(100.0);
        assert!(!clock.is_night());
        assert_eq!(clock.darkness(), 0.0);
        assert_eq!(clock.hour_minute(), (6, 0));
        clock.set_time_hours(12.0); // midday: full day
        assert!(!clock.is_night());
        assert_eq!(clock.darkness(), 0.0);
        assert_eq!(clock.hour_minute(), (12, 0));
        clock.set_time_hours(23.0); // 23:00: night
        assert!(clock.is_night());
        assert!(clock.darkness() > 0.0);
        assert_eq!(clock.hour_minute(), (23, 0));
        clock.set_time_hours(3.0); // 03:00: still night (before 06:00)
        assert!(clock.is_night());
    }

    #[test]
    fn clock_hour_minute_start_is_0600() {
        let clock = GameClock::new(600.0);
        let (h, m) = clock.hour_minute();
        assert_eq!((h, m), (6, 0));
    }

    #[test]
    fn stats_regen_above_hunger_threshold() {
        let mut stats = PlayerStats::new();
        stats.health = 10.0;
        stats.hunger = 80.0;
        stats.thirst = 80.0;
        stats.tick(2.0);
        assert!(stats.health > 10.0, "should regen with food and water");
        assert!(stats.hunger < 80.0, "hunger decays");
    }

    #[test]
    fn stats_no_regen_when_dehydrated() {
        let mut stats = PlayerStats::new();
        stats.health = 10.0;
        stats.hunger = 80.0;
        stats.thirst = 10.0;
        stats.tick(2.0);
        assert!(
            stats.health <= 10.0,
            "thirst below threshold must block regen"
        );
    }

    #[test]
    fn stats_dehydrate_at_zero_thirst() {
        let mut stats = PlayerStats::new();
        stats.hunger = 80.0;
        stats.thirst = 0.0;
        stats.tick(2.0);
        assert!(stats.health < 20.0, "dehydration damages");
    }

    #[test]
    fn stats_drink_restores_thirst() {
        let mut stats = PlayerStats::new();
        stats.thirst = 0.0;
        stats.drink(40.0);
        assert_eq!(stats.thirst, 40.0);
        stats.drink(500.0);
        assert_eq!(stats.thirst, stats.max_thirst);
    }

    #[test]
    fn stats_starve_at_zero_hunger() {
        let mut stats = PlayerStats::new();
        stats.hunger = 0.0;
        stats.tick(2.0);
        assert!(stats.health < 20.0, "starvation damages");
    }

    #[test]
    fn stats_death_and_reset() {
        let mut stats = PlayerStats::new();
        assert!(stats.damage(100.0));
        assert!(stats.is_dead());
        stats.reset();
        assert!(!stats.is_dead());
        assert_eq!(stats.health, 20.0);
    }

    #[test]
    fn stats_eat_clamps_to_max() {
        let mut stats = PlayerStats::new();
        stats.eat(500.0);
        assert_eq!(stats.hunger, stats.max_hunger);
    }

    #[test]
    fn enemies_spawn_only_at_night_and_chase() {
        let mut mgr = EnemyManager::new();
        mgr.spawn_interval = 0.0;
        // day: no spawns, cleared
        let events = mgr.update(1.0, (0.0, 0.0), false);
        assert!(mgr.enemies.is_empty());
        assert!(events.is_empty());
        // night: spawn happens
        let events = mgr.update(1.0, (0.0, 0.0), true);
        assert_eq!(mgr.enemies.len(), 1);
        assert!(events.iter().any(|e| matches!(e, EnemyEvent::Spawned(_))));
        // enemy chases toward the player
        let e = &mgr.enemies[0];
        let dist_before = (e.x * e.x + e.z * e.z).sqrt();
        mgr.update(1.0, (0.0, 0.0), true);
        let e = &mgr.enemies[0];
        let dist_after = (e.x * e.x + e.z * e.z).sqrt();
        assert!(dist_after < dist_before, "enemy should approach the player");
    }

    #[test]
    fn enemies_attack_in_range() {
        let mut mgr = EnemyManager::new();
        mgr.spawn_near((0.0, 0.0));
        // teleport the enemy onto the player
        mgr.enemies[0].x = 0.0;
        mgr.enemies[0].z = 0.0;
        mgr.enemies[0].attack_cooldown = 0.0;
        let events = mgr.update(0.1, (0.0, 0.0), true);
        assert!(events.iter().any(|e| matches!(e, EnemyEvent::DamagedPlayer(_))));
    }

    #[test]
    fn enemies_capped_and_daylight_clears() {
        let mut mgr = EnemyManager::new();
        mgr.max_enemies = 3;
        mgr.spawn_interval = 0.0;
        for _ in 0..10 {
            mgr.update(1.0, (0.0, 0.0), true);
        }
        assert!(mgr.enemies.len() <= 3);
        mgr.update(1.0, (0.0, 0.0), false);
        assert!(mgr.enemies.is_empty());
    }

    #[test]
    fn player_attack_kills_and_counts() {
        let mut mgr = EnemyManager::new();
        mgr.spawn_near((0.0, 0.0));
        let id = mgr.enemies[0].id;
        mgr.enemies[0].x = 0.0;
        mgr.enemies[0].z = 0.0;
        mgr.enemies[0].hp = 1.0; // one hit
        let hit = mgr.attack_nearest((0.0, 0.0), 4.0);
        assert_eq!(hit, Some(id));
        assert_eq!(mgr.kills, 1);
        assert!(mgr.enemies.is_empty());
    }

    #[test]
    fn tools_wear_break_and_repair() {
        let mut wear = ToolWear::new(3);
        // only real tools are tracked
        assert!(wear.use_tool(BlockType::TreeTrunk).is_none());
        let tool = tool_block();
        assert_eq!(wear.use_tool(tool), Some(2));
        assert_eq!(wear.use_tool(tool), Some(1));
        assert_eq!(wear.use_tool(tool), None); // broken
        assert!(wear.broken(tool));
        wear.repair(tool);
        assert!(!wear.broken(tool));
        assert_eq!(wear.remaining(tool), 3);
    }

    #[test]
    fn achievements_unlock_once() {
        let mut a = AchievementTracker::new();
        assert!(a.record_kill());
        assert!(!a.record_kill()); // second kill, no new unlock yet
        assert!(a.unlocked.contains(&"first-kill".to_string()));
        // nights: 1 unlocks, 2/3/4 don't, 5 unlocks
        assert!(a.record_night_survived());
        assert!(!a.record_night_survived());
        assert!(!a.record_night_survived());
        assert!(!a.record_night_survived());
        assert!(a.record_night_survived()); // 5th night → five-nights
        assert!(a.unlocked.contains(&"five-nights".to_string()));
        // craft
        assert!(a.record_craft());
        assert!(a.unlocked.contains(&"master-crafter".to_string()));
    }

    #[test]
    fn session_night_transition_grants_achievement() {
        let mut session = GameSession::new();
        session.clock.set_time_hours(22.0); // night
        // simulate until dawn (6:00 = 0 hours of game time)
        session.clock.set_time_hours(5.0);
        let fb = session.update(0.0, (0.0, 0.0));
        // prev_night was false (clock jumped), so no transition — reset state
        // by stepping: enter night, then exit it
        session.clock.set_time_hours(22.0);
        let _ = session.update(0.0, (0.0, 0.0)); // now prev_night = true
        session.clock.set_time_hours(6.0);
        let fb = session.update(0.0, (0.0, 0.0));
        assert_eq!(session.achievements.nights_survived, 1);
        assert!(fb.messages.iter().any(|m| m.contains("first-night")));
        assert_eq!(session.clock.hour_minute(), (6, 0));
    }

    #[test]
    fn session_hud_and_respawn() {
        let mut session = GameSession::new();
        let hud = session.hud_line();
        assert!(hud.contains("♥"), "hud shows health: {hud}");
        assert!(hud.contains("🍗"), "hud shows hunger: {hud}");
        assert!(session.damage_player(100.0));
        let msg = session.respawn();
        assert!(msg.contains("respawned"));
        assert_eq!(session.stats.health, 20.0);
    }

    /// A known tool block (stone pickaxe, id 56).
    fn tool_block() -> BlockType {
        BlockType::StonePickaxe
    }

    #[test]
    fn some_tool_blocks_exist() {
        let tools: Vec<&str> = crate::world::block::BLOCK_REGISTRY
            .iter()
            .filter_map(|(_, name, _)| BlockType::from_name(name))
            .filter(|b| is_tool(*b))
            .map(|b| b.display_name())
            .collect();
        assert!(
            !tools.is_empty(),
            "registry must contain tools (pickaxes/axes/swords/shovels/hoes): {tools:?}"
        );
        assert!(tools.iter().any(|t| t.contains("Pickaxe")));
    }
}
