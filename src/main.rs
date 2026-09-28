#![deny(warnings)]
#![forbid(unsafe_code)]

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let background_bytes: &[u8] = include_bytes!("../assets/000.ktx2");
    let menu_bytes: &[u8] = include_bytes!("../assets/001.ktx2");
    let cursor_bytes: &[u8] = include_bytes!("../assets/cursor.ktx2");
    let background_transcoder =
        basisu::Transcoder::new(background_bytes).expect("background ktx2 container");
    let menu_transcoder = basisu::Transcoder::new(menu_bytes).expect("menu ktx2 container");
    let cursor_transcoder = basisu::Transcoder::new(cursor_bytes).expect("cursor ktx2 container");
    let (background_source_width, background_source_height) = background_transcoder.base_dimensions();
    let (menu_source_width, menu_source_height) = menu_transcoder.base_dimensions();
    let (cursor_source_width, cursor_source_height) = cursor_transcoder.base_dimensions();
    let background_level_count = background_transcoder.level_count();
    let menu_level_count = menu_transcoder.level_count();
    let cursor_level_count = cursor_transcoder.level_count();
    let background_size = bevy::math::Vec2::new(
        background_source_width as f32,
        background_source_height as f32,
    );
    let mut phase: u8 = 0;
    let mut background: Option<bevy::ecs::entity::Entity> = None;
    let mut panel: Option<bevy::ecs::entity::Entity> = None;
    let mut cursor: Option<bevy::ecs::entity::Entity> = None;
    let mut menu_texture: Option<bevy::asset::Handle<bevy::image::Image>> = None;
    let mut viewport: (u32, u32) = (0, 0);
    let mut background_level: u32 = u32::MAX;
    let mut menu_level: u32 = u32::MAX;
    let mut cursor_level: u32 = u32::MAX;
    let mut panel_open = false;
    let mut panel_progress: f32 = 0.0;
    let mut panel_animation: u8 = 0;
    let mut panel_drawn = false;
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
                            memory_hints: bevy::render::settings::MemoryHints::Performance,
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
                    &mut bevy::camera::visibility::Visibility,
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
                let width = size.0 as f32;
                let height = size.1 as f32;
                let cover = (width / background_size.x).max(height / background_size.y);
                let panel_width = width * 0.21;
                let panel_open_x = -width / 2.0 + panel_width / 2.0;
                let panel_closed_x = panel_open_x - panel_width * 1.05;
                let cursor_size = (height * 0.055).clamp(28.0, 320.0);
                let mut needed_background_level = 0u32;
                while needed_background_level + 1 < background_level_count
                    && (background_source_width >> (needed_background_level + 1)) >= size.0
                    && (background_source_height >> (needed_background_level + 1)) >= size.1
                {
                    needed_background_level += 1;
                }
                let mut needed_menu_level = 0u32;
                while needed_menu_level + 1 < menu_level_count
                    && (menu_source_width >> (needed_menu_level + 1))
                        >= (panel_width.ceil() as u32).max(1)
                    && (menu_source_height >> (needed_menu_level + 1)) >= size.1
                {
                    needed_menu_level += 1;
                }
                let mut needed_cursor_level = 0u32;
                while needed_cursor_level + 1 < cursor_level_count
                    && (cursor_source_width >> (needed_cursor_level + 1))
                        >= (cursor_size.ceil() as u32).max(1)
                {
                    needed_cursor_level += 1;
                }
                let background_mips = if cover >= 1.0 {
                    1
                } else {
                    background_level_count - needed_background_level
                };
                let menu_mips = if panel_width >= (menu_source_width >> needed_menu_level) as f32
                    && height >= (menu_source_height >> needed_menu_level) as f32
                {
                    1
                } else {
                    menu_level_count - needed_menu_level
                };
                let cursor_mips = if cursor_size >= (cursor_source_width >> needed_cursor_level) as f32
                {
                    1
                } else {
                    cursor_level_count - needed_cursor_level
                };
                if needed_background_level != background_level
                    || needed_menu_level != menu_level
                    || needed_cursor_level != cursor_level
                {
                    let (background_image, menu_image, cursor_image) = std::thread::scope(|scope| {
                        let menu_thread = scope.spawn(|| {
                            let level = needed_menu_level;
                            let mut level_bytes = 0usize;
                            let mut level_index = 0u32;
                            while level_index < menu_mips {
                                level_bytes += (((menu_source_width >> (level + level_index)).max(1) + 3) / 4)
                                    as usize
                                    * (((menu_source_height >> (level + level_index)).max(1) + 3) / 4)
                                        as usize
                                    * 16;
                                level_index += 1;
                            }
                            let mut pixels = Vec::with_capacity(level_bytes);
                            level_index = 0u32;
                            while level_index < menu_mips {
                                pixels.extend_from_slice(
                                    &menu_transcoder
                                        .transcode(
                                            level + level_index,
                                            basisu::TargetFormat::Bc7Rgba,
                                            basisu::DecodeFlags::NONE,
                                        )
                                        .expect("menu ktx2 bc7 transcode"),
                                );
                                level_index += 1;
                            }
                            let mut image = bevy::image::Image::new(
                                bevy::render::render_resource::Extent3d {
                                    width: menu_source_width >> level,
                                    height: menu_source_height >> level,
                                    depth_or_array_layers: 1,
                                },
                                bevy::render::render_resource::TextureDimension::D2,
                                pixels,
                                bevy::render::render_resource::TextureFormat::Bc7RgbaUnormSrgb,
                                bevy::asset::RenderAssetUsages::RENDER_WORLD,
                            );
                            image.texture_descriptor.mip_level_count = menu_mips;
                            image.sampler = bevy::image::ImageSampler::Descriptor(
                                bevy::render::render_resource::SamplerDescriptor {
                                    address_mode_u:
                                        bevy::render::render_resource::AddressMode::ClampToEdge,
                                    address_mode_v:
                                        bevy::render::render_resource::AddressMode::ClampToEdge,
                                    address_mode_w:
                                        bevy::render::render_resource::AddressMode::ClampToEdge,
                                    mag_filter: bevy::render::render_resource::FilterMode::Linear,
                                    min_filter: bevy::render::render_resource::FilterMode::Linear,
                                    mipmap_filter:
                                        bevy::render::render_resource::MipmapFilterMode::Linear,
                                    ..Default::default()
                                }
                                .into(),
                            );
                            image
                        });
                        let cursor_thread = scope.spawn(|| {
                            let level = needed_cursor_level;
                            let mut level_bytes = 0usize;
                            let mut level_index = 0u32;
                            while level_index < cursor_mips {
                                level_bytes += (((cursor_source_width >> (level + level_index)).max(1) + 3) / 4)
                                    as usize
                                    * (((cursor_source_height >> (level + level_index)).max(1) + 3) / 4)
                                        as usize
                                    * 16;
                                level_index += 1;
                            }
                            let mut pixels = Vec::with_capacity(level_bytes);
                            level_index = 0u32;
                            while level_index < cursor_mips {
                                pixels.extend_from_slice(
                                    &cursor_transcoder
                                        .transcode(
                                            level + level_index,
                                            basisu::TargetFormat::Bc7Rgba,
                                            basisu::DecodeFlags::NONE,
                                        )
                                        .expect("cursor ktx2 bc7 transcode"),
                                );
                                level_index += 1;
                            }
                            let mut image = bevy::image::Image::new(
                                bevy::render::render_resource::Extent3d {
                                    width: cursor_source_width >> level,
                                    height: cursor_source_height >> level,
                                    depth_or_array_layers: 1,
                                },
                                bevy::render::render_resource::TextureDimension::D2,
                                pixels,
                                bevy::render::render_resource::TextureFormat::Bc7RgbaUnormSrgb,
                                bevy::asset::RenderAssetUsages::RENDER_WORLD,
                            );
                            image.texture_descriptor.mip_level_count = cursor_mips;
                            image.sampler = bevy::image::ImageSampler::Descriptor(
                                bevy::render::render_resource::SamplerDescriptor {
                                    address_mode_u:
                                        bevy::render::render_resource::AddressMode::ClampToEdge,
                                    address_mode_v:
                                        bevy::render::render_resource::AddressMode::ClampToEdge,
                                    address_mode_w:
                                        bevy::render::render_resource::AddressMode::ClampToEdge,
                                    mag_filter: bevy::render::render_resource::FilterMode::Linear,
                                    min_filter: bevy::render::render_resource::FilterMode::Linear,
                                    mipmap_filter:
                                        bevy::render::render_resource::MipmapFilterMode::Linear,
                                    ..Default::default()
                                }
                                .into(),
                            );
                            image
                        });
                        let background_image = {
                            let level = needed_background_level;
                            let mut level_bytes = 0usize;
                            let mut level_index = 0u32;
                            while level_index < background_mips {
                                level_bytes += (((background_source_width >> (level + level_index)).max(1) + 3) / 4)
                                    as usize
                                    * (((background_source_height >> (level + level_index)).max(1) + 3) / 4)
                                        as usize
                                    * 16;
                                level_index += 1;
                            }
                            let mut pixels = Vec::with_capacity(level_bytes);
                            level_index = 0u32;
                            while level_index < background_mips {
                                pixels.extend_from_slice(
                                    &background_transcoder
                                        .transcode(
                                            level + level_index,
                                            basisu::TargetFormat::Bc7Rgba,
                                            basisu::DecodeFlags::NONE,
                                        )
                                        .expect("background ktx2 bc7 transcode"),
                                );
                                level_index += 1;
                            }
                            let mut image = bevy::image::Image::new(
                                bevy::render::render_resource::Extent3d {
                                    width: background_source_width >> level,
                                    height: background_source_height >> level,
                                    depth_or_array_layers: 1,
                                },
                                bevy::render::render_resource::TextureDimension::D2,
                                pixels,
                                bevy::render::render_resource::TextureFormat::Bc7RgbaUnormSrgb,
                                bevy::asset::RenderAssetUsages::RENDER_WORLD,
                            );
                            image.texture_descriptor.mip_level_count = background_mips;
                            image.sampler = bevy::image::ImageSampler::Descriptor(
                                bevy::render::render_resource::SamplerDescriptor {
                                    address_mode_u:
                                        bevy::render::render_resource::AddressMode::ClampToEdge,
                                    address_mode_v:
                                        bevy::render::render_resource::AddressMode::ClampToEdge,
                                    address_mode_w:
                                        bevy::render::render_resource::AddressMode::ClampToEdge,
                                    mag_filter: bevy::render::render_resource::FilterMode::Linear,
                                    min_filter: bevy::render::render_resource::FilterMode::Linear,
                                    mipmap_filter:
                                        bevy::render::render_resource::MipmapFilterMode::Linear,
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
                    let menu_handle = images.add(menu_image);
                    let background_handle = images.add(background_image);
                    let cursor_handle = images.add(cursor_image);
                    if phase == 0 {
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
                                    bevy::camera::visibility::Visibility::Visible,
                                ))
                                .id(),
                        );
                        commands.spawn((
                            bevy::camera::Camera2d,
                            bevy::camera::Camera {
                                clear_color: bevy::camera::ClearColorConfig::Custom(
                                    bevy::color::Color::BLACK,
                                ),
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
                                        panel_closed_x,
                                        0.0,
                                        1.0,
                                    ),
                                    bevy::camera::visibility::Visibility::Hidden,
                                ))
                                .id(),
                        );
                        cursor = Some(
                            commands
                                .spawn((
                                    bevy::sprite::Sprite {
                                        image: cursor_handle,
                                        custom_size: Some(bevy::math::Vec2::splat(cursor_size)),
                                        color: bevy::color::Color::WHITE,
                                        ..Default::default()
                                    },
                                    bevy::transform::components::Transform::from_xyz(
                                        0.0, 0.0, 2.0,
                                    ),
                                    bevy::camera::visibility::Visibility::Visible,
                                ))
                                .id(),
                        );
                        menu_texture = Some(menu_handle);
                        window.visible = true;
                        viewport = size;
                        phase = 1;
                        idle_at = Some(now + std::time::Duration::from_millis(1200));
                    } else {
                        if let Some(background_entity) = background {
                            if let Ok((_, mut sprite, _)) = sprites.get_mut(background_entity) {
                                sprite.image = background_handle;
                            }
                        }
                        if let Some(panel_entity) = panel {
                            if let Ok((_, mut sprite, _)) = sprites.get_mut(panel_entity) {
                                sprite.image = menu_handle.clone();
                            }
                        }
                        if let Some(cursor_entity) = cursor {
                            if let Ok((_, mut sprite, _)) = sprites.get_mut(cursor_entity) {
                                sprite.image = cursor_handle;
                            }
                        }
                        menu_texture = Some(menu_handle);
                    }
                    background_level = needed_background_level;
                    menu_level = needed_menu_level;
                    cursor_level = needed_cursor_level;
                }
                if menu_texture.is_some() && size != viewport {
                    viewport = size;
                    if let Some(background_entity) = background {
                        if let Ok((mut transform, _, _)) = sprites.get_mut(background_entity) {
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
                let previous_position = cursor_position;
                cursor_position += cursor_velocity * delta;
                let limit_x = (width * 0.5 - cursor_size * 0.5).max(0.0);
                let limit_y = (height * 0.5 - cursor_size * 0.5).max(0.0);
                cursor_position.x = cursor_position.x.clamp(-limit_x, limit_x);
                cursor_position.y = cursor_position.y.clamp(-limit_y, limit_y);
                if cursor_position != previous_position {
                    if let Some(cursor_entity) = cursor {
                        if let Ok((mut transform, mut sprite, _)) =
                            sprites.get_mut(cursor_entity)
                        {
                            transform.translation.x = cursor_position.x;
                            transform.translation.y = cursor_position.y;
                            transform.translation.z = 2.0;
                            sprite.custom_size = Some(bevy::math::Vec2::splat(cursor_size));
                        }
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
                        if let Ok((mut transform, mut sprite, mut visibility)) =
                            sprites.get_mut(panel_entity)
                        {
                            transform.translation.x =
                                panel_closed_x + (panel_open_x - panel_closed_x) * eased;
                            transform.translation.z = 1.0;
                            sprite.custom_size =
                                Some(bevy::math::Vec2::new(panel_width, height));
                            sprite.color =
                                bevy::color::Color::srgba(1.0, 1.0, 1.0, 0.92 * eased);
                            if eased > 0.0 {
                                *visibility = bevy::camera::visibility::Visibility::Visible;
                                panel_drawn = true;
                            } else if panel_drawn {
                                *visibility = bevy::camera::visibility::Visibility::Hidden;
                                panel_drawn = false;
                            }
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
                            *winit = bevy::winit::WinitSettings {
                                focused_mode: bevy::winit::UpdateMode::reactive(
                                    std::time::Duration::from_secs(30),
                                ),
                                unfocused_mode: bevy::winit::UpdateMode::reactive_low_power(
                                    std::time::Duration::from_secs(60),
                                ),
                            };
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
