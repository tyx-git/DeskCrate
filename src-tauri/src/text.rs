// 跨层共享的文本归一化工具。
//
// service 层和 system 层都会接收来自 JSON 或快捷方式的可选文本。两边
// 对空白字符串的处理必须一致，否则保存阶段认为“有参数”，启动阶段又把
// 它当成“没有参数”，最终会出现难以定位的行为差异。

/// 去掉首尾空白，并把空字符串归一化为 `None`。
pub(crate) fn optional_trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
