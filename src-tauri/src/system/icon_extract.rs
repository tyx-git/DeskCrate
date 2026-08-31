use std::{
    ffi::c_void,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr,
};

use image::RgbaImage;
use windows::{
    core::PCWSTR,
    Win32::{
        Graphics::Gdi::{
            CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BITMAPINFO,
            BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HBRUSH, HDC, HGDIOBJ,
        },
        Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
        UI::{
            Shell::{ExtractIconExW, SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON},
            WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL, HICON},
        },
    },
};

use crate::{
    error::AppResult,
    models::{AppError, IconExtractionResult},
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
        validate_app_id(app_id)?;

        let icon = extract_icon_from_file(source_path, icon_index)?;
        self.save_icon(app_id, icon)
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

        let icon = extract_associated_icon(source_path)?;
        self.save_icon(app_id, icon)
    }

    // 将 Windows 图标转换成 PNG，并返回后端缓存路径。
    fn save_icon(&self, app_id: &str, icon: OwnedIcon) -> AppResult<IconExtractionResult> {
        let (width, height, pixels) = render_icon(&icon)?;
        let image = RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| AppError::new("icon_image_invalid", "图标像素数据无法构造图像"))?;

        std::fs::create_dir_all(&self.icons_dir)?;

        let file_name = format!("{app_id}.png");
        let cache_path = self.icons_dir.join(&file_name);
        image.save(&cache_path).map_err(|error| {
            AppError::new(
                "icon_save_failed",
                format!("无法保存图标 {}: {error}", cache_path.display()),
            )
        })?;

        Ok(IconExtractionResult { cache_path })
    }
}

// 从文件的图标资源中提取一个 `HICON`。
fn extract_icon_from_file(source_path: &Path, icon_index: i32) -> AppResult<OwnedIcon> {
    let wide_path = to_wide(source_path);
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
    let wide_path = to_wide(source_path);
    let mut file_info = SHFILEINFOW::default();

    let result = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide_path.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut file_info as *mut SHFILEINFOW),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };

    if result == 0 || file_info.hIcon.is_invalid() {
        destroy_icon_if_valid(file_info.hIcon);
        return Err(AppError::new(
            "associated_icon_failed",
            format!("无法获取路径关联图标: {}", source_path.display()),
        ));
    }

    Ok(OwnedIcon {
        handle: file_info.hIcon,
    })
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
fn to_wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
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
