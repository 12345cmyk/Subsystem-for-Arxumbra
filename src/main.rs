#![forbid(unsafe_code)]
#![deny(warnings)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let background_source: &'static [u8] = include_bytes!("../assets/000.ktx2");
    let menu_source: &'static [u8] = include_bytes!("../assets/001.ktx2");
    let cursor_source: &'static [u8] = include_bytes!("../assets/cursor.ktx2");
    let background = basisu::Transcoder::new(background_source).expect("background ktx2");
    let menu = basisu::Transcoder::new(menu_source).expect("menu ktx2");
    let cursor = basisu::Transcoder::new(cursor_source).expect("cursor ktx2");
    let (background_width, background_height) = background.base_dimensions();
    let (menu_width, menu_height) = menu.base_dimensions();
    let (cursor_width, cursor_height) = cursor.base_dimensions();
    let background_levels = background.level_count().max(1);
    let menu_levels = menu.level_count().max(1);
    let cursor_levels = cursor.level_count().max(1);
    let background_head_bytes = (((background_width + 3) >> 2) as usize)
        * (((background_height + 3) >> 2) as usize)
        * 16;
    let background_total_bytes = (0..background_levels).fold(0usize, |acc, level| {
        let w = (background_width >> level).max(1);
        let h = (background_height >> level).max(1);
        acc + (((w + 3) >> 2) as usize) * (((h + 3) >> 2) as usize) * 16
    });
    let menu_head_bytes =
        (((menu_width + 3) >> 2) as usize) * (((menu_height + 3) >> 2) as usize) * 16;
    let menu_total_bytes = (0..menu_levels).fold(0usize, |acc, level| {
        let w = (menu_width >> level).max(1);
        let h = (menu_height >> level).max(1);
        acc + (((w + 3) >> 2) as usize) * (((h + 3) >> 2) as usize) * 16
    });
    let cursor_total_bytes = (0..cursor_levels).fold(0usize, |acc, level| {
        let w = (cursor_width >> level).max(1);
        let h = (cursor_height >> level).max(1);
        acc + (((w + 3) >> 2) as usize) * (((h + 3) >> 2) as usize) * 16
    });
    let mut background_pixels = vec![0u8; background_total_bytes];
    let mut menu_pixels = vec![0u8; menu_total_bytes];
    let mut cursor_pixels = vec![0u8; cursor_total_bytes];
    {
        let (background_head, background_tail) =
            background_pixels.split_at_mut(background_head_bytes);
        let (menu_head, menu_tail) = menu_pixels.split_at_mut(menu_head_bytes);
        let cursor_slice = &mut cursor_pixels[..];
        let background_ref = &background;
        let menu_ref = &menu;
        let cursor_ref = &cursor;
        std::thread::scope(|scope| {
            let bg_head_job = scope.spawn(move || {
                background_ref
                    .transcode_into(
                        0,
                        basisu::TargetFormat::Bc7Rgba,
                        basisu::DecodeFlags::NONE,
                        background_head,
                    )
                    .expect("background head transcode");
            });
            let bg_tail_job = scope.spawn(move || {
                let mut offset = 0usize;
                let mut level = 1u32;
                while level < background_levels {
                    let w = (background_width >> level).max(1);
                    let h = (background_height >> level).max(1);
                    let size = (((w + 3) >> 2) as usize) * (((h + 3) >> 2) as usize) * 16;
                    background_ref
                        .transcode_into(
                            level,
                            basisu::TargetFormat::Bc7Rgba,
                            basisu::DecodeFlags::NONE,
                            &mut background_tail[offset..offset + size],
                        )
                        .expect("background tail transcode");
                    offset += size;
                    level += 1;
                }
            });
            let menu_head_job = scope.spawn(move || {
                menu_ref
                    .transcode_into(
                        0,
                        basisu::TargetFormat::Bc7Rgba,
                        basisu::DecodeFlags::NONE,
                        menu_head,
                    )
                    .expect("menu head transcode");
            });
            let menu_tail_job = scope.spawn(move || {
                let mut offset = 0usize;
                let mut level = 1u32;
                while level < menu_levels {
                    let w = (menu_width >> level).max(1);
                    let h = (menu_height >> level).max(1);
                    let size = (((w + 3) >> 2) as usize) * (((h + 3) >> 2) as usize) * 16;
                    menu_ref
                        .transcode_into(
                            level,
                            basisu::TargetFormat::Bc7Rgba,
                            basisu::DecodeFlags::NONE,
                            &mut menu_tail[offset..offset + size],
                        )
                        .expect("menu tail transcode");
                    offset += size;
                    level += 1;
                }
            });
            let cursor_job = scope.spawn(move || {
                let mut offset = 0usize;
                let mut level = 0u32;
                while level < cursor_levels {
                    let w = (cursor_width >> level).max(1);
                    let h = (cursor_height >> level).max(1);
                    let size = (((w + 3) >> 2) as usize) * (((h + 3) >> 2) as usize) * 16;
                    cursor_ref
                        .transcode_into(
                            level,
                            basisu::TargetFormat::Bc7Rgba,
                            basisu::DecodeFlags::NONE,
                            &mut cursor_slice[offset..offset + size],
                        )
                        .expect("cursor transcode");
                    offset += size;
                    level += 1;
                }
            });
            bg_head_job.join().expect("background head thread");
            bg_tail_job.join().expect("background tail thread");
            menu_head_job.join().expect("menu head thread");
            menu_tail_job.join().expect("menu tail thread");
            cursor_job.join().expect("cursor thread");
        });
    }
    drop(cursor);
    drop(menu);
    drop(background);
    let sampler_descriptor = bevy::image::ImageSampler::Descriptor(
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
    let mut background_image = bevy::image::Image::new(
        bevy::render::render_resource::Extent3d {
            width: background_width,
            height: background_height,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        background_pixels,
        bevy::render::render_resource::TextureFormat::Bc7RgbaUnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    background_image.texture_descriptor.mip_level_count = background_levels;
    background_image.sampler = sampler_descriptor.clone();
    let mut menu_image = bevy::image::Image::new(
        bevy::render::render_resource::Extent3d {
            width: menu_width,
            height: menu_height,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        menu_pixels,
        bevy::render::render_resource::TextureFormat::Bc7RgbaUnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    menu_image.texture_descriptor.mip_level_count = menu_levels;
    menu_image.sampler = sampler_descriptor.clone();
    let mut cursor_image = bevy::image::Image::new(
        bevy::render::render_resource::Extent3d {
            width: cursor_width,
            height: cursor_height,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        cursor_pixels,
        bevy::render::render_resource::TextureFormat::Bc7RgbaUnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    cursor_image.texture_descriptor.mip_level_count = cursor_levels;
    cursor_image.sampler = sampler_descriptor;
    let mut background_upload = Some(background_image);
    let mut menu_upload = Some(menu_image);
    let mut cursor_upload = Some(cursor_image);
    let mut phase: u8 = 0;
    let mut warmup_frames: u16 = 180;
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
        .insert_resource(bevy::color::ClearColor(bevy::color::Color::BLACK))
        .add_plugins((
            bevy::app::TaskPoolPlugin::default(),
            bevy::diagnostic::FrameCountPlugin,
            bevy::time::TimePlugin,
            bevy::transform::TransformPlugin,
            bevy::input::InputPlugin,
            bevy::input_focus::InputFocusPlugin,
            bevy::window::WindowPlugin {
                primary_window: Some(bevy::window::Window {
                    title: String::from("Subsystem for Arxumbra"),
                    name: Some(String::from("subsystem-for-arxumbra")),
                    mode: bevy::window::WindowMode::BorderlessFullscreen(
                        bevy::window::MonitorSelection::Primary,
                    ),
                    present_mode: bevy::window::PresentMode::AutoVsync,
                    decorations: false,
                    resizable: false,
                    visible: true,
                    desired_maximum_frame_latency: core::num::NonZeroU32::new(2),
                    ..Default::default()
                }),
                primary_cursor_options: Some(bevy::window::CursorOptions {
                    visible: false,
                    grab_mode: bevy::window::CursorGrabMode::None,
                    hit_test: true,
                }),
                exit_condition: bevy::window::ExitCondition::OnPrimaryClosed,
                close_when_requested: true,
            },
            bevy::asset::AssetPlugin::default(),
            bevy::winit::WinitPlugin::default(),
            bevy::image::ImagePlugin::default(),
            bevy::mesh::MeshPlugin,
            bevy::camera::CameraPlugin,
            bevy::light::LightPlugin,
            bevy::render::RenderPlugin {
                render_creation: bevy::render::settings::RenderCreation::Automatic(
                    bevy::render::settings::WgpuSettings {
                        backends: Some(bevy::render::settings::Backends::PRIMARY),
                        power_preference:
                            bevy::render::settings::PowerPreference::HighPerformance,
                        disabled_features: Some(
                            bevy::render::settings::WgpuFeatures::TEXTURE_BINDING_ARRAY
                                | bevy::render::settings::WgpuFeatures::BUFFER_BINDING_ARRAY,
                        ),
                        ..Default::default()
                    },
                ),
                synchronous_pipeline_compilation: true,
                ..Default::default()
            },
            bevy::core_pipeline::CorePipelinePlugin,
            bevy::sprite::SpritePlugin,
            bevy::sprite_render::SpriteRenderPlugin,
        ))
        .insert_resource(bevy::winit::WinitSettings {
            focused_mode: bevy::winit::UpdateMode::Continuous,
            unfocused_mode: bevy::winit::UpdateMode::Continuous,
        })
        .add_systems(
            bevy::app::Update,
            move |mut commands: bevy::ecs::system::Commands,
                  mut windows: bevy::ecs::system::Query<
                      (bevy::ecs::entity::Entity, &mut bevy::window::Window),
                      bevy::ecs::query::With<bevy::window::PrimaryWindow>,
                  >,
                  mut window_cursors: bevy::ecs::system::Query<
                      &mut bevy::window::CursorOptions,
                      bevy::ecs::query::With<bevy::window::PrimaryWindow>,
                  >,
                  mut images: bevy::ecs::system::ResMut<bevy::asset::Assets<bevy::image::Image>>,
                  mut sprites: bevy::ecs::system::Query<(
                      &mut bevy::transform::components::Transform,
                      &mut bevy::sprite::Sprite,
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
                let (window_entity, mut window) = windows.single_mut().expect("primary window");
                if window.mode
                    != bevy::window::WindowMode::BorderlessFullscreen(
                        bevy::window::MonitorSelection::Primary,
                    )
                {
                    window.mode = bevy::window::WindowMode::BorderlessFullscreen(
                        bevy::window::MonitorSelection::Primary,
                    );
                }
                if window.present_mode != bevy::window::PresentMode::AutoVsync {
                    window.present_mode = bevy::window::PresentMode::AutoVsync;
                }
                if !window.visible {
                    window.visible = true;
                }
                if window_cursors.is_empty() {
                    commands
                        .entity(window_entity)
                        .insert(bevy::window::CursorOptions {
                            visible: false,
                            grab_mode: bevy::window::CursorGrabMode::None,
                            hit_test: true,
                        });
                } else {
                    let mut cursor_options =
                        window_cursors.single_mut().expect("window cursor options");
                    if cursor_options.visible {
                        cursor_options.visible = false;
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
                    let background_sheet =
                        images.add(background_upload.take().expect("background image"));
                    let menu_sheet = images.add(menu_upload.take().expect("menu image"));
                    let cursor_sheet = images.add(cursor_upload.take().expect("cursor image"));
                    let cover =
                        (width / background_width as f32).max(height / background_height as f32);
                    cursor_position = bevy::math::Vec2::ZERO;
                    cursor_velocity = bevy::math::Vec2::ZERO;
                    commands.spawn(bevy::camera::Camera2d);
                    background_entity = commands
                        .spawn((
                            bevy::sprite::Sprite {
                                image: background_sheet,
                                custom_size: Some(bevy::math::Vec2::new(
                                    background_width as f32 * cover,
                                    background_height as f32 * cover,
                                )),
                                alpha_mode: bevy::sprite::SpriteAlphaMode::Opaque,
                                ..Default::default()
                            },
                            bevy::transform::components::Transform::from_xyz(0.0, 0.0, 0.0),
                            bevy::camera::visibility::NoFrustumCulling,
                        ))
                        .id();
                    panel_entity = commands
                        .spawn((
                            bevy::sprite::Sprite {
                                image: menu_sheet,
                                custom_size: Some(bevy::math::Vec2::new(panel_width, height)),
                                color: bevy::color::Color::srgba(1.0, 1.0, 1.0, 0.92),
                                alpha_mode: bevy::sprite::SpriteAlphaMode::Blend,
                                ..Default::default()
                            },
                            bevy::transform::components::Transform::from_xyz(closed, 0.0, 1.0),
                            bevy::camera::visibility::Visibility::Hidden,
                            bevy::camera::visibility::NoFrustumCulling,
                        ))
                        .id();
                    cursor_entity = commands
                        .spawn((
                            bevy::sprite::Sprite {
                                image: cursor_sheet,
                                custom_size: Some(bevy::math::Vec2::splat(cursor_extent)),
                                alpha_mode: bevy::sprite::SpriteAlphaMode::Blend,
                                ..Default::default()
                            },
                            bevy::transform::components::Transform::from_xyz(0.0, 0.0, 2.0),
                            bevy::camera::visibility::NoFrustumCulling,
                        ))
                        .id();
                    viewport = (physical_width, physical_height);
                    warmup_frames = 180;
                    let ready_now = std::time::Instant::now();
                    winit.focused_mode = bevy::winit::UpdateMode::Continuous;
                    winit.unfocused_mode = bevy::winit::UpdateMode::Continuous;
                    idle_deadline = Some(ready_now + std::time::Duration::from_millis(2500));
                    phase = 1;
                    return;
                }
                if geometry_changed {
                    let cover =
                        (width / background_width as f32).max(height / background_height as f32);
                    cursor_position = cursor_position.max(-cursor_limit).min(cursor_limit);
                    if let Ok((_, mut sprite, _, _)) = sprites.get_mut(background_entity) {
                        sprite.custom_size = Some(bevy::math::Vec2::new(
                            background_width as f32 * cover,
                            background_height as f32 * cover,
                        ));
                    }
                    if let Ok((mut transform, mut sprite, mut visibility, _)) =
                        sprites.get_mut(panel_entity)
                    {
                        let eased = panel_progress
                            * panel_progress
                            * panel_progress
                            * (panel_progress * (panel_progress * 6.0 - 15.0) + 10.0);
                        transform.translation.x = closed + (open_x - closed) * eased;
                        sprite.custom_size = Some(bevy::math::Vec2::new(panel_width, height));
                        if panel_progress > 0.0 {
                            *visibility = bevy::camera::visibility::Visibility::Visible;
                            panel_drawn = true;
                        } else {
                            *visibility = bevy::camera::visibility::Visibility::Hidden;
                            panel_drawn = false;
                        }
                    }
                    if let Ok((mut transform, mut sprite, _, _)) = sprites.get_mut(cursor_entity) {
                        sprite.custom_size = Some(bevy::math::Vec2::splat(cursor_extent));
                        transform.translation.x = cursor_position.x;
                        transform.translation.y = cursor_position.y;
                    }
                    viewport = (physical_width, physical_height);
                    warmup_frames = 180;
                    let resize_now = std::time::Instant::now();
                    winit.focused_mode = bevy::winit::UpdateMode::Continuous;
                    winit.unfocused_mode = bevy::winit::UpdateMode::Continuous;
                    idle_deadline = Some(resize_now + std::time::Duration::from_millis(2500));
                    phase = 1;
                }
                if warmup_frames > 0 {
                    warmup_frames -= 1;
                    if warmup_frames == 176
                        || warmup_frames == 168
                        || warmup_frames == 152
                        || warmup_frames == 120
                        || warmup_frames == 60
                        || warmup_frames == 1
                    {
                        for (_, _, _, mesh2d) in sprites.iter_mut() {
                            if let Some(mut mesh) = mesh2d {
                                mesh.set_changed();
                            }
                        }
                    }
                }
                let now = std::time::Instant::now();
                let control = input.pressed(bevy::input::keyboard::KeyCode::ControlLeft)
                    || input.pressed(bevy::input::keyboard::KeyCode::ControlRight);
                let direction = if control {
                    let right = input.pressed(bevy::input::keyboard::KeyCode::ArrowRight) as i8
                        - input.pressed(bevy::input::keyboard::KeyCode::ArrowLeft) as i8;
                    let up = input.pressed(bevy::input::keyboard::KeyCode::ArrowUp) as i8
                        - input.pressed(bevy::input::keyboard::KeyCode::ArrowDown) as i8;
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
                    if let Ok((mut transform, _, _, _)) = sprites.get_mut(cursor_entity) {
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
                    let eased = progress
                        * progress
                        * progress
                        * (progress * (progress * 6.0 - 15.0) + 10.0);
                    if let Ok((mut transform, _, mut visibility, _)) =
                        sprites.get_mut(panel_entity)
                    {
                        transform.translation.x = closed + (open_x - closed) * eased;
                        if progress > 0.0 {
                            if !panel_drawn {
                                *visibility = bevy::camera::visibility::Visibility::Visible;
                                panel_drawn = true;
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
                        winit.focused_mode = bevy::winit::UpdateMode::Continuous;
                        winit.unfocused_mode = bevy::winit::UpdateMode::Continuous;
                        phase = 1;
                    }
                } else if phase == 1 {
                    if let Some(deadline) = idle_deadline {
                        if deadline <= now {
                            winit.focused_mode = bevy::winit::UpdateMode::reactive_low_power(
                                std::time::Duration::from_secs(1),
                            );
                            winit.unfocused_mode = bevy::winit::UpdateMode::reactive_low_power(
                                std::time::Duration::from_secs(1),
                            );
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
