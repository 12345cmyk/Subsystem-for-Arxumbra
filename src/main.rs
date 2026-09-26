#![deny(warnings)]
#![forbid(unsafe_code)]

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let (background_image, menu_texture) = std::thread::scope(|scope| {
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
        (background_image, menu_thread.join().expect("menu ktx2 decode"))
    });
    let background_size = bevy::math::Vec2::new(
        background_image.width() as f32,
        background_image.height() as f32,
    );
    let mut background_texture = Some(background_image);
    let mut menu_asset = Some(menu_texture);
    let mut phase: u8 = 0;
    let mut background: Option<bevy::ecs::entity::Entity> = None;
    let mut panel: Option<bevy::ecs::entity::Entity> = None;
    let mut progress: f32 = 0.0;
    let mut animation: u8 = 0;
    let mut stepped_at: std::time::Instant = std::time::Instant::now();
    let mut menu_image: Option<bevy::asset::Handle<bevy::image::Image>> = None;
    let mut viewport: (u32, u32) = (0, 0);
    let mut sleep_at: Option<std::time::Instant> = None;
    let mut escape_held_since: Option<std::time::Instant> = None;
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
                  mut images: bevy::ecs::system::ResMut<
                bevy::asset::Assets<bevy::image::Image>,
            >,
                  input: bevy::ecs::system::Res<
                bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>,
            >,
                  mut winit: bevy::ecs::system::ResMut<bevy::winit::WinitSettings>| {
                let now = std::time::Instant::now();
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
                if phase == 0 {
                    let cover =
                        (size.0 as f32 / background_size.x).max(size.1 as f32 / background_size.y);
                    let menu_handle = images.add(menu_asset.take().expect("menu texture"));
                    let background_handle =
                        images.add(background_texture.take().expect("background texture"));
                    background = Some(
                        commands
                            .spawn((
                                bevy::sprite::Sprite {
                                    image: background_handle,
                                    custom_size: Some(background_size),
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
                    window.visible = true;
                    menu_image = Some(menu_handle);
                    viewport = size;
                    phase = 1;
                    sleep_at = Some(now + std::time::Duration::from_millis(500));
                }
                if menu_image.is_some() && size != viewport {
                    viewport = size;
                    let cover =
                        (size.0 as f32 / background_size.x).max(size.1 as f32 / background_size.y);
                    if let Some(background_entity) = background {
                        commands.entity(background_entity).insert(
                            bevy::transform::components::Transform::from_scale(
                                bevy::math::Vec3::splat(cover),
                            ),
                        );
                    }
                    if let Some(panel_entity) = panel.take() {
                        commands.entity(panel_entity).despawn();
                        animation = 0;
                        progress = 0.0;
                    }
                    winit.focused_mode = bevy::winit::UpdateMode::Continuous;
                    phase = 1;
                    sleep_at = Some(now + std::time::Duration::from_millis(500));
                }
                if phase == 1 {
                    if let Some(deadline) = sleep_at {
                        if deadline <= now {
                            *winit = bevy::winit::WinitSettings::desktop_app();
                            sleep_at = None;
                            phase = 2;
                        }
                    }
                }
                if (phase == 1 || phase == 2)
                    && input.just_pressed(bevy::input::keyboard::KeyCode::F1)
                {
                    if panel.is_some() {
                        if animation == 2 {
                            animation = 1;
                        } else {
                            animation = 2;
                        }
                    } else if menu_image.is_some() {
                        let width = viewport.0 as f32;
                        let closed = -width / 2.0 + width * 0.21 / 2.0 - width * 0.21 * 1.05;
                        panel = Some(
                            commands
                                .spawn(bevy::transform::components::Transform::from_xyz(
                                    closed, 0.0, 1.0,
                                ))
                                .id(),
                        );
                        progress = 0.0;
                        animation = 1;
                        stepped_at = now;
                        winit.focused_mode = bevy::winit::UpdateMode::Continuous;
                        phase = 1;
                        sleep_at = None;
                    }
                }
                if animation != 0 {
                    if let (Some(panel_entity), Some(menu_handle)) =
                        (panel, menu_image.clone())
                    {
                        let step = (now - stepped_at).as_secs_f32() / 0.25;
                        stepped_at = now;
                        if animation == 1 {
                            progress = (progress + step).min(1.0);
                        } else {
                            progress = (progress - step).max(0.0);
                        }
                        let eased = progress * progress * (3.0 - 2.0 * progress);
                        let width = viewport.0 as f32;
                        let height = viewport.1 as f32;
                        let x =
                            -width / 2.0 + width * 0.21 / 2.0 - (1.0 - eased) * width * 0.21 * 1.05;
                        commands.entity(panel_entity).insert((
                            bevy::sprite::Sprite {
                                image: menu_handle,
                                custom_size: Some(bevy::math::Vec2::new(width * 0.21, height)),
                                color: bevy::color::Color::srgba(1.0, 1.0, 1.0, 0.86 * eased),
                                ..Default::default()
                            },
                            bevy::transform::components::Transform::from_xyz(x, 0.0, 1.0),
                        ));
                        if animation == 1 && progress >= 1.0 {
                            animation = 0;
                        }
                        if animation == 2 && progress <= 0.0 {
                            commands.entity(panel_entity).despawn();
                            panel = None;
                            animation = 0;
                            phase = 1;
                            sleep_at = Some(now + std::time::Duration::from_millis(250));
                        }
                    }
                }
                if input.just_pressed(bevy::input::keyboard::KeyCode::Escape) {
                    escape_held_since = Some(now);
                    winit.focused_mode =
                        bevy::winit::UpdateMode::reactive(std::time::Duration::from_millis(100));
                }
                if input.just_released(bevy::input::keyboard::KeyCode::Escape) {
                    escape_held_since = None;
                    if panel.is_some() {
                        winit.focused_mode = bevy::winit::UpdateMode::Continuous;
                    } else {
                        *winit = bevy::winit::WinitSettings::desktop_app();
                    }
                }
                if let Some(pressed_at) = escape_held_since {
                    if now - pressed_at >= std::time::Duration::from_secs(7) {
                        commands.write_message(bevy::app::AppExit::Success);
                    }
                }
            },
        )
        .run();
}
