//! Player-action feedback for the AI vegetation system (NV2.0).
//!
//! Whenever the player places or breaks a block, the interaction layer calls
//! [`record_place`] / [`record_break`]. This module converts the world
//! context (surface height, biome climate) into the AI's 8-feature vector
//! and pushes a training sample into the background model — so the AI
//! literally learns from what the player does.

use cgmath::Vector3;
use super::ai_generator::{features_from_context, one_hot, vegetation_class};
use super::block::BlockType;
use super::World;

/// Record that the player placed `block` at `pos`.
pub fn record_place(world: &World, pos: Vector3<i32>, block: BlockType) {
    // Only vegetation classes the AI decides about are useful signals.
    let Some(class) = vegetation_class(block) else {
        return;
    };

    let features = world_features(world, pos);
    let target = one_hot(class);
    world.ai_system.record_player_action(features, target);
}

/// Record that the player broke `block` at `pos`.
///
/// Breaking vegetation the AI would have placed is treated as a negative
/// signal: the target is pushed away from that class (distributed over the
/// others), teaching the model not to suggest it there.
pub fn record_break(world: &World, pos: Vector3<i32>, block: BlockType) {
    let Some(class) = vegetation_class(block) else {
        return;
    };

    let features = world_features(world, pos);
    // Negative example: zero weight on the broken class, spread over the rest.
    let mut target = [0.0f32; 4];
    for i in 0..4 {
        if i != class {
            target[i] = 1.0 / 3.0;
        }
    }
    world.ai_system.record_player_action(features, target);
}

/// Build the 8-feature vector for a world position.
fn world_features(world: &World, pos: Vector3<i32>) -> [f32; 8] {
    let visuals = world.visuals_at(pos.x, pos.z);
    let surface = world.surface_height(pos.x, pos.z) as f32;
    // warmth ≈ temperature, moisture ≈ humidity (both 0..1 in SurfaceVisuals).
    features_from_context(surface, visuals.warmth, visuals.moisture)
}
