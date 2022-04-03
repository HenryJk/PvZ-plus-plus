use std::{
    ffi::{c_void, CString},
    io::{self, stderr, stdin, stdout},
    iter::once,
    mem::{size_of, transmute},
    os::windows::prelude::{AsRawHandle, OsStrExt},
    path::Path,
    ptr::{null, null_mut},
};

use pelite::{
    pe32::{Pe, PeView},
    ImageMap,
};
use windows::{
    core::{PCSTR, PCWSTR, PWSTR},
    Win32::{
        Foundation::{CloseHandle, BOOL, HANDLE},
        System::{
            Diagnostics::Debug::{
                GetThreadContext, ReadProcessMemory, WriteProcessMemory, CONTEXT,
            },
            LibraryLoader::{GetModuleHandleA, GetProcAddress},
            Memory::{VirtualAllocEx, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE},
            SystemServices::CONTEXT_i386,
            Threading::{
                CreateProcessW, CreateRemoteThread, ResumeThread, SuspendThread,
                WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED, PROCESS_INFORMATION,
                STARTF_USESTDHANDLES, STARTUPINFOW,
            },
        },
    },
};

pub struct PvZManager {
    pinfo: PROCESS_INFORMATION,
    entry_point: *const c_void,
    original_code: [u8; 2],
}

impl PvZManager {
    pub fn init<P>(appname: P) -> Result<Self, io::Error>
    where
        P: AsRef<Path>,
    {
        let image = ImageMap::open(&appname)?;
        let header = PeView::from_bytes(&image).unwrap().optional_header();
        let entry_point = (header.ImageBase + header.AddressOfEntryPoint) as *mut c_void;

        let sinfo = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            hStdInput: HANDLE(stdin().as_raw_handle() as isize),
            hStdOutput: HANDLE(stdout().as_raw_handle() as isize),
            hStdError: HANDLE(stderr().as_raw_handle() as isize),
            dwFlags: STARTF_USESTDHANDLES,
            ..Default::default()
        };
        let mut pinfo = Default::default();
        let wc_appname: Vec<_> = appname
            .as_ref()
            .as_os_str()
            .encode_wide()
            .chain(once(0))
            .collect();

        let mut original_code = [0u8; 2];
        unsafe {
            CreateProcessW(
                PCWSTR(wc_appname.as_ptr()),
                PWSTR(null_mut()),
                null_mut(),
                null_mut(),
                BOOL(true as i32),
                CREATE_SUSPENDED | CREATE_NO_WINDOW,
                null_mut(),
                PCWSTR(null()),
                &sinfo,
                &mut pinfo,
            );

            ReadProcessMemory(
                pinfo.hProcess,
                entry_point,
                original_code.as_mut_ptr() as *mut c_void,
                2,
                null_mut(),
            );

            WriteProcessMemory(
                pinfo.hProcess,
                entry_point,
                [0xEBu8, 0xFE].as_ptr() as *const c_void,
                2,
                null_mut(),
            );

            ResumeThread(pinfo.hThread);

            let mut context = CONTEXT {
                ContextFlags: (CONTEXT_i386 | 0x00000001) as u32,
                ..Default::default()
            };

            GetThreadContext(pinfo.hThread, &mut context);
            while context.Eip != entry_point as u32 {
                std::thread::sleep(std::time::Duration::from_millis(10));
                GetThreadContext(pinfo.hThread, &mut context);
            }

            SuspendThread(pinfo.hThread);
        }

        Ok(Self {
            pinfo,
            entry_point,
            original_code,
        })
    }

    pub fn inject<P>(&mut self, dll: P) -> io::Result<()>
    where
        P: AsRef<Path>,
    {
        let full_path: Vec<u16> = dll
            .as_ref()
            .canonicalize()?
            .as_os_str()
            .encode_wide()
            .chain(once(0))
            .collect();

        // allocate space for the path inside target proc
        unsafe {
            let dll_addr = VirtualAllocEx(
                self.pinfo.hProcess,
                null_mut(),
                full_path.len() * 2,
                MEM_RESERVE | MEM_COMMIT,
                PAGE_EXECUTE_READWRITE,
            );

            WriteProcessMemory(
                self.pinfo.hProcess,
                dll_addr,
                full_path.as_ptr() as *const _,
                full_path.len() * 2,
                null_mut(),
            );

            let krnl = CString::new("Kernel32").unwrap();
            let krnl = GetModuleHandleA(PCSTR(krnl.as_ptr() as *const _));
            let loadlib = CString::new("LoadLibraryW").unwrap();
            let loadlib = GetProcAddress(krnl, PCSTR(loadlib.as_ptr() as *const _));
            let hthread = CreateRemoteThread(
                self.pinfo.hProcess,
                null_mut(),
                0,
                transmute(loadlib),
                dll_addr,
                0,
                null_mut(),
            );
            ResumeThread(hthread);
            WaitForSingleObject(hthread, u32::MAX);
            CloseHandle(hthread);
        }

        Ok(())
    }

    pub fn start(&self) {
        unsafe {
            WriteProcessMemory(
                self.pinfo.hProcess,
                self.entry_point,
                self.original_code.as_ptr() as *const c_void,
                2,
                null_mut(),
            );

            // std::thread::sleep(std::time::Duration::from_secs(20));
            ResumeThread(self.pinfo.hThread);
        }
    }

    pub fn set_pakfile(&self, pakfile: &str) {
        let c_pakfile = CString::new(pakfile).unwrap();
        unsafe {
            let pakfile_address = VirtualAllocEx(
                self.pinfo.hProcess,
                null_mut(),
                pakfile.len() + 1,
                MEM_RESERVE | MEM_COMMIT,
                PAGE_EXECUTE_READWRITE,
            );

            WriteProcessMemory(
                self.pinfo.hProcess,
                pakfile_address,
                c_pakfile.as_ptr() as *const _,
                pakfile.len() + 1,
                null_mut(),
            );

            WriteProcessMemory(
                self.pinfo.hProcess,
                0x553D7E as *mut _,
                transmute(&pakfile_address),
                4,
                null_mut(),
            );
        }
    }
}
