use std::{
    ffi::c_void,
    path::{Path, PathBuf},
    ptr,
};

use image::{Rgba, RgbaImage};
use windows::{
    core::PCWSTR,
    Win32::{
        Graphics::Gdi::{
            CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BITMAPINFO,
            BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HBRUSH, HDC, HGDIOBJ,
        },
        Storage::FileSystem::{
            FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_FLAGS_AND_ATTRIBUTES,
        },
        UI::{
            Shell::{
                ExtractIconExW, SHGetFileInfoW, SHGetStockIconInfo, SHFILEINFOW, SHGFI_FLAGS,
                SHGFI_ICON, SHGFI_LARGEICON, SHGFI_USEFILEATTRIBUTES, SHGSI_ICON, SHGSI_LARGEICON,
                SHSTOCKICONINFO, SIID_DOCNOASSOC, SIID_FOLDER,
            },
            WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL, HICON},
        },
    },
};

use crate::{
    error::AppResult,
    models::{AppError, IconExtractionResult},
    system::wide,
};

// 写入图标缓存的统一尺寸。
//
// Windows 图标本身可能包含多个尺寸。先绘制到透明的 256×256 DIB，
// 可以让前端后续按 CSS 尺寸缩放，同时避免把 Windows 句柄暴露给上层。
const ICON_SIZE: u32 = 256;

// 图标提取器。
//
// 该类型只管理图标缓存目录和 Windows 图标 API，不负责决定应用节点
// 的业务字段。业务流程由 `services::icon::IconService` 负责。
pub(crate) struct IconExtractor {
    icons_dir: PathBuf,
}

impl IconExtractor {
    // 使用指定目录创建图标提取器。
    pub(crate) fn new(icons_dir: impl Into<PathBuf>) -> Self {
        Self {
            icons_dir: icons_dir.into(),
        }
    }

    // 从 exe、dll 或 ico 等显式图标来源中提取指定索引的图标。
    pub(crate) fn extract_file(
        &self,
        source_path: &Path,
        icon_index: i32,
        app_id: &str,
    ) -> AppResult<IconExtractionResult> {
        self.extract_and_save(app_id, || extract_icon_from_file(source_path, icon_index))
    }

    // 使用 Windows Shell 的文件关联图标提取策略。
    //
    // 该方法适用于普通文件、文件夹以及 Shell 能识别的系统路径。
    pub(crate) fn extract_associated(
        &self,
        source_path: &Path,
        app_id: &str,
    ) -> AppResult<IconExtractionResult> {
        validate_app_id(app_id)?;

        match extract_associated_icon(source_path) {
            Ok(icon) => self.save_icon(app_id, icon),
            Err(error) => {
                // 路径可能在 service 的初始 metadata 检查后被删除；
                // 此时不能把竞态造成的失败误判为“无文件关联”。
                if !source_path.exists() {
                    return Err(error);
                }

                // 无文件关联或无桌面 Shell 时仍允许保存节点，使用内置
                // 占位图标保证前端得到稳定的 PNG 资源。
                log::debug!(
                    "无法获取 {} 的 Shell 图标，使用占位图标: {}",
                    source_path.display(),
                    error
                );
                self.save_placeholder_icon(app_id, source_path.is_dir())
            }
        }
    }

    fn extract_and_save<F>(&self, app_id: &str, extract: F) -> AppResult<IconExtractionResult>
    where
        F: FnOnce() -> AppResult<OwnedIcon>,
    {
        validate_app_id(app_id)?;
        let icon = extract()?;
        self.save_icon(app_id, icon)
    }

    // 将 Windows 图标转换成 PNG，并返回后端缓存路径。
    fn save_icon(&self, app_id: &str, icon: OwnedIcon) -> AppResult<IconExtractionResult> {
        let (width, height, pixels) = render_icon(&icon)?;
        let image = RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| AppError::new("icon_image_invalid", "图标像素数据无法构造图像"))?;

