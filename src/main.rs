#![deny(warnings)]
#![forbid(unsafe_code)]

use bevy::a11y::AccessibilityPlugin;
use bevy::app::{App, AppExit, TaskPoolOptions, TaskPoolPlugin, Update};
use bevy::asset::{AssetPlugin, Assets, Handle};
use bevy::camera::{Camera, Camera2d, CameraPlugin, ClearColorConfig};
use bevy::color::Color;
use bevy::core_pipeline::CorePipelinePlugin;
use bevy::diagnostic::FrameCountPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Local, Query, Res, ResMut};
use bevy::image::{
    ktx2_buffer_to_image, CompressedImageFormatSupport, CompressedImageFormats, Image, ImagePlugin,
    ImageSampler,
};
use bevy::input::keyboard::KeyCode;
use bevy::input::{ButtonInput, InputPlugin};
use bevy::input_focus::InputFocusPlugin;
use bevy::math::{Vec2, Vec3};
use bevy::mesh::MeshPlugin;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::render::settings::{Backends, PowerPreference, RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bevy::sprite::{Sprite, SpritePlugin};
use bevy::sprite_render::SpriteRenderPlugin;
use bevy::time::TimePlugin;
use bevy::transform::components::Transform;
use bevy::transform::TransformPlugin;
use bevy::window::{
    ExitCondition, MonitorSelection, PresentMode, Window, WindowMode, WindowPlugin,
};
use bevy::winit::{UpdateMode, WinitPlugin, WinitSettings};
use std::num::NonZero;
use std::time::{Duration, Instant};

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

const BACKGROUND_TEXTURE: &[u8] = include_bytes!("../assets/000.ktx2");
const MENU_TEXTURE: &[u8] = include_bytes!("../assets/menu.ktx2");

const SHOWCASE_TIME: Duration = Duration::from_millis(500);
const MENU_DRAIN_TIME: Duration = Duration::from_millis(250);
const MENU_SLIDE_SECS: f32 = 0.25;
const ESCAPE_POLL: Duration = Duration::from_millis(100);
const ESCAPE_HOLD: Duration = Duration::from_secs(7);

const MENU_WIDTH: f32 = 0.21;
const MENU_EDGE_WIDTH: f32 = 0.0035;
const MENU_EDGE_MIN: f32 = 3.0;
const MENU_INSET_WIDTH: f32 = 0.0018;
const MENU_INSET_MIN: f32 = 2.0;
const MENU_GAP: f32 = 10.0;
const MENU_OVERSHOOT: f32 = 1.05;
const MENU_LAYER: f32 = 1.0;
const MENU_BAR_LAYER: f32 = 2.0;
const MENU_PANEL_ALPHA: f32 = 0.86;
const MENU_EDGE_ALPHA: f32 = 0.92;
const MENU_INSET_ALPHA: f32 = 0.75;

fn main() {
    App::new()
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        })
        .add_plugins((
            (
                TaskPoolPlugin {
                    task_pool_options: TaskPoolOptions {
                        min_total_threads: 1,
                        max_total_threads: 1,
                        ..Default::default()
                    },
                },
                TimePlugin,
                FrameCountPlugin,
                TransformPlugin,
                InputPlugin,
                InputFocusPlugin,
                WindowPlugin {
                    primary_window: Some(Window {
                        mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                        present_mode: PresentMode::Mailbox,
                        desired_maximum_frame_latency: NonZero::new(1),
                        resizable: false,
                        decorations: false,
                        transparent: false,
                        focused: true,
                        visible: false,
                        fit_canvas_to_parent: false,
                        ..Default::default()
                    }),
                    close_when_requested: false,
                    exit_condition: ExitCondition::DontExit,
                    ..Default::default()
                },
                AccessibilityPlugin,
                AssetPlugin {
                    file_path: String::new(),
                    watch_for_changes_override: Some(false),
                    ..Default::default()
                },
                MeshPlugin,
            ),
            (
                CameraPlugin,
                WinitPlugin::default(),
                RenderPlugin {
                    render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                        backends: Some(Backends::VULKAN),
                        power_preference: PowerPreference::HighPerformance,
                        ..Default::default()
                    })),
                    synchronous_pipeline_compilation: false,
                    ..Default::default()
                },
                ImagePlugin::default(),
                PipelinedRenderingPlugin,
                CorePipelinePlugin,
                SpritePlugin,
                SpriteRenderPlugin,
            ),
        ))
        .add_systems(Update, frame)
        .run();
}

