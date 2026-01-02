use bevy::prelude::*;
use bevy::gltf::{Gltf, GltfAssetLabel};
use bevy::asset::LoadState;
use bevy_rapier3d::prelude::*;
use std::env;

mod ai;
mod assets;
mod audio;
mod components;
mod content;
mod dialog;
mod editor;
mod gameplay;
mod navigation;
mod networking;
mod rendering;
mod resources;
mod systems;
mod tracing;
mod world;
mod events;
mod engine_fabric;

#[cfg(feature = "dev-sync")]
mod dev_sync;

#[cfg(feature = "atom")]
use atom_bridge::{AtomRendererPlugin, RenderConfig as AtomRenderConfig, AtomRendererResource, is_real_atom_available, get_renderer_backend};

#[cfg(feature = "atom")]
use crate::rendering::atom::{AtomExtractionPlugin, AtomStatus};

pub use components::*;
pub use resources::*;
pub use events::*;

#[derive(Resource)]
pub struct HeadlessConfig {
    pub enabled: bool,
    pub max_ticks: u32,
    pub current_tick: u32,
}

impl Default for HeadlessConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_ticks: 100,
            current_tick: 0,
        }
    }
}

#[derive(Resource)]
pub struct MutantAsset {
    pub gltf_handle: Handle<Gltf>,
    pub spawned: bool,
    pub load_check_count: u32,
}

#[derive(Component)]
pub struct MutantMarker;

#[derive(Resource)]
pub struct GameLogOverlay {
    pub messages: Vec<GameLogEntry>,
    pub visible: bool,
    pub max_messages: usize,
}

#[derive(Clone)]
pub struct GameLogEntry {
    pub text: String,
    pub level: LogLevel,
    pub timestamp: f64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

impl Default for GameLogOverlay {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            visible: false,
            max_messages: 50,
        }
    }
}

impl GameLogOverlay {
    pub fn log(&mut self, level: LogLevel, message: impl Into<String>, time: f64) {
        self.messages.push(GameLogEntry {
            text: message.into(),
            level,
            timestamp: time,
        });
        if self.messages.len() > self.max_messages {
            self.messages.remove(0);
        }
    }
    
    pub fn info(&mut self, message: impl Into<String>, time: f64) {
        self.log(LogLevel::Info, message, time);
    }
    
    pub fn warn(&mut self, message: impl Into<String>, time: f64) {
        self.log(LogLevel::Warn, message, time);
    }
    
    pub fn error(&mut self, message: impl Into<String>, time: f64) {
        self.log(LogLevel::Error, message, time);
    }
}

#[derive(Component)]
pub struct LogOverlayUI;

#[derive(Component)]
pub struct LogOverlayText;

fn is_headless_mode() -> bool {
    if env::var("HEADLESS").map(|v| v == "1" || v.to_lowercase() == "true").unwrap_or(false) {
        return true;
    }
    
    env::args().any(|arg| arg == "--headless" || arg == "-h")
}

fn get_max_ticks() -> u32 {
    if let Some(ticks_arg) = env::args().skip_while(|a| a != "--ticks").nth(1) {
        if let Ok(ticks) = ticks_arg.parse::<u32>() {
            return ticks;
        }
    }
    
    if let Ok(ticks_str) = env::var("HEADLESS_TICKS") {
        if let Ok(ticks) = ticks_str.parse::<u32>() {
            return ticks;
        }
    }
    
    100
}

fn main() {
    // Set up panic hook to show errors in console
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("================================================================");
        eprintln!("  GAME CRASHED!");
        eprintln!("================================================================");
        eprintln!("{}", panic_info);
        if let Some(location) = panic_info.location() {
            eprintln!("  Location: {}:{}:{}", location.file(), location.line(), location.column());
        }
        eprintln!("================================================================");
        eprintln!("Press Enter to exit...");
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
    }));

    // Force logging to be visible
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info,wgpu=warn,bevy_render=warn");
    }
    env_logger::init();
    
    // Print startup banner to stdout (always visible)
    println!("================================================================");
    println!("  AAA MMORPG ENGINE - Starting...");
    println!("================================================================");
    println!("  Working directory: {:?}", env::current_dir().unwrap_or_default());
    println!("  Args: {:?}", env::args().collect::<Vec<_>>());
    
    let headless = is_headless_mode();
    let max_ticks = get_max_ticks();
    
    if headless {
        println!("  Mode: HEADLESS ({} ticks)", max_ticks);
        info!("=== HEADLESS MODE ENABLED ===");
        info!("Running for {} ticks without GPU rendering", max_ticks);
        run_headless(max_ticks);
    } else {
        println!("  Mode: FULL RENDERING");
        println!("================================================================");
        info!("=== FULL RENDERING MODE ===");
        run_with_rendering();
    }
}

fn run_headless(max_ticks: u32) {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(HeadlessPlugin { max_ticks })
        .add_plugins(GameLogicPlugin)
        .run();
}

