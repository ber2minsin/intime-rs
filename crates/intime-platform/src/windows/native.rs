use crate::error::PlatformError;

use std::{ffi::c_void, os::windows::ffi::OsStrExt, path::Path, ptr};

use intime_core::{
    models::{AppDetails, Event, EventData, SignatureInfo, VersionInfo},
    time::Timestamp,
};
use tracing::debug;
use uiautomation::UIAutomation;
use windows::Win32::{
    Foundation::{CloseHandle, HWND},
    Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute},
    Security::Cryptography::{
        CERT_CONTEXT, CERT_FIND_SUBJECT_CERT, CERT_INFO, CERT_NAME_ATTR_TYPE,
        CERT_NAME_ISSUER_FLAG, CERT_NAME_SIMPLE_DISPLAY_TYPE,
        CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED, CERT_QUERY_CONTENT_TYPE,
        CERT_QUERY_ENCODING_TYPE, CERT_QUERY_FORMAT_FLAG_BINARY, CERT_QUERY_FORMAT_TYPE,
        CERT_QUERY_OBJECT_FILE, CMSG_SIGNER_INFO, CMSG_SIGNER_INFO_PARAM, CertCloseStore,
        CertFindCertificateInStore, CertFreeCertificateContext, CertGetNameStringW, CryptMsgClose,
        CryptMsgGetParam, CryptQueryObject, HCERTSTORE, PKCS_7_ASN_ENCODING, X509_ASN_ENCODING,
    },
    Storage::{
        EnhancedStorage::PKEY_AppUserModel_ID,
        FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW},
    },
    System::{
        Com::CoUninitialize,
        Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
    },
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
        Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow},
        WindowsAndMessaging::{
            DispatchMessageW, EVENT_OBJECT_NAMECHANGE, EVENT_SYSTEM_FOREGROUND, GA_ROOTOWNER,
            GWL_EXSTYLE, GetAncestor, GetForegroundWindow, GetMessageW, GetWindowLongW,
            GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, MSG, OBJID_WINDOW,
            TranslateMessage, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WS_EX_TOOLWINDOW,
        },
    },
};
use windows::core::PCWSTR;
use windows::core::PWSTR;

use crate::windows::event_source::push_event;

pub unsafe extern "system" fn win_event_proc(
    _h_win_event_hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    if id_object != OBJID_WINDOW.0 || hwnd.0.is_null() {
        return;
    }

    unsafe {
        let details = match get_details(hwnd) {
            Ok(details) => details,
            Err(_) => {
                eprintln!("Could not retrieve details of the app");
                return;
            }
        };

        let fingerprint = details.fingerprint();
        let is_bg = is_background_window(hwnd);

        let timestamp = Timestamp::now();
        if is_bg {
            let event_data = EventData::Background { fingerprint };
            push_event(Event {
                timestamp,
                data: event_data,
            });
        } else {
            let mut event_data = None;
            match event {
                EVENT_SYSTEM_FOREGROUND => {
                    event_data = Some(EventData::WindowFocus {
                        fingerprint,
                        window_handle: hwnd.0 as u64,
                    });
                }
                EVENT_OBJECT_NAMECHANGE => {
                    if hwnd == GetForegroundWindow() {
                        event_data = Some(EventData::TitleChange {
                            new_title: details.title.clone(),
                            fingerprint,
                            window_handle: hwnd.0 as u64
                        });
                    }
                }
                _ => {tracing::info!("Ignored system hook event: {event}");}
            };
            if let Some(data) = event_data {
                push_event(Event {
                    timestamp,
                    data,
                });
            }

            // TODO maybe add a cache so we do not trigger this every time
            // also we can add this to outside of the if else, to log even the
            // system related events
            push_event(Event {
                timestamp: Timestamp::now(),
                data: EventData::AppSeen {
                    fingerprint,
                    details,
                    window_handle: hwnd.0 as u64
                },
            });
        }
    }
}

unsafe fn is_background_window(hwnd: HWND) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            debug!("IsWindowVisible is false, classified as background event");
            return true;
        }

        let mut cloaked: u32 = 0;
        let dwm_status = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        );

        if dwm_status.is_ok() && cloaked != 0 {
            debug!("Invisible rendering (DWMWA_CLOAKED classified as background event");
            return true;
        }

        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        if (ex_style as u32 & WS_EX_TOOLWINDOW.0) != 0 {
            debug!(
                "Overlay or peripheral element (GetWindowLongW check) classified as background event"
            );
            return true;
        }

        let root_owner = GetAncestor(hwnd, GA_ROOTOWNER);
        if root_owner != hwnd {
            debug!("Inner layout triggered (GetAncestor) classified as background event");
            return true;
        }

        // All tests passed, might be more but for now it is enough
        false
    }
}

