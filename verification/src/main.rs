#![deny(warnings)]

fn main() {
    let main_source: &'static str = include_str!("../../src/main.rs");
    let expected_formula = "(31 - (((source_width / need_width).min(source_height / need_height)).max(1)).leading_zeros()).min(levels - 1)";
    assert!(
        main_source.contains(expected_formula),
        "shipped closed-form formula missing in src/main.rs"
    );

    let background_source: &'static [u8] = include_bytes!("../../assets/000.ktx2");
    let menu_source: &'static [u8] = include_bytes!("../../assets/001.ktx2");
    let cursor_source: &'static [u8] = include_bytes!("../../assets/cursor.ktx2");

    let background = basisu::Transcoder::new(background_source).expect("background ktx2");
    let menu = basisu::Transcoder::new(menu_source).expect("menu ktx2");
    let cursor = basisu::Transcoder::new(cursor_source).expect("cursor ktx2");

    let sheets = [
        ("background", &background),
        ("menu", &menu),
        ("cursor", &cursor),
    ];

    let level_bytes = |source_width: u32, source_height: u32, mip: u32| -> usize {
        (((((source_width >> mip).max(1) + 3) / 4) as usize)
            * ((((source_height >> mip).max(1) + 3) / 4) as usize))
            * 16
    };

    let select_level_closed = |source_width: u32,
                               source_height: u32,
                               levels: u32,
                               need_width: u32,
                               need_height: u32|
     -> u32 {
        (31 - (((source_width / need_width).min(source_height / need_height)).max(1)).leading_zeros()).min(levels - 1)
    };

    let select_level_loop = |source_width: u32,
                             source_height: u32,
                             levels: u32,
                             need_width: u32,
                             need_height: u32|
     -> u32 {
        let mut level = 0u32;
        while level + 1 < levels
            && (source_width >> (level + 1)) >= need_width
            && (source_height >> (level + 1)) >= need_height
        {
            level += 1;
        }
        level
    };

    let mut verified_levels = 0usize;
    let mut level_mismatches = 0usize;

    for (name, transcoder) in sheets {
        let (w, h) = transcoder.base_dimensions();
        let levels = transcoder.level_count();
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

    let mut viewport_count = 0usize;
    let mut comparison_count = 0usize;
    let mut formula_mismatches = 0usize;

    let (bg_w, bg_h) = background.base_dimensions();
    let bg_levels = background.level_count();
    let (menu_w, menu_h) = menu.base_dimensions();
    let menu_levels = menu.level_count();
    let (cur_w, cur_h) = cursor.base_dimensions();
    let cur_levels = cursor.level_count();

    for step_w in 1u32..=1000u32 {
        let physical_width = step_w * 8;
        let panel_need = (((physical_width as f32) * 0.21).ceil() as u32).max(1);
        for step_h in 1u32..=1000u32 {
            let physical_height = step_h * 8;
            let cursor_need = (((physical_height as f32) * 0.055)
                .clamp(28.0, 320.0)
                .ceil() as u32)
                .max(1);
            viewport_count += 1;

            if select_level_closed(bg_w, bg_h, bg_levels, physical_width, physical_height)
                != select_level_loop(bg_w, bg_h, bg_levels, physical_width, physical_height)
            {
                formula_mismatches += 1;
            }
            if select_level_closed(menu_w, menu_h, menu_levels, panel_need, physical_height)
                != select_level_loop(menu_w, menu_h, menu_levels, panel_need, physical_height)
            {
                formula_mismatches += 1;
            }
            if select_level_closed(cur_w, cur_h, cur_levels, cursor_need, cursor_need)
                != select_level_loop(cur_w, cur_h, cur_levels, cursor_need, cursor_need)
            {
                formula_mismatches += 1;
            }
            comparison_count += 3;
        }
    }
    assert_eq!(formula_mismatches, 0);
    println!(
        "ladder_proof: viewports={} level_sizes_verified={} comparisons={} mismatches={}",
        viewport_count, verified_levels, comparison_count, formula_mismatches
    );

    for (name, transcoder) in sheets {
        let (w, h) = transcoder.base_dimensions();
        let levels = transcoder.level_count();
        let tail_level = levels - 1;
        let mid_level = levels / 2;
        let tail_size = level_bytes(w, h, tail_level);
        let mid_size = level_bytes(w, h, mid_level);
        let mut tail_buf = vec![0u8; tail_size];
        let mut mid_buf = vec![0u8; mid_size];
        transcoder
            .transcode_into(
                tail_level,
                basisu::TargetFormat::Bc7Rgba,
                basisu::DecodeFlags::NONE,
                &mut tail_buf,
            )
            .expect("tail transcode");
        transcoder
            .transcode_into(
                mid_level,
                basisu::TargetFormat::Bc7Rgba,
                basisu::DecodeFlags::NONE,
                &mut mid_buf,
            )
            .expect("mid transcode");
        println!(
            "transcode_probe: sheet={} tail_level={} tail_bytes={} mid_level={} mid_bytes={}",
            name, tail_level, tail_size, mid_level, mid_size
        );
    }

    for (vw, vh) in [(1920u32, 1080u32), (3840u32, 2160u32)] {
        let width = vw as f32;
        let height = vh as f32;
        let panel_width = width * 0.21;
        let cursor_extent = (height * 0.055).clamp(28.0, 320.0);
        let cover = (width / bg_w as f32).max(height / bg_h as f32);
        let panel_need = (panel_width.ceil() as u32).max(1);
        let cursor_need = (cursor_extent.ceil() as u32).max(1);

        let bg_lvl = select_level_closed(bg_w, bg_h, bg_levels, vw, vh);
        let menu_lvl = select_level_closed(menu_w, menu_h, menu_levels, panel_need, vh);
        let cur_lvl = select_level_closed(cur_w, cur_h, cur_levels, cursor_need, cursor_need);

        let bg_mips = if cover >= 1.0 { 1 } else { bg_levels - bg_lvl };
        let menu_mips = if panel_width >= (menu_w >> menu_lvl) as f32
            && height >= (menu_h >> menu_lvl) as f32
        {
            1
        } else {
            menu_levels - menu_lvl
        };
        let cur_mips = if cursor_extent >= (cur_w >> cur_lvl) as f32 {
            1
        } else {
            cur_levels - cur_lvl
        };

        let start = std::time::Instant::now();
        let bg_bytes: usize = (0..bg_mips).map(|i| level_bytes(bg_w, bg_h, bg_lvl + i)).sum();
        let menu_bytes: usize = (0..menu_mips)
            .map(|i| level_bytes(menu_w, menu_h, menu_lvl + i))
            .sum();
        let cur_bytes: usize = (0..cur_mips)
            .map(|i| level_bytes(cur_w, cur_h, cur_lvl + i))
            .sum();
        let mut bg_buf = vec![0u8; bg_bytes];
        let mut off = 0usize;
        for i in 0..bg_mips {
            let sz = level_bytes(bg_w, bg_h, bg_lvl + i);
            background
                .transcode_into(
                    bg_lvl + i,
                    basisu::TargetFormat::Bc7Rgba,
                    basisu::DecodeFlags::NONE,
                    &mut bg_buf[off..off + sz],
                )
                .expect("bg chain");
            off += sz;
        }
        let mut menu_buf = vec![0u8; menu_bytes];
        off = 0;
        for i in 0..menu_mips {
            let sz = level_bytes(menu_w, menu_h, menu_lvl + i);
            menu.transcode_into(
                menu_lvl + i,
                basisu::TargetFormat::Bc7Rgba,
                basisu::DecodeFlags::NONE,
                &mut menu_buf[off..off + sz],
            )
            .expect("menu chain");
            off += sz;
        }
        let mut cur_buf = vec![0u8; cur_bytes];
        off = 0;
        for i in 0..cur_mips {
            let sz = level_bytes(cur_w, cur_h, cur_lvl + i);
            cursor
                .transcode_into(
                    cur_lvl + i,
                    basisu::TargetFormat::Bc7Rgba,
                    basisu::DecodeFlags::NONE,
                    &mut cur_buf[off..off + sz],
                )
                .expect("cursor chain");
            off += sz;
        }
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        let total_vram = bg_bytes + menu_bytes + cur_bytes;
        println!(
            "vram_probe: viewport={}x{} bg=(L{},mips={},bytes={}) menu=(L{},mips={},bytes={}) cursor=(L{},mips={},bytes={}) total_vram_bytes={} cold_decode_ms={:.3}",
            vw,
            vh,
            bg_lvl,
            bg_mips,
            bg_bytes,
            menu_lvl,
            menu_mips,
            menu_bytes,
            cur_lvl,
            cur_mips,
            cur_bytes,
            total_vram,
            elapsed_ms
        );
    }
}