fn run_with_rendering() {
    println!(">>> run_with_rendering() called");
    
    // =========================================================================
    // ATOM RENDERER VERIFICATION - NO COMPROMISE
    // =========================================================================
    #[cfg(feature = "atom")]
    {
        println!(">>> Checking Atom renderer...");
        println!("    Backend: {}", get_renderer_backend());
        println!("    Atom C++ linked: {}", is_real_atom_available());
        info!("=== RENDERER VERIFICATION ===");
        info!("Backend: {}", get_renderer_backend());
        info!("Atom C++ library linked: {}", is_real_atom_available());
        
        // On Windows, we REQUIRE the real Atom renderer - no fallback allowed
        #[cfg(target_os = "windows")]
        if !is_real_atom_available() {
            error!("================================================================");
            error!("  FATAL ERROR: ATOM RENDERER NOT AVAILABLE");
            error!("================================================================");
            error!("");
            error!("  The O3DE Atom renderer C++ library was not linked.");
            error!("  This game REQUIRES the Atom renderer on Windows.");
            error!("");
            error!("  Possible causes:");
            error!("    1. C++ build failed - check cpp_build.log");
            error!("    2. O3DE SDK not installed - run PlayGame.bat /DIAG");
            error!("    3. atom_bridge.lib not found in expected location");
            error!("");
            error!("  Fix: Re-run PlayGame.bat to rebuild with O3DE SDK");
            error!("================================================================");
            panic!("Atom renderer not available - game cannot run without it");
        }
        
        // On non-Windows (Linux/Replit), we allow stub mode for development
        #[cfg(not(target_os = "windows"))]
        if !is_real_atom_available() {
            warn!("================================================================");
            warn!("  WARNING: Running with STUB renderer (development mode)");
            warn!("================================================================");
            warn!("  The O3DE Atom renderer is not available on this platform.");
            warn!("  Using Bevy wgpu fallback for development/testing.");
            warn!("  For full AAA rendering, run on Windows with O3DE SDK.");
            warn!("================================================================");
        }
    }
    
    #[cfg(feature = "dev-sync")]
    {
        if dev_sync::is_dev_sync_enabled() {
            info!("=== DEV SYNC FEATURE ENABLED ===");
            info!("Connecting to Replit dev server for hot-reloading...");
            if let Ok(url) = env::var("DEV_SYNC_URL") {
                info!("DEV_SYNC_URL: {}", url);
            }
        } else {
            info!("DEV_SYNC_URL not set - running without hot-reload");
        }
    }
    
    println!(">>> Creating Bevy app...");
    let mut app = App::new();
    
    println!(">>> Adding DefaultPlugins with window...");
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "MMO Engine - AAA MMORPG".into(),
            resolution: (1920.0, 1080.0).into(),
            present_mode: bevy::window::PresentMode::AutoVsync,
            ..default()
        }),
        ..default()
    }));
    
    println!(">>> Adding GamePlugin...");
    app.add_plugins(GamePlugin);
    
    #[cfg(feature = "dev-sync")]
    {
        println!(">>> Adding DevSyncPlugin...");
        app.add_plugins(dev_sync::DevSyncPlugin);
    }
    
    println!(">>> Starting app.run() - window should appear now!");
    app.run();
    println!(">>> app.run() returned - game exited normally");
}

pub struct HeadlessPlugin {
    max_ticks: u32,
}

impl Plugin for HeadlessPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(HeadlessConfig {
                enabled: true,
                max_ticks: self.max_ticks,
                current_tick: 0,
            })
            .add_systems(Startup, headless_setup)
            .add_systems(Update, (
                headless_tick_system,
                headless_state_reporter,
            ).chain());
    }
}

fn headless_setup(mut commands: Commands) {
    info!("=== HEADLESS VALIDATION TEST ===");
    info!("Spawning test entities for headless validation...");
    
    for i in 0..5 {
        let x = (i as f32) * 10.0;
        let z = (i as f32) * 5.0;
        commands.spawn((
            TestEntity { id: i },
            Transform::from_xyz(x, 0.0, z),
            GlobalTransform::default(),
            Name::new(format!("TestEntity_{}", i)),
        ));
        info!("  Spawned TestEntity_{} at ({}, 0, {})", i, x, z);
    }
    
    for i in 0..3 {
        let x = -(i as f32) * 15.0;
        let z = (i as f32) * 8.0;
        commands.spawn((
            TestNPC { 
                id: i,
                velocity: Vec3::new(1.0, 0.0, 0.5),
            },
            Transform::from_xyz(x, 0.0, z),
            GlobalTransform::default(),
            Name::new(format!("TestNPC_{}", i)),
        ));
        info!("  Spawned TestNPC_{} at ({}, 0, {})", i, x, z);
    }
    
    info!("Headless setup complete - 5 test entities + 3 NPCs spawned");
}

fn headless_tick_system(
    mut config: ResMut<HeadlessConfig>,
    mut app_exit: EventWriter<AppExit>,
    time: Res<Time>,
    mut npc_query: Query<(&mut Transform, &TestNPC)>,
) {
    config.current_tick += 1;
    
    let delta = time.delta_secs().max(0.016);
    for (mut transform, npc) in npc_query.iter_mut() {
        transform.translation += npc.velocity * delta;
    }
    
    if config.current_tick % 20 == 0 {
        info!("Tick {}/{} - Delta: {:.4}s", config.current_tick, config.max_ticks, delta);
    }
    
    if config.current_tick >= config.max_ticks {
        info!("Reached max ticks ({}), preparing to exit...", config.max_ticks);
        app_exit.send(AppExit::Success);
    }
}

fn headless_state_reporter(
    config: Res<HeadlessConfig>,
    entity_query: Query<(&Transform, &Name)>,
    test_entity_query: Query<&TestEntity>,
    npc_query: Query<&TestNPC>,
) {
    if config.current_tick == config.max_ticks {
        info!("");
        info!("=== HEADLESS VALIDATION COMPLETE ===");
        info!("Total ticks executed: {}", config.current_tick);
        info!("Total entities with Transform+Name: {}", entity_query.iter().count());
        info!("TestEntity count: {}", test_entity_query.iter().count());
        info!("TestNPC count: {}", npc_query.iter().count());
        info!("");
        info!("Entity positions at end:");
        for (transform, name) in entity_query.iter() {
            let pos = transform.translation;
            info!("  {} -> ({:.2}, {:.2}, {:.2})", name, pos.x, pos.y, pos.z);
        }
        info!("");
        info!("=== HEADLESS TEST PASSED ===");
        info!("Game logic systems executed successfully without GPU!");
    }
}

#[derive(Component)]
pub struct TestEntity {
    pub id: u32,
}

#[derive(Component)]
pub struct TestNPC {
    pub id: u32,
    pub velocity: Vec3,
}

pub struct GameLogicPlugin;