        self.save_image(app_id, image)
    }

    // 直接保存一张已经准备好的图像。
    //
    // 网址图标走这条路：它不来自任何 Windows 图标句柄，而是下载下来解码好的
    // 位图。存盘方式与提取出来的图标完全一致，因此后续清理和孤儿对账都不用
    // 区分来源。
    pub(crate) fn save_image(
        &self,
        app_id: &str,
        image: RgbaImage,
    ) -> AppResult<IconExtractionResult> {
        validate_app_id(app_id)?;
        self.write_image(app_id, image)
    }

    fn save_placeholder_icon(
        &self,
        app_id: &str,
        is_directory: bool,
    ) -> AppResult<IconExtractionResult> {
        self.save_image(app_id, placeholder_image(is_directory))
    }

    fn write_image(&self, app_id: &str, image: RgbaImage) -> AppResult<IconExtractionResult> {
        std::fs::create_dir_all(&self.icons_dir)?;

        let cache_path = self.icon_path(app_id);
        image.save(&cache_path).map_err(|error| {
            AppError::new(
                "icon_save_failed",
                format!("无法保存图标 {}: {error}", cache_path.display()),
            )
        })?;

        Ok(IconExtractionResult { cache_path })
    }

    // 图标缓存文件的约定路径。
    //
    // 文件名固定为 `{app_id}.png`（与 `write_image` 一致）。清理和孤儿对账都靠这个
    // 约定按 ID 找文件，因此不能改成信任 JSON 里可能被改过的路径。
    fn icon_path(&self, app_id: &str) -> PathBuf {
        self.icon_path_with_extension(app_id, "png")
    }

    fn icon_path_with_extension(&self, app_id: &str, extension: &str) -> PathBuf {
        self.icons_dir.join(format!("{app_id}.{extension}"))
    }

    // 保存 SVG 原文。
    //
    // `image` crate 不支持 SVG，但 WebView 的 `<img>` 原生能渲染，而且矢量在
    // 44px 的格子里比位图还清楚。所以 SVG 不解码，直接把字节按 `.svg` 落盘。
    pub(crate) fn save_svg(&self, app_id: &str, bytes: &[u8]) -> AppResult<IconExtractionResult> {
        validate_app_id(app_id)?;
        std::fs::create_dir_all(&self.icons_dir)?;

        let cache_path = self.icon_path_with_extension(app_id, "svg");
        std::fs::write(&cache_path, bytes).map_err(|error| {
            AppError::new(
                "icon_save_failed",
                format!("无法保存图标 {}: {error}", cache_path.display()),
            )
        })?;

        Ok(IconExtractionResult { cache_path })
    }

    // 找到 `app_id` 已有的图标缓存，返回路径和扩展名。
    //
    // 网址图标的产物可能是 png（位图解出来了）也可能是 svg（原文落盘），所以两个
    // 扩展名都要认。png 优先：历史数据全是 png，先找它少一次系统调用。
    fn find_icon_cache(&self, app_id: &str) -> Option<(PathBuf, &'static str)> {
        for extension in ["png", "svg"] {
            let path = self.icon_path_with_extension(app_id, extension);
            if path.is_file() {
                return Some((path, extension));
            }
        }

        None
    }

    // 把另一个应用的图标缓存复制成 `app_id` 的一份。
    //
    // 同源站点第二次添加时走这条路：不联网就拿到图标，而且同一站点在侧栏里长得
    // 必然一样（各抓各的会因为重试时机不同而拿到不同尺寸的图）。
    //
    // 是**复制**而不是共用同一份文件：每个应用各自持有自己的一份，删掉其中一个
    // 不会让另一个变成裂图，`remove_app_icon` / `sweep_orphan_icons` 那套按 ID
    // 找文件的约定也不用改。
    pub(crate) fn copy_icon(
        &self,
        source_app_id: &str,
        app_id: &str,
    ) -> AppResult<IconExtractionResult> {
        validate_app_id(source_app_id)?;
        validate_app_id(app_id)?;

        let (source, extension) = self.find_icon_cache(source_app_id).ok_or_else(|| {
            AppError::new(
                "icon_cache_missing",
                format!(
                    "{} 没有可复用的图标缓存",
                    self.icons_dir.join(source_app_id).display()
                ),
            )
        })?;

        std::fs::create_dir_all(&self.icons_dir)?;
        // 扩展名跟着来源一起带过去：源是 svg，复制过来也必须是 svg，否则前端
        // 拿到的 `icon_url` 会指向一个内容与扩展名不符的文件。
        let target = self.icon_path_with_extension(app_id, extension);
        std::fs::copy(&source, &target).map_err(|error| {
            AppError::new(
                "icon_copy_failed",
                format!("无法复用图标到 {}: {error}", target.display()),
            )
        })?;

        Ok(IconExtractionResult { cache_path: target })
    }
}