pub unsafe fn get_details(hwnd: HWND) -> Result<AppDetails, PlatformError> {
    unsafe {
        let title = get_title(hwnd);
        let aumid = get_aumid(hwnd)?;
        let file_name = get_file_name(hwnd)?;
        let file_path = Path::new(&file_name);

        let version = VersionResource::open(file_path)?;
        let auth = AuthenticodeContext::open(file_path)?;

        Ok(AppDetails {
            title,
            aumid,
            file_path: file_name,
            product_name: version.as_ref().and_then(|v| v.product_name()),
            company_name: version.as_ref().and_then(|v| v.company_name()),
            version_info: version.as_ref().map(|v| v.version_info()),
            signature_info: auth.as_ref().map(|a| a.signature_info()),
        })
    }
}

unsafe fn get_file_name(hwnd: HWND) -> Result<String, PlatformError> {
    // GetWindowModuleFileNameW returns "" for cross-process windows, which is
    // every window we see from a WINEVENT_OUTOFCONTEXT hook. Resolve via PID.
    unsafe {
        let mut pid: u32 = 0;
        let tid = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if tid == 0 || pid == 0 {
            return Ok(String::new());
        }

        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)?;

        let mut buffer = vec![0u16; 1024];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        result?;

        Ok(String::from_utf16_lossy(&buffer[..size as usize]))
    }
}

fn wide_z_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Opened VS_VERSIONINFO resource. Each accessor is one `VerQueryValueW`
/// call against the in-memory buffer.
struct VersionResource {
    buf: Vec<u8>,
    field_prefix: String,
}

impl VersionResource {
    /// `Ok(None)` if the binary has no VERSIONINFO or no translation table.
    unsafe fn open(exe_path: &Path) -> Result<Option<Self>, PlatformError> {
        let wide_path = wide_z_path(exe_path);
        unsafe {
            let path = PCWSTR(wide_path.as_ptr());
            let size = GetFileVersionInfoSizeW(path, None);
            if size == 0 {
                return Ok(None);
            }
            let mut buf = vec![0u8; size as usize];
            GetFileVersionInfoW(path, Some(0), size, buf.as_mut_ptr() as *mut c_void)?;

            let Some((lang, codepage)) = first_translation(&buf) else {
                return Ok(None);
            };
            Ok(Some(Self {
                buf,
                field_prefix: format!("\\StringFileInfo\\{:04x}{:04x}\\", lang, codepage),
            }))
        }
    }

    fn product_name(&self) -> Option<String> {
        unsafe { self.read("ProductName") }
    }
    fn company_name(&self) -> Option<String> {
        unsafe { self.read("CompanyName") }
    }
    fn file_description(&self) -> Option<String> {
        unsafe { self.read("FileDescription") }
    }
    fn product_version(&self) -> Option<String> {
        unsafe { self.read("ProductVersion") }
    }
    fn file_version(&self) -> Option<String> {
        unsafe { self.read("FileVersion") }
    }
    fn original_filename(&self) -> Option<String> {
        unsafe { self.read("OriginalFilename") }
    }
    fn internal_name(&self) -> Option<String> {
        unsafe { self.read("InternalName") }
    }
    fn legal_copyright(&self) -> Option<String> {
        unsafe { self.read("LegalCopyright") }
    }

    fn version_info(&self) -> VersionInfo {
        VersionInfo {
            file_description: self.file_description(),
            product_version: self.product_version(),
            file_version: self.file_version(),
            original_filename: self.original_filename(),
            internal_name: self.internal_name(),
            legal_copyright: self.legal_copyright(),
        }
    }

    unsafe fn read(&self, field: &str) -> Option<String> {
        let q = wide_z(&format!("{}{}", self.field_prefix, field));
        let mut value_ptr: *mut c_void = ptr::null_mut();
        let mut value_len: u32 = 0;
        unsafe {
            let ok = VerQueryValueW(
                self.buf.as_ptr() as *const _,
                PCWSTR(q.as_ptr()),
                &mut value_ptr,
                &mut value_len,
            );
            if !ok.as_bool() || value_ptr.is_null() || value_len == 0 {
                return None;
            }
            let chars = std::slice::from_raw_parts(value_ptr as *const u16, value_len as usize);
            let trimmed = chars.split(|&c| c == 0).next().unwrap_or(chars);
            let s = String::from_utf16_lossy(trimmed);
            (!s.is_empty()).then_some(s)
        }
    }
}

