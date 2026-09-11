const PROMPT: &str = "[Help-From-the-Void-Independent-Systems@Arxumbra]$ ";

#[derive(bevy::ecs::resource::Resource, Default)]
struct Term {
    log: String,
    buf: String,
    out: String,
    dirty: bool,
}

#[derive(bevy::ecs::component::Component)]
struct TerminalText;

#[inline]
fn bg(v: f32) -> bevy::color::Color {
    bevy::color::Color::srgb(v, v, v)
}

fn silence() {
    std::panic::set_hook(Box::new(|_| {}));
    let fd = unsafe { libc::open(b"/dev/null\0".as_ptr() as *const _, libc::O_RDWR) };
    unsafe {
        libc::dup2(fd, 1);
        libc::dup2(fd, 2);
    }
}

fn spawn_gui(mut c: bevy::ecs::system::Commands) {
    c.spawn(bevy::camera::Camera2d);
    c.spawn((
        bevy::ui::Node {
            position_type: bevy::ui::PositionType::Absolute,
            width: bevy::ui::Val::Percent(100.0),
            height: bevy::ui::Val::Percent(100.0),
            flex_direction: bevy::ui::FlexDirection::Column,
            ..bevy::ui::Node::default()
        },
        bevy::ui::BackgroundColor(bg(0.19)),
    ))
    .with_children(|root| {
        root.spawn((
            bevy::ui::Node {
                width: bevy::ui::Val::Percent(100.0),
                height: bevy::ui::Val::Px(42.0),
                flex_direction: bevy::ui::FlexDirection::Row,
                align_items: bevy::ui::AlignItems::Center,
                justify_content: bevy::ui::JustifyContent::SpaceBetween,
                padding: bevy::ui::UiRect::horizontal(bevy::ui::Val::Px(16.0)),
                ..bevy::ui::Node::default()
            },
            bevy::ui::BackgroundColor(bg(0.10)),
        ))
        .with_children(|t| {
            t.spawn((
                bevy::ui::widget::Text::new("Arxumbra OS - Terminal"),
                bevy::text::TextFont {
                    font_size: bevy::text::FontSize::Px(18.0),
                    ..bevy::text::TextFont::default()
                },
            ));
            t.spawn((
                bevy::ui::widget::Text::new("   [-]  [ ]  [x]"),
                bevy::text::TextFont {
                    font_size: bevy::text::FontSize::Px(18.0),
                    ..bevy::text::TextFont::default()
                },
            ));
        });
        root
            .spawn(bevy::ui::Node {
                width: bevy::ui::Val::Percent(100.0),
                flex_grow: 1.0,
                padding: bevy::ui::UiRect::all(bevy::ui::Val::Px(16.0)),
                align_items: bevy::ui::AlignItems::FlexStart,
                flex_direction: bevy::ui::FlexDirection::Column,
                ..bevy::ui::Node::default()
            })
            .with_children(|b| {
                b.spawn((
                    TerminalText,
                    bevy::ui::widget::Text::new(PROMPT),
                    bevy::text::TextFont {
                        font_size: bevy::text::FontSize::Px(15.0),
                        ..bevy::text::TextFont::default()
                    },
                ));
            });
        root.spawn((
            bevy::ui::Node {
                width: bevy::ui::Val::Percent(100.0),
                height: bevy::ui::Val::Px(30.0),
                flex_direction: bevy::ui::FlexDirection::Row,
                align_items: bevy::ui::AlignItems::Center,
                justify_content: bevy::ui::JustifyContent::SpaceBetween,
                padding: bevy::ui::UiRect::horizontal(bevy::ui::Val::Px(16.0)),
                ..bevy::ui::Node::default()
            },
            bevy::ui::BackgroundColor(bg(0.10)),
        ))
        .with_children(|s| {
            s.spawn((
                bevy::ui::widget::Text::new("Ready"),
                bevy::text::TextFont {
                    font_size: bevy::text::FontSize::Px(13.0),
                    ..bevy::text::TextFont::default()
                },
            ));
            s.spawn((
                bevy::ui::widget::Text::new("Arxumbra v1.0  -  fullscreen  -  wayland"),
                bevy::text::TextFont {
                    font_size: bevy::text::FontSize::Px(13.0),
                    ..bevy::text::TextFont::default()
                },
            ));
        });
    });
}

