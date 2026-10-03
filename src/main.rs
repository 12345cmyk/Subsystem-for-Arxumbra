#![forbid(unsafe_code)]
#![deny(warnings)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let fullscreen =
        bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Primary);
    let grab = bevy::window::CursorGrabMode::Confined;
    let reactive_mode: fn(std::time::Duration) -> bevy::winit::UpdateMode =
        bevy::winit::UpdateMode::reactive_low_power;
    let active_power = reactive_mode(std::time::Duration::from_millis(16));
    let low_power = reactive_mode(std::time::Duration::from_secs(1));
    let smootherstep: fn(f32) -> f32 = |t| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let mut phase: u8 = 0;
    let mut warmup_frames: u16 = 8;
    let mut viewport: (u32, u32) = (0, 0);
    let mut background_entity = bevy::ecs::entity::Entity::PLACEHOLDER;
    let mut panel_entity = bevy::ecs::entity::Entity::PLACEHOLDER;
    let mut cursor_entity = bevy::ecs::entity::Entity::PLACEHOLDER;
    let mut panel_open = false;
    let mut panel_motion: i8 = 0;
    let mut panel_progress: f32 = 0.0;
    let mut panel_drawn = false;
    let mut cursor_position = bevy::math::Vec2::ZERO;
    let mut cursor_velocity = bevy::math::Vec2::ZERO;
    let mut escape_since: Option<std::time::Instant> = None;
    let mut idle_deadline: Option<std::time::Instant> = None;
    bevy::app::App::new()
        .insert_resource(bevy::camera::ClearColor(bevy::color::Color::BLACK))
        .add_plugins((
            (
                bevy::app::TaskPoolPlugin::default(),
                bevy::app::TerminalCtrlCHandlerPlugin,
                bevy::time::TimePlugin,
                bevy::diagnostic::FrameCountPlugin,
                bevy::transform::TransformPlugin,
                bevy::input::InputPlugin,
                bevy::input_focus::InputFocusPlugin,
                bevy::window::WindowPlugin {
                    primary_window: Some(bevy::window::Window {
                        title: String::from("Subsystem for Arxumbra"),
                        name: Some(String::from("subsystem-for-arxumbra")),
                        present_mode: bevy::window::PresentMode::AutoNoVsync,
                        resolution: bevy::window::WindowResolution::new(1920, 1080)
                            .with_scale_factor_override(1.0),
                        decorations: false,
                        resizable: true,
                        visible: true,
                        desired_maximum_frame_latency: core::num::NonZeroU32::new(2),
                        ..Default::default()
                    }),
                    primary_cursor_options: Some(bevy::window::CursorOptions {
                        visible: false,
                        grab_mode: grab,
                        hit_test: true,
                    }),
                    exit_condition: bevy::window::ExitCondition::OnPrimaryClosed,
                    close_when_requested: true,
                },
                bevy::a11y::AccessibilityPlugin,
                bevy::asset::AssetPlugin {
                    file_path: String::new(),
                    watch_for_changes_override: Some(false),
                    ..Default::default()
                },
            ),
            (
                bevy::winit::WinitPlugin::default(),
                bevy::render::RenderPlugin {
                    render_creation: bevy::render::settings::RenderCreation::Automatic(Box::new(
                        bevy::render::settings::WgpuSettings {
                            backends: Some(bevy::render::settings::Backends::PRIMARY),
                            power_preference:
                                bevy::render::settings::PowerPreference::HighPerformance,
                            disabled_features: Some(
                                bevy::render::settings::WgpuFeatures::TEXTURE_BINDING_ARRAY
                                    | bevy::render::settings::WgpuFeatures::BUFFER_BINDING_ARRAY
                                    | bevy::render::settings::WgpuFeatures::INDIRECT_FIRST_INSTANCE,
                            ),
                            ..Default::default()
                        },
                    )),
                    synchronous_pipeline_compilation: true,
                    ..Default::default()
                },
                bevy::image::ImagePlugin::default(),
                bevy::mesh::MeshPlugin,
                bevy::camera::CameraPlugin,
                bevy::core_pipeline::CorePipelinePlugin,
                bevy::sprite::SpritePlugin,
                bevy::sprite_render::SpriteRenderPlugin,
            ),
        ))
        .insert_resource(bevy::winit::WinitSettings {
            focused_mode: active_power,
            unfocused_mode: active_power,
        })
        .add_systems(
            bevy::app::Update,
            move |mut commands: bevy::ecs::system::Commands,
                  mut windows: bevy::ecs::system::Query<
                      &mut bevy::window::Window,
                      bevy::ecs::query::With<bevy::window::PrimaryWindow>,
                  >,
                  mut window_cursors: bevy::ecs::system::Query<
                      &mut bevy::window::CursorOptions,
                      bevy::ecs::query::With<bevy::window::PrimaryWindow>,
                  >,
                  mut images: bevy::ecs::system::ResMut<bevy::asset::Assets<bevy::image::Image>>,
                  mut sprites: bevy::ecs::system::Query<(
                      &mut bevy::transform::components::Transform,
                      &mut bevy::camera::visibility::Visibility,
                      Option<&mut bevy::mesh::Mesh2d>,
                  )>,
                  input: bevy::ecs::system::Res<
                      bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>,
                  >,
                  time: bevy::ecs::system::Res<bevy::time::Time>,
                  mut winit: bevy::ecs::system::ResMut<bevy::winit::WinitSettings>| {
                let delta = time.delta_secs().min(0.05);
                let booting = phase == 0;
                let mut window = windows.single_mut().expect("primary window");
                if window.mode != fullscreen {
                    window.mode = fullscreen;
                }
                if window.present_mode != bevy::window::PresentMode::AutoNoVsync {
                    window.present_mode = bevy::window::PresentMode::AutoNoVsync;
                }
                if window.decorations {
                    window.decorations = false;
                }
                if !window.resizable {
                    window.resizable = true;
                }
                if let Ok(mut cursor_options) = window_cursors.single_mut() {
                    if cursor_options.visible {
                        cursor_options.visible = false;
                    }
                    if cursor_options.grab_mode != grab {
                        cursor_options.grab_mode = grab;
                    }
                    if !cursor_options.hit_test {
                        cursor_options.hit_test = true;
                    }
                }
                let physical_width = window.physical_width();
                let physical_height = window.physical_height();
                if physical_width == 0 || physical_height == 0 {
                    return;
                }
                let width = window.width();
                let height = window.height();
                if width <= 0.0 || height <= 0.0 {
                    return;
                }
                let geometry_changed = (physical_width, physical_height) != viewport;
                let panel_width = width * 0.21;
                let cursor_extent = (height * 0.055).clamp(28.0, 320.0);
                let cursor_limit = bevy::math::Vec2::new(
                    (width * 0.5 - cursor_extent * 0.5).max(0.0),
                    (height * 0.5 - cursor_extent * 0.5).max(0.0),
                );
                let closed = panel_width * 0.5 - width * 0.5 - panel_width * 1.05;
                let open_x = panel_width * 0.5 - width * 0.5;
                if booting {
                    let pixel = images.add(bevy::image::Image::new(
                        bevy::render::render_resource::Extent3d {
                            width: 1,
                            height: 1,
                            depth_or_array_layers: 1,
                        },
                        bevy::render::render_resource::TextureDimension::D2,
                        vec![255u8, 255, 255, 255],
                        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                        bevy::asset::RenderAssetUsages::RENDER_WORLD,
                    ));
                    cursor_position = bevy::math::Vec2::ZERO;
                    cursor_velocity = bevy::math::Vec2::ZERO;
                    commands.spawn((
                        bevy::camera::Camera2d,
                        bevy::camera::Camera {
                            clear_color: bevy::camera::ClearColorConfig::Custom(
                                bevy::color::Color::BLACK,
                            ),
                            ..Default::default()
                        },
                        bevy::render::view::Msaa::Off,
                        bevy::core_pipeline::tonemapping::Tonemapping::None,
                        bevy::core_pipeline::tonemapping::DebandDither::Disabled,
                    ));
                    let mut spawn_layer = |image: bevy::asset::Handle<bevy::image::Image>,
                                           color: bevy::color::Color,
                                           x: f32,
                                           z: f32,
                                           sx: f32,
                                           sy: f32,
                                           visibility: bevy::camera::visibility::Visibility|
                     -> bevy::ecs::entity::Entity {
                        commands
                            .spawn((
                                bevy::sprite::Sprite {
                                    image,
                                    custom_size: Some(bevy::math::Vec2::ONE),
                                    color,
                                    alpha_mode: bevy::sprite::SpriteAlphaMode::Blend,
                                    ..Default::default()
                                },
                                bevy::transform::components::Transform::from_xyz(x, 0.0, z)
                                    .with_scale(bevy::math::Vec3::new(sx, sy, 1.0)),
                                visibility,
                                bevy::camera::visibility::NoFrustumCulling,
                            ))
                            .id()
                    };
                    background_entity = spawn_layer(
                        pixel.clone(),
                        bevy::color::Color::BLACK,
                        0.0,
                        0.0,
                        width,
                        height,
                        bevy::camera::visibility::Visibility::Visible,
                    );
                    panel_entity = spawn_layer(
                        pixel.clone(),
                        bevy::color::Color::srgba(0.25, 0.25, 0.25, 0.92),
                        closed,
                        1.0,
                        panel_width,
                        height,
                        bevy::camera::visibility::Visibility::Hidden,
                    );
                    cursor_entity = spawn_layer(
                        pixel,
                        bevy::color::Color::srgb(0.0, 1.0, 1.0),
                        0.0,
                        2.0,
                        cursor_extent,
                        cursor_extent,
                        bevy::camera::visibility::Visibility::Visible,
                    );
                    viewport = (physical_width, physical_height);
                    warmup_frames = 8;
                    let ready_now = std::time::Instant::now();
                    winit.focused_mode = active_power;
                    winit.unfocused_mode = active_power;
                    idle_deadline = Some(ready_now + std::time::Duration::from_millis(2500));
                    phase = 1;
                    return;
                }
                if geometry_changed {
                    cursor_position = cursor_position.max(-cursor_limit).min(cursor_limit);
                    if let Ok((mut transform, _, _)) = sprites.get_mut(background_entity) {
                        transform.scale = bevy::math::Vec3::new(width, height, 1.0);
                    }
                    if let Ok((mut transform, mut visibility, _)) = sprites.get_mut(panel_entity) {
                        let eased = smootherstep(panel_progress);
                        transform.translation.x = closed + (open_x - closed) * eased;
                        transform.scale = bevy::math::Vec3::new(panel_width, height, 1.0);
                        if panel_progress > 0.0 {
                            *visibility = bevy::camera::visibility::Visibility::Visible;
                            panel_drawn = true;
                        } else {
                            *visibility = bevy::camera::visibility::Visibility::Hidden;
                            panel_drawn = false;
                        }
                    }
                    if let Ok((mut transform, _, _)) = sprites.get_mut(cursor_entity) {
                        transform.translation.x = cursor_position.x;
                        transform.translation.y = cursor_position.y;
                        transform.scale = bevy::math::Vec3::new(cursor_extent, cursor_extent, 1.0);
                    }
                    viewport = (physical_width, physical_height);
                    warmup_frames = 8;
                    let resize_now = std::time::Instant::now();
                    winit.focused_mode = active_power;
                    winit.unfocused_mode = active_power;
                    idle_deadline = Some(resize_now + std::time::Duration::from_millis(2500));
                    phase = 1;
                }
                if warmup_frames > 0 {
                    warmup_frames -= 1;
                }
                if !window.visible {
                    window.visible = true;
                }
                let now = std::time::Instant::now();
                let control = input.pressed(bevy::input::keyboard::KeyCode::ControlLeft)
                    || input.pressed(bevy::input::keyboard::KeyCode::ControlRight);
                let direction = if control {
                    let right = (input.pressed(bevy::input::keyboard::KeyCode::ArrowRight)
                        || input.pressed(bevy::input::keyboard::KeyCode::KeyD))
                        as i8
                        - (input.pressed(bevy::input::keyboard::KeyCode::ArrowLeft)
                            || input.pressed(bevy::input::keyboard::KeyCode::KeyA))
                            as i8;
                    let up = (input.pressed(bevy::input::keyboard::KeyCode::ArrowUp)
                        || input.pressed(bevy::input::keyboard::KeyCode::KeyW))
                        as i8
                        - (input.pressed(bevy::input::keyboard::KeyCode::ArrowDown)
                            || input.pressed(bevy::input::keyboard::KeyCode::KeyS))
                            as i8;
                    bevy::math::Vec2::new(right as f32, up as f32).normalize_or_zero()
                } else {
                    bevy::math::Vec2::ZERO
                };
                let precision = if input.pressed(bevy::input::keyboard::KeyCode::ShiftLeft)
                    || input.pressed(bevy::input::keyboard::KeyCode::ShiftRight)
                {
                    0.25
                } else {
                    1.0
                };
                let target_velocity = direction * (height * 0.55 * precision);
                cursor_velocity +=
                    (target_velocity - cursor_velocity) * (1.0 - (-delta / 0.045).exp());
                let previous = cursor_position;
                cursor_position += cursor_velocity * delta;
                cursor_position = cursor_position.max(-cursor_limit).min(cursor_limit);
                if cursor_position != previous {
                    if let Ok((mut transform, _, _)) = sprites.get_mut(cursor_entity) {
                        transform.translation.x = cursor_position.x;
                        transform.translation.y = cursor_position.y;
                    }
                }
                if input.just_pressed(bevy::input::keyboard::KeyCode::F1) {
                    panel_open = !panel_open;
                    panel_motion = if panel_open { 1 } else { -1 };
                }
                if panel_motion != 0 {
                    let progress = if panel_motion > 0 {
                        (panel_progress + delta / 0.8).min(1.0)
                    } else {
                        (panel_progress - delta / 0.8).max(0.0)
                    };
                    panel_progress = progress;
                    let eased = smootherstep(progress);
                    if let Ok((mut transform, mut visibility, mesh2d)) =
                        sprites.get_mut(panel_entity)
                    {
                        transform.translation.x = closed + (open_x - closed) * eased;
                        if progress > 0.0 {
                            if !panel_drawn {
                                *visibility = bevy::camera::visibility::Visibility::Visible;
                                panel_drawn = true;
                                if let Some(mut mesh) = mesh2d {
                                    mesh.0 = mesh.0.clone();
                                }
                            }
                        } else if panel_drawn {
                            *visibility = bevy::camera::visibility::Visibility::Hidden;
                            panel_drawn = false;
                        }
                    }
                    if (panel_motion > 0 && progress >= 1.0)
                        || (panel_motion < 0 && progress <= 0.0)
                    {
                        panel_motion = 0;
                    }
                }
                let exiting = input.just_pressed(bevy::input::keyboard::KeyCode::Escape);
                let released = input.just_released(bevy::input::keyboard::KeyCode::Escape);
                if exiting {
                    escape_since = Some(now);
                }
                if released {
                    escape_since = None;
                }
                if let Some(pressed_at) = escape_since {
                    if now - pressed_at >= std::time::Duration::from_secs(7) {
                        commands.write_message(bevy::app::AppExit::Success);
                        escape_since = None;
                    }
                }
                if warmup_frames > 0
                    || panel_motion != 0
                    || control
                    || direction != bevy::math::Vec2::ZERO
                    || cursor_velocity.length_squared() > 16.0
                    || escape_since.is_some()
                {
                    idle_deadline = Some(now + std::time::Duration::from_millis(1200));
                    if phase != 1 {
                        winit.focused_mode = active_power;
                        winit.unfocused_mode = active_power;
                        phase = 1;
                    }
                } else if phase == 1 {
                    if let Some(deadline) = idle_deadline {
                        if deadline <= now {
                            winit.focused_mode = low_power;
                            winit.unfocused_mode = low_power;
                            idle_deadline = None;
                            phase = 2;
                        }
                    } else {
                        idle_deadline = Some(now + std::time::Duration::from_millis(1200));
                    }
                }
            },
        )
        .run();
}