/// First `(lang, codepage)` entry of `\VarFileInfo\Translation`.
unsafe fn first_translation(buf: &[u8]) -> Option<(u16, u16)> {
    let q = wide_z("\\VarFileInfo\\Translation");
    let mut ptr: *mut c_void = ptr::null_mut();
    let mut len: u32 = 0;
    unsafe {
        let ok = VerQueryValueW(
            buf.as_ptr() as *const _,
            PCWSTR(q.as_ptr()),
            &mut ptr,
            &mut len,
        );
        if !ok.as_bool() || ptr.is_null() || len < 4 {
            return None;
        }
        let p = ptr as *const u16;
        Some((*p, *p.add(1)))
    }
}

/// szOID_COMMON_NAME, used as `pvTypePara` for `CertGetNameStringW`.
const SZOID_COMMON_NAME: &[u8] = b"2.5.4.3\0";

/// Authenticode envelope. Owns cert store, message, signer info buffer,
/// and signer cert; cleanup in `Drop`.
struct AuthenticodeContext {
    store: HCERTSTORE,
    msg: *mut c_void,
    signer_buf: Vec<u8>,
    cert: *const CERT_CONTEXT,
}

impl AuthenticodeContext {
    /// `Ok(None)` for unsigned binaries or unparseable signatures.
    unsafe fn open(exe_path: &Path) -> Result<Option<Self>, PlatformError> {
        let wide_path = wide_z_path(exe_path);
        unsafe {
            let mut encoding = CERT_QUERY_ENCODING_TYPE(0);
            let mut content_type = CERT_QUERY_CONTENT_TYPE(0);
            let mut format_type = CERT_QUERY_FORMAT_TYPE(0);
            let mut store = HCERTSTORE(ptr::null_mut());
            let mut msg: *mut c_void = ptr::null_mut();

            if CryptQueryObject(
                CERT_QUERY_OBJECT_FILE,
                wide_path.as_ptr() as *const _,
                CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED,
                CERT_QUERY_FORMAT_FLAG_BINARY,
                0,
                Some(&mut encoding),
                Some(&mut content_type),
                Some(&mut format_type),
                Some(&mut store),
                Some(&mut msg),
                None,
            )
            .is_err()
            {
                return Ok(None);
            }

            // Take ownership now so any error path frees `store` and `msg`.
            let mut ctx = Self {
                store,
                msg,
                signer_buf: Vec::new(),
                cert: ptr::null(),
            };

            let Some(buf) = read_signer_info(msg)? else {
                return Ok(None);
            };
            ctx.signer_buf = buf;

            let signer = ctx.signer_buf.as_ptr() as *const CMSG_SIGNER_INFO;
            let cert = find_signer_cert(store, signer);
            if cert.is_null() {
                return Ok(None);
            }
            ctx.cert = cert;

            Ok(Some(ctx))
        }
    }

    fn publisher(&self) -> Option<String> {
        unsafe { read_cert_name(self.cert, CERT_NAME_ATTR_TYPE, 0, Some(SZOID_COMMON_NAME)) }
    }

    fn subject_full(&self) -> Option<String> {
        unsafe { read_cert_name(self.cert, CERT_NAME_SIMPLE_DISPLAY_TYPE, 0, None) }
    }

    fn issuer(&self) -> Option<String> {
        unsafe {
            read_cert_name(
                self.cert,
                CERT_NAME_ATTR_TYPE,
                CERT_NAME_ISSUER_FLAG,
                Some(SZOID_COMMON_NAME),
            )
        }
    }

    /// Big-endian hex of the signer cert's serial number.
    fn serial_number(&self) -> Option<String> {
        unsafe {
            let signer = self.signer_buf.as_ptr() as *const CMSG_SIGNER_INFO;
            let sn = (*signer).SerialNumber;
            if sn.cbData == 0 || sn.pbData.is_null() {
                return None;
            }
            let bytes = std::slice::from_raw_parts(sn.pbData, sn.cbData as usize);
            Some(bytes.iter().rev().map(|b| format!("{:02x}", b)).collect())
        }
    }

    fn signature_info(&self) -> SignatureInfo {
        SignatureInfo {
            publisher: self.publisher(),
            subject_full: self.subject_full(),
            issuer: self.issuer(),
            serial_number: self.serial_number(),
        }
    }
}

