use std::{env, fs, path::PathBuf};

fn main() {
    generate_window_spec();
    tauri_build::build()
}

fn generate_window_spec() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let spec_path = manifest_dir.join("../react/src/spec/window.json");
    println!("cargo:rerun-if-changed={}", spec_path.display());

    let raw = fs::read_to_string(&spec_path)
        .unwrap_or_else(|error| panic!("无法读取窗口规格 {}: {error}", spec_path.display()));
    let spec: serde_json::Value = serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("窗口规格 JSON 无效 {}: {error}", spec_path.display()));

    let integer_fields = [
        ("sidebarLeftPx", "SIDEBAR_LEFT_PX"),
        ("sidebarVerticalPaddingPx", "SIDEBAR_VERTICAL_PADDING_PX"),
        ("sidebarBorderRadiusPx", "SIDEBAR_BORDER_RADIUS_PX"),
        ("sidebarScrollbarWidthPx", "SIDEBAR_SCROLLBAR_WIDTH_PX"),
        ("sidebarBackdropBlurPx", "SIDEBAR_BACKDROP_BLUR_PX"),
        ("dividerHeightPx", "DIVIDER_HEIGHT_PX"),
        ("iconGridGapMinPx", "ICON_GRID_GAP_MIN_PX"),
        ("iconGridGapMaxPx", "ICON_GRID_GAP_MAX_PX"),
        ("iconItemWidthMinPx", "ICON_ITEM_WIDTH_MIN_PX"),
        ("iconItemWidthMaxPx", "ICON_ITEM_WIDTH_MAX_PX"),
        ("iconEmojiSizeMinPx", "ICON_EMOJI_SIZE_MIN_PX"),
        ("iconEmojiSizeMaxPx", "ICON_EMOJI_SIZE_MAX_PX"),
        ("collapsedStripLeftPx", "COLLAPSED_STRIP_LEFT_PX"),
        ("collapsedStripWidthPx", "COLLAPSED_STRIP_WIDTH_PX"),
        ("collapsedStripHeightPx", "COLLAPSED_STRIP_HEIGHT_PX"),
        ("overlayShadowGutterPx", "OVERLAY_SHADOW_GUTTER_PX"),
        ("overlayWidthMinPx", "OVERLAY_WIDTH_MIN_PX"),
        ("overlayWidthMaxPx", "OVERLAY_WIDTH_MAX_PX"),
        ("overlayHeightMinPx", "OVERLAY_HEIGHT_MIN_PX"),
        ("overlayHeightMaxPx", "OVERLAY_HEIGHT_MAX_PX"),
    ];
    let float_fields = [
        ("sidebarTopPercent", "SIDEBAR_TOP_PERCENT"),
        ("sidebarWidthPercent", "SIDEBAR_WIDTH_PERCENT"),
        ("sidebarHeightPercent", "SIDEBAR_HEIGHT_PERCENT"),
        ("collapsedStripTopPercent", "COLLAPSED_STRIP_TOP_PERCENT"),
        ("collapsedStripRadiusPx", "COLLAPSED_STRIP_RADIUS_PX"),
        ("searchMarginTopPercent", "SEARCH_MARGIN_TOP_PERCENT"),
        (
            "searchMarginHorizontalPercent",
            "SEARCH_MARGIN_HORIZONTAL_PERCENT",
        ),
        ("searchMarginBottomPercent", "SEARCH_MARGIN_BOTTOM_PERCENT"),
        (
            "iconGridGapViewportPercent",
            "ICON_GRID_GAP_VIEWPORT_PERCENT",
        ),
        (
            "iconItemWidthViewportPercent",
            "ICON_ITEM_WIDTH_VIEWPORT_PERCENT",
        ),
        (
            "iconEmojiSizeViewportPercent",
            "ICON_EMOJI_SIZE_VIEWPORT_PERCENT",
        ),
    ];

    let mut generated = String::from("// Generated from react/src/spec/window.json.\n");
    for (json_name, rust_name) in integer_fields {
        let value = spec[json_name]
            .as_u64()
            .unwrap_or_else(|| panic!("窗口规格字段 {json_name} 必须是非负整数"));
        generated.push_str(&format!("pub const {rust_name}: u32 = {value};\n"));
    }
    for (json_name, rust_name) in float_fields {
        let value = spec[json_name]
            .as_f64()
            .unwrap_or_else(|| panic!("窗口规格字段 {json_name} 必须是数字"));
        generated.push_str(&format!("pub const {rust_name}: f32 = {value}f32;\n"));
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out_dir.join("window_spec.rs"), generated).expect("无法写入生成的窗口规格 Rust 文件");
}
