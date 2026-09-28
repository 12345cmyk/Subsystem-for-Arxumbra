#![deny(warnings)]
#![forbid(unsafe_code)]

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let (background_image, menu_image, cursor_image) = std::thread::scope(|scope| {
        let menu_thread = scope.spawn(|| {
            let transcoder = basisu::Transcoder::new(&include_bytes!("../assets/001.ktx2")[..])
                .expect("menu ktx2 container");
            let (source_width, source_height) = transcoder.base_dimensions();
            let level_count = transcoder.level_count();
            let mut pixels: Vec<u8> = Vec::new();
            let mut level = 0u32;
            while level < level_count {
                pixels.extend_from_slice(
                    &transcoder
                        .transcode(level, basisu::TargetFormat::Bc7Rgba, basisu::DecodeFlags::NONE)
                        .expect("menu ktx2 bc7 transcode"),
                );
                level += 1;
            }
            let mut image = bevy::image::Image::new(
                bevy::render::render_resource::Extent3d {
                    width: source_width,
                    height: source_height,
                    depth_or_array_layers: 1,
                },
                bevy::render::render_resource::TextureDimension::D2,
                pixels,
                bevy::render::render_resource::TextureFormat::Bc7RgbaUnormSrgb,
                bevy::asset::RenderAssetUsages::RENDER_WORLD,
            );
            image.texture_descriptor.mip_level_count = level_count;
            image.sampler = bevy::image::ImageSampler::Descriptor(
                bevy::render::render_resource::SamplerDescriptor {
                    address_mode_u: bevy::render::render_resource::AddressMode::ClampToEdge,
                    address_mode_v: bevy::render::render_resource::AddressMode::ClampToEdge,
                    address_mode_w: bevy::render::render_resource::AddressMode::ClampToEdge,
                    mag_filter: bevy::render::render_resource::FilterMode::Linear,
                    min_filter: bevy::render::render_resource::FilterMode::Linear,
                    mipmap_filter: bevy::render::render_resource::MipmapFilterMode::Linear,
                    ..Default::default()
                }
                .into(),
            );
            image
        });
        let cursor_thread = scope.spawn(|| {
            let transcoder = basisu::Transcoder::new(&include_bytes!("../assets/cursor.ktx2")[..])
                .expect("cursor ktx2 container");
            let (source_width, source_height) = transcoder.base_dimensions();
            let level_count = transcoder.level_count();
            let mut pixels: Vec<u8> = Vec::new();
            let mut level = 0u32;
            while level < level_count {
                pixels.extend_from_slice(
                    &transcoder
                        .transcode(level, basisu::TargetFormat::Bc7Rgba, basisu::DecodeFlags::NONE)
                        .expect("cursor ktx2 bc7 transcode"),
                );
                level += 1;
            }
            let mut image = bevy::image::Image::new(
                bevy::render::render_resource::Extent3d {
                    width: source_width,
                    height: source_height,
                    depth_or_array_layers: 1,
                },
                bevy::render::render_resource::TextureDimension::D2,
                pixels,
                bevy::render::render_resource::TextureFormat::Bc7RgbaUnormSrgb,
                bevy::asset::RenderAssetUsages::RENDER_WORLD,
            );
            image.texture_descriptor.mip_level_count = level_count;
            image.sampler = bevy::image::ImageSampler::Descriptor(
                bevy::render::render_resource::SamplerDescriptor {
                    address_mode_u: bevy::render::render_resource::AddressMode::ClampToEdge,
                    address_mode_v: bevy::render::render_resource::AddressMode::ClampToEdge,
                    address_mode_w: bevy::render::render_resource::AddressMode::ClampToEdge,
                    mag_filter: bevy::render::render_resource::FilterMode::Linear,
                    min_filter: bevy::render::render_resource::FilterMode::Linear,
                    mipmap_filter: bevy::render::render_resource::MipmapFilterMode::Linear,
                    ..Default::default()
                }
                .into(),
            );
            image
        });
        let background_image = {
            let transcoder = basisu::Transcoder::new(&include_bytes!("../assets/000.ktx2")[..])
                .expect("background ktx2 container");
            let (source_width, source_height) = transcoder.base_dimensions();
            let level_count = transcoder.level_count();
            let mut pixels: Vec<u8> = Vec::new();
            let mut level = 0u32;
            while level < level_count {
                pixels.extend_from_slice(
                    &transcoder
                        .transcode(level, basisu::TargetFormat::Bc7Rgba, basisu::DecodeFlags::NONE)
                        .expect("background ktx2 bc7 transcode"),
                );
                level += 1;
            }
            let mut image = bevy::image::Image::new(
                bevy::render::render_resource::Extent3d {
                    width: source_width,
                    height: source_height,
                    depth_or_array_layers: 1,
                },
                bevy::render::render_resource::TextureDimension::D2,
                pixels,
                bevy::render::render_resource::TextureFormat::Bc7RgbaUnormSrgb,
                bevy::asset::RenderAssetUsages::RENDER_WORLD,
            );
            image.texture_descriptor.mip_level_count = level_count;
            image.sampler = bevy::image::ImageSampler::Descriptor(
                bevy::render::render_resource::SamplerDescriptor {
                    address_mode_u: bevy::render::render_resource::AddressMode::ClampToEdge,
                    address_mode_v: bevy::render::render_resource::AddressMode::ClampToEdge,
                    address_mode_w: bevy::render::render_resource::AddressMode::ClampToEdge,
                    mag_filter: bevy::render::render_resource::FilterMode::Linear,
                    min_filter: bevy::render::render_resource::FilterMode::Linear,
                    mipmap_filter: bevy::render::render_resource::MipmapFilterMode::Linear,
                    ..Default::default()
                }
                .into(),
            );
            image
        };
        (
            background_image,
            menu_thread.join().expect("menu ktx2 decode"),
            cursor_thread.join().expect("cursor ktx2 decode"),
        )
    });
    let background_size = bevy::math::Vec2::new(
        background_image.width() as f32,
        background_image.height() as f32,
    );
    let mut background_asset = Some(background_image);
    let mut menu_asset = Some(menu_image);
    let mut cursor_asset = Some(cursor_image);
    let mut phase: u8 = 0;
    let mut background: Option<bevy::ecs::entity::Entity> = None;
    let mut panel: Option<bevy::ecs::entity::Entity> = None;
    let mut cursor: Option<bevy::ecs::entity::Entity> = None;
    let mut menu_texture: Option<bevy::asset::Handle<bevy::image::Image>> = None;
    let mut viewport: (u32, u32) = (0, 0);
    let mut panel_open = false;
    let mut panel_progress: f32 = 0.0;
    let mut panel_animation: u8 = 0;
    let mut cursor_position = bevy::math::Vec2::ZERO;
    let mut cursor_velocity = bevy::math::Vec2::ZERO;
    let mut idle_at: Option<std::time::Instant> = None;
    let mut escape_since: Option<std::time::Instant> = None;
    #[cfg(target_os = "linux")]
    let backends = bevy::render::settings::Backends::VULKAN;
    #[cfg(target_os = "macos")]
    let backends = bevy::render::settings::Backends::METAL;
    #[cfg(target_os = "windows")]
    let backends = bevy::render::settings::Backends::DX12;
    bevy::app::App::new()
        .insert_resource(bevy::winit::WinitSettings {
            focused_mode: bevy::winit::UpdateMode::Continuous,
            unfocused_mode: bevy::winit::UpdateMode::Continuous,
        })
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
                    primary_cursor_options: Some(bevy::window::CursorOptions {
                        visible: false,
                        grab_mode: bevy::window::CursorGrabMode::None,
                        hit_test: false,
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
                            backends: Some(backends),
                            power_preference:
                                bevy::render::settings::PowerPreference::HighPerformance,
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
        .add_systems(
            bevy::app::Update,
            move |mut commands: bevy::ecs::system::Commands,
                  mut windows: bevy::ecs::system::Query<&mut bevy::window::Window>,
                  mut sprites: bevy::ecs::system::Query<
                (
                    &mut bevy::transform::components::Transform,
                    &mut bevy::sprite::Sprite,
                ),
                bevy::ecs::query::Without<bevy::camera::Camera2d>,
            >,
                  mut images: bevy::ecs::system::ResMut<
                bevy::asset::Assets<bevy::image::Image>,
            >,
                  input: bevy::ecs::system::Res<
                bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>,
            >,
                  time: bevy::ecs::system::Res<bevy::time::Time>,
                  mut winit: bevy::ecs::system::ResMut<bevy::winit::WinitSettings>| {
                let now = std::time::Instant::now();
                let delta = time.delta_secs().min(0.05);
                let mut window = windows.single_mut().expect("primary window");
                if window.mode
                    != bevy::window::WindowMode::BorderlessFullscreen(
                        bevy::window::MonitorSelection::Primary,
                    )
                {
                    window.mode = bevy::window::WindowMode::BorderlessFullscreen(
                        bevy::window::MonitorSelection::Primary,
                    );
                }
                if window.decorations {
                    window.decorations = false;
                }
                if window.resizable {
                    window.resizable = false;
                }
                let size = (window.physical_width(), window.physical_height());
                if size.0 == 0 || size.1 == 0 {
                    return;
                }
                if phase == 0 {
                    let width = size.0 as f32;
                    let height = size.1 as f32;
                    let cover =
                        (width / background_size.x).max(height / background_size.y);
                    let panel_width = width * 0.21;
                    let menu_handle = images.add(menu_asset.take().expect("menu texture"));
                    let background_handle =
                        images.add(background_asset.take().expect("background texture"));
                    let cursor_handle =
                        images.add(cursor_asset.take().expect("cursor texture"));
                    background = Some(
                        commands
                            .spawn((
                                bevy::sprite::Sprite {
                                    image: background_handle,
                                    custom_size: Some(background_size),
                                    color: bevy::color::Color::WHITE,
                                    ..Default::default()
                                },
                                bevy::transform::components::Transform::from_scale(
                                    bevy::math::Vec3::splat(cover),
                                ),
                            ))
                            .id(),
                    );
                    commands.spawn((
                        bevy::camera::Camera2d,
                        bevy::camera::Camera {
                            clear_color: bevy::camera::ClearColorConfig::None,
                            ..Default::default()
                        },
                    ));
                    panel = Some(
                        commands
                            .spawn((
                                bevy::sprite::Sprite {
                                    image: menu_handle.clone(),
                                    custom_size: Some(bevy::math::Vec2::new(
                                        panel_width,
                                        height,
                                    )),
                                    color: bevy::color::Color::srgba(1.0, 1.0, 1.0, 0.0),
                                    ..Default::default()
                                },
                                bevy::transform::components::Transform::from_xyz(
                                    -width / 2.0 + panel_width / 2.0 - panel_width * 1.05,
                                    0.0,
                                    1.0,
                                ),
                            ))
                            .id(),
                    );
                    cursor = Some(
                        commands
                            .spawn((
                                bevy::sprite::Sprite {
                                    image: cursor_handle,
                                    custom_size: Some(bevy::math::Vec2::splat(
                                        (height * 0.055).clamp(28.0, 320.0),
                                    )),
                                    color: bevy::color::Color::WHITE,
                                    ..Default::default()
                                },
                                bevy::transform::components::Transform::from_xyz(
                                    0.0, 0.0, 2.0,
                                ),
                            ))
                            .id(),
                    );
                    menu_texture = Some(menu_handle);
                    window.visible = true;
                    viewport = size;
                    phase = 1;
                    idle_at = Some(now + std::time::Duration::from_millis(1200));
                }
                let width = size.0 as f32;
                let height = size.1 as f32;
                let cover = (width / background_size.x).max(height / background_size.y);
                let panel_width = width * 0.21;
                let panel_open_x = -width / 2.0 + panel_width / 2.0;
                let panel_closed_x = panel_open_x - panel_width * 1.05;
                let cursor_size = (height * 0.055).clamp(28.0, 320.0);
                if menu_texture.is_some() && size != viewport {
                    viewport = size;
                    if let Some(background_entity) = background {
                        if let Ok((mut transform, _)) = sprites.get_mut(background_entity) {
                            transform.scale = bevy::math::Vec3::splat(cover);
                        }
                    }
                    winit.focused_mode = bevy::winit::UpdateMode::Continuous;
                    phase = 1;
                    idle_at = Some(now + std::time::Duration::from_millis(1200));
                }
                if input.just_pressed(bevy::input::keyboard::KeyCode::F1) {
                    panel_open = !panel_open;
                    if panel_open {
                        panel_animation = 1;
                    } else {
                        panel_animation = 2;
                    }
                }
                let control = input.pressed(bevy::input::keyboard::KeyCode::ControlLeft)
                    || input.pressed(bevy::input::keyboard::KeyCode::ControlRight);
                let mut direction = bevy::math::Vec2::ZERO;
                if control {
                    if input.pressed(bevy::input::keyboard::KeyCode::KeyW) {
                        direction.y += 1.0;
                    }
                    if input.pressed(bevy::input::keyboard::KeyCode::KeyS) {
                        direction.y -= 1.0;
                    }
                    if input.pressed(bevy::input::keyboard::KeyCode::KeyA) {
                        direction.x -= 1.0;
                    }
                    if input.pressed(bevy::input::keyboard::KeyCode::KeyD) {
                        direction.x += 1.0;
                    }
                }
                if direction != bevy::math::Vec2::ZERO {
                    direction = direction.normalize();
                }
                let target = direction * height * 0.55;
                cursor_velocity += (target - cursor_velocity) * (1.0 - (-delta / 0.045).exp());
                cursor_position += cursor_velocity * delta;
                let limit_x = (width * 0.5 - cursor_size * 0.5).max(0.0);
                let limit_y = (height * 0.5 - cursor_size * 0.5).max(0.0);
                cursor_position.x = cursor_position.x.clamp(-limit_x, limit_x);
                cursor_position.y = cursor_position.y.clamp(-limit_y, limit_y);
                if let Some(cursor_entity) = cursor {
                    if let Ok((mut transform, mut sprite)) = sprites.get_mut(cursor_entity) {
                        transform.translation.x = cursor_position.x;
                        transform.translation.y = cursor_position.y;
                        transform.translation.z = 2.0;
                        sprite.custom_size = Some(bevy::math::Vec2::splat(cursor_size));
                    }
                }
                if panel_animation != 0 {
                    let step = delta / 0.8;
                    if panel_animation == 1 {
                        panel_progress = (panel_progress + step).min(1.0);
                    } else {
                        panel_progress = (panel_progress - step).max(0.0);
                    }
                    let eased = panel_progress
                        * panel_progress
                        * panel_progress
                        * (panel_progress * (panel_progress * 6.0 - 15.0) + 10.0);
                    if let Some(panel_entity) = panel {
                        if let Ok((mut transform, mut sprite)) = sprites.get_mut(panel_entity) {
                            transform.translation.x =
                                panel_closed_x + (panel_open_x - panel_closed_x) * eased;
                            transform.translation.z = 1.0;
                            sprite.custom_size =
                                Some(bevy::math::Vec2::new(panel_width, height));
                            sprite.color =
                                bevy::color::Color::srgba(1.0, 1.0, 1.0, 0.92 * eased);
                        }
                    }
                    if panel_animation == 1 && panel_progress >= 1.0 {
                        panel_animation = 0;
                    }
                    if panel_animation == 2 && panel_progress <= 0.0 {
                        panel_animation = 0;
                    }
                }
                if panel_animation != 0 || control || cursor_velocity.length_squared() > 4.0 {
                    winit.focused_mode = bevy::winit::UpdateMode::Continuous;
                    phase = 1;
                    idle_at = Some(now + std::time::Duration::from_millis(1200));
                } else if phase == 1 {
                    if let Some(deadline) = idle_at {
                        if deadline <= now {
                            *winit = bevy::winit::WinitSettings::desktop_app();
                            idle_at = None;
                            phase = 2;
                        }
                    }
                }
                if input.just_pressed(bevy::input::keyboard::KeyCode::Escape) {
                    escape_since = Some(now);
                    winit.focused_mode =
                        bevy::winit::UpdateMode::reactive(std::time::Duration::from_millis(100));
                }
                if input.just_released(bevy::input::keyboard::KeyCode::Escape) {
                    escape_since = None;
                    winit.focused_mode = bevy::winit::UpdateMode::Continuous;
                    phase = 1;
                    idle_at = Some(now + std::time::Duration::from_millis(1200));
                }
                if let Some(pressed_at) = escape_since {
                    if now - pressed_at >= std::time::Duration::from_secs(7) {
                        commands.write_message(bevy::app::AppExit::Success);
                        escape_since = None;
                    }
                }
            },
        )
        .run();
}
