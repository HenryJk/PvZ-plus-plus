use std::{ffi::CString, fs::File, io::Read, path::Path};

use serde::{de::DeserializeOwned, Deserialize};
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{MessageBoxA, MB_ICONERROR, MB_OK, MESSAGEBOX_RESULT},
    },
};

fn error_messagebox<T>(msg: T) -> MESSAGEBOX_RESULT
where
    T: AsRef<str>,
{
    let title = CString::new("Error").unwrap();
    let msg = CString::new(msg.as_ref()).unwrap();
    unsafe {
        MessageBoxA(
            HWND(0),
            PCSTR(msg.as_ptr() as *const u8),
            PCSTR(title.as_ptr() as *const u8),
            MB_OK | MB_ICONERROR,
        )
    }
}

pub trait TomlConfig: DeserializeOwned {
    fn init<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();

        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(_) => {
                let msg = format!("Unable to open {}", path.as_os_str().to_str().unwrap());
                error_messagebox(msg);
                panic!();
            }
        };

        let mut data = String::new();
        if let Err(_) = file.read_to_string(&mut data) {
            let msg = format!("Unable to read {}", path.as_os_str().to_str().unwrap());
            error_messagebox(msg);
            panic!();
        }

        match toml::from_str(&data) {
            Ok(modlist) => modlist,
            Err(_) => {
                let msg = format!("Unable to parse {}", path.as_os_str().to_str().unwrap());
                error_messagebox(msg);
                panic!();
            }
        }
    }
}

#[derive(Deserialize)]
pub struct ModList {
    pub mods: Vec<String>,
}

#[derive(Deserialize)]
pub struct ModInfo {
    pub name: String,
    pub version: String,
    pub compatible_pvz_versions: Vec<String>,
    pub title: String,
    pub author: String,
    pub description: String,
    pub homepage: Option<String>,
    pub target_dlls: Vec<String>,
}

impl TomlConfig for ModList {}
impl TomlConfig for ModInfo {}
