#![windows_subsystem = "windows"]

mod configs;
mod pak;
mod pvz_manager;

use std::{collections::HashMap, fs, path::PathBuf};

use crate::configs::{ModInfo, ModList, TomlConfig};

use pak::{PakManager, PakRecord};
use pvz_manager::PvZManager;
use walkdir::WalkDir;

fn main() {
    let mods_dir = PathBuf::from("mods");
    let modlist = ModList::init(mods_dir.join("mod-list.toml"));
    let mod_folders = modlist.mods.iter().map(|m| mods_dir.join(m));

    let pakdata = fs::read("main.pak").unwrap();

    let mut pak_manager = PakManager::from_bytes(&pakdata);

    let mut pak_map = HashMap::new();
    for i in 0..pak_manager.records.len() {
        let record = &pak_manager.records[i];
        pak_map.insert(record.name.clone(), i);
    }

    let mut to_inject = vec![];
    for mod_folder in mod_folders {
        let extra_resources =
            String::from_utf8_lossy(&fs::read(mod_folder.join("extra_resources.xml")).unwrap())
                .to_string();
        let resources_file =
            &mut pak_manager.records[pak_map[&String::from("properties\\resources.xml")]];
        let mut resources_string = String::from_utf8_lossy(&resources_file.data).to_string();
        let index = resources_string.rfind("</ResourceManifest>").unwrap();
        let closing_line = resources_string.split_off(index);
        resources_string.extend(extra_resources.chars());
        resources_string.extend(closing_line.chars());
        resources_file.data = Vec::from(resources_string.as_bytes());

        let asset_folder = mod_folder.join("assets");
        for entry in WalkDir::new(&asset_folder)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.metadata().unwrap().is_dir() {
                continue;
            }

            let full_path = entry.path();
            let filetime = entry.metadata().unwrap().modified().unwrap();
            let filename = full_path
                .strip_prefix(&asset_folder)
                .unwrap()
                .as_os_str()
                .to_string_lossy()
                .to_string();
            let data = fs::read(full_path).unwrap();
            if pak_map.contains_key(&filename) {
                pak_manager.records[pak_map[&filename]] = PakRecord {
                    name: filename.clone(),
                    filetime,
                    data,
                };
            } else {
                pak_map.insert(filename.clone(), pak_manager.records.len());
                pak_manager.records.push(PakRecord {
                    name: filename.clone(),
                    filetime,
                    data,
                });
            }
        }

        let modinfo = ModInfo::init(mod_folder.join("info.toml"));
        to_inject.extend(modinfo.target_dlls.iter().map(|dll| mod_folder.join(dll)));
    }

    fs::write("res.pak", &pak_manager.to_bytes()).unwrap();

    let mut app_manager = PvZManager::init("PlantsVsZombies.exe").unwrap();
    for dllpath in &to_inject {
        match app_manager.inject(dllpath) {
            Ok(()) => (),
            Err(_) => {
                println!("{:?} failed to inject", dllpath);
            }
        }
    }

    app_manager.set_pakfile("res.pak");
    app_manager.start();
}
