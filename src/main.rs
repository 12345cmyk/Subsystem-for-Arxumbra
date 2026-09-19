#![deny(warnings)]
#![forbid(unsafe_code)]

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    #[derive(Clone, Copy)]
    enum Phase {
        Decoding,
        Awake {
            sleep_at: Option<std::time::Instant>,
        },
        Sleeping,
        Failed,
    }

    #[derive(Clone, Copy)]
    enum Direction {
        Opening,
        Closing,
    }

    struct Textures {
        background_size: bevy::math::Vec2,
        menu: bevy::asset::Handle<bevy::image::Image>,
    }

    struct MenuPanel {
        panel: bevy::ecs::entity::Entity,
        edge: bevy::ecs::entity::Entity,
        inset: bevy::ecs::entity::Entity,
        progress: f32,
        animation: Option<Direction>,
        stepped_at: std::time::Instant,
    }

    struct Runtime {
        phase: Phase,
        background: Option<bevy::ecs::entity::Entity>,
        camera: Option<bevy::ecs::entity::Entity>,
        menu: Option<MenuPanel>,
        textures: Option<Textures>,
        viewport: (u32, u32),
        escape_held_since: Option<std::time::Instant>,
    }

    impl Default for Runtime {
        fn default() -> Self {
            Runtime {
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

    fn spawn_camera(commands: &mut bevy::ecs::system::Commands) -> bevy::ecs::entity::Entity {
        commands
            .spawn((
                bevy::camera::Camera2d,
                bevy::camera::Camera {
                    clear_color: bevy::camera::ClearColorConfig::None,
                    ..Default::default()
                },
            ))
            .id()
    }

    fn decode_texture(
        bytes: &[u8],
        format: bevy::image::CompressedImageFormats,
    ) -> Result<bevy::image::Image, String> {
        bevy::image::ktx2_buffer_to_image(bytes, format, true)
            .map(|mut image| {
                image.sampler = bevy::image::ImageSampler::linear();
                image
            })
            .map_err(|error| error.to_string())
    }

    fn cover_scale(viewport: (u32, u32), texture: bevy::math::Vec2) -> f32 {
        (viewport.0 as f32 / texture.x).max(viewport.1 as f32 / texture.y)
    }

    fn slide(eased: f32, width: f32) -> f32 {
        -width / 2.0 + width * 0.21 / 2.0 - (1.0 - eased) * width * 0.21 * 1.05
    }

    fn draw_menu(
        commands: &mut bevy::ecs::system::Commands,
        menu: &MenuPanel,
        textures: &Textures,
        viewport: (u32, u32),
        eased: f32,
    ) {
        let width = viewport.0 as f32;
        let height = viewport.1 as f32;
        let menu_width = width * 0.21;
        let edge_width = (width * 0.0035).max(3.0);
        let inset_width = (width * 0.0018).max(2.0);
        let x = slide(eased, width);
        commands.entity(menu.panel).insert((
            bevy::sprite::Sprite {
                image: textures.menu.clone(),
                custom_size: Some(bevy::math::Vec2::new(menu_width, height)),
                color: bevy::color::Color::srgba(1.0, 1.0, 1.0, 0.86 * eased),
                ..Default::default()
            },
            bevy::transform::components::Transform::from_xyz(x, 0.0, 1.0),
        ));
        commands.entity(menu.edge).insert((
            bevy::sprite::Sprite::from_color(
                bevy::color::Color::srgba(0.478, 0.635, 0.969, 0.92 * eased),
                bevy::math::Vec2::new(edge_width, height),
            ),
            bevy::transform::components::Transform::from_xyz(
                x + menu_width / 2.0 - edge_width / 2.0,
                0.0,
                2.0,
            ),
        ));
        commands.entity(menu.inset).insert((
            bevy::sprite::Sprite::from_color(
                bevy::color::Color::srgba(0.733, 0.604, 0.969, 0.75 * eased),
                bevy::math::Vec2::new(inset_width, height),
            ),
            bevy::transform::components::Transform::from_xyz(
                x + menu_width / 2.0 - edge_width - 10.0 - inset_width / 2.0,
                0.0,
                2.0,
            ),
        ));
    }

    fn frame(
        mut commands: bevy::ecs::system::Commands,
        mut windows: bevy::ecs::system::Query<&mut bevy::window::Window>,
        mut images: bevy::ecs::system::ResMut<bevy::asset::Assets<bevy::image::Image>>,
        input: bevy::ecs::system::Res<bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>>,
        support: Option<bevy::ecs::system::Res<bevy::image::CompressedImageFormatSupport>>,
        mut winit: bevy::ecs::system::ResMut<bevy::winit::WinitSettings>,
        mut app: bevy::ecs::system::Local<Runtime>,
    ) {
        let app: &mut Runtime = &mut *app;
        let now = std::time::Instant::now();
        let mut window = windows.single_mut().expect("primary window");

        if window.mode
            != bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Primary)
        {
            window.mode =
                bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Primary);
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
                    commands.entity(background).insert(
                        bevy::transform::components::Transform::from_scale(bevy::math::Vec3::splat(
                            cover_scale(size, textures.background_size),
                        )),
                    );
                }
                if app.camera.is_none() {
                    app.camera = Some(spawn_camera(&mut commands));
                }
                if let Some(menu) = app.menu.take() {
                    commands.entity(menu.panel).despawn();
                    commands.entity(menu.edge).despawn();
                    commands.entity(menu.inset).despawn();
                }
                winit.focused_mode = bevy::winit::UpdateMode::Continuous;
                app.phase = Phase::Awake {
                    sleep_at: Some(now + std::time::Duration::from_millis(500)),
                };
            }
        }

        match app.phase {
            Phase::Decoding => {
                if let Some(supported) = support.as_deref() {
                    let format = supported.0;
                    let decoded = decode_texture(include_bytes!("../assets/000.ktx2"), format)
                        .and_then(|background| {
                            decode_texture(include_bytes!("../assets/menu.ktx2"), format)
                                .map(|menu| (background, menu))
                        });
                    match decoded {
                        Ok((background, menu)) => {
                            let background_size = bevy::math::Vec2::new(
                                background.width() as f32,
                                background.height() as f32,
                            );
                            let menu_handle = images.add(menu);
                            let background_handle = images.add(background);
                            app.background = Some(
                                commands
                                    .spawn((
                                        bevy::sprite::Sprite {
                                            image: background_handle,
                                            custom_size: Some(background_size),
                                            ..Default::default()
                                        },
                                        bevy::transform::components::Transform::from_scale(
                                            bevy::math::Vec3::splat(cover_scale(
                                                size,
                                                background_size,
                                            )),
                                        ),
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
                                sleep_at: Some(now + std::time::Duration::from_millis(500)),
                            };
                        }
                        Err(error) => {
                            eprintln!("arxumbra: {error}");
                            commands.write_message(bevy::app::AppExit::error());
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
                *winit = bevy::winit::WinitSettings::desktop_app();
                app.phase = Phase::Sleeping;
            }
            Phase::Awake { .. } | Phase::Sleeping | Phase::Failed => {}
        }

        if input.just_pressed(bevy::input::keyboard::KeyCode::F1) {
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
                let menu_width = width * 0.21;
                let edge_width = (width * 0.0035).max(3.0);
                let inset_width = (width * 0.0018).max(2.0);
                let closed = slide(0.0, width);
                app.menu = Some(MenuPanel {
                    panel: commands
                        .spawn(bevy::transform::components::Transform::from_xyz(
                            closed, 0.0, 1.0,
                        ))
                        .id(),
                    edge: commands
                        .spawn(bevy::transform::components::Transform::from_xyz(
                            closed + menu_width / 2.0 - edge_width / 2.0,
                            0.0,
                            2.0,
                        ))
                        .id(),
                    inset: commands
                        .spawn(bevy::transform::components::Transform::from_xyz(
                            closed + menu_width / 2.0 - edge_width - 10.0 - inset_width / 2.0,
                            0.0,
                            2.0,
                        ))
                        .id(),
                    progress: 0.0,
                    animation: Some(Direction::Opening),
                    stepped_at: now,
                });
                winit.focused_mode = bevy::winit::UpdateMode::Continuous;
                app.phase = Phase::Awake { sleep_at: None };
            }
        }

        let mut closing_finished = false;
        if let (Some(menu), Some(textures)) = (app.menu.as_mut(), app.textures.as_ref()) {
            if let Some(direction) = menu.animation {
                let step = (now - menu.stepped_at).as_secs_f32() / 0.25;
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
                sleep_at: Some(now + std::time::Duration::from_millis(250)),
            };
        }

        if input.just_pressed(bevy::input::keyboard::KeyCode::Escape) {
            app.escape_held_since = Some(now);
            winit.focused_mode =
                bevy::winit::UpdateMode::reactive(std::time::Duration::from_millis(100));
        }
        if input.just_released(bevy::input::keyboard::KeyCode::Escape) {
            app.escape_held_since = None;
            if app.camera.is_some() {
                winit.focused_mode = bevy::winit::UpdateMode::Continuous;
            } else {
                *winit = bevy::winit::WinitSettings::desktop_app();
            }
        }
        if app
            .escape_held_since
            .is_some_and(|pressed_at| now - pressed_at >= std::time::Duration::from_secs(7))
        {
            commands.write_message(bevy::app::AppExit::Success);
        }
    }

    bevy::app::App::new()
        .insert_resource(bevy::winit::WinitSettings {
            focused_mode: bevy::winit::UpdateMode::Continuous,
            unfocused_mode: bevy::winit::UpdateMode::Continuous,
        })
        .add_plugins((
            (
                bevy::app::TaskPoolPlugin {
                    task_pool_options: bevy::app::TaskPoolOptions {
                        min_total_threads: 1,
                        max_total_threads: 1,
                        ..Default::default()
                    },
                },
                bevy::time::TimePlugin,
                bevy::diagnostic::FrameCountPlugin,
                bevy::transform::TransformPlugin,
                bevy::input::InputPlugin,
                bevy::input_focus::InputFocusPlugin,
                bevy::window::WindowPlugin {
                    primary_window: Some(bevy::window::Window {
                        mode: bevy::window::WindowMode::BorderlessFullscreen(
                            bevy::window::MonitorSelection::Primary,
                        ),
                        present_mode: bevy::window::PresentMode::Mailbox,
                        desired_maximum_frame_latency: std::num::NonZero::new(1),
                        resizable: false,
                        decorations: false,
                        transparent: false,
                        focused: true,
                        visible: false,
                        fit_canvas_to_parent: false,
                        ..Default::default()
                    }),
                    close_when_requested: false,
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    ..Default::default()
                },
                bevy::a11y::AccessibilityPlugin,
                bevy::asset::AssetPlugin {
                    file_path: String::new(),
                    watch_for_changes_override: Some(false),
                    ..Default::default()
                },
                bevy::mesh::MeshPlugin,
            ),
            (
                bevy::camera::CameraPlugin,
                bevy::winit::WinitPlugin::default(),
                bevy::render::RenderPlugin {
                    render_creation: bevy::render::settings::RenderCreation::Automatic(Box::new(
                        bevy::render::settings::WgpuSettings {
                            backends: Some(bevy::render::settings::Backends::VULKAN),
                            power_preference: bevy::render::settings::PowerPreference::HighPerformance,
                            ..Default::default()
                        },
                    )),
                    synchronous_pipeline_compilation: false,
                    ..Default::default()
                },
                bevy::image::ImagePlugin::default(),
                bevy::render::pipelined_rendering::PipelinedRenderingPlugin,
                bevy::core_pipeline::CorePipelinePlugin,
                bevy::sprite::SpritePlugin,
                bevy::sprite_render::SpriteRenderPlugin,
            ),
        ))
        .add_systems(bevy::app::Update, frame)
        .run();
}
