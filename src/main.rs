#![deny(warnings)]
#![forbid(unsafe_code)]

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let mut phase: u8 = 0;
    let mut background: Option<bevy::ecs::entity::Entity> = None;
    let mut camera: Option<bevy::ecs::entity::Entity> = None;
    let mut panel: Option<bevy::ecs::entity::Entity> = None;
    let mut edge: Option<bevy::ecs::entity::Entity> = None;
    let mut inset: Option<bevy::ecs::entity::Entity> = None;
    let mut progress: f32 = 0.0;
    let mut animation: u8 = 0;
    let mut stepped_at: std::time::Instant = std::time::Instant::now();
    let mut menu_image: Option<bevy::asset::Handle<bevy::image::Image>> = None;
    let mut background_size: bevy::math::Vec2 = bevy::math::Vec2::ZERO;
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
                bevy::app::TaskPoolPlugin {
                    task_pool_options: bevy::app::TaskPoolOptions {
                        min_total_threads: 1,
                        max_total_threads: 1,
                        ..Default::default()
                    },
                },
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
                    let panel_target = size.0 as f32 * 0.21;
                    let mut background_image = bevy::image::Image::default();
                    let mut menu_texture = bevy::image::Image::default();
                    for (ktx2_bytes, image, target_width, target_height) in [
                        (
                            &include_bytes!("../assets/000.ktx2")[..],
                            &mut background_image,
                            size.0 as f32,
                            size.1 as f32,
                        ),
                        (
                            &include_bytes!("../assets/001.ktx2")[..],
                            &mut menu_texture,
                            panel_target,
                            size.1 as f32,
                        ),
                    ] {
                        let transcoder =
                            basisu::Transcoder::new(ktx2_bytes).expect("ktx2 container");
                        let (source_width, source_height) = transcoder.base_dimensions();
                        let mut pixels = transcoder
                            .transcode(0, basisu::TargetFormat::Rgba32, basisu::DecodeFlags::NONE)
                            .expect("ktx2 transcode");
                        image.texture_descriptor.format =
                            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb;
                        image.sampler = bevy::image::ImageSampler::linear();
                        let mut final_width = source_width;
                        let mut final_height = source_height;
                        let downscale = (target_width / source_width as f32)
                            .max(target_height / source_height as f32);
                        if downscale < 1.0 {
                            let target_w =
                                ((source_width as f32 * downscale).round().max(1.0)) as u32;
                            let target_h =
                                ((source_height as f32 * downscale).round().max(1.0)) as u32;
                            let mut current_w = source_width;
                            let mut current_h = source_height;
                            while current_w >= target_w * 2 && current_h >= target_h * 2 {
                                let half_w = current_w / 2;
                                let half_h = current_h / 2;
                                let mut reduced = vec![0u8; (half_w * half_h * 4) as usize];
                                let mut y = 0u32;
                                while y < half_h {
                                    let mut x = 0u32;
                                    while x < half_w {
                                        let mut channel = 0usize;
                                        while channel < 4 {
                                            let top_left =
                                                pixels[(((y * 2) * current_w) + x * 2) as usize * 4
                                                    + channel]
                                                    as u32;
                                            let top_right =
                                                pixels[(((y * 2) * current_w) + x * 2 + 1) as usize
                                                    * 4
                                                    + channel]
                                                    as u32;
                                            let bottom_left =
                                                pixels[(((y * 2 + 1) * current_w) + x * 2) as usize
                                                    * 4
                                                    + channel]
                                                    as u32;
                                            let bottom_right =
                                                pixels[(((y * 2 + 1) * current_w) + x * 2 + 1)
                                                    as usize
                                                    * 4
                                                    + channel]
                                                    as u32;
                                            reduced[((y * half_w) + x) as usize * 4 + channel] =
                                                ((top_left
                                                    + top_right
                                                    + bottom_left
                                                    + bottom_right
                                                    + 2)
                                                    / 4)
                                                    as u8;
                                            channel += 1;
                                        }
                                        x += 1;
                                    }
                                    y += 1;
                                }
                                pixels = reduced;
                                current_w = half_w;
                                current_h = half_h;
                            }
                            let mut resampled = vec![0u8; (target_w * target_h * 4) as usize];
                            let step_x = current_w as f32 / target_w as f32;
                            let step_y = current_h as f32 / target_h as f32;
                            let mut y = 0u32;
                            while y < target_h {
                                let origin_y = ((y as f32 + 0.5) * step_y - 0.5).max(0.0);
                                let y0 = origin_y as u32;
                                let y1 = (y0 + 1).min(current_h - 1);
                                let blend_y = origin_y - y0 as f32;
                                let mut x = 0u32;
                                while x < target_w {
                                    let origin_x = ((x as f32 + 0.5) * step_x - 0.5).max(0.0);
                                    let x0 = origin_x as u32;
                                    let x1 = (x0 + 1).min(current_w - 1);
                                    let blend_x = origin_x - x0 as f32;
                                    let mut channel = 0usize;
                                    while channel < 4 {
                                        let top_left = pixels
                                            [((y0 * current_w) + x0) as usize * 4 + channel]
                                            as f32;
                                        let top_right = pixels
                                            [((y0 * current_w) + x1) as usize * 4 + channel]
                                            as f32;
                                        let bottom_left = pixels
                                            [((y1 * current_w) + x0) as usize * 4 + channel]
                                            as f32;
                                        let bottom_right = pixels
                                            [((y1 * current_w) + x1) as usize * 4 + channel]
                                            as f32;
                                        let top = top_left + (top_right - top_left) * blend_x;
                                        let bottom =
                                            bottom_left + (bottom_right - bottom_left) * blend_x;
                                        resampled[((y * target_w) + x) as usize * 4 + channel] =
                                            (top + (bottom - top) * blend_y)
                                                .round()
                                                .clamp(0.0, 255.0)
                                                as u8;
                                        channel += 1;
                                    }
                                    x += 1;
                                }
                                y += 1;
                            }
                            pixels = resampled;
                            final_width = target_w;
                            final_height = target_h;
                        }
                        image.data = Some(pixels);
                        image.texture_descriptor.size = bevy::render::render_resource::Extent3d {
                            width: final_width,
                            height: final_height,
                            depth_or_array_layers: 1,
                        };
                    }
                    background_size = bevy::math::Vec2::new(
                        background_image.width() as f32,
                        background_image.height() as f32,
                    );
                    let cover =
                        (size.0 as f32 / background_size.x).max(size.1 as f32 / background_size.y);
                    let menu_handle = images.add(menu_texture);
                    let background_handle = images.add(background_image);
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
                    camera = Some(
                        commands
                            .spawn((
                                bevy::camera::Camera2d,
                                bevy::camera::Camera {
                                    clear_color: bevy::camera::ClearColorConfig::None,
                                    ..Default::default()
                                },
                            ))
                            .id(),
                    );
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
                    if camera.is_none() {
                        camera = Some(
                            commands
                                .spawn((
                                    bevy::camera::Camera2d,
                                    bevy::camera::Camera {
                                        clear_color: bevy::camera::ClearColorConfig::None,
                                        ..Default::default()
                                    },
                                ))
                                .id(),
                        );
                    }
                    if let (Some(panel_entity), Some(edge_entity), Some(inset_entity)) =
                        (panel.take(), edge.take(), inset.take())
                    {
                        commands.entity(panel_entity).despawn();
                        commands.entity(edge_entity).despawn();
                        commands.entity(inset_entity).despawn();
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
                            if let Some(camera_entity) = camera.take() {
                                commands.entity(camera_entity).despawn();
                            }
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
                        if camera.is_none() {
                            camera = Some(
                                commands
                                    .spawn((
                                        bevy::camera::Camera2d,
                                        bevy::camera::Camera {
                                            clear_color: bevy::camera::ClearColorConfig::None,
                                            ..Default::default()
                                        },
                                    ))
                                    .id(),
                            );
                        }
                        let width = viewport.0 as f32;
                        let menu_width = width * 0.21;
                        let edge_width = (width * 0.0035).max(3.0);
                        let inset_width = (width * 0.0018).max(2.0);
                        let closed = -width / 2.0 + width * 0.21 / 2.0 - width * 0.21 * 1.05;
                        panel = Some(
                            commands
                                .spawn(bevy::transform::components::Transform::from_xyz(
                                    closed, 0.0, 1.0,
                                ))
                                .id(),
                        );
                        edge = Some(
                            commands
                                .spawn(bevy::transform::components::Transform::from_xyz(
                                    closed + menu_width / 2.0 - edge_width / 2.0,
                                    0.0,
                                    2.0,
                                ))
                                .id(),
                        );
                        inset = Some(
                            commands
                                .spawn(bevy::transform::components::Transform::from_xyz(
                                    closed + menu_width / 2.0
                                        - edge_width
                                        - 10.0
                                        - inset_width / 2.0,
                                    0.0,
                                    2.0,
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
                    if let (
                        Some(panel_entity),
                        Some(edge_entity),
                        Some(inset_entity),
                        Some(menu_handle),
                    ) = (panel, edge, inset, menu_image.clone())
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
                        let menu_width = width * 0.21;
                        let edge_width = (width * 0.0035).max(3.0);
                        let inset_width = (width * 0.0018).max(2.0);
                        let x =
                            -width / 2.0 + width * 0.21 / 2.0 - (1.0 - eased) * width * 0.21 * 1.05;
                        commands.entity(panel_entity).insert((
                            bevy::sprite::Sprite {
                                image: menu_handle,
                                custom_size: Some(bevy::math::Vec2::new(menu_width, height)),
                                color: bevy::color::Color::srgba(1.0, 1.0, 1.0, 0.86 * eased),
                                ..Default::default()
                            },
                            bevy::transform::components::Transform::from_xyz(x, 0.0, 1.0),
                        ));
                        commands.entity(edge_entity).insert((
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
                        commands.entity(inset_entity).insert((
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
                        if animation == 1 && progress >= 1.0 {
                            animation = 0;
                        }
                        if animation == 2 && progress <= 0.0 {
                            commands.entity(panel_entity).despawn();
                            commands.entity(edge_entity).despawn();
                            commands.entity(inset_entity).despawn();
                            panel = None;
                            edge = None;
                            inset = None;
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
                    if camera.is_some() {
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
