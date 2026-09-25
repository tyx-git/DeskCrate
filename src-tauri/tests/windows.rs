mod support;

use std::fs;

use lib::{
    models::{AppData, AppEXE, Category, CssSize, IconType, Item, RootItem, ViewMode, WindowSize},
    services::{self, system},
    storage::DataRepository,
};

use support::TestDir;

fn app_record(source_path: &str, icon_type: IconType) -> AppEXE {
    AppEXE {
        id: "app-1".to_owned(),
        name: "测试应用".to_owned(),
        source_path: source_path.to_owned(),
        arguments: None,
        working_dir: None,
        icon_url: "data:image/svg+xml,test".to_owned(),
        icon_type,
        created_time: "2026-08-31T00:00:00Z".to_owned(),
    }
}

fn state_with_app(directory: &TestDir, app: AppEXE) -> lib::state::AppState {
    let store = directory.store();
    let data = AppData {
        version: 2,
        items: vec![RootItem::Category(Category {
            id: "category-1".to_owned(),
            name: "系统测试".to_owned(),
            is_collapsed: false,
            groups: vec![Item::App(app)],
            created_time: "2026-08-31T00:00:00Z".to_owned(),
        })],
        ..AppData::default()
    };
    store.save_data(&data).expect("fixture data should save");
    lib::state::AppState::new(store)
}

#[test]
fn missing_app_id_is_rejected_before_any_system_operation() {
    let directory = TestDir::new("system-missing");
    let state = directory.state();

    let error = system::launch_app(&state, "missing").expect_err("missing ID should fail");
    assert_eq!(error.code, "app_not_found");
}

#[test]
fn scripts_are_saved_as_files_but_blocked_by_the_launch_service() {
    let directory = TestDir::new("system-script");
    let script = directory.child("danger.ps1");
    fs::write(&script, "Write-Output blocked").expect("script fixture should be written");
    let state = state_with_app(
        &directory,
        app_record(&script.to_string_lossy(), IconType::File),
    );

    let error = system::launch_app(&state, "app-1").expect_err("scripts must be blocked");
    assert_eq!(error.code, "script_execution_not_allowed");
}

#[test]
fn executor_rejects_empty_and_nul_targets_without_launching_anything() {
    let empty_dir = TestDir::new("system-empty");
    let empty_state = state_with_app(&empty_dir, app_record("", IconType::File));
    let error = system::launch_app(&empty_state, "app-1").expect_err("empty target should fail");
    assert_eq!(error.code, "exec_target_empty");

    let nul_dir = TestDir::new("system-nul");
    let nul_source = "C:\\invalid\0path";
    let nul_state = state_with_app(&nul_dir, app_record(nul_source, IconType::File));
    let error = system::launch_app(&nul_state, "app-1").expect_err("NUL target should fail");
    assert_eq!(error.code, "exec_invalid_target");
}

#[test]
fn malformed_shortcut_is_reported_by_the_shortcut_parser_boundary() {
    let directory = TestDir::new("system-shortcut");
    let shortcut = directory.child("broken.lnk");
    fs::write(&shortcut, b"not a shell link").expect("shortcut fixture should be written");
    let state = state_with_app(
        &directory,
        app_record(&shortcut.to_string_lossy(), IconType::Shortcut),
    );

    let error = system::launch_app(&state, "app-1").expect_err("malformed shortcut should fail");
    assert!(matches!(
        error.code.as_str(),
        "shortcut_load_failed" | "shortcut_target_failed"
    ));
}

#[test]
fn expanded_overlay_covers_the_sidebar_box_on_a_negative_monitor() {
    let (position, size) =
        services::window::overlay_geometry(-1920, -100, 1920, 1080, ViewMode::Expanded, 1.0);

    // 工作区 1920×1080：侧栏为 2 + 360px 宽、135px 起 540px 高，
    // 窗口再向右、向上、向下各留 16px 阴影留白。
    // 窗口左边缘贴工作区左边缘，面板自身的 2px 左边距由 CSS padding 提供。
    assert_eq!(size.width, 378);
    assert_eq!(size.height, 572);
    assert_eq!(position.x, -1920);
    assert_eq!(position.y, 19);
}