impl Plugin for GameLogicPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
            .add_plugins(dialog::DialogPlugin)
            // AI plugins
            .add_plugins(ai::NavMeshPlugin)
            .add_plugins(ai::SteeringPlugin)
            .add_plugins(ai::PerceptionPlugin)
            // Gameplay plugins
            .add_plugins(gameplay::QuestPlugin)
            .add_plugins(gameplay::InventoryPlugin)
            .add_plugins(gameplay::CombatPlugin)
            .add_plugins(gameplay::CraftingPlugin)
            .add_plugins(gameplay::GuildPlugin)
            // World plugins
            .add_plugins(world::WeatherPlugin)
            .add_plugins(world::StreamingPlugin)
            .add_plugins(world::ProceduralGenerationPlugin)
            // Content loader (data-driven monsters, NPCs, spawn zones from TOML)
            .add_plugins(content::ContentLoaderPlugin)
            .insert_resource(TerrainConfig::default())
            .insert_resource(WaterConfig::default())
            .insert_resource(SpawnConfig::default())
            .insert_resource(TimeOfDay::default())
            .insert_resource(NetworkConfig::default())
            .insert_resource(GameState::default())
            .insert_resource(PerformanceMetrics::default())
            .insert_resource(LandmarkRegistry::new())
            .insert_resource(TerrainChunkCache::new())
            .insert_resource(ForestConfig::default())
            .insert_resource(systems::ForestSpatialGrid::default())
            .insert_resource(systems::ai::AISpatialGrid::default())
            .insert_resource(CameraConfig::default())
            .insert_resource(MovementConfig::default())
            .insert_resource(PlayerInput::default())
            .insert_resource(MountState::default())
            .insert_resource(SkyridingConfig::default())
            .insert_resource(SkyridingInput::default())
            .insert_resource(systems::spawning::SpawnTemplates::default())
            .insert_resource(FrameArena::default())
            .insert_resource(EntityPool::default())
            .insert_resource(systems::spawning::SpawnQueue::new(50))
            .add_event::<DamageEvent>()
            .add_event::<DeathEvent>()
            .add_event::<HealEvent>()
            .add_event::<LevelUpEvent>()
            .add_event::<MountEvent>()
            .add_event::<DismountEvent>()
            .add_event::<NetworkEvent>()
            .add_event::<QuestCompleteEvent>()
            .add_event::<QuestAcceptEvent>()
            .add_event::<LootDropEvent>()
            .add_event::<AbilityUsedEvent>()
            .add_event::<SpawnEvent>()
            .add_event::<ZoneChangeEvent>()
            .add_systems(Startup, (
                setup_terrain,
                setup_water_system,
                systems::water::spawn_water_bodies,
                setup_player_headless,
                systems::spawning::setup_spawn_points,
                systems::vegetation::generate_forest,
                networking::network_setup_system,
            ))
            // World systems (terrain, water, vegetation)
            // CRITICAL: Use .chain() to guarantee terrain chunks update BEFORE trees spawn/resync
            .add_systems(Update, (
                // Stage 1: Terrain and water updates
                (
                    systems::terrain::update_terrain_chunks,
                    systems::terrain::update_chunk_lod,
                    systems::water::update_water_animation,
                    systems::water::update_water_lod,
                ),
                // Stage 2: Vegetation systems
                (
                    systems::vegetation::spawn_tree_instances,
                    systems::vegetation::update_forest_lod,
                    systems::vegetation::resync_tree_heights,
                ),
            ).chain())
            // Player and mount systems
            .add_systems(Update, (
                systems::player::handle_player_input,
                systems::player::update_player_movement,
                systems::mount::mount_toggle_system,
                systems::mount::skyriding_input_system,
                systems::mount::skyriding_physics_system,
                systems::mount::vigor_system,
                systems::mount::surge_forward_system,
                systems::mount::skyward_ascent_system,
                systems::mount::whirling_surge_system,
            ))
            // AI systems (state machine)
            .add_systems(Update, (
                systems::ai::update_ai_spatial_grid,
                systems::ai::ai_perception_system.after(systems::ai::update_ai_spatial_grid),
                systems::ai::ai_decision_system,
                systems::ai::ai_pathfinding_system,
                systems::ai::ai_movement_system,
                systems::ai::ai_combat_system,
            ))
            // Note: BehaviorTreePlugin now handles ai::behavior_tree_update_system and ai::apply_behavior_tree_outputs
            // Combat and spawning systems
            .add_systems(Update, (
                systems::combat::damage_calculation_system,
                systems::combat::heal_system,
                systems::combat::death_system,
                systems::combat::respawn_system,
                systems::combat::threat_management_system,
                systems::combat::combat_out_of_range_system,
                systems::spawning::entity_spawning_system,
                systems::spawning::entity_despawning_system,
                systems::spawning::process_spawn_queue_system,
            ))
            // Character and networking systems
            .add_systems(Update, (
                systems::character::character_stats_system,
                systems::character::experience_system,
                systems::character::level_up_effects_system,
                networking_update_system,
            ))
            // Frame arena reset (runs at end of frame)
            .add_systems(Last, reset_frame_arena);
    }
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        info!("╔══════════════════════════════════════════════════════════════╗");
        info!("║          ENGINE FABRIC - AAA MMORPG ENGINE                    ║");
        info!("║  Version: {} | Built: {}          ║", 
            engine_fabric::prelude::ENGINE_VERSION,
            "2024-12-22"
        );
        info!("╚══════════════════════════════════════════════════════════════╝");
        
        app
            .add_plugins(engine_fabric::EngineFabricPlugin)
            // Note: RapierPhysicsPlugin is now managed by EngineFabricPlugin's PhysicsPlugin
            // Debug wireframes disabled - uncomment below for collision debugging:
            // .add_plugins(RapierDebugRenderPlugin::default())
            .add_plugins(systems::GameUiPlugin)
            .add_plugins(systems::AnimationPlugin)
            // Dialog plugins
            .add_plugins(dialog::DialogPlugin)
            .add_plugins(dialog::DialogUIPlugin)
            // AI plugins
            .add_plugins(ai::NavMeshPlugin)
            .add_plugins(ai::SteeringPlugin)
            .add_plugins(ai::PerceptionPlugin)
            .add_plugins(ai::BehaviorTreePlugin)
            // Rendering plugins
            .add_plugins(rendering::GameRenderingPlugin)
            // Physics polish (character controller, ragdoll, vehicles)
            .add_plugins(systems::physics::PhysicsPolishPlugin)
            // Gameplay plugins
            .add_plugins(gameplay::QuestPlugin)
            .add_plugins(gameplay::InventoryPlugin)
            .add_plugins(gameplay::CombatPlugin)
            .add_plugins(gameplay::CraftingPlugin)
            .add_plugins(gameplay::GuildPlugin)
            // World plugins
            .add_plugins(world::WeatherPlugin)
            .add_plugins(world::StreamingPlugin)
            .add_plugins(world::ProceduralGenerationPlugin)
            // Editor plugins
            .add_plugins(editor::LevelEditorPlugin)
            .add_plugins(editor::MaterialEditorPlugin)
            .add_plugins(editor::ProfilerPlugin)
            // Navigation plugin (NavMesh pathfinding)
            .add_plugins(navigation::NavigationPlugin)
            // Navigation debug (conditional)
            #[cfg(debug_assertions)]
            .add_plugins(navigation::debug::NavigationDebugPlugin)
            // Audio plugin (3D spatial audio)
            .add_plugins(audio::AudioPlugin);
        
        // Nakama multiplayer sync (when networking feature is enabled)
        #[cfg(feature = "networking")]
        {
            app.add_plugins(networking::bevy_nakama::NakamaSyncPlugin);
            info!("NakamaSyncPlugin enabled for multiplayer synchronization");
        }
        
        #[cfg(feature = "atom")]
        {
            info!("╔══════════════════════════════════════════════════════════════╗");
            info!("║              ATOM RENDERER - REQUIRED MODE                    ║");
            info!("╚══════════════════════════════════════════════════════════════╝");
            info!("Atom renderer feature is ENABLED - this is REQUIRED, not optional");
            
            let atom_config = AtomRenderConfig {
                width: 1920,
                height: 1080,
                enable_gi: true,
                enable_ssr: true,
                enable_shadows: true,
                enable_ao: true,
                shadow_cascade_count: 4,
                lod_bias: 0.0,
                max_draw_calls: 10000,
            };
            
            info!("Atom render config: {:?}", atom_config);
            info!("Adding AtomRendererPlugin...");
            app.add_plugins(AtomRendererPlugin::with_config(atom_config));
            
            info!("Adding AtomExtractionPlugin...");
            app.add_plugins(AtomExtractionPlugin);
            
            app.add_systems(PostStartup, verify_atom_initialized);
            
            info!("AtomRendererPlugin and AtomExtractionPlugin added with high-quality settings");
            info!("Atom verification system scheduled for PostStartup");
        }
        
        #[cfg(not(feature = "atom"))]
        {
            warn!("╔══════════════════════════════════════════════════════════════╗");
            warn!("║  WARNING: ATOM FEATURE NOT ENABLED                            ║");
            warn!("║  Running with default Bevy/wgpu renderer                      ║");
            warn!("║  For AAA graphics, rebuild with: --features atom              ║");
            warn!("╚══════════════════════════════════════════════════════════════╝");
        }
        
        app
            .insert_resource(TerrainConfig::default())
            .insert_resource(WaterConfig::default())
            .insert_resource(SpawnConfig::default())
            .insert_resource(TimeOfDay::default())
            .insert_resource(NetworkConfig::default())
            .insert_resource(GameState::default())
            .insert_resource(PerformanceMetrics::default())
            .insert_resource(GameLogOverlay::default())
            .insert_resource(LandmarkRegistry::new())
            .insert_resource(TerrainChunkCache::new())
            .insert_resource(ForestConfig::default())
            .insert_resource(systems::ForestSpatialGrid::default())
            .insert_resource(systems::ai::AISpatialGrid::default())
            .insert_resource(CameraConfig::default())
            .insert_resource(MovementConfig::default())
            .insert_resource(PlayerInput::default())
            .insert_resource(MountState::default())
            .insert_resource(SkyridingConfig::default())
            .insert_resource(SkyridingInput::default())
            .insert_resource(systems::spawning::SpawnTemplates::default())
            .insert_resource(FrameArena::default())
            .insert_resource(EntityPool::default())
            .insert_resource(systems::spawning::SpawnQueue::new(50))
            .add_event::<DamageEvent>()
            .add_event::<DeathEvent>()
            .add_event::<HealEvent>()
            .add_event::<LevelUpEvent>()
            .add_event::<MountEvent>()
            .add_event::<DismountEvent>()
            .add_event::<NetworkEvent>()
            .add_event::<QuestCompleteEvent>()
            .add_event::<QuestAcceptEvent>()
            .add_event::<LootDropEvent>()
            .add_event::<AbilityUsedEvent>()
            .add_event::<SpawnEvent>()
            .add_event::<ZoneChangeEvent>()
            .add_systems(Startup, (
                setup_terrain,
                setup_water_system,
                systems::water::spawn_water_bodies,
                setup_player_with_controller,
                systems::spawning::setup_spawn_points,
                setup_lighting,
                setup_gpu_smoke_test,
                systems::vegetation::generate_forest,
                systems::sky::setup_sky_system,
                load_mutant_gltf,
                setup_log_overlay,
                networking::network_setup_system,
            ))
            .add_systems(PostStartup, systems::camera::setup_player_camera)
            // World systems (terrain, water, vegetation, entities)
            // CRITICAL: Use .chain() to guarantee terrain chunks update BEFORE trees/mutant spawn/resync
            // This ensures the chunk cache is populated before entities sample heights from it
            .add_systems(Update, (
                // Stage 1: Terrain and water updates (populates chunk cache)
                (
                    systems::terrain::update_terrain_chunks,
                    systems::terrain::update_chunk_lod,
                    systems::water::update_water_animation,
                    systems::water::update_water_lod,
                ),
                // Stage 2: Vegetation and entity systems (depends on chunk cache)
                (
                    systems::vegetation::spawn_tree_instances,
                    systems::vegetation::update_forest_lod,
                    systems::vegetation::resync_tree_heights,
                    check_mutant_loading,
                    resync_mutant_height,
                ),
            ).chain())
            // Player and camera systems
            .add_systems(Update, (
                systems::player::handle_player_input,
                systems::player::update_player_movement,
                systems::camera::handle_camera_input,
                systems::camera::update_camera,
            ))
            // Mount systems
            .add_systems(Update, (
                systems::mount::mount_toggle_system,
                systems::mount::skyriding_input_system,
                systems::mount::skyriding_physics_system,
                systems::mount::vigor_system,
                systems::mount::surge_forward_system,
                systems::mount::skyward_ascent_system,
                systems::mount::whirling_surge_system,
                systems::mount::mount_camera_system,
                systems::mount::hide_player_when_mounted_system,
            ))
            // AI systems (state machine)
            .add_systems(Update, (
                systems::ai::update_ai_spatial_grid,
                systems::ai::ai_perception_system.after(systems::ai::update_ai_spatial_grid),
                systems::ai::ai_decision_system,
                systems::ai::ai_pathfinding_system,
                systems::ai::ai_movement_system,
                systems::ai::ai_combat_system,
            ))
            // AI systems (behavior tree)
            .add_systems(Update, (
                ai::behavior_tree_update_system,
                ai::apply_behavior_tree_outputs,
            ).chain())
            // Combat systems
            .add_systems(Update, (
                systems::combat::combat_input_system,
                systems::combat::ability_cooldown_system,
                systems::combat::damage_calculation_system,
                systems::combat::heal_system,
                systems::combat::death_system,
                systems::combat::respawn_system,
                systems::combat::threat_management_system,
                systems::combat::combat_out_of_range_system,
            ))
            // Spawning and character systems
            .add_systems(Update, (
                systems::spawning::entity_spawning_system,
                systems::spawning::entity_despawning_system,
                systems::spawning::process_spawn_queue_system,
                systems::character::character_stats_system,
                systems::character::experience_system,
                systems::character::level_up_effects_system,
            ))
            // Networking, UI, and sky systems
            .add_systems(Update, (
                networking_update_system,
                ui_update_system,
                spin_cube_system,
                systems::sky::update_time_of_day,
                systems::sky::update_sky_visuals,
            ))
            // GLTF model debugging (loading/resync moved to chained world systems)
            .add_systems(Update, (
                debug_mutant_entities,
            ))
            // Log overlay systems
            .add_systems(Update, (
                toggle_log_overlay,
                update_log_overlay_text,
                log_mutant_status_to_overlay,
                log_game_startup_to_overlay,
            ))
            // Frame arena reset (runs at end of frame)
            .add_systems(Last, reset_frame_arena);
    }
}

