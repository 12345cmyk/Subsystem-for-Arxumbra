#![deny(warnings)]
#![forbid(unsafe_code)]

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

// Constant ladder: every tunable below folds into the instruction stream at compile time.
const BC7: basisu::TargetFormat = basisu::TargetFormat::Bc7Rgba;
const RAW: basisu::DecodeFlags = basisu::DecodeFlags::NONE;
const BLOCK_TEXELS: u32 = 4;
const BLOCK_BYTES: usize = 16;
const CURSOR_HEIGHT_SHARE: f32 = 0.055;
const CURSOR_MIN: f32 = 28.0;
const CURSOR_MAX: f32 = 320.0;
const CURSOR_SPEED_SHARE: f32 = 0.55;
const CURSOR_RESPONSE: f32 = 0.045;
const CURSOR_PRECISION: f32 = 0.25;
const CURSOR_WAKE_SPEED: f32 = 4.0;
const PANEL_WIDTH_SHARE: f32 = 0.21;
const PANEL_ALPHA: f32 = 0.92;
const PANEL_SECONDS: f32 = 0.8;
const DELTA_CEILING: f32 = 0.05;
const IDLE_PARK: std::time::Duration = std::time::Duration::from_millis(1200);
const PARK_TICK: std::time::Duration = std::time::Duration::from_secs(1);
const EXIT_HOLD: std::time::Duration = std::time::Duration::from_secs(7);

#[inline(always)]
const fn blocks_of(texels: u32) -> u32 {
    (texels + BLOCK_TEXELS - 1) / BLOCK_TEXELS
}

#[inline(always)]
const fn level_extent(source: u32, level: u32) -> u32 {
    let shifted = source >> level;
    if shifted == 0 { 1 } else { shifted }
}

#[inline(always)]
const fn level_bytes(width: u32, height: u32) -> usize {
    (blocks_of(width) as usize) * (blocks_of(height) as usize) * BLOCK_BYTES
}

#[inline(always)]
const fn chain_bytes(source_width: u32, source_height: u32, level: u32, mips: u32) -> usize {
    let mut total = 0usize;
    let mut index = 0u32;
    while index < mips {
        total += level_bytes(
            level_extent(source_width, level + index),
            level_extent(source_height, level + index),
        );
        index += 1;
    }
    total
}

// Deepest mip that still covers the request on both axes; halving ladder, branch-bound like the reference.
#[inline(always)]
const fn level_covering(
    source_width: u32,
    source_height: u32,
    levels: u32,
    need_width: u32,
    need_height: u32,
) -> u32 {
    let mut level = 0u32;
    while level + 1 < levels
        && (source_width >> (level + 1)) >= need_width
        && (source_height >> (level + 1)) >= need_height
    {
        level += 1;
    }
    level
}

// Cursor sprite is square, so the shipped ladder binds on the width axis alone.
#[inline(always)]
const fn level_covering_width(source_width: u32, levels: u32, need_width: u32) -> u32 {
    let mut level = 0u32;
    while level + 1 < levels && (source_width >> (level + 1)) >= need_width {
        level += 1;
    }
    level
}

#[inline(always)]
fn smootherstep(position: f32) -> f32 {
    position * position * position * (position * (position * 6.0 - 15.0) + 10.0)
}

// Transcode one sheet's mip chain straight into its final buffer: one allocation, exact size, no copies.
fn transcode_chain(
    source: &basisu::Transcoder<'static>,
    level: u32,
    mips: u32,
    tag: &str,
) -> Vec<u8> {
    let (source_width, source_height) = source.base_dimensions();
    let mut pixels = vec![0u8; chain_bytes(source_width, source_height, level, mips)];
    let mut offset = 0usize;
    let mut index = 0u32;
    while index < mips {
        let size = level_bytes(
            level_extent(source_width, level + index),
            level_extent(source_height, level + index),
        );
        source
            .transcode_into(
                level + index,
                BC7,
                RAW,
                &mut pixels[offset..offset + size],
            )
            .expect(tag);
        offset += size;
        index += 1;
    }
    pixels
}