// 从文件的图标资源中提取一个 `HICON`。
fn extract_icon_from_file(source_path: &Path, icon_index: i32) -> AppResult<OwnedIcon> {
    let wide_path = to_wide(source_path)?;
    let mut large_icon = HICON::default();
    let mut small_icon = HICON::default();

    let count = unsafe {
        ExtractIconExW(
            PCWSTR(wide_path.as_ptr()),
            icon_index,
            Some(&mut large_icon as *mut HICON),
            Some(&mut small_icon as *mut HICON),
            1,
        )
    };

    // `ExtractIconExW` 同时可能创建大小图标；本服务只保存大图标。
    destroy_icon_if_valid(small_icon);

    if count == 0 || large_icon.is_invalid() {
        destroy_icon_if_valid(large_icon);
        return Err(AppError::new(
            "icon_extract_failed",
            format!("无法从文件提取图标: {}", source_path.display()),
        ));
    }

    Ok(OwnedIcon { handle: large_icon })
}

// 获取 Windows Shell 为指定路径选择的关联图标。
fn extract_associated_icon(source_path: &Path) -> AppResult<OwnedIcon> {
    let wide_path = to_wide(source_path)?;
    let flags = SHGFI_ICON | SHGFI_LARGEICON;

    // 先请求真实路径，以保留文件自身或目录的自定义图标。
    if let Some(icon) = query_shell_icon(&wide_path, FILE_FLAGS_AND_ATTRIBUTES(0), flags) {
        return Ok(icon);
    }

    // 临时文件、尚未注册关联的扩展名或无 UI Shell 环境可能无法通过
    // 真实路径返回图标；使用文件属性请求系统默认图标作为稳定回退。
    let attributes = if source_path.is_dir() {
        FILE_ATTRIBUTE_DIRECTORY
    } else {
        FILE_ATTRIBUTE_NORMAL
    };
    if let Some(icon) = query_shell_icon(&wide_path, attributes, flags | SHGFI_USEFILEATTRIBUTES) {
        return Ok(icon);
    }

    // 没有扩展名关联时使用 Windows 的通用 stock icon，保证普通文件
    // 仍然能够被保存为可用的缓存资源。
    extract_stock_icon(source_path.is_dir()).map_err(|error| {
        AppError::new(
            "associated_icon_failed",
            format!("无法获取路径关联图标: {} ({error})", source_path.display()),
        )
    })
}

fn query_shell_icon(
    wide_path: &[u16],
    attributes: FILE_FLAGS_AND_ATTRIBUTES,
    flags: SHGFI_FLAGS,
) -> Option<OwnedIcon> {
    let mut file_info = SHFILEINFOW::default();
    let result = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide_path.as_ptr()),
            attributes,
            Some(&mut file_info as *mut SHFILEINFOW),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        )
    };

    if result == 0 || file_info.hIcon.is_invalid() {
        destroy_icon_if_valid(file_info.hIcon);
        return None;
    }

    Some(OwnedIcon {
        handle: file_info.hIcon,
    })
}

fn extract_stock_icon(is_directory: bool) -> AppResult<OwnedIcon> {
    let icon_id = if is_directory {
        SIID_FOLDER
    } else {
        SIID_DOCNOASSOC
    };
    let mut stock_info = SHSTOCKICONINFO {
        cbSize: std::mem::size_of::<SHSTOCKICONINFO>() as u32,
        ..SHSTOCKICONINFO::default()
    };

    let result =
        unsafe { SHGetStockIconInfo(icon_id, SHGSI_ICON | SHGSI_LARGEICON, &mut stock_info) };
    if let Err(error) = result {
        destroy_icon_if_valid(stock_info.hIcon);
        return Err(windows_error("stock_icon_failed", error));
    }

    if stock_info.hIcon.is_invalid() {
        return Err(AppError::new(
            "stock_icon_empty",
            "Windows 没有返回有效的默认图标",
        ));
    }

    Ok(OwnedIcon {
        handle: stock_info.hIcon,
    })
}

// Shell 不可用时使用的轻量占位图标。它只作为最后回退，不影响真实图标提取。
fn placeholder_image(is_directory: bool) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(ICON_SIZE, ICON_SIZE, Rgba([0, 0, 0, 0]));
    let color = if is_directory {
        Rgba([236, 184, 72, 255])
    } else {
        Rgba([202, 211, 224, 255])
    };

    fill_rect(&mut image, 40, 52, 216, 208, color);
    if is_directory {
        fill_rect(&mut image, 40, 40, 126, 72, color);
    } else {
        let line_color = Rgba([112, 126, 146, 255]);
        for (top, right) in [(92, 192), (124, 176), (156, 160)] {
            fill_rect(&mut image, 64, top, right, top + 12, line_color);
        }
    }

    image
}

fn fill_rect(image: &mut RgbaImage, left: u32, top: u32, right: u32, bottom: u32, color: Rgba<u8>) {
    for y in top..bottom {
        for x in left..right {
            image.put_pixel(x, y, color);
        }
    }
}