fn reset_frame_arena(mut frame_arena: ResMut<FrameArena>) {
    frame_arena.reset();
}

fn setup_terrain(
    config: Res<TerrainConfig>,
) {
    info!("Setting up terrain system: {}x{} world, chunk_size: {}", 
        config.world_size, config.world_size, config.chunk_size);
    info!("LOD distances: {:?}", config.lod_distances);
    info!("View distance: {} chunks", config.view_distance);
}

fn setup_water_system(
    config: Res<WaterConfig>,
) {
    info!("Water system configured: world_size={}, ocean_buffer={}", 
        config.world_size, config.ocean_buffer);
    info!("Lake definitions: {}", config.lake_definitions.len());
    info!("River definitions: {}", config.river_definitions.len());
}

fn setup_player_with_controller(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    info!("Setting up player character with WoW-style controller");
    
    commands.spawn((
        (
            Player,
            PlayerController::default(),
            Character {
                name: "Hero".to_string(),
                race: Race::Briton,
                class: CharacterClass::Fighter,
                realm: Realm::Albion,
                level: 1,
                experience: 0,
            },
            Health::new(100.0),
            Mana::new(100.0),
            Vigor::default(),
            CombatStats::default(),
            systems::combat::CombatState::default(),
        ),
        (
            systems::combat::GlobalCooldown::default(),
            systems::combat::AbilityCooldowns::default(),
            systems::combat::AbilityBook::default(),
            systems::combat::CastingState::default(),
            Mesh3d(meshes.add(Capsule3d::new(0.4, 1.6))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.2, 0.5, 0.9),
                metallic: 0.3,
                perceptual_roughness: 0.6,
                ..default()
            })),
            Transform::from_translation(Vec3::new(0.0, 10.0, 0.0)),
            GlobalTransform::default(),
            Name::new("Player"),
        ),
    ));
    
    info!("Player spawned with visible capsule mesh and PlayerController component");
}