// GPU-resident sheet: BC7 + linear mips + clamped edge sampling, ready for the sprite pipeline as-is.
fn sheet_image(
    pixels: Vec<u8>,
    source_width: u32,
    source_height: u32,
    level: u32,
    mips: u32,
) -> bevy::image::Image {
    let mut image = bevy::image::Image::new(
        bevy::render::render_resource::Extent3d {
            width: level_extent(source_width, level),
            height: level_extent(source_height, level),
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
}

// Blending is only paid for when the sheet actually carries coverage.
#[inline(always)]
fn cheap_alpha_mode(has_alpha: bool) -> bevy::sprite::SpriteAlphaMode {
    if has_alpha {
        bevy::sprite::SpriteAlphaMode::Blend
    } else {
        bevy::sprite::SpriteAlphaMode::Opaque
    }
}

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
    let background_alpha_mode = cheap_alpha_mode(background.has_alpha());
    let cursor_alpha_mode = cheap_alpha_mode(cursor.has_alpha());
    let background_size = bevy::math::Vec2::new(background_width as f32, background_height as f32);
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
                // Stage 1 - window invariants, every write guarded by an actual divergence.
                let now = std::time::Instant::now();
                let delta = time.delta_secs().min(DELTA_CEILING);
                let booting = phase == 0;
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
                let geometry_changed = (physical_width, physical_height) != viewport;

                // Stage 2 - layout solved once per frame, in closed form.
                let cover = (width / background_size.x).max(height / background_size.y);
                let panel_width = width * PANEL_WIDTH_SHARE;
                let panel_open_x = panel_width * 0.5 - width * 0.5;
                let panel_closed_x = panel_open_x - panel_width * 1.05;
                let cursor_extent = (height * CURSOR_HEIGHT_SHARE).clamp(CURSOR_MIN, CURSOR_MAX);
                let cursor_limit = bevy::math::Vec2::new(
                    (width * 0.5 - cursor_extent * 0.5).max(0.0),
                    (height * 0.5 - cursor_extent * 0.5).max(0.0),
                );

                // Stage 3 - residency ladder: the cheapest mip chain that still covers the target.
                let background_target_level = level_covering(
                    background_width,
                    background_height,
                    background_levels,
                    physical_width,
                    physical_height,
                );
                let menu_target_level = level_covering(
                    menu_width,
                    menu_height,
                    menu_levels,
                    (panel_width.ceil() as u32).max(1),
                    physical_height,
                );
                let cursor_target_level = level_covering_width(
                    cursor_width,
                    cursor_levels,
                    (cursor_extent.ceil() as u32).max(1),
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

                // Stage 4 - transcode only what went stale, all sheets in parallel, one output buffer each.
                if background_stale || menu_stale || cursor_stale {
                    let (background_pixels, menu_pixels, cursor_pixels) =
                        std::thread::scope(|scope| {
                            let background_job = background_stale.then(|| {
                                scope.spawn(|| {
                                    transcode_chain(
                                        &background,
                                        background_target_level,
                                        background_target_mips,
                                        "background ktx2 bc7 transcode",
                                    )
                                })
                            });
                            let menu_job = menu_stale.then(|| {
                                scope.spawn(|| {
                                    transcode_chain(
                                        &menu,
                                        menu_target_level,
                                        menu_target_mips,
                                        "menu ktx2 bc7 transcode",
                                    )
                                })
                            });
                            let cursor_job = cursor_stale.then(|| {
                                scope.spawn(|| {
                                    transcode_chain(
                                        &cursor,
                                        cursor_target_level,
                                        cursor_target_mips,
                                        "cursor ktx2 bc7 transcode",
                                    )
                                })
                            });
                            (
                                background_job
                                    .map(|job| job.join().expect("background ktx2 decode")),
                                menu_job.map(|job| job.join().expect("menu ktx2 decode")),
                                cursor_job.map(|job| job.join().expect("cursor ktx2 decode")),
                            )
                        });
                    if let Some(pixels) = background_pixels {
                        let retired = background_handle.replace(images.add(sheet_image(
                            pixels,
                            background_width,
                            background_height,
                            background_target_level,
                            background_target_mips,
                        )));
                        background_level = background_target_level;
                        background_mips = background_target_mips;
                        if !booting {
                            if let Ok((_, mut sprite, _)) = sprites.get_mut(background_entity) {
                                sprite.image = background_handle
                                    .clone()
                                    .expect("background handle");
                            }
                            if let Some(retired) = retired {
                                images.remove(&retired);
                            }
                        }
                    }
                    if let Some(pixels) = menu_pixels {
                        let retired = panel_handle.replace(images.add(sheet_image(
                            pixels,
                            menu_width,
                            menu_height,
                            menu_target_level,
                            menu_target_mips,
                        )));
                        menu_level = menu_target_level;
                        menu_mips = menu_target_mips;
                        if !booting {
                            if let Ok((_, mut sprite, _)) = sprites.get_mut(panel_entity) {
                                sprite.image =
                                    panel_handle.clone().expect("panel handle");
                            }
                            if let Some(retired) = retired {
                                images.remove(&retired);
                            }
                        }
                    }
                    if let Some(pixels) = cursor_pixels {
                        let retired = cursor_handle.replace(images.add(sheet_image(
                            pixels,
                            cursor_width,
                            cursor_height,
                            cursor_target_level,
                            cursor_target_mips,
                        )));
                        cursor_level = cursor_target_level;
                        cursor_mips = cursor_target_mips;
                        if !booting {
                            if let Ok((_, mut sprite, _)) = sprites.get_mut(cursor_entity) {
                                sprite.image = cursor_handle.clone().expect("cursor handle");
                            }
                            if let Some(retired) = retired {
                                images.remove(&retired);
                            }
                        }
                    }
                }

                // Stage 5 - one-shot staging: camera, sheets, revealed only once every texture is resident.
                if booting {
                    let (Some(background_current), Some(panel_current), Some(cursor_current)) = (
                        background_handle.as_ref(),
                        panel_handle.as_ref(),
                        cursor_handle.as_ref(),
                    ) else {
                        return;
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
                                alpha_mode: background_alpha_mode,
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
                                alpha_mode: cursor_alpha_mode,
                                ..Default::default()
                            },
                            bevy::transform::components::Transform::from_xyz(0.0, 0.0, 2.0),
                            bevy::camera::visibility::Visibility::Visible,
                        ))
                        .id();
                    window.visible = true;
                    viewport = (physical_width, physical_height);
                    phase = 1;
                    idle_deadline = Some(now + IDLE_PARK);
                }

                // Stage 6 - re-projection: only on a real resolution change, and only on the entities it moves.
                if !booting && geometry_changed {
                    if let Ok((mut transform, mut sprite, _)) = sprites.get_mut(background_entity) {
                        transform.scale = bevy::math::Vec3::splat(cover);
                        sprite.custom_size = Some(background_size);
                    }
                    if panel_motion == 0 {
                        if let Ok((mut transform, mut sprite, mut visibility)) =
                            sprites.get_mut(panel_entity)
                        {
                            transform.translation.x = if panel_open {
                                panel_open_x
                            } else {
                                panel_closed_x
                            };
                            sprite.custom_size = Some(bevy::math::Vec2::new(panel_width, height));
                            sprite.color = bevy::color::Color::srgba(
                                1.0,
                                1.0,
                                1.0,
                                if panel_open { PANEL_ALPHA } else { 0.0 },
                            );
                            *visibility = if panel_open {
                                bevy::camera::visibility::Visibility::Visible
                            } else {
                                bevy::camera::visibility::Visibility::Hidden
                            };
                            panel_drawn = panel_open;
                        }
                    }
                    if let Ok((mut transform, mut sprite, _)) = sprites.get_mut(cursor_entity) {
                        sprite.custom_size = Some(bevy::math::Vec2::splat(cursor_extent));
                        transform.translation.x = cursor_position.x;
                        transform.translation.y = cursor_position.y;
                    }
                    viewport = (physical_width, physical_height);
                }

                // Stage 7 - keyboard cursor: Ctrl + WASD/arrows, Shift for precision, critically damped on a 45 ms horizon.
                let mut control = false;
                let mut direction = bevy::math::Vec2::ZERO;
                if !booting {
                    control = input.pressed(bevy::input::keyboard::KeyCode::ControlLeft)
                        || input.pressed(bevy::input::keyboard::KeyCode::ControlRight);
                    if control {
                        if input.pressed(bevy::input::keyboard::KeyCode::KeyW)
                            || input.pressed(bevy::input::keyboard::KeyCode::ArrowUp)
                        {
                            direction.y += 1.0;
                        }
                        if input.pressed(bevy::input::keyboard::KeyCode::KeyS)
                            || input.pressed(bevy::input::keyboard::KeyCode::ArrowDown)
                        {
                            direction.y -= 1.0;
                        }
                        if input.pressed(bevy::input::keyboard::KeyCode::KeyA)
                            || input.pressed(bevy::input::keyboard::KeyCode::ArrowLeft)
                        {
                            direction.x -= 1.0;
                        }
                        if input.pressed(bevy::input::keyboard::KeyCode::KeyD)
                            || input.pressed(bevy::input::keyboard::KeyCode::ArrowRight)
                        {
                            direction.x += 1.0;
                        }
                    }
                    if direction != bevy::math::Vec2::ZERO {
                        direction = direction.normalize();
                    }
                    let speed_share = if input.pressed(bevy::input::keyboard::KeyCode::ShiftLeft)
                        || input.pressed(bevy::input::keyboard::KeyCode::ShiftRight)
                    {
                        CURSOR_SPEED_SHARE * CURSOR_PRECISION
                    } else {
                        CURSOR_SPEED_SHARE
                    };
                    cursor_velocity += (direction * (height * speed_share) - cursor_velocity)
                        * (1.0 - (-delta / CURSOR_RESPONSE).exp());
                    let previous = cursor_position;
                    cursor_position += cursor_velocity * delta;
                    cursor_position = cursor_position.clamp(-cursor_limit, cursor_limit);
                    if cursor_position != previous || geometry_changed {
                        if let Ok((mut transform, mut sprite, _)) = sprites.get_mut(cursor_entity) {
                            transform.translation.x = cursor_position.x;
                            transform.translation.y = cursor_position.y;
                            sprite.custom_size = Some(bevy::math::Vec2::splat(cursor_extent));
                        }
                    }
                }

                // Stage 8 - panel: quintic lift, integrated only while it is actually travelling.
                if !booting {
                    if input.just_pressed(bevy::input::keyboard::KeyCode::F1) {
                        panel_open = !panel_open;
                        panel_motion = if panel_open { 1 } else { -1 };
                    }
                    if panel_motion != 0 {
                        let step = delta / PANEL_SECONDS;
                        if panel_motion > 0 {
                            panel_progress = (panel_progress + step).min(1.0);
                        } else {
                            panel_progress = (panel_progress - step).max(0.0);
                        }
                        let eased = smootherstep(panel_progress);
                        if let Ok((mut transform, mut sprite, mut visibility)) =
                            sprites.get_mut(panel_entity)
                        {
                            transform.translation.x =
                                panel_closed_x + (panel_open_x - panel_closed_x) * eased;
                            sprite.custom_size = Some(bevy::math::Vec2::new(panel_width, height));
                            sprite.color =
                                bevy::color::Color::srgba(1.0, 1.0, 1.0, PANEL_ALPHA * eased);
                            if eased > 0.0 {
                                if !panel_drawn {
                                    *visibility = bevy::camera::visibility::Visibility::Visible;
                                    panel_drawn = true;
                                }
                            } else if panel_drawn {
                                *visibility = bevy::camera::visibility::Visibility::Hidden;
                                panel_drawn = false;
                            }
                        }
                        if (panel_motion > 0 && panel_progress >= 1.0)
                            || (panel_motion < 0 && panel_progress <= 0.0)
                        {
                            panel_motion = 0;
                        }
                    }
                }

                // Stage 9 - escape contract: seven seconds of held escape exits, no throttling in between.
                if input.just_pressed(bevy::input::keyboard::KeyCode::Escape) {
                    escape_since = Some(now);
                }
                if input.just_released(bevy::input::keyboard::KeyCode::Escape) {
                    escape_since = None;
                }
                if let Some(pressed_at) = escape_since {
                    if now - pressed_at >= EXIT_HOLD {
                        commands.write_message(bevy::app::AppExit::Success);
                        escape_since = None;
                    }
                }

                // Stage 10 - governor: continuous while anything moves, one-second park tick when nothing does.
                if panel_motion != 0
                    || control
                    || direction != bevy::math::Vec2::ZERO
                    || cursor_velocity.length_squared() > CURSOR_WAKE_SPEED
                    || escape_since.is_some()
                {
                    idle_deadline = Some(now + IDLE_PARK);
                    if phase == 2 {
                        winit.focused_mode = bevy::winit::UpdateMode::Continuous;
                        winit.unfocused_mode = bevy::winit::UpdateMode::Continuous;
                        phase = 1;
                    }
                } else if phase == 1 {
                    if let Some(deadline) = idle_deadline {
                        if deadline <= now {
                            winit.focused_mode =
                                bevy::winit::UpdateMode::reactive_low_power(PARK_TICK);
                            winit.unfocused_mode =
                                bevy::winit::UpdateMode::reactive_low_power(PARK_TICK);
                            idle_deadline = None;
                            phase = 2;
                        }
                    }
                }
            },
        )
        .run();
}
