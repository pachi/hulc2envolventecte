#![cfg(windows)]
// Let's put this so that it won't open the console
#![windows_subsystem = "windows"]

/// GUI for the hulc2envolventecte app
///
/// Windows has a button to open a dialog to select a directory
/// This dir is shown in a label
/// Output file is also selected.
///
/// See https://docs.microsoft.com/en-us/windows/desktop/learnwin32/learn-to-program-for-windows
/// See Tomaka's error handling strategy for HRESULT (check_result): https://github.com/tomaka/cpal/blob/master/src/wasapi/mod.rs
/// See retep998's string handling in https://users.rust-lang.org/t/tidy-pattern-to-work-with-lpstr-mutable-char-array/2976
use std::error::Error;
use std::path::Path;
use std::ptr::null_mut;
use uuid::Uuid;

use winapi::shared::minwindef::*;
use winapi::shared::ntdef::*;
use winapi::shared::windef::*;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::winuser::*;

// Global Model to keep state
struct Model {
    dir_in: &'static str,
    h_btn_prj_in: HWND,
    h_label_prj_in: HWND,
    h_btn_run: HWND,
    h_edit_msg: HWND,
}

static mut MODEL: Model = Model {
    dir_in: "",
    h_btn_prj_in: 0 as HWND,
    h_label_prj_in: 0 as HWND,
    h_btn_run: 0 as HWND,
    h_edit_msg: 0 as HWND,
};

// Configura carpetas de entrada a directorio por defecto de HULC2018 y de salida al HOME
fn setup_folders() {
    use winapi::shared::winerror::SUCCEEDED;
    use winapi::um::shlobj::{SHGetFolderPathW, CSIDL_PROFILE};

    unsafe {
        // Dir out - por defecto es el directorio de proyectos de CTEHE2018 o el home del usuario
        const DEFAULT_DIR_IN: &str = "C:\\ProyectosCTEyCEE\\CTEHE2018\\Proyectos";

        let dir_in = if Path::new(DEFAULT_DIR_IN).is_dir() {
            DEFAULT_DIR_IN.to_string()
        } else {
             let mut buffer = [0; MAX_PATH];
            if !SUCCEEDED(SHGetFolderPathW(
                null_mut(),
                CSIDL_PROFILE,
                null_mut(),
                0,
                buffer.as_mut_ptr(),
            )) {
                return; // TODO: sacar error
            }
            let len = (0_usize..MAX_PATH)
                .find(|&n| buffer[n] == 0)
                .expect("Couldn't find null terminator");
            String::from_utf16_lossy(&buffer[..len])
        };
        MODEL.dir_in = Box::leak(dir_in.into_boxed_str());
    }
}

// Control IDs
const IDC_BUTTON_DIRIN: WORD = 101;
const IDC_LABEL_DIRIN: WORD = 102;
const IDC_BUTTON_RUN: WORD = 114;
const IDC_LABEL_MSG: WORD = 115;

// Get a win32 lpstr from a &str, converting u8 to u16 and appending '\0'
fn to_wstring(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

// Get a String from a string as wide pointer (PWSTR)
pub unsafe fn pwstr_to_string(ptr: PWSTR) -> String {
    use std::slice::from_raw_parts;
    let len = (0_usize..)
        .find(|&n| *ptr.add(n) == 0)
        .expect("Couldn't find null terminator");
    let array: &[u16] = from_raw_parts(ptr, len);
    String::from_utf16_lossy(array)
}

// Window procedure function to handle events
pub unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CLOSE => {
            DestroyWindow(hwnd);
        }
        WM_DESTROY => {
            PostQuitMessage(0);
        }
        WM_COMMAND => {
            let wm_id = LOWORD(wparam as DWORD);
            let wm_event = HIWORD(wparam as DWORD);
            match wm_id {
                IDC_BUTTON_DIRIN => {
                    if wm_event == BN_CLICKED {
                        // Clicked button 1
                        MODEL.dir_in = Box::leak(get_folder_path().into_boxed_str());
                        SetWindowTextW(MODEL.h_label_prj_in, to_wstring(&MODEL.dir_in).as_ptr());
                    }
                }
                IDC_BUTTON_RUN => {
                    // Clicked button 3
                    append_to_edit("\n\n**Generando archivo EnvolventeCTE**\n");
                    do_convert();
                }
                _ => {
                    // dbg!(("id: ", wm_id, "wm_event:", wm_event));
                }
            }
        }
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    0
}

