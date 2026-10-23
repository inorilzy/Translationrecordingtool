#[cfg(target_os = "windows")]
mod platform {
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
        UIA_TextPatternId,
    };

    /// Chromium browsers often report the focused element as a generic group
    /// or container while the selectable text lives on an ancestor Document.
    /// Walk a bounded ancestor chain so Chrome/Edge selections are not lost.
    const MAX_TEXT_PATTERN_ANCESTOR_DEPTH: usize = 12;

    fn find_text_pattern(
        automation: &IUIAutomation,
        focused: &IUIAutomationElement,
    ) -> Result<IUIAutomationTextPattern, String> {
        let walker = unsafe { automation.ControlViewWalker() }
            .map_err(|error| format!("创建 UI Automation 遍历器失败: {}", error))?;

        let mut element = focused.clone();
        for depth in 0..=MAX_TEXT_PATTERN_ANCESTOR_DEPTH {
            if let Ok(pattern) = unsafe {
                element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
            } {
                return Ok(pattern);
            }

            if depth == MAX_TEXT_PATTERN_ANCESTOR_DEPTH {
                break;
            }

            match unsafe { walker.GetParentElement(&element) } {
                Ok(parent) => element = parent,
                Err(_) => break,
            }
        }

        Err("当前控件及父级均不支持直接读取选中文本".to_string())
    }

    pub fn read_selected_text() -> Result<String, String> {
        let _com = ComApartment::initialize()?;

        let automation: IUIAutomation = unsafe {
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| format!("初始化 UI Automation 失败: {}", error))?
        };

        let focused = unsafe {
            automation
                .GetFocusedElement()
                .map_err(|error| format!("获取当前焦点控件失败: {}", error))?
        };

        let text_pattern = find_text_pattern(&automation, &focused)?;

        let selection = unsafe {
            text_pattern
                .GetSelection()
                .map_err(|error| format!("读取选区失败: {}", error))?
        };
        let selection_count = unsafe {
            selection
                .Length()
                .map_err(|error| format!("读取选区数量失败: {}", error))?
        };

        let mut selected_text = String::new();
        for index in 0..selection_count {
            let range = unsafe {
                selection
                    .GetElement(index)
                    .map_err(|error| format!("读取选区片段失败: {}", error))?
            };
            let text = unsafe {
                range
                    .GetText(-1)
                    .map_err(|error| format!("读取选区文本失败: {}", error))?
            };
            let text = text.to_string();
            if !text.trim().is_empty() {
                if !selected_text.is_empty() {
                    selected_text.push('\n');
                }
                selected_text.push_str(text.trim());
            }
        }

        if selected_text.trim().is_empty() {
            return Err("未读取到选中文本".to_string());
        }

        Ok(selected_text.trim().to_string())
    }

    struct ComApartment(bool);

    impl ComApartment {
        fn initialize() -> Result<Self, String> {
            let result =
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };

            if result.is_ok() {
                Ok(Self(true))
            } else if result.0 == windows::Win32::Foundation::RPC_E_CHANGED_MODE.0 {
                Ok(Self(false))
            } else {
                Err(format!("初始化 COM 失败: {}", result.message()))
            }
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    pub fn read_selected_text() -> Result<String, String> {
        Err("当前平台暂不支持直接读取选中文本".to_string())
    }
}

pub fn read_selected_text() -> Result<String, String> {
    platform::read_selected_text()
}
