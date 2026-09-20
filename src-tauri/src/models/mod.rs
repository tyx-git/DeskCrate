pub mod catalog;
pub mod settings;
pub mod system;
pub use catalog::{
    AddAppRequest, AddCategoryRequest, AddGroupRequest, AppData, AppEXE, AppError, Category,
    DeleteRequest, Group, IconType, Item, RenameRequest,
};
pub use settings::{
    CssSize, ResizeWindowRequest, Settings, Theme, ViewMode, WindowPosition, WindowSettings,
    WindowSize,
};
pub(crate) use system::IconExtractionResult;
pub use system::{IconLocation, ParsedShortcut};
