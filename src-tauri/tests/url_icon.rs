// 网址应用的图标抓取与顶层落位（真实网络）。
//
// 默认**不跑**：它会真的去访问外网，在离线或受限网络下失败是正常现象，
// 让它参与质量门禁只会把门禁变得不稳定。想确认这条链路时手动执行：
//
//     cargo test --test url_icon -- --ignored --nocapture
//
// 脚本会打出每个站点的实际结果（图标 URL、是否落盘、字节数），人工核对
// 比断言一个固定的字节数更有意义。
mod support;

use std::fs;

use lib::models::{AddAppRequest, IconType};
use lib::services::catalog;

use support::TestDir;

#[test]
#[ignore = "会访问外网，默认跳过"]
fn url_apps_cache_a_real_site_icon_and_land_at_the_top_level() {
    let directory = TestDir::new("favicon-probe");
    let state = directory.state();

    for url in [
        "https://www.baidu.com",
        "https://github.com",
        "https://www.rust-lang.org",
    ] {
        let app = catalog::add_app(
            &state,
            AddAppRequest {
                parent_id: None,
                source_path: url.to_owned(),
                name: None,
                root_index: None,
            },
        )
        .expect("url app should be added");

        assert_eq!(app.icon_type, IconType::Url);
        let icon_path = directory.child(&format!("icons/{}.png", app.id));
        let exists = icon_path.is_file();
        let size = fs::metadata(&icon_path).map(|m| m.len()).unwrap_or(0);
        println!(
            "{url} -> icon_url={} cached={exists} bytes={size}",
            app.icon_url
        );
    }

    let data = catalog::get_all_data(&state).unwrap();
    assert_eq!(data.apps.len(), 3, "三个网址都应挂在顶层");
    assert!(data.categories.is_empty());
}