impl Drop for AuthenticodeContext {
    fn drop(&mut self) {
        unsafe {
            if !self.cert.is_null() {
                let _ = CertFreeCertificateContext(Some(self.cert));
            }
            if !self.msg.is_null() {
                let _ = CryptMsgClose(Some(self.msg as *const _));
            }
            let _ = CertCloseStore(Some(self.store), 0);
        }
    }
}

unsafe fn read_signer_info(msg: *mut c_void) -> Result<Option<Vec<u8>>, PlatformError> {
    unsafe {
        let mut size: u32 = 0;
        CryptMsgGetParam(msg as *const _, CMSG_SIGNER_INFO_PARAM, 0, None, &mut size)?;
        if size == 0 {
            return Ok(None);
        }
        let mut buf = vec![0u8; size as usize];
        CryptMsgGetParam(
            msg as *const _,
            CMSG_SIGNER_INFO_PARAM,
            0,
            Some(buf.as_mut_ptr() as *mut c_void),
            &mut size,
        )?;
        Ok(Some(buf))
    }
}

unsafe fn find_signer_cert(
    store: HCERTSTORE,
    signer: *const CMSG_SIGNER_INFO,
) -> *const CERT_CONTEXT {
    unsafe {
        let mut info: CERT_INFO = std::mem::zeroed();
        info.Issuer = (*signer).Issuer;
        info.SerialNumber = (*signer).SerialNumber;
        CertFindCertificateInStore(
            store,
            CERT_QUERY_ENCODING_TYPE(X509_ASN_ENCODING.0 | PKCS_7_ASN_ENCODING.0),
            0,
            CERT_FIND_SUBJECT_CERT,
            Some(&info as *const _ as *const _),
            None,
        )
    }
}

unsafe fn read_cert_name(
    cert: *const CERT_CONTEXT,
    dw_type: u32,
    dw_flags: u32,
    oid: Option<&[u8]>,
) -> Option<String> {
    unsafe {
        let oid_ptr: *const c_void = oid.map_or(ptr::null(), |o| o.as_ptr() as *const _);
        let len = CertGetNameStringW(cert, dw_type, dw_flags, Some(oid_ptr), None);
        if len <= 1 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        let _ = CertGetNameStringW(cert, dw_type, dw_flags, Some(oid_ptr), Some(&mut buf));
        let trimmed: Vec<u16> = buf.into_iter().take_while(|&c| c != 0).collect();
        let s = String::from_utf16_lossy(&trimmed);
        (!s.is_empty()).then_some(s)
    }
}

unsafe fn get_aumid(hwnd: HWND) -> Result<Option<String>, PlatformError> {
    unsafe {
        let prop_store: IPropertyStore = SHGetPropertyStoreForWindow(hwnd)?;

        let variant = prop_store.GetValue(&PKEY_AppUserModel_ID)?;
        let p_str = variant.Anonymous.Anonymous.Anonymous.pwszVal;
        if p_str.is_null() {
            return Ok(None);
        }

        let aumid = p_str.to_string();
        if aumid.is_ok() {
            return Ok(Some(aumid.unwrap()));
        } else {
            return Ok(None);
        }
    }
}

unsafe fn get_title(hwnd: HWND) -> String {
    let mut buffer = vec![0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    let title = String::from_utf16_lossy(&buffer[..len as usize]);
    title
}

fn dump_focused_element() {
    let automation = match UIAutomation::new() {
        Ok(a) => a,
        Err(_) => return,
    };

    let element = match automation.get_focused_element() {
        Ok(e) => e,
        Err(_) => return,
    };

    let name = element.get_name().unwrap_or_default();

    let class_name = element.get_classname().unwrap_or_default();

    let automation_id = element.get_automation_id().unwrap_or_default();

    let control_type = element
        .get_control_type()
        .map(|c| format!("{:?}", c))
        .unwrap_or_default();

    println!(
        "Focused element: {} | {} | {} | {}",
        name,
        control_type,
        class_name,
        automation_id
    );
}

pub unsafe fn install_hooks() {
    let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
    unsafe {
        let hook_fg = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(win_event_proc),
            0,
            0,
            flags,
        );

        let hook_name = SetWinEventHook(
            EVENT_OBJECT_NAMECHANGE,
            EVENT_OBJECT_NAMECHANGE,
            None,
            Some(win_event_proc),
            0,
            0,
            flags,
        );

        if hook_fg.0.is_null() || hook_name.0.is_null() {
            eprintln!("SetWinEventHook failed");
        }

        // Message pump hooks fire on this thread
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = UnhookWinEvent(hook_fg);
        let _ = UnhookWinEvent(hook_name);
        CoUninitialize();
    }
}