// Apend text to the edit control
fn append_to_edit(txt: &str) {
    unsafe {
        let h_edit = MODEL.h_edit_msg;
        let tlen = GetWindowTextLengthW(h_edit);
        SendMessageW(h_edit, EM_SETSEL.into(), tlen as WPARAM, tlen as LPARAM); // Select the end pos
        SendMessageW(
            h_edit,
            EM_REPLACESEL.into(),
            0,
            to_wstring(txt).as_ptr() as LPARAM,
        ); // Append text to current pos and scroll down
    }
}

// Declare class and instantiate window
fn create_main_window(name: &str, title: &str) -> Result<HWND, Box<dyn Error>> {
    let name = to_wstring(name);
    let title = to_wstring(title);

    unsafe {
        // Get handle to the file used to create the calling process
        let hinstance = GetModuleHandleW(null_mut());

        // Create and register window class
        let wnd_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance, // Handle to the instance that contains the window procedure for the class
            hIcon: LoadIconW(null_mut(), IDI_APPLICATION),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: COLOR_WINDOW as HBRUSH,
            lpszMenuName: null_mut(),
            lpszClassName: name.as_ptr(),
            hIconSm: LoadIconW(null_mut(), IDI_APPLICATION),
        };

        // Register window class
        if RegisterClassExW(&wnd_class) == 0 {
            MessageBoxW(
                null_mut(),
                to_wstring("Window Registration Failed!").as_ptr(),
                to_wstring("Error").as_ptr(),
                MB_ICONEXCLAMATION | MB_OK,
            );
            return Err("Window Registration Failed".into());
        };

        // Create a window based on registered class
        let handle = CreateWindowExW(
            0,                                // dwExStyle
            name.as_ptr(),                    // lpClassName
            title.as_ptr(),                   // lpWindowName
            WS_OVERLAPPEDWINDOW | WS_VISIBLE, // dwStyle
            CW_USEDEFAULT,                    // Int x
            CW_USEDEFAULT,                    // Int y
            630,                              // Int nWidth
            470,                              // Int nHeight
            null_mut(),                       // hWndParent
            null_mut(),                       // hMenu
            hinstance,                        // hInstance
            null_mut(),                       // lpParam
        );

        if handle.is_null() {
            MessageBoxW(
                null_mut(),
                to_wstring("Window Creation Failed!").as_ptr(),
                to_wstring("Error!").as_ptr(),
                MB_ICONEXCLAMATION | MB_OK,
            );
            return Err("Window Creation Failed!".into());
        }

        // Custom GUI
        create_gui(handle);

        ShowWindow(handle, SW_SHOW);
        UpdateWindow(handle);

        Ok(handle)
    }
}

// Build GUI elements inside main window
unsafe fn create_gui(hparent: HWND) {
    let hinstance = GetWindowLongW(hparent, GWL_HINSTANCE) as HINSTANCE;
    //let hinstance = GetModuleHandleW(null_mut());

    MODEL.h_btn_prj_in = CreateWindowExW(
        0,
        to_wstring("Button").as_ptr(),
        to_wstring("Directorio de proyecto de HULC").as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON | BS_TEXT,
        10,  // x
        10,  // y
        300, // w
        30,  // h
        hparent,
        IDC_BUTTON_DIRIN as HMENU,
        hinstance,
        null_mut(),
    );

    MODEL.h_label_prj_in = CreateWindowExW(
        0,
        to_wstring("static").as_ptr(),
        to_wstring(MODEL.dir_in).as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | SS_LEFT,
        320, // x
        10,  // y
        300, // w
        30,  // h
        hparent,
        IDC_LABEL_DIRIN as HMENU,
        hinstance,
        null_mut(),
    );

    MODEL.h_btn_run = CreateWindowExW(
        0,
        to_wstring("button").as_ptr(),
        to_wstring("¡Generar archivo de EnvolventeCTE!").as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON | BS_TEXT,
        10,  // x
        50,  // y
        300, // w
        60,  // h
        hparent,
        IDC_BUTTON_RUN as HMENU,
        hinstance,
        null_mut(),
    );

    MODEL.h_edit_msg = CreateWindowExW(
        0,
        to_wstring("edit").as_ptr(),
        to_wstring(&crate::get_copytxt()).as_ptr(),
        WS_VSCROLL
            | WS_BORDER
            | WS_CHILD
            | ES_MULTILINE
            | ES_READONLY
            | WS_VISIBLE
            | WS_TABSTOP
            | SS_LEFT,
        10,  // x
        120, // y
        600, // w
        300, // h
        hparent,
        IDC_LABEL_MSG as HMENU,
        hinstance,
        null_mut(),
    );
}