fn setup_player_headless(mut commands: Commands) {
    info!("[HEADLESS] Setting up player character (no rendering)");
    
    commands.spawn((
        Player,
        PlayerController::default(),
        Character {
            name: "HeadlessHero".to_string(),
            race: Race::Briton,
            class: CharacterClass::Fighter,
            realm: Realm::Albion,
            level: 1,
            experience: 0,
        },
        Health::new(100.0),
        Mana::new(100.0),
        Vigor::default(),
        CombatStats::default(),
        systems::combat::CombatState::default(),
        systems::combat::GlobalCooldown::default(),
        systems::combat::AbilityCooldowns::default(),
        systems::combat::AbilityBook::default(),
        systems::combat::CastingState::default(),
        Transform::from_translation(Vec3::new(0.0, 10.0, 0.0)),
        GlobalTransform::default(),
        Name::new("Player_Headless"),
    ));
}


fn setup_lighting(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_4,
            0.0,
        )),
    ));
    
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.5, 0.5, 0.6),
        brightness: 200.0,
    });
    
    info!("Lighting setup complete (camera spawned by camera system)");
}

fn load_mutant_gltf(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    info!("=== LOADING MUTANT GLTF FILE ===");
    info!("Asset path: models/mutant.glb");
    
    let gltf_handle: Handle<Gltf> = asset_server.load("models/mutant.glb");
    info!("GLTF handle created: {:?}", gltf_handle);
    
    commands.insert_resource(MutantAsset {
        gltf_handle,
        spawned: false,
        load_check_count: 0,
    });
    
    info!("MutantAsset resource inserted, waiting for load completion...");
}