#[test]
fn collapsed_overlay_shrinks_to_the_indicator_strip() {
    let (position, size) =
        services::window::overlay_geometry(-1920, -100, 1920, 1080, ViewMode::Collapsed, 1.0);

    // 收起态窗口必须真的缩到指示条大小，否则透明窗口仍会拦截桌面点击。
    assert_eq!(size.width, 19);
    assert_eq!(size.height, 112);
    assert_eq!(position.x, -1920);
    assert_eq!(position.y, 19);
}

#[test]
fn overlay_geometry_never_returns_zero_dimensions() {
    let (position, size) =
        services::window::overlay_geometry(10, 20, 0, 0, ViewMode::Expanded, 1.0);

    assert_eq!(position.x, 10);
    assert_eq!(position.y, 4);
    assert_eq!(size.width, 238);
    assert_eq!(size.height, 352);
}

#[test]
fn overlay_geometry_clamps_the_sidebar_on_very_large_work_areas() {
    let (_, size) = services::window::overlay_geometry(0, 0, 7680, 4320, ViewMode::Expanded, 1.0);

    assert_eq!(size.width, 2 + 480 + 16);
    assert_eq!(size.height, 900 + 32);
}

// 回归：曾经把设计稿的 CSS 像素直接当成物理像素用，在 150% 缩放的屏幕上
// 收起态窗口只有 19 个物理像素宽（≈ 12.7 CSS 像素），装不下 16px 的阴影留白，
// 内容盒被压成 0 —— 指示条既看不见也点不到。
#[test]
fn collapsed_overlay_keeps_the_strip_visible_at_any_display_scale() {
    for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
        let (_, size) =
            services::window::overlay_geometry(0, 0, 2560, 1528, ViewMode::Collapsed, scale);

        // 换算回 CSS 像素再减去 `.dc-window-collapsed` 的 16px 右留白与上下各 16px
        // 留白，剩下的才是指示条实际占据的内容盒。
        let content_width = size.width as f64 / scale - 16.0;
        let content_height = size.height as f64 / scale - 32.0;

        assert!(
            content_width >= 2.5,
            "缩放 {scale} 下指示条只剩 {content_width} CSS 像素宽，屏幕上将不可见"
        );
        assert!(
            (content_height - 80.0).abs() <= 1.0,
            "缩放 {scale} 下指示条高度为 {content_height} CSS 像素，应接近 80"
        );
    }
}

// 展开态同理：窗口换算回 CSS 像素后，面板必须等于设计稿的
// `clamp(场景宽 18.75%, 220, 480)` × `clamp(场景高 50%, 320, 900)`。
#[test]
fn expanded_overlay_keeps_the_sidebar_box_in_css_pixels_at_any_display_scale() {
    for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
        let (_, size) =
            services::window::overlay_geometry(0, 0, 2560, 1528, ViewMode::Expanded, scale);

        // 百分比按工作区比例算，与缩放比无关；夹取上下限是设计稿的 CSS 像素。
        let expected_panel_width = (2560.0 / scale * 0.1875).clamp(220.0, 480.0);
        let expected_panel_height = (1528.0 / scale * 0.5).clamp(320.0, 900.0);

        let content_width = size.width as f64 / scale - 2.0 - 16.0;
        let content_height = size.height as f64 / scale - 32.0;

        assert!(
            (content_width - expected_panel_width).abs() <= 1.5,
            "缩放 {scale} 下面板宽 {content_width}，期望 {expected_panel_width}"
        );
        assert!(
            (content_height - expected_panel_height).abs() <= 1.5,
            "缩放 {scale} 下面板高 {content_height}，期望 {expected_panel_height}"
        );
    }
}