fn frame(
    mut commands: Commands,
    mut windows: Query<&mut Window>,
    mut images: ResMut<Assets<Image>>,
    input: Res<ButtonInput<KeyCode>>,
    support: Option<Res<CompressedImageFormatSupport>>,
    mut winit: ResMut<WinitSettings>,
    mut app: Local<Runtime>,
) {
    let app = &mut *app;
    let now = Instant::now();
    let mut window = windows.single_mut().expect("primary window");

    if window.mode != WindowMode::BorderlessFullscreen(MonitorSelection::Primary) {
        window.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Primary);
    }
    if window.decorations {
        window.decorations = false;
    }
    if window.resizable {
        window.resizable = false;
    }

    let size = (window.physical_width(), window.physical_height());

    if let Some(textures) = app.textures.as_ref() {
        if size != app.viewport {
            app.viewport = size;
            if let Some(background) = app.background {
                commands
                    .entity(background)
                    .insert(Transform::from_scale(Vec3::splat(cover_scale(
                        size,
                        textures.background_size,
                    ))));
            }
            if app.camera.is_none() {
                app.camera = Some(spawn_camera(&mut commands));
            }
            if let Some(menu) = app.menu.take() {
                commands.entity(menu.panel).despawn();
                commands.entity(menu.edge).despawn();
                commands.entity(menu.inset).despawn();
            }
            winit.focused_mode = UpdateMode::Continuous;
            app.phase = Phase::Awake {
                sleep_at: Some(now + SHOWCASE_TIME),
            };
        }
    }

    match app.phase {
        Phase::Decoding => {
            if let Some(supported) = support.as_deref() {
                let format = supported.0;
                let decoded = decode_texture(BACKGROUND_TEXTURE, format).and_then(|background| {
                    decode_texture(MENU_TEXTURE, format).map(|menu| (background, menu))
                });
                match decoded {
                    Ok((background, menu)) => {
                        let background_size =
                            Vec2::new(background.width() as f32, background.height() as f32);
                        let menu_handle = images.add(menu);
                        let background_handle = images.add(background);
                        app.background = Some(
                            commands
                                .spawn((
                                    Sprite {
                                        image: background_handle,
                                        custom_size: Some(background_size),
                                        ..Default::default()
                                    },
                                    Transform::from_scale(Vec3::splat(cover_scale(
                                        size,
                                        background_size,
                                    ))),
                                ))
                                .id(),
                        );
                        app.camera = Some(spawn_camera(&mut commands));
                        window.visible = true;
                        app.textures = Some(Textures {
                            background_size,
                            menu: menu_handle,
                        });
                        app.viewport = size;
                        app.phase = Phase::Awake {
                            sleep_at: Some(now + SHOWCASE_TIME),
                        };
                    }
                    Err(error) => {
                        eprintln!("arxumbra: {error}");
                        commands.write_message(AppExit::error());
                        app.phase = Phase::Failed;
                    }
                }
            }
        }
        Phase::Awake {
            sleep_at: Some(sleep_at),
        } if sleep_at <= now => {
            if let Some(camera) = app.camera.take() {
                commands.entity(camera).despawn();
            }
            *winit = WinitSettings::desktop_app();
            app.phase = Phase::Sleeping;
        }
        Phase::Awake { .. } | Phase::Sleeping | Phase::Failed => {}
    }

    if input.just_pressed(KeyCode::F1) {
        if let Some(menu) = app.menu.as_mut() {
            menu.animation = match menu.animation {
                Some(Direction::Opening) | None => Some(Direction::Closing),
                Some(Direction::Closing) => Some(Direction::Opening),
            };
        } else if app.textures.is_some() {
            if app.camera.is_none() {
                app.camera = Some(spawn_camera(&mut commands));
            }
            let width = app.viewport.0 as f32;
            let menu_width = width * MENU_WIDTH;
            let edge_width = (width * MENU_EDGE_WIDTH).max(MENU_EDGE_MIN);
            let inset_width = (width * MENU_INSET_WIDTH).max(MENU_INSET_MIN);
            let closed = slide(0.0, width);
            app.menu = Some(MenuPanel {
                panel: commands
                    .spawn(Transform::from_xyz(closed, 0.0, MENU_LAYER))
                    .id(),
                edge: commands
                    .spawn(Transform::from_xyz(
                        closed + menu_width / 2.0 - edge_width / 2.0,
                        0.0,
                        MENU_BAR_LAYER,
                    ))
                    .id(),
                inset: commands
                    .spawn(Transform::from_xyz(
                        closed + menu_width / 2.0 - edge_width - MENU_GAP - inset_width / 2.0,
                        0.0,
                        MENU_BAR_LAYER,
                    ))
                    .id(),
                progress: 0.0,
                animation: Some(Direction::Opening),
                stepped_at: now,
            });
            winit.focused_mode = UpdateMode::Continuous;
            app.phase = Phase::Awake { sleep_at: None };
        }
    }

    let mut closing_finished = false;
    if let (Some(menu), Some(textures)) = (app.menu.as_mut(), app.textures.as_ref()) {
        if let Some(direction) = menu.animation {
            let step = (now - menu.stepped_at).as_secs_f32() / MENU_SLIDE_SECS;
            menu.stepped_at = now;
            menu.progress = match direction {
                Direction::Opening => (menu.progress + step).min(1.0),
                Direction::Closing => (menu.progress - step).max(0.0),
            };
            let eased = menu.progress * menu.progress * (3.0 - 2.0 * menu.progress);
            draw_menu(&mut commands, menu, textures, app.viewport, eased);
            let finished = match direction {
                Direction::Opening => menu.progress >= 1.0,
                Direction::Closing => menu.progress <= 0.0,
            };
            if finished {
                match direction {
                    Direction::Opening => menu.animation = None,
                    Direction::Closing => {
                        commands.entity(menu.panel).despawn();
                        commands.entity(menu.edge).despawn();
                        commands.entity(menu.inset).despawn();
                        closing_finished = true;
                    }
                }
            }
        }
    }
    if closing_finished {
        app.menu = None;
        app.phase = Phase::Awake {
            sleep_at: Some(now + MENU_DRAIN_TIME),
        };
    }

    if input.just_pressed(KeyCode::Escape) {
        app.escape_held_since = Some(now);
        winit.focused_mode = UpdateMode::reactive(ESCAPE_POLL);
    }
    if input.just_released(KeyCode::Escape) {
        app.escape_held_since = None;
        if app.camera.is_some() {
            winit.focused_mode = UpdateMode::Continuous;
        } else {
            *winit = WinitSettings::desktop_app();
        }
    }
    if app
        .escape_held_since
        .is_some_and(|pressed_at| now - pressed_at >= ESCAPE_HOLD)
    {
        commands.write_message(AppExit::Success);
    }
}

