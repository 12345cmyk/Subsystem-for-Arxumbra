fn main() {
    bevy::app::App::new()
        .set_runner(|_| loop {
            std::io::Write::write_all(&mut std::io::stderr(), b"[Help-From-the-Void-Independent-Systems@Arxumbra]$ ").ok();
            if std::io::stdin().lines().next().map_or(true, |r| matches!(r.as_deref(), Ok(s) if s.trim() == "exit")) {
                return bevy::app::AppExit::Success;
            }
        })
        .run();
}