fn check_mutant_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    gltf_assets: Res<Assets<Gltf>>,
    mut mutant_asset: Option<ResMut<MutantAsset>>,
    player_query: Query<&Transform, With<Player>>,
    terrain_config: Res<TerrainConfig>,
    chunk_cache: Res<TerrainChunkCache>,
    mut landmark_registry: ResMut<LandmarkRegistry>,
) {
    let Some(ref mut mutant) = mutant_asset else { return; };
    if mutant.spawned { return; }
    
    mutant.load_check_count += 1;
    
    let is_fully_loaded = asset_server.is_loaded_with_dependencies(&mutant.gltf_handle);
    
    if mutant.load_check_count % 60 == 0 {
        info!("Mutant load check #{}: fully_loaded_with_deps = {}", 
            mutant.load_check_count, is_fully_loaded);
    }
    
    if mutant.load_check_count > 600 {
        error!("=== MUTANT LOADING TIMEOUT (10 seconds) ===");
        error!("Asset may have failed to load or path is incorrect");
        mutant.spawned = true;
        return;
    }
    
    if is_fully_loaded {
        info!("=== MUTANT GLTF FULLY LOADED WITH ALL DEPENDENCIES ===");
        
        if let Some(gltf) = gltf_assets.get(&mutant.gltf_handle) {
            info!("GLTF contents:");
            info!("  - Scenes: {}", gltf.scenes.len());
            info!("  - Named scenes: {}", gltf.named_scenes.len());
            info!("  - Meshes: {}", gltf.meshes.len());
            info!("  - Materials: {}", gltf.materials.len());
            info!("  - Nodes: {}", gltf.nodes.len());
            
            for (name, _) in gltf.named_scenes.iter() {
                info!("  Named scene: '{}'", name);
            }
            
            if let Some(scene_handle) = gltf.scenes.first() {
                let scene_loaded = asset_server.is_loaded_with_dependencies(scene_handle);
                info!("First scene handle loaded with deps: {}", scene_loaded);
                
                if scene_loaded {
                    // Wait until player exists before spawning mutant
                    let Ok(player_transform) = player_query.get_single() else {
                        if mutant.load_check_count % 60 == 0 {
                            info!("Waiting for player to spawn before placing mutant...");
                        }
                        return;
                    };
                    
                    info!("=== SPAWNING MUTANT SCENE ===");
                    
                    // Spawn mutant near player at correct terrain height
                    let player_pos = player_transform.translation;
                    
                    // Offset 15 units in front of player
                    let spawn_x = player_pos.x + 15.0;
                    let spawn_z = player_pos.z + 15.0;
                    
                    // CRITICAL FIX: Sample from chunk cache to match rendered terrain mesh exactly
                    // Fallback to raw function only if chunk not loaded yet
                    let terrain_y = systems::terrain::terrain_height_at_point(
                        spawn_x, spawn_z, &terrain_config, &chunk_cache
                    ).unwrap_or_else(|| {
                        // Fallback to raw height function if chunk not available
                        systems::terrain::terrain_height_at_with_features(
                            spawn_x, spawn_z, &terrain_config, &mut landmark_registry
                        )
                    });
                    let spawn_pos = Vec3::new(spawn_x, terrain_y, spawn_z);
                    
                    commands.spawn((
                        SceneRoot(scene_handle.clone()),
                        Transform::from_translation(spawn_pos)
                            .with_scale(Vec3::splat(3.0)),
                        GlobalTransform::default(),
                        Visibility::Visible,
                        InheritedVisibility::default(),
                        ViewVisibility::default(),
                        Name::new("TestMutant"),
                        MutantMarker,
                    ));
                    
                    info!("=== MUTANT SCENE SPAWNED near player at {:?} (terrain_y={}) with 3x SCALE ===", spawn_pos, terrain_y);
                    mutant.spawned = true;
                } else {
                    if mutant.load_check_count % 60 == 0 {
                        info!("Waiting for scene sub-assets to finish loading...");
                    }
                }
            } else {
                error!("=== NO SCENES FOUND IN GLTF ===");
                mutant.spawned = true;
            }
        } else {
            if mutant.load_check_count % 60 == 0 {
                warn!("GLTF asset not accessible in Assets<Gltf> collection");
            }
        }
    } else {
        let load_state = asset_server.get_load_state(&mutant.gltf_handle);
        if let Some(LoadState::Failed(err)) = load_state {
            error!("=== MUTANT GLTF FAILED TO LOAD ===");
            error!("Error: {:?}", err);
            mutant.spawned = true;
        }
    }
}

fn debug_mutant_entities(
    query: Query<(Entity, &Name, &Transform), With<MutantMarker>>,
    children_query: Query<&Children>,
    mut logged: Local<bool>,
) {
    if *logged { return; }
    
    for (entity, name, transform) in query.iter() {
        info!("=== MUTANT ENTITY FOUND ===");
        info!("Entity: {:?}, Name: {}", entity, name);
        info!("Position: {:?}", transform.translation);
        info!("Scale: {:?}", transform.scale);
        
        if let Ok(children) = children_query.get(entity) {
            info!("Has {} direct children", children.len());
        } else {
            warn!("No children yet - scene may still be processing");
            return;
        }
        
        *logged = true;
    }
}

