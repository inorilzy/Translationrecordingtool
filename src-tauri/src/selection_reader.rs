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

        // 控件支持 TextPattern 但选区为空：明确表示“没有选中任何文本”，
        // 返回空串交由调用方静默处理，而不是当作读取失败。
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

#[cfg(target_os = "macos")]
mod platform {
    use core_foundation::{
        base::{CFRelease, CFTypeRef, TCFType},
        boolean::CFBoolean,
        dictionary::CFDictionary,
        string::{CFString, CFStringRef},
    };
    use std::ffi::c_void;

    type AXUIElementRef = *const c_void;
    type AXError = i32;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXUIElementCreateSystemWide() -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
        fn AXIsProcessTrustedWithOptions(options: core_foundation::dictionary::CFDictionaryRef)
            -> bool;
    }

    const MAX_TEXT_ATTRIBUTE_ANCESTOR_DEPTH: usize = 12;

    /// Returns whether the process may use the Accessibility API. The first
    /// call also asks macOS to surface the app in Privacy & Security settings.
    fn ensure_accessibility_permission() -> Result<(), String> {
        let prompt_key = CFString::new("AXTrustedCheckOptionPrompt");
        let prompt_value = CFBoolean::true_value();
        let options = CFDictionary::from_CFType_pairs(&[(
            prompt_key.as_CFType(),
            prompt_value.as_CFType(),
        )]);

        let trusted = unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) };
        if trusted {
            Ok(())
        } else {
            Err("未获得辅助功能权限。请在“系统设置 → 隐私与安全性 → 辅助功能”中勾选 translation-tool 后重试".to_string())
        }
    }

    fn copy_attribute(element: AXUIElementRef, attribute: &str) -> Option<CFTypeRef> {
        let attribute = CFString::new(attribute);
        let mut value: CFTypeRef = std::ptr::null();
        let result = unsafe {
            AXUIElementCopyAttributeValue(element, attribute.as_concrete_TypeRef(), &mut value)
        };
        (result == 0 && !value.is_null()).then_some(value)
    }

    fn selected_text_of(element: AXUIElementRef) -> Option<String> {
        let value = copy_attribute(element, "AXSelectedText")?;
        // AXSelectedText is documented as CFString; wrap_under_create_rule
        // takes ownership of the +1 reference returned by CopyAttributeValue.
        let text = unsafe { CFString::wrap_under_create_rule(value as CFStringRef) }.to_string();
        Some(text)
    }

    pub fn read_selected_text() -> Result<String, String> {
        ensure_accessibility_permission()?;

        let system = unsafe { AXUIElementCreateSystemWide() };
        if system.is_null() {
            return Err("初始化 macOS 辅助功能接口失败".to_string());
        }

        let focused = copy_attribute(system, "AXFocusedUIElement");
        unsafe { CFRelease(system as CFTypeRef) };
        let Some(first_element) = focused else {
            return Err("未能获取当前焦点控件".to_string());
        };

        let mut element = first_element as AXUIElementRef;
        let mut owned = true;
        let result = (|| {
            for depth in 0..=MAX_TEXT_ATTRIBUTE_ANCESTOR_DEPTH {
                if let Some(text) = selected_text_of(element) {
                    return Ok(text.trim().to_string());
                }
                if depth == MAX_TEXT_ATTRIBUTE_ANCESTOR_DEPTH {
                    break;
                }
                match copy_attribute(element, "AXParent") {
                    Some(parent) => {
                        if owned {
                            unsafe { CFRelease(element as CFTypeRef) };
                        }
                        element = parent as AXUIElementRef;
                        owned = true;
                    }
                    None => break,
                }
            }
            // Reaching here means the focused element supports no selected
            // text (empty selection) rather than a hard failure.
            Ok(String::new())
        })();
        if owned {
            unsafe { CFRelease(element as CFTypeRef) };
        }
        result
    }
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
mod platform {
    pub fn read_selected_text() -> Result<String, String> {
        Err("当前平台暂不支持直接读取选中文本".to_string())
    }
}

pub fn read_selected_text() -> Result<String, String> {
    platform::read_selected_text()
}
