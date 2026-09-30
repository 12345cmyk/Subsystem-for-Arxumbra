#![deny(warnings)]
#![forbid(unsafe_code)]

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let background_source: &'static [u8] = include_bytes!("../assets/000.ktx2");
    let menu_source: &'static [u8] = include_bytes!("../assets/001.ktx2");
    let cursor_source: &'static [u8] = include_bytes!("../assets/cursor.ktx2");
    let background = basisu::Transcoder::new(background_source).expect("background ktx2 container");
    let menu = basisu::Transcoder::new(menu_source).expect("menu ktx2 container");
    let cursor = basisu::Transcoder::new(cursor_source).expect("cursor ktx2 container");
    let (background_width, background_height) = background.base_dimensions();
    let (menu_width, menu_height) = menu.base_dimensions();
    let (cursor_width, cursor_height) = cursor.base_dimensions();
    let background_levels = background.level_count();
    let menu_levels = menu.level_count();
    let cursor_levels = cursor.level_count();
    let background_size =
        bevy::math::Vec2::new(background_width as f32, background_height as f32);
    let low_power =
        bevy::winit::UpdateMode::reactive_low_power(std::time::Duration::from_secs(1));
    let smootherstep: fn(f32) -> f32 = |t| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let mut phase: u8 = 0;
    let mut viewport: (u32, u32) = (0, 0);
    let mut background_level: u32 = u32::MAX;
    let mut background_mips: u32 = u32::MAX;
    let mut menu_level: u32 = u32::MAX;
    let mut menu_mips: u32 = u32::MAX;
    let mut cursor_level: u32 = u32::MAX;
    let mut cursor_mips: u32 = u32::MAX;
    let mut background_entity = bevy::ecs::entity::Entity::PLACEHOLDER;
    let mut panel_entity = bevy::ecs::entity::Entity::PLACEHOLDER;
    let mut cursor_entity = bevy::ecs::entity::Entity::PLACEHOLDER;
    let mut background_handle: Option<bevy::asset::Handle<bevy::image::Image>> = None;
    let mut panel_handle: Option<bevy::asset::Handle<bevy::image::Image>> = None;
    let mut cursor_handle: Option<bevy::asset::Handle<bevy::image::Image>> = None;
    let mut panel_open = false;
    let mut panel_progress: f32 = 0.0;
    let mut panel_motion: i8 = 0;
    let mut panel_drawn = false;
    let mut cursor_position = bevy::math::Vec2::ZERO;
    let mut cursor_velocity = bevy::math::Vec2::ZERO;
    let mut idle_deadline: Option<std::time::Instant> = None;
    let mut escape_since: Option<std::time::Instant> = None;
    let fullscreen = bevy::window::WindowMode::BorderlessFullscreen(
        bevy::window::MonitorSelection::Primary,
    );
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
                        mode: fullscreen,
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
                        grab_mode: bevy::window::CursorGrabMode::Confined,
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
                  mut cursor_options: bevy::ecs::system::Query<
                &mut bevy::window::CursorOptions,
            >,
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
                let booting = phase == 0;
                let mut window = windows.single_mut().expect("primary window");
                if window.mode != fullscreen {
                    window.mode = fullscreen;
                }
                if window.decorations {
                    window.decorations = false;
                }
                if window.resizable {
                    window.resizable = false;
                }
                let mut os_cursor = cursor_options.single_mut().expect("primary cursor options");
                if os_cursor.visible {
                    os_cursor.visible = false;
                }
                if os_cursor.grab_mode != bevy::window::CursorGrabMode::Confined {
                    os_cursor.grab_mode = bevy::window::CursorGrabMode::Confined;
                }
                if os_cursor.hit_test {
                    os_cursor.hit_test = false;
                }
                let (physical_width, physical_height) =
                    (window.physical_width(), window.physical_height());
                if physical_width == 0 || physical_height == 0 {
                    return;
                }
                let width = physical_width as f32;
                let height = physical_height as f32;
                let panel_width = width * 0.21;
                let panel_open_x = panel_width * 0.5 - width * 0.5;
                let panel_closed_x = panel_open_x - panel_width * 1.05;
                let cursor_extent = (height * 0.055).clamp(28.0, 320.0);
                let cover = (width / background_size.x).max(height / background_size.y);
                let cursor_limit = bevy::math::Vec2::new(
                    (width - cursor_extent).max(0.0) * 0.5,
                    (height - cursor_extent).max(0.0) * 0.5,
                );
                let geometry_changed = (physical_width, physical_height) != viewport;
                if booting || geometry_changed {
                    let select_level: fn(u32, u32, u32, u32, u32) -> u32 =
                        |source_width, source_height, levels, need_width, need_height| {
                            (31 - (((source_width / need_width).min(source_height / need_height)).max(1)).leading_zeros()).min(levels - 1)
                        };
                    let panel_need = (panel_width.ceil() as u32).max(1);
                    let cursor_need = (cursor_extent.ceil() as u32).max(1);
                    let background_target_level = select_level(
                        background_width,
                        background_height,
                        background_levels,
                        physical_width,
                        physical_height,
                    );
                    let menu_target_level = select_level(
                        menu_width,
                        menu_height,
                        menu_levels,
                        panel_need,
                        physical_height,
                    );
                    let cursor_target_level = select_level(
                        cursor_width,
                        cursor_height,
                        cursor_levels,
                        cursor_need,
                        cursor_need,
                    );
                    let background_target_mips = if cover >= 1.0 {
                        1
                    } else {
                        background_levels - background_target_level
                    };
                    let menu_target_mips = if panel_width
                        >= (menu_width >> menu_target_level) as f32
                        && height >= (menu_height >> menu_target_level) as f32
                    {
                        1
                    } else {
                        menu_levels - menu_target_level
                    };
                    let cursor_target_mips = if cursor_extent
                        >= (cursor_width >> cursor_target_level) as f32
                    {
                        1
                    } else {
                        cursor_levels - cursor_target_level
                    };
                    let background_stale = (background_target_level, background_target_mips)
                        != (background_level, background_mips);
                    let menu_stale =
                        (menu_target_level, menu_target_mips) != (menu_level, menu_mips);
                    let cursor_stale =
                        (cursor_target_level, cursor_target_mips) != (cursor_level, cursor_mips);
                    if background_stale || menu_stale || cursor_stale {
                        let build_image: fn(
                            &basisu::Transcoder<'static>,
                            u32,
                            u32,
                            u32,
                            u32,
                            &'static str,
                        ) -> bevy::image::Image = |transcoder,
                                                   source_width,
                                                   source_height,
                                                   level,
                                                   mips,
                                                   label| {
                            let mut sizes = [0usize; 16];
                            let mut total_bytes = 0usize;
                            for (i, slot) in sizes[..mips as usize].iter_mut().enumerate() {
                                let mip = level + i as u32;
                                let bw = (((source_width >> mip).max(1) + 3) >> 2) as usize;
                                let bh = (((source_height >> mip).max(1) + 3) >> 2) as usize;
                                let bytes = (bw * bh) << 4;
                                *slot = bytes;
                                total_bytes += bytes;
                            }
                            let mut pixels = vec![0u8; total_bytes];
                            let mut offset = 0usize;
                            for (i, &size) in sizes[..mips as usize].iter().enumerate() {
                                transcoder
                                    .transcode_into(
                                        level + i as u32,
                                        basisu::TargetFormat::Bc7Rgba,
                                        basisu::DecodeFlags::NONE,
                                        &mut pixels[offset..offset + size],
                                    )
                                    .expect(label);
                                offset += size;
                            }
                            let mut image = bevy::image::Image::new(
                                bevy::render::render_resource::Extent3d {
                                    width: (source_width >> level).max(1),
                                    height: (source_height >> level).max(1),
                                    depth_or_array_layers: 1,
                                },
                                bevy::render::render_resource::TextureDimension::D2,
                                pixels,
                                bevy::render::render_resource::TextureFormat::Bc7RgbaUnormSrgb,
                                bevy::asset::RenderAssetUsages::RENDER_WORLD,
                            );
                            image.texture_descriptor.mip_level_count = mips;
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
                        let background_ref = &background;
                        let menu_ref = &menu;
                        let (background_image, menu_image, cursor_image) =
                            std::thread::scope(|scope| {
                                let background_job = background_stale.then(|| {
                                    scope.spawn(move || {
                                        build_image(
                                            background_ref,
                                            background_width,
                                            background_height,
                                            background_target_level,
                                            background_target_mips,
                                            "background ktx2 bc7 transcode",
                                        )
                                    })
                                });
                                let menu_job = menu_stale.then(|| {
                                    scope.spawn(move || {
                                        build_image(
                                            menu_ref,
                                            menu_width,
                                            menu_height,
                                            menu_target_level,
                                            menu_target_mips,
                                            "menu ktx2 bc7 transcode",
                                        )
                                    })
                                });
                                let cursor_out = cursor_stale.then(|| {
                                    build_image(
                                        &cursor,
                                        cursor_width,
                                        cursor_height,
                                        cursor_target_level,
                                        cursor_target_mips,
                                        "cursor ktx2 bc7 transcode",
                                    )
                                });
                                (
                                    background_job
                                        .map(|job| job.join().expect("background ktx2 decode")),
                                    menu_job.map(|job| job.join().expect("menu ktx2 decode")),
                                    cursor_out,
                                )
                            });
                        for (
                            built_image,
                            target_level,
                            target_mips,
                            handle,
                            level_slot,
                            mips_slot,
                            entity,
                        ) in [
                            (
                                background_image,
                                background_target_level,
                                background_target_mips,
                                &mut background_handle,
                                &mut background_level,
                                &mut background_mips,
                                background_entity,
                            ),
                            (
                                menu_image,
                                menu_target_level,
                                menu_target_mips,
                                &mut panel_handle,
                                &mut menu_level,
                                &mut menu_mips,
                                panel_entity,
                            ),
                            (
                                cursor_image,
                                cursor_target_level,
                                cursor_target_mips,
                                &mut cursor_handle,
                                &mut cursor_level,
                                &mut cursor_mips,
                                cursor_entity,
                            ),
                        ] {
                            if let Some(image) = built_image {
                                let next = images.add(image);
                                let retired = handle.replace(next.clone());
                                *level_slot = target_level;
                                *mips_slot = target_mips;
                                if !booting {
                                    if let Ok((_, mut sprite, _)) = sprites.get_mut(entity) {
                                        sprite.image = next;
                                    }
                                    if let Some(retired) = retired {
                                        images.remove(&retired);
                                    }
                                }
                            }
                        }
                    }
                }
                if booting {
                    let (Some(background_current), Some(panel_current), Some(cursor_current)) = (
                        background_handle.as_ref(),
                        panel_handle.as_ref(),
                        cursor_handle.as_ref(),
                    ) else {
                        return;
                    };
                    let alpha_mode: fn(bool) -> bevy::sprite::SpriteAlphaMode = |has_alpha| {
                        if has_alpha {
                            bevy::sprite::SpriteAlphaMode::Blend
                        } else {
                            bevy::sprite::SpriteAlphaMode::Opaque
                        }
                    };
                    commands.spawn((
                        bevy::camera::Camera2d,
                        bevy::camera::Camera {
                            clear_color: bevy::camera::ClearColorConfig::Custom(
                                bevy::color::Color::BLACK,
                            ),
                            ..Default::default()
                        },
                        bevy::render::view::Msaa::Off,
                    ));
                    background_entity = commands
                        .spawn((
                            bevy::sprite::Sprite {
                                image: background_current.clone(),
                                custom_size: Some(background_size),
                                color: bevy::color::Color::WHITE,
                                alpha_mode: alpha_mode(background.has_alpha()),
                                ..Default::default()
                            },
                            bevy::transform::components::Transform::from_scale(
                                bevy::math::Vec3::splat(cover),
                            ),
                            bevy::camera::visibility::Visibility::Visible,
                        ))
                        .id();
                    panel_entity = commands
                        .spawn((
                            bevy::sprite::Sprite {
                                image: panel_current.clone(),
                                custom_size: Some(bevy::math::Vec2::new(panel_width, height)),
                                color: bevy::color::Color::srgba(1.0, 1.0, 1.0, 0.0),
                                alpha_mode: alpha_mode(menu.has_alpha()),
                                ..Default::default()
                            },
                            bevy::transform::components::Transform::from_xyz(
                                panel_closed_x,
                                0.0,
                                1.0,
                            ),
                            bevy::camera::visibility::Visibility::Hidden,
                        ))
                        .id();
                    cursor_entity = commands
                        .spawn((
                            bevy::sprite::Sprite {
                                image: cursor_current.clone(),
                                custom_size: Some(bevy::math::Vec2::splat(cursor_extent)),
                                color: bevy::color::Color::WHITE,
                                alpha_mode: alpha_mode(cursor.has_alpha()),
                                ..Default::default()
                            },
                            bevy::transform::components::Transform::from_xyz(0.0, 0.0, 2.0),
                            bevy::camera::visibility::Visibility::Visible,
                        ))
                        .id();
                    window.visible = true;
                    viewport = (physical_width, physical_height);
                    phase = 1;
                    idle_deadline = Some(now + std::time::Duration::from_millis(1200));
                } else if geometry_changed {
                    if let Ok((mut transform, mut sprite, _)) = sprites.get_mut(background_entity) {
                        transform.scale = bevy::math::Vec3::splat(cover);
                        sprite.custom_size = Some(background_size);
                    }
                    if let Ok((mut transform, mut sprite, mut visibility)) =
                        sprites.get_mut(panel_entity)
                    {
                        let eased = smootherstep(panel_progress);
                        transform.translation.x =
                            panel_closed_x + (panel_open_x - panel_closed_x) * eased;
                        sprite.custom_size = Some(bevy::math::Vec2::new(panel_width, height));
                        sprite.color = bevy::color::Color::srgba(1.0, 1.0, 1.0, 0.92 * eased);
                        let visible = panel_progress > 0.0;
                        *visibility = if visible {
                            bevy::camera::visibility::Visibility::Visible
                        } else {
                            bevy::camera::visibility::Visibility::Hidden
                        };
                        panel_drawn = visible;
                    }
                    cursor_position = cursor_position.max(-cursor_limit).min(cursor_limit);
                    if let Ok((mut transform, mut sprite, _)) = sprites.get_mut(cursor_entity) {
                        sprite.custom_size = Some(bevy::math::Vec2::splat(cursor_extent));
                        transform.translation.x = cursor_position.x;
                        transform.translation.y = cursor_position.y;
                    }
                    viewport = (physical_width, physical_height);
                } else {
                    let control = input.pressed(bevy::input::keyboard::KeyCode::ControlLeft)
                        || input.pressed(bevy::input::keyboard::KeyCode::ControlRight);
                    let shift = input.pressed(bevy::input::keyboard::KeyCode::ShiftLeft)
                        || input.pressed(bevy::input::keyboard::KeyCode::ShiftRight);
                    let toggle_panel = input.just_pressed(bevy::input::keyboard::KeyCode::F1);
                    let exiting = input.just_pressed(bevy::input::keyboard::KeyCode::Escape);
                    let released = input.just_released(bevy::input::keyboard::KeyCode::Escape);
                    let direction = if control {
                        let dx = i8::from(
                            input.pressed(bevy::input::keyboard::KeyCode::KeyD)
                                || input.pressed(bevy::input::keyboard::KeyCode::ArrowRight),
                        ) - i8::from(
                            input.pressed(bevy::input::keyboard::KeyCode::KeyA)
                                || input.pressed(bevy::input::keyboard::KeyCode::ArrowLeft),
                        );
                        let dy = i8::from(
                            input.pressed(bevy::input::keyboard::KeyCode::KeyW)
                                || input.pressed(bevy::input::keyboard::KeyCode::ArrowUp),
                        ) - i8::from(
                            input.pressed(bevy::input::keyboard::KeyCode::KeyS)
                                || input.pressed(bevy::input::keyboard::KeyCode::ArrowDown),
                        );
                        bevy::math::Vec2::new(dx as f32, dy as f32).normalize_or_zero()
                    } else {
                        bevy::math::Vec2::ZERO
                    };
                    if direction != bevy::math::Vec2::ZERO
                        || cursor_velocity != bevy::math::Vec2::ZERO
                    {
                        let target_velocity =
                            direction * (height * if shift { 0.55 * 0.25 } else { 0.55 });
                        cursor_velocity +=
                            (target_velocity - cursor_velocity) * (1.0 - (-delta / 0.045).exp());
                        if direction == bevy::math::Vec2::ZERO
                            && cursor_velocity.length_squared() < 1e-4
                        {
                            cursor_velocity = bevy::math::Vec2::ZERO;
                        }
                        let next_position = (cursor_position + cursor_velocity * delta)
                            .max(-cursor_limit)
                            .min(cursor_limit);
                        if next_position != cursor_position {
                            cursor_position = next_position;
                            if let Ok((mut transform, _, _)) = sprites.get_mut(cursor_entity) {
                                transform.translation.x = cursor_position.x;
                                transform.translation.y = cursor_position.y;
                            }
                        }
                    }
                    if toggle_panel {
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
                        if let Ok((mut transform, mut sprite, mut visibility)) =
                            sprites.get_mut(panel_entity)
                        {
                            transform.translation.x =
                                panel_closed_x + (panel_open_x - panel_closed_x) * eased;
                            sprite.color = bevy::color::Color::srgba(1.0, 1.0, 1.0, 0.92 * eased);
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
                    if panel_motion != 0
                        || control
                        || direction != bevy::math::Vec2::ZERO
                        || cursor_velocity.length_squared() > 4.0
                        || escape_since.is_some()
                    {
                        idle_deadline = Some(now + std::time::Duration::from_millis(1200));
                        if phase == 2 {
                            winit.focused_mode = bevy::winit::UpdateMode::Continuous;
                            winit.unfocused_mode = bevy::winit::UpdateMode::Continuous;
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
                        }
                    }
                }
            },
        )
        .run();
}