// 用户拖出来的尺寸只对展开态生效。收起态窗口只有一条 3px 指示条，一旦被
// 拖大就会变成一块挡住桌面、又看不见内容的透明区域 —— 而透明窗口是会拦截
// 鼠标事件的，用户既找不到它也没法点它。
#[test]
fn collapsed_overlay_ignores_the_user_size_override() {
    let (_, design) =
        services::window::overlay_geometry(0, 0, 2560, 1528, ViewMode::Collapsed, 1.5);
    let override_size = CssSize {
        width: 400,
        height: 700,
    };

    let resolved =
        services::window::resolve_size(ViewMode::Collapsed, design, Some(override_size), 1.5);

    assert_eq!(
        resolved, design,
        "收起态必须永远回到设计稿推导出的指示条尺寸"
    );
}

// 展开态则相反：用户拖出来的尺寸比设计稿推导值更能表达他的意图。
#[test]
fn expanded_overlay_prefers_the_user_size_override() {
    let (_, design) = services::window::overlay_geometry(0, 0, 2560, 1528, ViewMode::Expanded, 1.5);
    // 覆盖值是设计稿 CSS 像素，落成窗口尺寸要乘当前缩放比。
    let override_size = CssSize {
        width: 440,
        height: 600,
    };

    let resolved =
        services::window::resolve_size(ViewMode::Expanded, design, Some(override_size), 1.5);

    assert_eq!(
        resolved,
        WindowSize {
            width: 660,
            height: 900,
        }
    );
    assert_ne!(
        resolved, design,
        "覆盖值应与设计稿推导值不同，否则这条用例证明不了覆盖生效"
    );
}

// 没有覆盖值时展开态保持设计稿推导结果，避免这条新分支改变原有行为。
#[test]
fn expanded_overlay_falls_back_to_the_design_size_without_an_override() {
    let (_, design) = services::window::overlay_geometry(0, 0, 2560, 1528, ViewMode::Expanded, 1.5);

    assert_eq!(
        services::window::resolve_size(ViewMode::Expanded, design, None, 1.5),
        design
    );
}

// 覆盖值要夹进与设计稿相同的可用范围：拖得太窄侧栏内容会挤成一团，拖得太宽
// 就失去了悬浮侧栏的意义。上下限是设计稿的 CSS 像素，因此随缩放比一起放大。
#[test]
fn user_size_override_is_clamped_to_the_design_range_at_any_display_scale() {
    for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
        let (_, design) =
            services::window::overlay_geometry(0, 0, 2560, 1528, ViewMode::Expanded, scale);
        let too_small = CssSize {
            width: 1,
            height: 1,
        };
        let too_large = CssSize {
            width: 99_999,
            height: 99_999,
        };

        let small =
            services::window::resolve_size(ViewMode::Expanded, design, Some(too_small), scale);
        let large =
            services::window::resolve_size(ViewMode::Expanded, design, Some(too_large), scale);

        assert_eq!(small.width as f64, (220.0 * scale).round());
        assert_eq!(small.height as f64, (320.0 * scale).round());
        assert_eq!(large.width as f64, (480.0 * scale).round());
        assert_eq!(large.height as f64, (900.0 * scale).round());
    }
}

// `restored_size_is_usable` 拿 `current_geometry` 的结果当「够用的下限」，
// 所以覆盖值无论怎么夹都不能小于设计稿下限：否则用户可以拖出一个比判据还
// 小的窗口，下次启动会被兜底逻辑判为坏值改掉，尺寸就记不住了。
#[test]
fn user_size_override_never_undercuts_the_design_minimum() {
    for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
        let (_, design) =
            services::window::overlay_geometry(0, 0, 2560, 1528, ViewMode::Expanded, scale);
        let degenerate = CssSize {
            width: 0,
            height: 0,
        };

        let resolved =
            services::window::resolve_size(ViewMode::Expanded, design, Some(degenerate), scale);

        assert!(
            resolved.width as f64 >= (220.0 * scale).round(),
            "缩放 {scale} 下覆盖值被夹到 {}，小于设计下限 220 CSS 像素",
            resolved.width as f64 / scale
        );
        assert!(
            resolved.height as f64 >= (320.0 * scale).round(),
            "缩放 {scale} 下覆盖值被夹到 {}，小于设计下限 320 CSS 像素",
            resolved.height as f64 / scale
        );
    }
}

