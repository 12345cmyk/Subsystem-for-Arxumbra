#![deny(warnings)]
#![forbid(unsafe_code)]

fn main() {
    let main_source: &'static str = include_str!("../../src/main.rs");
    for required_token in [
        "split_at_mut",
        "synchronous_pipeline_compilation: true",
        "TEXTURE_BINDING_ARRAY",
        "BUFFER_BINDING_ARRAY",
        "NoFrustumCulling",
        "FrameCountPlugin",
        "mesh.set_changed()",
    ] {
        assert!(
            main_source.contains(required_token),
            "required token missing in src/main.rs: {}",
            required_token
        );
    }

    let background_source: &'static [u8] = include_bytes!("../../assets/000.ktx2");
    let menu_source: &'static [u8] = include_bytes!("../../assets/001.ktx2");
    let cursor_source: &'static [u8] = include_bytes!("../../assets/cursor.ktx2");

    let background = basisu::Transcoder::new(background_source).expect("background ktx2");
    let menu = basisu::Transcoder::new(menu_source).expect("menu ktx2");
    let cursor = basisu::Transcoder::new(cursor_source).expect("cursor ktx2");
    assert!(!background.has_alpha() && menu.has_alpha() && cursor.has_alpha());

    let sheets = [
        ("background", &background),
        ("menu", &menu),
        ("cursor", &cursor),
    ];

    let level_bytes = |source_width: u32, source_height: u32, mip: u32| -> usize {
        (((((source_width >> mip).max(1) + 3) >> 2) as usize)
            * ((((source_height >> mip).max(1) + 3) >> 2) as usize))
            << 4
    };

    let mut verified_levels = 0usize;
    let mut level_mismatches = 0usize;

    for (name, transcoder) in sheets {
        let (w, h) = transcoder.base_dimensions();
        let levels = transcoder.level_count();
        assert_eq!(w % 4, 0, "{} base width must be 4-aligned for BC7", name);
        assert_eq!(h % 4, 0, "{} base height must be 4-aligned for BC7", name);
        println!(
            "sheet={} base={}x{} levels={} alpha={}",
            name,
            w,
            h,
            levels,
            transcoder.has_alpha()
        );
        for mip in 0..levels {
            let expected = level_bytes(w, h, mip);
            let actual = transcoder
                .output_size(mip, basisu::TargetFormat::Bc7Rgba)
                .expect("output_size");
            if expected != actual {
                level_mismatches += 1;
            }
            verified_levels += 1;
        }
    }
    assert_eq!(level_mismatches, 0);
    println!("verified_levels={} mismatches={}", verified_levels, level_mismatches);

    let (bg_w, bg_h) = background.base_dimensions();
    let bg_levels = background.level_count();
    let (menu_w, menu_h) = menu.base_dimensions();
    let menu_levels = menu.level_count();
    let (cur_w, cur_h) = cursor.base_dimensions();
    let cur_levels = cursor.level_count();

    let bg_head_bytes = level_bytes(bg_w, bg_h, 0);
    let bg_total_bytes = (0..bg_levels).map(|mip| level_bytes(bg_w, bg_h, mip)).sum::<usize>();
    let menu_head_bytes = level_bytes(menu_w, menu_h, 0);
    let menu_total_bytes = (0..menu_levels)
        .map(|mip| level_bytes(menu_w, menu_h, mip))
        .sum::<usize>();
    let cur_total_bytes = (0..cur_levels)
        .map(|mip| level_bytes(cur_w, cur_h, mip))
        .sum::<usize>();

    let start = std::time::Instant::now();
    let mut bg_pixels = vec![0u8; bg_total_bytes];
    let mut menu_pixels = vec![0u8; menu_total_bytes];
    let mut cur_pixels = vec![0u8; cur_total_bytes];
    {
        let (bg_head, bg_tail) = bg_pixels.split_at_mut(bg_head_bytes);
        let (menu_head, menu_tail) = menu_pixels.split_at_mut(menu_head_bytes);
        let cur_slice = &mut cur_pixels[..];
        let bg_ref = &background;
        let menu_ref = &menu;
        let cur_ref = &cursor;
        std::thread::scope(|scope| {
            let bg_head_job = scope.spawn(move || {
                bg_ref
                    .transcode_into(
                        0,
                        basisu::TargetFormat::Bc7Rgba,
                        basisu::DecodeFlags::NONE,
                        bg_head,
                    )
                    .expect("bg head");
            });
            let bg_tail_job = scope.spawn(move || {
                let mut offset = 0usize;
                for level in 1..bg_levels {
                    let sz = level_bytes(bg_w, bg_h, level);
                    bg_ref
                        .transcode_into(
                            level,
                            basisu::TargetFormat::Bc7Rgba,
                            basisu::DecodeFlags::NONE,
                            &mut bg_tail[offset..offset + sz],
                        )
                        .expect("bg tail");
                    offset += sz;
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
                    .expect("menu head");
            });
            let menu_tail_job = scope.spawn(move || {
                let mut offset = 0usize;
                for level in 1..menu_levels {
                    let sz = level_bytes(menu_w, menu_h, level);
                    menu_ref
                        .transcode_into(
                            level,
                            basisu::TargetFormat::Bc7Rgba,
                            basisu::DecodeFlags::NONE,
                            &mut menu_tail[offset..offset + sz],
                        )
                        .expect("menu tail");
                    offset += sz;
                }
            });
            let cur_job = scope.spawn(move || {
                let mut offset = 0usize;
                for level in 0..cur_levels {
                    let sz = level_bytes(cur_w, cur_h, level);
                    cur_ref
                        .transcode_into(
                            level,
                            basisu::TargetFormat::Bc7Rgba,
                            basisu::DecodeFlags::NONE,
                            &mut cur_slice[offset..offset + sz],
                        )
                        .expect("cur");
                    offset += sz;
                }
            });
            bg_head_job.join().expect("bg head join");
            bg_tail_job.join().expect("bg tail join");
            menu_head_job.join().expect("menu head join");
            menu_tail_job.join().expect("menu tail join");
            cur_job.join().expect("cur join");
        });
    }
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "full_chain_transcode: bg_bytes={} menu_bytes={} cursor_bytes={} total_bytes={} elapsed_ms={:.3}",
        bg_total_bytes,
        menu_total_bytes,
        cur_total_bytes,
        bg_total_bytes + menu_total_bytes + cur_total_bytes,
        elapsed_ms
    );
}
