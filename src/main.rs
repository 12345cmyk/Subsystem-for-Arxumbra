use bevy::prelude::*;
use bevy::ecs::message::MessageReader;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::window::{MonitorSelection, WindowMode};

const P: &str = "[Help-From-the-Void-Independent-Systems@Arxumbra]$ ";

#[derive(Resource, Default)]
struct Term {
    log: String,
    buf: String,
    dirty: bool,
}

#[derive(Resource, Default)]
struct FullscreenDone(bool);

#[derive(Component)]
struct TerminalText;

#[inline]
fn bg(v: f32) -> Color {
    Color::srgb(v, v, v)
}

fn spawn_gui(mut c: Commands) {
    c.spawn(Camera2d);
    c.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(bg(0.19)),
    )).with_children(|root| {
        root.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(42.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(bg(0.10)),
        )).with_children(|t| {
            t.spawn((Text::new("Arxumbra OS — Terminal"), TextFont { font_size: FontSize::Px(18.0), ..default() }));
            t.spawn((Text::new("   [—]  [□]  [×]"), TextFont { font_size: FontSize::Px(18.0), ..default() }));
        });
        root.spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            padding: UiRect::all(Val::Px(16.0)),
            align_items: AlignItems::FlexStart,
            flex_direction: FlexDirection::Column,
            ..default()
        }).with_children(|b| {
            b.spawn((TerminalText, Text::new(P), TextFont { font_size: FontSize::Px(15.0), ..default() }));
        });
        root.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(bg(0.10)),
        )).with_children(|s| {
            s.spawn((Text::new("Ready"), TextFont { font_size: FontSize::Px(13.0), ..default() }));
            s.spawn((Text::new("Arxumbra v1.0  ·  fullscreen  ·  wayland"), TextFont { font_size: FontSize::Px(13.0), ..default() }));
        });
    });
}

fn set_fullscreen(mut windows: Query<&mut Window>, mut done: ResMut<FullscreenDone>) {
    if done.0 {
        return;
    }
    for mut w in &mut windows {
        w.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Primary);
    }
    done.0 = true;
}

fn on_keyboard(mut kb: MessageReader<KeyboardInput>, mut t: ResMut<Term>, mut exit: MessageWriter<bevy::app::AppExit>) {
    for e in kb.read() {
        if e.state != ButtonState::Pressed {
            continue;
        }
        match &e.logical_key {
            Key::Enter => {
                let cmd = t.buf.trim().to_string();
                let death = matches!(cmd.as_str(), "Death");
                t.log.push_str(P);
                t.log.push_str(&cmd);
                t.log.push('\n');
                match cmd.as_str() {
                    "FS" => t.log.push_str("    Hello, World!\n"),
                    "Death" => t.log.push_str("    Arxumbra is powering off…\n"),
                    "G" => t.log.clear(),
                    _ => {}
                }
                t.buf.clear();
                t.dirty = true;
                if death {
                    exit.write(bevy::app::AppExit::Success);
                }
            }
            Key::Backspace => {
                if t.buf.pop().is_some() {
                    t.dirty = true;
                }
            }
            Key::Character(s) => {
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

fn render(mut t: ResMut<Term>, mut q: Query<&mut Text, With<TerminalText>>) {
    if !t.dirty {
        return;
    }
    let mut out = String::with_capacity(t.log.len() + P.len() + t.buf.len());
    out.push_str(&t.log);
    out.push_str(P);
    out.push_str(&t.buf);
    if let Some(mut text) = q.iter_mut().next() {
        *text = Text::new(out);
    }
    t.dirty = false;
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<Term>()
        .init_resource::<FullscreenDone>()
        .add_systems(Startup, spawn_gui)
        .add_systems(Update, (set_fullscreen, on_keyboard, render))
        .run();
}