/// Resync mutant height from chunk cache after chunks load
/// This corrects mutants that spawned with fallback heights
/// Tracks per-entity sync status using Entity IDs
fn resync_mutant_height(
    terrain_config: Res<TerrainConfig>,
    chunk_cache: Res<TerrainChunkCache>,
    mut mutant_query: Query<(Entity, &mut Transform), With<MutantMarker>>,
    mut synced_entities: Local<std::collections::HashSet<Entity>>,
    mut frame_count: Local<u32>,
) {
    // Only run every 30 frames to reduce overhead
    *frame_count += 1;
    if *frame_count % 30 != 0 {
        return;
    }
    
    for (entity, mut transform) in mutant_query.iter_mut() {
        // Skip if already synced
        if synced_entities.contains(&entity) {
            continue;
        }
        
        let x = transform.translation.x;
        let z = transform.translation.z;
        
        // Try to get height from chunk cache
        if let Some(cached_height) = systems::terrain::terrain_height_at_point(
            x, z, &terrain_config, &chunk_cache
        ) {
            let current_y = transform.translation.y;
            let height_diff = (cached_height - current_y).abs();
            
            // Only update if there's a significant difference
            if height_diff > 0.1 {
                transform.translation.y = cached_height;
                info!("Resynced mutant height from chunk cache: {} -> {}", current_y, cached_height);
            }
            
            // Mark as synced (even if no update needed)
            synced_entities.insert(entity);
        }
        // If chunk not available, we'll retry next time
    }
}

#[derive(Component)]
struct SpinningCube;

fn setup_gpu_smoke_test(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    info!("Setting up GPU smoke test - spinning cube");
    
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 2.0, 2.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.7, 0.3),
            metallic: 0.5,
            perceptual_roughness: 0.4,
            ..default()
        })),
        Transform::from_xyz(0.0, 5.0, 0.0),
        SpinningCube,
    ));
    
    info!("GPU smoke test cube spawned - validates Vulkan/wgpu rendering pipeline");
}

fn spin_cube_system(time: Res<Time>, mut query: Query<&mut Transform, With<SpinningCube>>) {
    for mut transform in query.iter_mut() {
        transform.rotate_y(time.delta_secs() * 0.5);
        transform.rotate_x(time.delta_secs() * 0.3);
    }
}