// Open FileOpenDialog in folder select mode to get a folder path
unsafe fn get_folder_path() -> String {
    use winapi::shared::winerror::SUCCEEDED;
    use winapi::um::combaseapi::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    };
    use winapi::um::objbase::{COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
    use winapi::um::shobjidl::*;
    use winapi::um::shobjidl_core::{IShellItem, SIGDN_FILESYSPATH};
    use winapi::Interface;

    // winapi::um::shobjidl_core::CLSID_FileOpenDialog is unreleased
    // This will be available as FileOpenDialog::uuidof()
    #[allow(non_upper_case_globals)]
    const CLSID_FileOpenDialog: winapi::shared::guiddef::GUID = winapi::shared::guiddef::GUID {
        Data1: 0xdc1c5a9c,
        Data2: 0xe88a,
        Data3: 0x4dde,
        Data4: [0xa5, 0xa1, 0x60, 0xf8, 0x2a, 0x20, 0xae, 0xf7],
    };
    let mut sel_dir: String = "".to_string();

    // Inicializar COM
    let mut hr = CoInitializeEx(
        null_mut(),
        COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE,
    );
    if SUCCEEDED(hr) {
        // Crear diálogo
        let mut pfd: *mut IFileDialog = std::mem::uninitialized();
        hr = CoCreateInstance(
            &CLSID_FileOpenDialog,
            null_mut(),
            CLSCTX_ALL,
            &IFileOpenDialog::uuidof(),
            &mut pfd as *mut *mut IFileDialog as *mut _,
        );
        if SUCCEEDED(hr) {
            // Fijar opciones del selector
            let mut fop: FILEOPENDIALOGOPTIONS = std::mem::zeroed();
            if SUCCEEDED((*pfd).GetOptions(&mut fop)) {
                (*pfd).SetOptions(
                    fop | FOS_PICKFOLDERS
                        | FOS_FORCESHOWHIDDEN
                        | FOS_PATHMUSTEXIST
                        | FOS_FORCEFILESYSTEM,
                );
            }

            // Mostrar diálogo
            if SUCCEEDED((*pfd).Show(null_mut())) {
                // Recoger resultados
                let mut psi: *mut IShellItem = std::mem::zeroed();
                if SUCCEEDED((*pfd).GetResult(&mut psi)) {
                    // Provide a pointer to a buffer so windows can swap it for its own buffer
                    let mut buffer: PWSTR = std::ptr::null_mut();
                    if SUCCEEDED((*psi).GetDisplayName(SIGDN_FILESYSPATH, &mut buffer)) {
                        sel_dir = pwstr_to_string(buffer);
                    }
                    // Free the windows provided buffer to avoid leaking it
                    CoTaskMemFree(buffer as *mut _);
                }
                (*psi).Release();
            }
            (*pfd).Release();
        }
        // Cerrar COM
        CoUninitialize();
    }
    sel_dir
}

// Message handling loop
fn run_message_loop(hwnd: HWND) -> WPARAM {
    unsafe {
        let mut msg: MSG = std::mem::uninitialized();
        loop {
            // Get message from message queue
            if GetMessageW(&mut msg, hwnd, 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                // Return on error (<0) or exit (=0) cases
                return msg.wParam;
            }
        }
    }
}