// 将 `HICON` 绘制到透明 DIB，并转换为 `image` 使用的 RGBA 顺序。
fn render_icon(icon: &OwnedIcon) -> AppResult<(u32, u32, Vec<u8>)> {
    let surface = DibSurface::new(ICON_SIZE, ICON_SIZE)?;

    unsafe {
        DrawIconEx(
            surface.dc,
            0,
            0,
            icon.handle,
            ICON_SIZE as i32,
            ICON_SIZE as i32,
            0,
            HBRUSH::default(),
            DI_NORMAL,
        )
        .map_err(|error| windows_error("icon_draw_failed", error))?;
    }

    let mut pixels = surface.pixels().to_vec();

    // GDI DIB 的 32 位像素顺序是 BGRA，而 PNG/RgbaImage 需要 RGBA。
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    Ok((ICON_SIZE, ICON_SIZE, pixels))
}

// 32 位、自上而下的内存 DIB。
//
// `Drop` 中先恢复原选入 DC 的对象，再删除位图和 DC，保证任何错误路径
// 都不会把 GDI 句柄泄漏出去。
struct DibSurface {
    dc: HDC,
    bitmap: HBITMAP,
    previous_object: HGDIOBJ,
    bits: *mut c_void,
    byte_len: usize,
}

impl DibSurface {
    fn new(width: u32, height: u32) -> AppResult<Self> {
        let dc = unsafe { CreateCompatibleDC(HDC::default()) };
        if dc.is_invalid() {
            return Err(AppError::new(
                "icon_dc_create_failed",
                "无法创建图标内存设备上下文",
            ));
        }

        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // 负数表示自上而下的 DIB。
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..BITMAPINFOHEADER::default()
            },
            ..BITMAPINFO::default()
        };

        let mut bits: *mut c_void = ptr::null_mut();
        let bitmap = match unsafe {
            CreateDIBSection(
                dc,
                &bitmap_info as *const BITMAPINFO,
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            )
        } {
            Ok(bitmap) => bitmap,
            Err(error) => {
                unsafe {
                    let _ = DeleteDC(dc);
                }
                return Err(windows_error("icon_dib_create_failed", error));
            }
        };

        if bits.is_null() {
            unsafe {
                let _ = DeleteObject(bitmap);
                let _ = DeleteDC(dc);
            }
            return Err(AppError::new(
                "icon_dib_bits_failed",
                "创建图标 DIB 后没有得到像素缓冲区",
            ));
        }

        let previous_object = unsafe { SelectObject(dc, bitmap) };
        if previous_object.is_invalid() {
            unsafe {
                let _ = DeleteObject(bitmap);
                let _ = DeleteDC(dc);
            }
            return Err(AppError::new(
                "icon_bitmap_select_failed",
                "无法将图标位图选入内存设备上下文",
            ));
        }

        let byte_len = width as usize * height as usize * 4;
        unsafe {
            ptr::write_bytes(bits.cast::<u8>(), 0, byte_len);
        }

        Ok(Self {
            dc,
            bitmap,
            previous_object,
            bits,
            byte_len,
        })
    }

    fn pixels(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.bits.cast::<u8>(), self.byte_len) }
    }
}

impl Drop for DibSurface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous_object);
            let _ = DeleteObject(self.bitmap);
            let _ = DeleteDC(self.dc);
        }
    }
}

// 图标句柄的 RAII 包装，确保每一条返回路径都会调用 `DestroyIcon`。
struct OwnedIcon {
    handle: HICON,
}

impl Drop for OwnedIcon {
    fn drop(&mut self) {
        destroy_icon_if_valid(self.handle);
    }
}

fn destroy_icon_if_valid(icon: HICON) {
    if !icon.is_invalid() {
        unsafe {
            let _ = DestroyIcon(icon);
        }
    }
}

// 将路径转换成以 NUL 结尾的 UTF-16 字符串。
fn to_wide(path: &Path) -> AppResult<Vec<u16>> {
    wide::nul_terminated(path.as_os_str(), "icon_invalid_path", "图标来源路径")
}

// 应用 ID 会参与构造文件名，只允许使用安全的文件名字符。
fn validate_app_id(app_id: &str) -> AppResult<()> {
    if app_id.is_empty()
        || !app_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(AppError::new(
            "invalid_app_id",
            "应用 ID 只能包含字母、数字、短横线和下划线",
        ));
    }

    Ok(())
}

// 将 Windows API 错误统一转换成项目错误模型。
fn windows_error(code: &'static str, error: windows::core::Error) -> AppError {
    AppError::new(code, format!("{error:?}"))
}