fn set_fullscreen(mut q: bevy::ecs::system::Query<&mut bevy::window::Window>) {
    if let Some(mut w) = q.iter_mut().next() {
        w.mode = bevy::window::WindowMode::BorderlessFullscreen(
            bevy::window::MonitorSelection::Primary,
        );
    }
}

fn on_keyboard(
    mut kb: bevy::ecs::message::MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut t: bevy::ecs::change_detection::ResMut<Term>,
    mut exit: bevy::ecs::message::MessageWriter<bevy::app::AppExit>,
) {
    for e in kb.read() {
        if e.state != bevy::input::ButtonState::Pressed {
            continue;
        }
        match &e.logical_key {
            bevy::input::keyboard::Key::Enter => {
                let cmd = t.buf.trim().to_string();
                let death = cmd == "Death";
                t.log.push_str(PROMPT);
                t.log.push_str(&cmd);
                t.log.push('\n');
                match cmd.as_str() {
                    "FS" => t.log.push_str("    Hello, World!\n"),
                    "Death" => t.log.push_str("    Arxumbra is powering off...\n"),
                    "G" => t.log.clear(),
                    _ => {}
                }
                t.buf.clear();
                t.dirty = true;
                if death {
                    exit.write(bevy::app::AppExit::Success);
                }
            }
            bevy::input::keyboard::Key::Backspace => {
                if t.buf.pop().is_some() {
                    t.dirty = true;
                }
            }
            bevy::input::keyboard::Key::Character(s) => {
                let s = s.as_str();
                if !s.chars().any(char::is_control) {
                    t.buf.push_str(s);
                    t.dirty = true;
                }
            }
            _ => {}
        }
    }
}

fn render(
    mut t: bevy::ecs::change_detection::ResMut<Term>,
    mut q: bevy::ecs::system::Query<&mut bevy::ui::widget::Text, bevy::ecs::query::With<TerminalText>>,
) {
    if !t.dirty {
        return;
    }
    let mut out = std::mem::take(&mut t.out);
    out.push_str(&t.log);
    out.push_str(PROMPT);
    out.push_str(&t.buf);
    t.out = out;
    if let Some(mut text) = q.iter_mut().next() {
        *text = bevy::ui::widget::Text::new(t.out.clone());
    }
    t.dirty = false;
}

fn main() {
    silence();
    bevy::app::App::new()
        .add_plugins((
            bevy::app::TaskPoolPlugin::default(),
            bevy::diagnostic::FrameCountPlugin,
            bevy::time::TimePlugin,
            bevy::transform::TransformPlugin,
            bevy::input::InputPlugin,
            bevy::window::WindowPlugin::default(),
            bevy::asset::AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            bevy::camera::CameraPlugin,
            bevy::a11y::AccessibilityPlugin,
            bevy::winit::WinitPlugin::default(),
        ))
        .add_plugins((
            bevy::render::RenderPlugin::default(),
            bevy::image::ImagePlugin::default(),
            bevy::render::pipelined_rendering::PipelinedRenderingPlugin,
            bevy::core_pipeline::CorePipelinePlugin,
            bevy::sprite::SpritePlugin,
            bevy::sprite_render::SpriteRenderPlugin,
            bevy::text::TextPlugin,
            bevy::ui::UiPlugin,
            bevy::ui_render::UiRenderPlugin,
        ))
        .init_resource::<Term>()
        .add_systems(bevy::app::Startup, (spawn_gui, set_fullscreen))
        .add_systems(bevy::app::Update, (on_keyboard, render))
        .run();
}