fn do_convert() {
    use crate::{ctehexml, kyg, serde_json, tbl, utils, EnvolventeCteData};
    let dir_in = unsafe { MODEL.dir_in };

    let hulcfiles = match crate::find_hulc_files(&dir_in) {
        Ok(hulcfiles) => {
            append_to_edit(&format!("\nLocalizados archivos de datos en '{}'", dir_in));
            append_to_edit(&format!("\n  - {}", hulcfiles.ctehexml));
            append_to_edit(&format!("\n  - {}", hulcfiles.tbl));
            append_to_edit(&format!("\n  - {}", hulcfiles.kyg));
            hulcfiles
        }
        _ => {
            append_to_edit(&format!("\nERROR: No se han encontrado los archivos .ctehexml, .tbl o .kyg en el directorio de proyecto {}.", dir_in));
            return;
        }
    };

    let ctehexmldata = match ctehexml::parse(&hulcfiles.ctehexml) {
        Ok(ctehexmldata) => {
            append_to_edit(&format!(
                "\nLocalizada zona climática {} y coeficientes de transmisión de energía solar g_gl;sh;wi",
                ctehexmldata.climate
            ));
            ctehexmldata
        }
        _ => {
            append_to_edit("\nERROR: No se ha encontrado la zona climática o los coeficientes de transmisión de energía solar g_gl;sh;wi");
            return;
        }
    };

    let tbl = match tbl::parse(&hulcfiles.tbl) {
        Ok(tbl) => {
            append_to_edit(&format!(
                "\nLocalizados {} espacios y {} elementos",
                tbl.spaces.len(),
                tbl.elements.len()
            ));
            tbl
        }
        _ => {
            append_to_edit("\nERROR: No se ha localizado la definición de espacios y elementos en el archivo .tbl");
            return;
        }
    };

    let elementos_envolvente = match kyg::parse(&hulcfiles.kyg, Some(ctehexmldata.gglshwi)) {
        Ok(elementos_envolvente) => {
            append_to_edit("\nEncontrada descripción de elementos de la envolvente");
            elementos_envolvente
        }
        _ => {
            append_to_edit("\nERROR: No se ha podido interpretar correctamente el archivo .kyg de elementos de la envolvente");
            return;
        }
    };

    let area_util = tbl.compute_autil(&elementos_envolvente);
    append_to_edit(&format!("\nArea útil: {} m2", area_util));

    // Salida en JSON
    let envolvente_data = EnvolventeCteData {
        autil: area_util,
        clima: ctehexmldata.climate,
        envolvente: elementos_envolvente,
    };
    match serde_json::to_string_pretty(&envolvente_data) {
        Ok(json) => {
            // No podemos hacer un hash repetible así que usamos uuid
            // Esto es porque los uuid de los elementos se regeneran en cada conversión
            let suuid = &(Uuid::new_v4()).to_hyphenated().to_string()[..8];
            let path = Path::new(dir_in).join(&format!("envolventecte-{}.json", &suuid));
            if write_file(&path, &json).is_err() {
                append_to_edit(&format!(
                    "\nERROR: no se ha podido escribir en la ruta {}",
                    path.display()
                ));
            }
            append_to_edit(
                "\n\nSe ha guardado el archivo de resultados en formato JSON de EnvolventeCTE:\n",
            );
            append_to_edit(&format!("    {}", path.display()));
        }
        _ => {
            append_to_edit(
                "\nERROR: no se ha podido generar la información en formato JSON de EnvolventeCTE",
            );
            return;
        }
    };
}

// Guarda archivo a disco
fn write_file(path: &std::path::Path, data: &str) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::prelude::*;

    let mut file = File::create(path)?;
    file.write_all(data.as_bytes())
}

pub fn run_wingui() {
    setup_folders();
    let hwnd = create_main_window(
        "hulc2envolventecte_gui",
        "Conversión de HULC a EnvolventeCTE",
    )
    .expect("Error al crear la ventana principal!");
    run_message_loop(hwnd);
}