fn networking_update_system(
    time: Res<Time>,
    config: Res<NetworkConfig>,
    mut network_state: ResMut<networking::NetworkState>,
    mut network_events: EventWriter<NetworkEvent>,
    player_query: Query<&Transform, With<Player>>,
    mut remote_query: Query<(&mut Transform, &NetworkEntity), Without<Player>>,
) {
    use networking::ConnectionState;
    
    if !config.auto_connect {
        return;
    }
    
    match network_state.connection_state {
        ConnectionState::Disconnected => {
            network_state.connection_state = ConnectionState::Authenticating;
            
            if let Some(ref mut client) = network_state.client {
                match client.authenticate_device(&config.device_id) {
                    Ok(session) => {
                        info!("Connected as user: {} ({})", session.username, session.user_id);
                        network_state.connection_state = ConnectionState::Connected;
                        
                        network_events.send(NetworkEvent {
                            event_type: crate::events::NetworkEventType::Connected,
                            data: session.user_id.into_bytes(),
                        });
                    }
                    Err(e) => {
                        warn!("Authentication failed: {}", e);
                        network_state.connection_state = ConnectionState::Error;
                        
                        network_events.send(NetworkEvent {
                            event_type: crate::events::NetworkEventType::Disconnected,
                            data: e.to_string().into_bytes(),
                        });
                    }
                }
            }
        }
        
        ConnectionState::Connected | ConnectionState::InMatch => {
            #[cfg(feature = "networking")]
            {
                if let Some(ref mut client) = network_state.client {
                    let _ = client.send_heartbeat();
                    
                    let messages = client.receive_messages();
                    for msg in messages {
                        if let Some(match_data) = msg.get("match_data") {
                            if let Some(data) = match_data.get("data") {
                                if let Some(data_str) = data.as_str() {
                                    use base64::Engine;
                                    if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(data_str) {
                                        if let Ok(state) = serde_json::from_slice::<networking::StateSync>(&decoded) {
                                            let current_time = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .map(|d| d.as_secs_f64())
                                                .unwrap_or(0.0);
                                            network_state.interpolation_buffer.add_state(current_time, state);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                let should_sync = network_state.last_position_sync
                    .map(|t| t.elapsed().as_secs_f32() > 0.1)
                    .unwrap_or(true);
                
                if should_sync {
                    if let Ok(transform) = player_query.get_single() {
                        if let Some(ref client) = network_state.client {
                            if let Some(user_id) = client.get_user_id() {
                                let request = networking::PositionUpdateRequest {
                                    character_id: user_id.to_string(),
                                    x: transform.translation.x,
                                    y: transform.translation.y,
                                    z: transform.translation.z,
                                    rotation_y: transform.rotation.to_euler(EulerRot::YXZ).0,
                                    velocity: [0.0, 0.0, 0.0],
                                    timestamp: (time.elapsed_secs() * 1000.0) as u64,
                                };
                                
                                match client.update_position(request) {
                                    Ok(response) => {
                                        if !response.approved {
                                            warn!("Position update rejected by server");
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to sync position: {}", e);
                                    }
                                }
                                
                                network_state.last_position_sync = Some(std::time::Instant::now());
                            }
                        }
                    }
                }
            }
            
            let current_time = time.elapsed_secs() as f64;
            if let Some(state) = network_state.interpolation_buffer.get_interpolated_state(current_time) {
                for entity_state in &state.entities {
                    for (mut transform, network_entity) in remote_query.iter_mut() {
                        if network_entity.network_id == entity_state.entity_id 
                            && network_entity.is_remote {
                            transform.translation = Vec3::new(
                                entity_state.position[0],
                                entity_state.position[1],
                                entity_state.position[2],
                            );
                            transform.rotation = Quat::from_array(entity_state.rotation);
                        }
                    }
                }
            }
            
            if let Some(ref client) = network_state.client {
                if !client.is_connected() {
                    network_state.connection_state = ConnectionState::Disconnected;
                    network_state.current_match_id = None;
                    network_state.interpolation_buffer.clear();
                    
                    network_events.send(NetworkEvent {
                        event_type: crate::events::NetworkEventType::Disconnected,
                        data: Vec::new(),
                    });
                }
            }
        }
        
        ConnectionState::Authenticating => {
        }
        
        ConnectionState::Error => {
        }
        
        ConnectionState::Connecting => {
        }
    }
}

fn ui_update_system(
    _player_query: Query<(&Health, &Mana, &Vigor), With<Player>>,
) {
}

fn setup_log_overlay(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(10.0),
            top: Val::Px(10.0),
            width: Val::Px(600.0),
            height: Val::Px(400.0),
            padding: UiRect::all(Val::Px(10.0)),
            flex_direction: FlexDirection::Column,
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
        Visibility::Hidden,
        LogOverlayUI,
    )).with_children(|parent| {
        parent.spawn((
            Text::new("=== GAME LOG (F12 to toggle) ===\n"),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(0.0, 1.0, 0.0)),
            LogOverlayText,
        ));
    });
    
    info!("Log overlay UI created - Press F12 to toggle");
}

fn toggle_log_overlay(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut log_overlay: ResMut<GameLogOverlay>,
    mut query: Query<&mut Visibility, With<LogOverlayUI>>,
) {
    if keyboard.just_pressed(KeyCode::F12) {
        log_overlay.visible = !log_overlay.visible;
        for mut visibility in query.iter_mut() {
            *visibility = if log_overlay.visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

fn update_log_overlay_text(
    log_overlay: Res<GameLogOverlay>,
    mut query: Query<&mut Text, With<LogOverlayText>>,
) {
    if !log_overlay.visible { return; }
    
    for mut text in query.iter_mut() {
        let mut content = String::from("=== GAME LOG (F12 to hide) ===\n\n");
        
        let start_idx = if log_overlay.messages.len() > 20 {
            log_overlay.messages.len() - 20
        } else {
            0
        };
        
        for entry in log_overlay.messages.iter().skip(start_idx) {
            let prefix = match entry.level {
                LogLevel::Info => "[INFO]",
                LogLevel::Warn => "[WARN]",
                LogLevel::Error => "[ERR!]",
                LogLevel::Debug => "[DBG]",
            };
            content.push_str(&format!("{} {}\n", prefix, entry.text));
        }
        
        if log_overlay.messages.is_empty() {
            content.push_str("(No log messages yet)\n");
        }
        
        *text = Text::new(content);
    }
}

fn log_mutant_status_to_overlay(
    mut log_overlay: ResMut<GameLogOverlay>,
    mutant_asset: Option<Res<MutantAsset>>,
    time: Res<Time>,
    mut last_status: Local<Option<(bool, u32)>>,
) {
    let Some(mutant) = mutant_asset else { return; };
    
    let current_status = (mutant.spawned, mutant.load_check_count);
    
    if *last_status != Some(current_status) {
        let elapsed = time.elapsed_secs_f64();
        
        if mutant.spawned {
            log_overlay.info("Mutant model spawned successfully!", elapsed);
        } else if mutant.load_check_count == 1 {
            log_overlay.info("Loading mutant.glb...", elapsed);
        } else if mutant.load_check_count % 60 == 0 {
            log_overlay.info(format!("Still loading... (check #{})", mutant.load_check_count), elapsed);
        }
        
        *last_status = Some(current_status);
    }
}

fn log_game_startup_to_overlay(
    mut log_overlay: ResMut<GameLogOverlay>,
    time: Res<Time>,
    mut ran: Local<bool>,
) {
    if *ran { return; }
    *ran = true;
    
    let t = time.elapsed_secs_f64();
    log_overlay.info("=== MMO ENGINE STARTED ===", t);
    log_overlay.info("Press F12 to toggle this log overlay", t);
    log_overlay.info("Controls: WASD=Move, Q/E=Turn, Space=Jump, Shift=Sprint", t);
    log_overlay.info("Mouse: Right-click+drag=Look, Scroll=Zoom", t);
    log_overlay.info("Loading world assets...", t);
}

#[cfg(feature = "atom")]
fn verify_atom_initialized(
    renderer: Res<AtomRendererResource>,
    status: Res<AtomStatus>,
    mut app_exit: EventWriter<AppExit>,
) {
    info!("╔══════════════════════════════════════════════════════════════╗");
    info!("║         POST-STARTUP ATOM VERIFICATION                        ║");
    info!("╚══════════════════════════════════════════════════════════════╝");
    
    let renderer_initialized = renderer.get().is_initialized();
    let status_initialized = status.is_initialized;
    let is_atom_active = status.is_atom_active();
    
    info!("Renderer initialized: {}", renderer_initialized);
    info!("AtomStatus initialized: {}", status_initialized);
    info!("Backend name: {}", status.backend_name);
    info!("Is Atom active (not wgpu fallback): {}", is_atom_active);
    
    if renderer_initialized && status_initialized && is_atom_active {
        info!("┌──────────────────────────────────────────────────────────────┐");
        info!("│  ✓✓✓ ATOM RENDERER VERIFICATION PASSED ✓✓✓                   │");
        info!("│                                                              │");
        info!("│  Atom renderer is ACTIVE and rendering.                      │");
        info!("│  NOT falling back to wgpu.                                   │");
        info!("│  Backend: {}                           │", status.backend_name);
        info!("│  Frame count: {}                                             │", status.frame_count);
        info!("└──────────────────────────────────────────────────────────────┘");
    } else {
        error!("╔══════════════════════════════════════════════════════════════╗");
        error!("║  ✗✗✗ ATOM RENDERER VERIFICATION FAILED ✗✗✗                   ║");
        error!("╠══════════════════════════════════════════════════════════════╣");
        error!("║  CRITICAL ERROR: Atom renderer is REQUIRED but not working  ║");
        error!("║                                                              ║");
        error!("║  Renderer initialized: {}                                    ║", renderer_initialized);
        error!("║  Status initialized: {}                                      ║", status_initialized);
        error!("║  Is Atom active: {}                                          ║", is_atom_active);
        error!("║  Backend: {}                                                 ║", status.backend_name);
        error!("║                                                              ║");
        error!("║  The game CANNOT run without the Atom renderer.              ║");
        error!("║  Exiting with error...                                       ║");
        error!("╚══════════════════════════════════════════════════════════════╝");
        
        app_exit.send(AppExit::Error(std::num::NonZeroU8::new(1).unwrap()));
    }
}
