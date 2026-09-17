#![deny(warnings)]
#![forbid(unsafe_code)]

#[global_allocator]
static AL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let k = include_bytes!("../assets/000.ktx2");
    let (ftx, frx) = std::sync::mpsc::channel::<bevy::image::CompressedImageFormats>();
    let (itx, irx) = std::sync::mpsc::channel::<Result<bevy::image::Image, String>>();
    std::thread::spawn(move || {
        if let Ok(f) = frx.recv() {
            let _ = itx.send(
                bevy::image::ktx2_buffer_to_image(k, f, true)
                    .map(|mut i| {
                        i.sampler = bevy::image::ImageSampler::linear();
                        i
                    })
                    .map_err(|e| e.to_string()),
            );
        }
    });
    let mut a = bevy::app::App::new();
    a.insert_resource(bevy::winit::WinitSettings {
        focused_mode: bevy::winit::UpdateMode::Continuous,
        unfocused_mode: bevy::winit::UpdateMode::Continuous,
    });
    a.add_plugins((
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
                    mode: bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Primary),
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
    ));
    let p = std::sync::Mutex::new(irx);
    let q = std::sync::Mutex::new(ftx);
    a.add_systems(
        bevy::app::Update,
        move |mut c: bevy::ecs::system::Commands,
              mut wq: bevy::ecs::system::Query<&mut bevy::window::Window>,
              mut ia: bevy::ecs::system::ResMut<bevy::asset::Assets<bevy::image::Image>>,
              k: bevy::ecs::system::Res<bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>>,
              fs: Option<bevy::ecs::system::Res<bevy::image::CompressedImageFormatSupport>>,
              mut ws: bevy::ecs::system::ResMut<bevy::winit::WinitSettings>,
              mut sf: bevy::ecs::system::Local<bool>,
              mut s: bevy::ecs::system::Local<Option<bevy::ecs::entity::Entity>>,
              mut im: bevy::ecs::system::Local<(f32, f32)>,
              mut cam: bevy::ecs::system::Local<Option<bevy::ecs::entity::Entity>>,
              mut px: bevy::ecs::system::Local<(u32, u32)>,
              mut id: bevy::ecs::system::Local<Option<std::time::Instant>>,
              mut hd: bevy::ecs::system::Local<Option<std::time::Instant>>,
              mut mn: bevy::ecs::system::Local<Option<(bevy::ecs::entity::Entity, bevy::ecs::entity::Entity, bevy::ecs::entity::Entity)>>,
              mut mc: bevy::ecs::system::Local<Option<std::time::Instant>>| {
            let mut w = wq.single_mut().unwrap();
            if w.mode != bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Primary) {
                w.mode = bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Primary);
            }
            if w.decorations {
                w.decorations = false;
            }
            if w.resizable {
                w.resizable = false;
            }
            let ww = w.physical_width();
            let wh = w.physical_height();
            if !*sf {
                if let Some(f) = fs.as_deref() {
                    let _ = q.lock().map(|t| t.send(f.0));
                    *sf = true;
                }
            }
            if s.is_none() {
                if let Some(r) = p.lock().ok().and_then(|g| g.try_recv().ok()) {
                    match r {
                        Err(e) => {
                            eprintln!("arxumbra: {e}");
                            c.write_message(bevy::app::AppExit::error());
                        }
                        Ok(i) => {
                            *im = (i.width() as f32, i.height() as f32);
                            let h = ia.add(i);
                            *s = Some(
                                c.spawn((
                                    bevy::sprite::Sprite {
                                        image: h,
                                        custom_size: Some(bevy::math::Vec2::new(im.0, im.1)),
                                        ..Default::default()
                                    },
                                    bevy::transform::components::Transform::from_scale(
                                        bevy::math::Vec3::splat((ww as f32 / im.0).max(wh as f32 / im.1)),
                                    ),
                                ))
                                .id(),
                            );
                            *cam = Some(
                                c.spawn((
                                    bevy::camera::Camera2d,
                                    bevy::camera::Camera {
                                        clear_color: bevy::camera::ClearColorConfig::None,
                                        ..Default::default()
                                    },
                                ))
                                .id(),
                            );
                            w.visible = true;
                            *px = (ww, wh);
                        }
                    }
                }
            } else if ww != px.0 || wh != px.1 {
                *px = (ww, wh);
                c.entity(s.unwrap()).insert(bevy::transform::components::Transform::from_scale(
                    bevy::math::Vec3::splat((ww as f32 / im.0).max(wh as f32 / im.1)),
                ));
                if cam.is_none() {
                    *cam = Some(
                        c.spawn((
                            bevy::camera::Camera2d,
                            bevy::camera::Camera {
                                clear_color: bevy::camera::ClearColorConfig::None,
                                ..Default::default()
                            },
                        ))
                        .id(),
                    );
                }
                *id = None;
                if let Some((p, b, u)) = *mn {
                    c.entity(p).despawn();
                    c.entity(b).despawn();
                    c.entity(u).despawn();
                    *mn = None;
                    *mc = None;
                }
                ws.focused_mode = bevy::winit::UpdateMode::Continuous;
            } else if cam.is_some() && mn.is_none() && mc.is_none() {
                if id.is_none() {
                    *id = Some(std::time::Instant::now());
                }
                if let Some(at) = *id {
                    if at.elapsed() >= std::time::Duration::from_millis(500) {
                        c.entity(cam.take().unwrap()).despawn();
                        *ws = bevy::winit::WinitSettings::desktop_app();
                        *id = None;
                    }
                }
            }
            if k.just_pressed(bevy::input::keyboard::KeyCode::F11) {
                if let Some((p, b, u)) = *mn {
                    c.entity(p).despawn();
                    c.entity(b).despawn();
                    c.entity(u).despawn();
                    *mn = None;
                    *mc = Some(std::time::Instant::now());
                } else {
                    *mc = None;
                    if cam.is_none() {
                        *cam = Some(
                            c.spawn((
                                bevy::camera::Camera2d,
                                bevy::camera::Camera {
                                    clear_color: bevy::camera::ClearColorConfig::None,
                                    ..Default::default()
                                },
                            ))
                            .id(),
                        );
                        *id = None;
                    }
                    let mw = 0.21 * ww as f32;
                    let sw = (0.0035 * ww as f32).max(3.0);
                    let pw = (0.0018 * ww as f32).max(2.0);
                    let mx = -(ww as f32) / 2.0 + mw / 2.0;
                    *mn = Some((
                        c.spawn((
                            bevy::sprite::Sprite::from_color(
                                bevy::color::Color::srgba(0.102, 0.106, 0.149, 0.86),
                                bevy::math::Vec2::new(mw, wh as f32),
                            ),
                            bevy::transform::components::Transform::from_xyz(mx, 0.0, 1.0),
                        ))
                        .id(),
                        c.spawn((
                            bevy::sprite::Sprite::from_color(
                                bevy::color::Color::srgba(0.478, 0.635, 0.969, 0.92),
                                bevy::math::Vec2::new(sw, wh as f32),
                            ),
                            bevy::transform::components::Transform::from_xyz(
                                mx + mw / 2.0 - sw / 2.0,
                                0.0,
                                2.0,
                            ),
                        ))
                        .id(),
                        c.spawn((
                            bevy::sprite::Sprite::from_color(
                                bevy::color::Color::srgba(0.733, 0.604, 0.969, 0.75),
                                bevy::math::Vec2::new(pw, wh as f32),
                            ),
                            bevy::transform::components::Transform::from_xyz(
                                mx + mw / 2.0 - sw - 10.0 - pw / 2.0,
                                0.0,
                                2.0,
                            ),
                        ))
                        .id(),
                    ));
                    ws.focused_mode = bevy::winit::UpdateMode::Continuous;
                }
            }
            if let Some(at) = *mc {
                if mn.is_none() && at.elapsed() >= std::time::Duration::from_millis(250) {
                    *mc = None;
                    if cam.is_some() {
                        c.entity(cam.take().unwrap()).despawn();
                    }
                    *ws = bevy::winit::WinitSettings::desktop_app();
                }
            }
            if k.just_pressed(bevy::input::keyboard::KeyCode::Escape) {
                *hd = Some(std::time::Instant::now());
                ws.focused_mode = bevy::winit::UpdateMode::reactive(std::time::Duration::from_millis(100));
            }
            if k.just_released(bevy::input::keyboard::KeyCode::Escape) {
                *hd = None;
                if cam.is_some() {
                    ws.focused_mode = bevy::winit::UpdateMode::Continuous;
                } else {
                    *ws = bevy::winit::WinitSettings::desktop_app();
                }
            }
            if let Some(at) = *hd {
                if at.elapsed() >= std::time::Duration::from_secs(7) {
                    std::process::exit(0);
                }
            }
        },
    );
    a.run();
}