fn spawn_camera(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Camera2d,
            Camera {
                clear_color: ClearColorConfig::None,
                ..Default::default()
            },
        ))
        .id()
}

fn decode_texture(bytes: &[u8], format: CompressedImageFormats) -> Result<Image, String> {
    ktx2_buffer_to_image(bytes, format, true)
        .map(|mut image| {
            image.sampler = ImageSampler::linear();
            image
        })
        .map_err(|error| error.to_string())
}

fn cover_scale(viewport: (u32, u32), texture: Vec2) -> f32 {
    (viewport.0 as f32 / texture.x).max(viewport.1 as f32 / texture.y)
}

fn slide(eased: f32, width: f32) -> f32 {
    let open_center = -width / 2.0 + width * MENU_WIDTH / 2.0;
    open_center - (1.0 - eased) * width * MENU_WIDTH * MENU_OVERSHOOT
}

fn draw_menu(
    commands: &mut Commands,
    menu: &MenuPanel,
    textures: &Textures,
    viewport: (u32, u32),
    eased: f32,
) {
    let width = viewport.0 as f32;
    let height = viewport.1 as f32;
    let menu_width = width * MENU_WIDTH;
    let edge_width = (width * MENU_EDGE_WIDTH).max(MENU_EDGE_MIN);
    let inset_width = (width * MENU_INSET_WIDTH).max(MENU_INSET_MIN);
    let x = slide(eased, width);
    commands.entity(menu.panel).insert((
        Sprite {
            image: textures.menu.clone(),
            custom_size: Some(Vec2::new(menu_width, height)),
            color: Color::srgba(1.0, 1.0, 1.0, MENU_PANEL_ALPHA * eased),
            ..Default::default()
        },
        Transform::from_xyz(x, 0.0, MENU_LAYER),
    ));
    commands.entity(menu.edge).insert((
        Sprite::from_color(
            Color::srgba(0.478, 0.635, 0.969, MENU_EDGE_ALPHA * eased),
            Vec2::new(edge_width, height),
        ),
        Transform::from_xyz(x + menu_width / 2.0 - edge_width / 2.0, 0.0, MENU_BAR_LAYER),
    ));
    commands.entity(menu.inset).insert((
        Sprite::from_color(
            Color::srgba(0.733, 0.604, 0.969, MENU_INSET_ALPHA * eased),
            Vec2::new(inset_width, height),
        ),
        Transform::from_xyz(
            x + menu_width / 2.0 - edge_width - MENU_GAP - inset_width / 2.0,
            0.0,
            MENU_BAR_LAYER,
        ),
    ));
}

#[derive(Clone, Copy)]
enum Phase {
    Decoding,
    Awake { sleep_at: Option<Instant> },
    Sleeping,
    Failed,
}

#[derive(Clone, Copy)]
enum Direction {
    Opening,
    Closing,
}

struct Textures {
    background_size: Vec2,
    menu: Handle<Image>,
}

struct MenuPanel {
    panel: Entity,
    edge: Entity,
    inset: Entity,
    progress: f32,
    animation: Option<Direction>,
    stepped_at: Instant,
}

struct Runtime {
    phase: Phase,
    background: Option<Entity>,
    camera: Option<Entity>,
    menu: Option<MenuPanel>,
    textures: Option<Textures>,
    viewport: (u32, u32),
    escape_held_since: Option<Instant>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            phase: Phase::Decoding,
            background: None,
            camera: None,
            menu: None,
            textures: None,
            viewport: (0, 0),
            escape_held_since: None,
        }
    }
}