// 用户尺寸覆盖必须按设计稿 CSS 像素落盘，而不是拖拽时那块屏幕的物理像素。
//
// 物理像素只在测量它的显示器上成立：150% 屏上拖出来的 657 物理像素，到 100%
// 屏上会被当成 657 CSS 像素解释，窗口一下子胀到原来的 1.5 倍，还会顶到宽度
// 上限。这条用例同时锁住「记录」和「应用」两个方向。
#[test]
fn user_size_override_is_recorded_in_design_pixels() {
    // 150% 屏上拖到 657×962 物理像素，用户实际想要的是设计稿上的 438×641。
    let dragged = WindowSize {
        width: 657,
        height: 962,
    };
    let stored = services::window::override_from_physical(dragged, 1.5);

    assert_eq!(
        stored,
        CssSize {
            width: 438,
            height: 641,
        }
    );
    assert_ne!(
        (stored.width, stored.height),
        (dragged.width, dragged.height),
        "覆盖值不能被原样按物理像素存下来，否则换屏后会被按新缩放比解释"
    );

    // 同一份覆盖值换到 100% 的屏幕上，仍然是用户当初想要的那个大小，
    // 而不是 657 CSS 像素。
    let (_, design) = services::window::overlay_geometry(0, 0, 1920, 1080, ViewMode::Expanded, 1.0);
    let resolved = services::window::resolve_size(ViewMode::Expanded, design, Some(stored), 1.0);

    assert_eq!(
        resolved,
        WindowSize {
            width: 438,
            height: 641,
        }
    );
}

// 记录与应用互为逆运算：同一份设计稿尺寸，无论在哪块缩放比的显示器上拖出来，
// 落盘的覆盖值都一样，再应用到别的缩放比上也仍然还原成那个设计稿尺寸。
#[test]
fn user_size_override_survives_a_display_scale_change() {
    let wanted = CssSize {
        width: 438,
        height: 641,
    };

    let stored: Vec<CssSize> = [1.0, 1.25, 1.5, 1.75, 2.0]
        .into_iter()
        .map(|scale| {
            let (_, design) =
                services::window::overlay_geometry(0, 0, 2560, 1528, ViewMode::Expanded, scale);
            // 用户在这块屏幕上拖出的物理尺寸，正是当前缩放比下的换算结果。
            let dragged =
                services::window::resolve_size(ViewMode::Expanded, design, Some(wanted), scale);

            services::window::override_from_physical(dragged, scale)
        })
        .collect();

    assert!(
        stored.iter().all(|size| *size == wanted),
        "不同缩放比下拖出的同一尺寸应当存成同一个设计稿尺寸，实际得到 {stored:?}"
    );
}

// 夹取在记录时就要做掉：落盘的覆盖值因此永远是合法值，手工改坏的配置还有
// `resolve_size` 兜第二道。上下限本身是设计稿 CSS 像素，所以夹取也在设计稿
// 空间完成，落盘值不随缩放比变化。
#[test]
fn oversized_drag_is_clamped_in_design_pixels() {
    for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
        let huge = WindowSize {
            width: 20_000,
            height: 20_000,
        };

        assert_eq!(
            services::window::override_from_physical(huge, scale),
            CssSize {
                width: 480,
                height: 900,
            },
            "缩放 {scale} 下的超大拖拽应当夹到设计稿上限"
        );
    }
}
