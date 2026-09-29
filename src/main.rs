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
                let now = std::time::Instant::now();
                let delta = time.delta_secs().min(0.05);
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
                let geometry_changed = (physical_width, physical_height) != viewport;
                if (31 - (((background_width / physical_width)
                    .min(background_height / physical_height))
                .max(1))
                .leading_zeros())
                .min(background_levels - 1)
                    != background_level
                    || (if (physical_width as f32 / background_width as f32)
                        .max(physical_height as f32 / background_height as f32)
                        >= 1.0
                    {
                        1
                    } else {
                        background_levels
                            - (31 - (((background_width / physical_width)
                                .min(background_height / physical_height))
                            .max(1))
                            .leading_zeros())
                            .min(background_levels - 1)
                    }) != background_mips
                    || (31 - (((menu_width / ((physical_width as f32 * 0.21).ceil() as u32).max(1)
                        .max(1))
                    .min(menu_height / physical_height))
                    .max(1))
                    .leading_zeros())
                    .min(menu_levels - 1)
                        != menu_level
                    || (if physical_width as f32 * 0.21
                        >= (menu_width
                            >> (31 - (((menu_width
                                / ((physical_width as f32 * 0.21).ceil() as u32).max(1)
                            .max(1))
                            .min(menu_height / physical_height))
                            .max(1))
                            .leading_zeros())
                            .min(menu_levels - 1)) as f32
                        && physical_height as f32
                            >= (menu_height
                                >> (31 - (((menu_width
                                    / ((physical_width as f32 * 0.21).ceil() as u32).max(1)
                                .max(1))
                                .min(menu_height / physical_height))
                                .max(1))
                                .leading_zeros())
                                .min(menu_levels - 1)) as f32
                    {
                        1
                    } else {
                        menu_levels
                            - (31 - (((menu_width
                                / ((physical_width as f32 * 0.21).ceil() as u32).max(1)
                            .max(1))
                            .min(menu_height / physical_height))
                            .max(1))
                            .leading_zeros())
                            .min(menu_levels - 1)
                    }) != menu_mips
                    || (31 - (cursor_width
                        / ((physical_height as f32 * 0.055).clamp(28.0, 320.0).ceil() as u32)
                            .max(1))
                    .max(1)
                    .leading_zeros())
                    .min(cursor_levels - 1)
                        != cursor_level
                    || (if (physical_height as f32 * 0.055).clamp(28.0, 320.0)
                        >= (cursor_width
                            >> (31 - (cursor_width
                                / ((physical_height as f32 * 0.055)
                                    .clamp(28.0, 320.0)
                                    .ceil() as u32)
                                .max(1))
                            .max(1)
                            .leading_zeros())
                            .min(cursor_levels - 1)) as f32
                    {
                        1
                    } else {
                        cursor_levels
                            - (31 - (cursor_width
                                / ((physical_height as f32 * 0.055)
                                    .clamp(28.0, 320.0)
                                    .ceil() as u32)
                                .max(1))
                            .max(1)
                            .leading_zeros())
                            .min(cursor_levels - 1)
                    }) != cursor_mips
                {
                    let (background_pixels, menu_pixels, cursor_pixels) =
                        std::thread::scope(|scope| {
                            let background_job = ((31 - (((background_width / physical_width)
                                .min(background_height / physical_height))
                            .max(1))
                            .leading_zeros())
                            .min(background_levels - 1)
                                != background_level
                                || (if (physical_width as f32 / background_width as f32)
                                    .max(physical_height as f32 / background_height as f32)
                                    >= 1.0
                                {
                                    1
                                } else {
                                    background_levels
                                        - (31 - (((background_width / physical_width)
                                            .min(background_height / physical_height))
                                        .max(1))
                                        .leading_zeros())
                                        .min(background_levels - 1)
                                }) != background_mips)
                                .then(|| {
                                    scope.spawn(|| {
                                        let level = (31 - (((background_width / physical_width)
                                            .min(background_height / physical_height))
                                        .max(1))
                                        .leading_zeros())
                                        .min(background_levels - 1);
                                        let mips = if (physical_width as f32
                                            / background_width as f32)
                                            .max(physical_height as f32
                                                / background_height as f32)
                                            >= 1.0
                                        {
                                            1
                                        } else {
                                            background_levels - level
                                        };
                                        let mut pixels = vec![
                                            0u8;
                                            (0..mips)
                                                .map(|i| {
                                                    (((((background_width >> (level + i))
                                                        .max(1)
                                                        + 3)
                                                        / 4) as usize)
                                                        * ((((background_height >> (level + i))
                                                            .max(1)
                                                            + 3)
                                                            / 4)
                                                            as usize))
                                                        * 16
                                                })
                                                .sum::<usize>()
                                        ];
                                        let mut offset = 0usize;
                                        for i in 0..mips {
                                            let size = (((((background_width >> (level + i))
                                                .max(1)
                                                + 3)
                                                / 4) as usize)
                                                * ((((background_height >> (level + i)).max(1) + 3)
                                                    / 4)
                                                    as usize))
                                                * 16;
                                            background
                                                .transcode_into(
                                                    level + i,
                                                    basisu::TargetFormat::Bc7Rgba,
                                                    basisu::DecodeFlags::NONE,
                                                    &mut pixels[offset..offset + size],
                                                )
                                                .expect("background ktx2 bc7 transcode");
                                            offset += size;
                                        }
                                        (pixels, level, mips)
                                    })
                                });
                            let menu_job = ((31 - (((menu_width
                                / ((physical_width as f32 * 0.21).ceil() as u32).max(1)
                            .max(1))
                            .min(menu_height / physical_height))
                            .max(1))
                            .leading_zeros())
                            .min(menu_levels - 1)
                                != menu_level
                                || (if physical_width as f32 * 0.21
                                    >= (menu_width
                                        >> (31 - (((menu_width
                                            / ((physical_width as f32 * 0.21).ceil() as u32).max(1)
                                        .max(1))
                                        .min(menu_height / physical_height))
                                        .max(1))
                                        .leading_zeros())
                                        .min(menu_levels - 1)) as f32
                                    && physical_height as f32
                                        >= (menu_height
                                            >> (31 - (((menu_width
                                                / ((physical_width as f32 * 0.21).ceil() as u32).max(1)
                                            .max(1))
                                            .min(menu_height / physical_height))
                                            .max(1))
                                            .leading_zeros())
                                            .min(menu_levels - 1)) as f32
                                {
                                    1
                                } else {
                                    menu_levels
                                        - (31 - (((menu_width
                                            / ((physical_width as f32 * 0.21).ceil() as u32).max(1)
                                        .max(1))
                                        .min(menu_height / physical_height))
                                        .max(1))
                                        .leading_zeros())
                                        .min(menu_levels - 1)
                                }) != menu_mips)
                                .then(|| {
                                    scope.spawn(|| {
                                        let level = (31 - (((menu_width
                                            / ((physical_width as f32 * 0.21).ceil() as u32).max(1)
                                        .max(1))
                                        .min(menu_height / physical_height))
                                        .max(1))
                                        .leading_zeros())
                                        .min(menu_levels - 1);
                                        let mips = if physical_width as f32 * 0.21
                                            >= (menu_width >> level) as f32
                                            && physical_height as f32
                                                >= (menu_height >> level) as f32
                                        {
                                            1
                                        } else {
                                            menu_levels - level
                                        };
                                        let mut pixels = vec![
                                            0u8;
                                            (0..mips)
                                                .map(|i| {
                                                    (((((menu_width >> (level + i)).max(1) + 3)
                                                        / 4)
                                                        as usize)
                                                        * ((((menu_height >> (level + i))
                                                            .max(1)
                                                            + 3)
                                                            / 4)
                                                            as usize))
                                                        * 16
                                                })
                                                .sum::<usize>()
                                        ];
                                        let mut offset = 0usize;
                                        for i in 0..mips {
                                            let size = (((((menu_width >> (level + i)).max(1) + 3)
                                                / 4)
                                                as usize)
                                                * ((((menu_height >> (level + i)).max(1) + 3) / 4)
                                                    as usize))
                                                * 16;
                                            menu.transcode_into(
                                                level + i,
                                                basisu::TargetFormat::Bc7Rgba,
                                                basisu::DecodeFlags::NONE,
                                                &mut pixels[offset..offset + size],
                                            )
                                            .expect("menu ktx2 bc7 transcode");
                                            offset += size;
                                        }
                                        (pixels, level, mips)
                                    })
                                });
                            let cursor_job = ((31 - (cursor_width
                                / ((physical_height as f32 * 0.055)
                                    .clamp(28.0, 320.0)
                                    .ceil() as u32)
                                .max(1))
                            .max(1)
                            .leading_zeros())
                            .min(cursor_levels - 1)
                                != cursor_level
                                || (if (physical_height as f32 * 0.055).clamp(28.0, 320.0)
                                    >= (cursor_width
                                        >> (31 - (cursor_width
                                            / ((physical_height as f32 * 0.055)
                                                .clamp(28.0, 320.0)
                                                .ceil() as u32)
                                            .max(1))
                                        .max(1)
                                        .leading_zeros())
                                        .min(cursor_levels - 1)) as f32
                                {
                                    1
                                } else {
                                    cursor_levels
                                        - (31 - (cursor_width
                                            / ((physical_height as f32 * 0.055)
                                                .clamp(28.0, 320.0)
                                                .ceil() as u32)
                                            .max(1))
                                        .max(1)
                                        .leading_zeros())
                                        .min(cursor_levels - 1)
                                }) != cursor_mips)
                                .then(|| {
                                    scope.spawn(|| {
                                        let level = (31 - (cursor_width
                                            / ((physical_height as f32 * 0.055)
                                                .clamp(28.0, 320.0)
                                                .ceil() as u32)
                                            .max(1))
                                        .max(1)
                                        .leading_zeros())
                                        .min(cursor_levels - 1);
                                        let mips = if (physical_height as f32 * 0.055)
                                            .clamp(28.0, 320.0)
                                            >= (cursor_width >> level) as f32
                                        {
                                            1
                                        } else {
                                            cursor_levels - level
                                        };
                                        let mut pixels = vec![
                                            0u8;
                                            (0..mips)
                                                .map(|i| {
                                                    (((((cursor_width >> (level + i)).max(1) + 3)
                                                        / 4)
                                                        as usize)
                                                        * ((((cursor_height >> (level + i))
                                                            .max(1)
                                                            + 3)
                                                            / 4)
                                                            as usize))
                                                        * 16
                                                })
                                                .sum::<usize>()
                                        ];
                                        let mut offset = 0usize;
                                        for i in 0..mips {
                                            let size = (((((cursor_width >> (level + i)).max(1)
                                                + 3)
                                                / 4)
                                                as usize)
                                                * ((((cursor_height >> (level + i)).max(1) + 3)
                                                    / 4)
                                                    as usize))
                                                * 16;
                                            cursor
                                                .transcode_into(
                                                    level + i,
                                                    basisu::TargetFormat::Bc7Rgba,
                                                    basisu::DecodeFlags::NONE,
                                                    &mut pixels[offset..offset + size],
                                                )
                                                .expect("cursor ktx2 bc7 transcode");
                                            offset += size;
                                        }
                                        (pixels, level, mips)
                                    })
                                });
                            (
                                background_job
                                    .map(|job| job.join().expect("background ktx2 decode")),
                                menu_job.map(|job| job.join().expect("menu ktx2 decode")),
                                cursor_job.map(|job| job.join().expect("cursor ktx2 decode")),
                            )
                        });
                    if let Some((pixels, level, mips)) = background_pixels {
                        let retired = background_handle.replace(images.add({
                            let mut image = bevy::image::Image::new(
                                bevy::render::render_resource::Extent3d {
                                    width: (background_width >> level).max(1),
                                    height: (background_height >> level).max(1),
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
                        }));
                        background_level = level;
                        background_mips = mips;
                        if !booting {
                            if let Ok((_, mut sprite, _)) = sprites.get_mut(background_entity) {
                                sprite.image =
                                    background_handle.clone().expect("background handle");
                            }
                            if let Some(retired) = retired {
                                images.remove(&retired);
                            }
                        }
                    }
                    if let Some((pixels, level, mips)) = menu_pixels {
                        let retired = panel_handle.replace(images.add({
                            let mut image = bevy::image::Image::new(
                                bevy::render::render_resource::Extent3d {
                                    width: (menu_width >> level).max(1),
                                    height: (menu_height >> level).max(1),
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
                        }));
                        menu_level = level;
                        menu_mips = mips;
                        if !booting {
                            if let Ok((_, mut sprite, _)) = sprites.get_mut(panel_entity) {
                                sprite.image = panel_handle.clone().expect("panel handle");
                            }
                            if let Some(retired) = retired {
                                images.remove(&retired);
                            }
                        }
                    }
                    if let Some((pixels, level, mips)) = cursor_pixels {
                        let retired = cursor_handle.replace(images.add({
                            let mut image = bevy::image::Image::new(
                                bevy::render::render_resource::Extent3d {
                                    width: (cursor_width >> level).max(1),
                                    height: (cursor_height >> level).max(1),
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
                        }));
                        cursor_level = level;
                        cursor_mips = mips;
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
                                custom_size: Some(bevy::math::Vec2::new(
                                    background_width as f32,
                                    background_height as f32,
                                )),
                                color: bevy::color::Color::WHITE,
                                alpha_mode: if background.has_alpha() {
                                    bevy::sprite::SpriteAlphaMode::Blend
                                } else {
                                    bevy::sprite::SpriteAlphaMode::Opaque
                                },
                                ..Default::default()
                            },
                            bevy::transform::components::Transform::from_scale(
                                bevy::math::Vec3::splat(
                                    (physical_width as f32 / background_width as f32).max(
                                        physical_height as f32 / background_height as f32,
                                    ),
                                ),
                            ),
                            bevy::camera::visibility::Visibility::Visible,
                        ))
                        .id();
                    panel_entity = commands
                        .spawn((
                            bevy::sprite::Sprite {
                                image: panel_current.clone(),
                                custom_size: Some(bevy::math::Vec2::new(
                                    physical_width as f32 * 0.21,
                                    physical_height as f32,
                                )),
                                color: bevy::color::Color::srgba(1.0, 1.0, 1.0, 0.0),
                                alpha_mode: if menu.has_alpha() {
                                    bevy::sprite::SpriteAlphaMode::Blend
                                } else {
                                    bevy::sprite::SpriteAlphaMode::Opaque
                                },
                                ..Default::default()
                            },
                            bevy::transform::components::Transform::from_xyz(
                                physical_width as f32 * 0.21 * 0.5 - physical_width as f32 * 0.5
                                    - physical_width as f32 * 0.21 * 1.05,
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
                                custom_size: Some(bevy::math::Vec2::splat(
                                    (physical_height as f32 * 0.055).clamp(28.0, 320.0),
                                )),
                                color: bevy::color::Color::WHITE,
                                alpha_mode: if cursor.has_alpha() {
                                    bevy::sprite::SpriteAlphaMode::Blend
                                } else {
                                    bevy::sprite::SpriteAlphaMode::Opaque
                                },
                                ..Default::default()
                            },
                            bevy::transform::components::Transform::from_xyz(0.0, 0.0, 2.0),
                            bevy::camera::visibility::Visibility::Visible,
                        ))
                        .id();
                    window.visible = true;
                    viewport = (physical_width, physical_height);
                    phase = 1;
                    idle_deadline =
                        Some(now + std::time::Duration::from_millis(1200));
                }
                if !booting && geometry_changed {
                    if let Ok((mut transform, mut sprite, _)) = sprites.get_mut(background_entity) {
                        transform.scale = bevy::math::Vec3::splat(
                            (physical_width as f32 / background_width as f32)
                                .max(physical_height as f32 / background_height as f32),
                        );
                        sprite.custom_size = Some(bevy::math::Vec2::new(
                            background_width as f32,
                            background_height as f32,
                        ));
                    }
                    if panel_motion == 0 {
                        if let Ok((mut transform, mut sprite, mut visibility)) =
                            sprites.get_mut(panel_entity)
                        {
                            transform.translation.x = if panel_open {
                                physical_width as f32 * 0.21 * 0.5 - physical_width as f32 * 0.5
                            } else {
                                physical_width as f32 * 0.21 * 0.5
                                    - physical_width as f32 * 0.5
                                    - physical_width as f32 * 0.21 * 1.05
                            };
                            sprite.custom_size = Some(bevy::math::Vec2::new(
                                physical_width as f32 * 0.21,
                                physical_height as f32,
                            ));
                            sprite.color = bevy::color::Color::srgba(
                                1.0,
                                1.0,
                                1.0,
                                if panel_open { 0.92 } else { 0.0 },
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
                        sprite.custom_size = Some(bevy::math::Vec2::splat(
                            (physical_height as f32 * 0.055).clamp(28.0, 320.0),
                        ));
                        transform.translation.x = cursor_position.x;
                        transform.translation.y = cursor_position.y;
                    }
                    viewport = (physical_width, physical_height);
                }
                if !booting {
                    let control = input.pressed(bevy::input::keyboard::KeyCode::ControlLeft)
                        || input.pressed(bevy::input::keyboard::KeyCode::ControlRight);
                    let direction = if control {
                        {
                            let mut direction = bevy::math::Vec2::ZERO;
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
                            if direction == bevy::math::Vec2::ZERO {
                                direction
                            } else {
                                direction.normalize()
                            }
                        }
                    } else {
                        bevy::math::Vec2::ZERO
                    };
                    cursor_velocity += (direction
                        * (physical_height as f32
                            * if input.pressed(bevy::input::keyboard::KeyCode::ShiftLeft)
                                || input.pressed(bevy::input::keyboard::KeyCode::ShiftRight)
                            {
                                0.55 * 0.25
                            } else {
                                0.55
                            })
                        - cursor_velocity)
                        * (1.0 - (-delta / 0.045).exp());
                    let previous = cursor_position;
                    cursor_position += cursor_velocity * delta;
                    cursor_position = cursor_position
                        .max(-bevy::math::Vec2::new(
                            (physical_width as f32 * 0.5
                                - (physical_height as f32 * 0.055).clamp(28.0, 320.0) * 0.5)
                                .max(0.0),
                            (physical_height as f32 * 0.5
                                - (physical_height as f32 * 0.055).clamp(28.0, 320.0) * 0.5)
                                .max(0.0),
                        ))
                        .min(bevy::math::Vec2::new(
                            (physical_width as f32 * 0.5
                                - (physical_height as f32 * 0.055).clamp(28.0, 320.0) * 0.5)
                                .max(0.0),
                            (physical_height as f32 * 0.5
                                - (physical_height as f32 * 0.055).clamp(28.0, 320.0) * 0.5)
                                .max(0.0),
                        ));
                    if cursor_position != previous || geometry_changed {
                        if let Ok((mut transform, mut sprite, _)) = sprites.get_mut(cursor_entity) {
                            transform.translation.x = cursor_position.x;
                            transform.translation.y = cursor_position.y;
                            sprite.custom_size = Some(bevy::math::Vec2::splat(
                                (physical_height as f32 * 0.055).clamp(28.0, 320.0),
                            ));
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
                    if input.just_pressed(bevy::input::keyboard::KeyCode::F1) {
                        panel_open = !panel_open;
                        panel_motion = if panel_open { 1 } else { -1 };
                    }
                    if panel_motion != 0 {
                        panel_progress = if panel_motion > 0 {
                            (panel_progress + delta / 0.8).min(1.0)
                        } else {
                            (panel_progress - delta / 0.8).max(0.0)
                        };
                        if let Ok((mut transform, mut sprite, mut visibility)) =
                            sprites.get_mut(panel_entity)
                        {
                            transform.translation.x = physical_width as f32 * 0.21 * 0.5
                                - physical_width as f32 * 0.5
                                - physical_width as f32 * 0.21 * 1.05
                                + ((physical_width as f32 * 0.21 * 0.5
                                    - physical_width as f32 * 0.5
                                    - (physical_width as f32 * 0.21 * 0.5
                                        - physical_width as f32 * 0.5
                                        - physical_width as f32 * 0.21 * 1.05))
                                    * (panel_progress
                                        * panel_progress
                                        * panel_progress
                                        * (panel_progress * (panel_progress * 6.0 - 15.0)
                                            + 10.0)));
                            sprite.custom_size = Some(bevy::math::Vec2::new(
                                physical_width as f32 * 0.21,
                                physical_height as f32,
                            ));
                            sprite.color = bevy::color::Color::srgba(
                                1.0,
                                1.0,
                                1.0,
                                0.92
                                    * (panel_progress
                                        * panel_progress
                                        * panel_progress
                                        * (panel_progress * (panel_progress * 6.0 - 15.0)
                                            + 10.0)),
                            );
                            if panel_progress > 0.0 {
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
                                winit.focused_mode =
                                    bevy::winit::UpdateMode::reactive_low_power(
                                        std::time::Duration::from_secs(1),
                                    );
                                winit.unfocused_mode =
                                    bevy::winit::UpdateMode::reactive_low_power(
                                        std::time::Duration::from_secs(1),
                                    );
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
