//! Hidden child execution with Windows startup feedback explicitly disabled.
//! Only the child's three standard handles are inherited (two OCR workers may spawn together).
#![allow(non_snake_case)]
#![cfg_attr(not(windows), allow(dead_code))]
use std::{
    ffi::{c_void, OsStr},
    process::Command,
    ptr::{null, null_mut},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
type Raw = isize;
#[repr(C)]
struct Security {
    length: u32,
    descriptor: *mut c_void,
    inherit: i32,
}
#[repr(C)]
struct Startup {
    size: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    chars_x: u32,
    chars_y: u32,
    fill: u32,
    flags: u32,
    show: u16,
    reserved_size: u16,
    reserved_bytes: *mut u8,
    input: Raw,
    output: Raw,
    error: Raw,
}
#[repr(C)]
struct StartupEx {
    info: Startup,
    attributes: *mut c_void,
}
#[repr(C)]
#[derive(Default)]
struct ProcessInfo {
    process: Raw,
    thread: Raw,
    process_id: u32,
    thread_id: u32,
}
// The application supports Windows x64; check the C ABI layouts during host checks too.
const _: [(); 24] = [(); std::mem::size_of::<Security>()];
const _: [(); 104] = [(); std::mem::size_of::<Startup>()];
const _: [(); 112] = [(); std::mem::size_of::<StartupEx>()];
const _: [(); 24] = [(); std::mem::size_of::<ProcessInfo>()];
#[link(name = "kernel32")]
extern "system" {
    fn CreatePipe(read: *mut Raw, write: *mut Raw, security: *const Security, size: u32) -> i32;
    fn SetHandleInformation(handle: Raw, mask: u32, flags: u32) -> i32;
    fn CloseHandle(handle: Raw) -> i32;
    fn CreateFileW(
        path: *const u16,
        access: u32,
        share: u32,
        security: *const Security,
        creation: u32,
        flags: u32,
        template: Raw,
    ) -> Raw;
    fn ReadFile(
        file: Raw,
        buffer: *mut u8,
        count: u32,
        read: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn InitializeProcThreadAttributeList(
        list: *mut c_void,
        count: u32,
        flags: u32,
        size: *mut usize,
    ) -> i32;
    fn UpdateProcThreadAttribute(
        list: *mut c_void,
        flags: u32,
        attribute: usize,
        value: *mut c_void,
        size: usize,
        previous: *mut c_void,
        returned: *mut usize,
    ) -> i32;
    fn DeleteProcThreadAttributeList(list: *mut c_void);
    fn CreateProcessW(
        application: *const u16,
        command: *mut u16,
        process_security: *const c_void,
        thread_security: *const c_void,
        inherit: i32,
        flags: u32,
        environment: *const c_void,
        directory: *const u16,
        startup: *const StartupEx,
        info: *mut ProcessInfo,
    ) -> i32;
    fn WaitForSingleObject(handle: Raw, timeout: u32) -> u32;
    fn GetExitCodeProcess(handle: Raw, code: *mut u32) -> i32;
    fn TerminateProcess(handle: Raw, code: u32) -> i32;
}
struct Handle(Raw);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct Attributes(Vec<usize>);
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.0.as_mut_ptr().cast());
        }
    }
}
struct Child(Handle);
impl Child {
    fn terminate(&self) {
        unsafe {
            TerminateProcess(self.0 .0, 1);
            WaitForSingleObject(self.0 .0, u32::MAX);
        }
    }
}
impl Drop for Child {
    fn drop(&mut self) {
        unsafe {
            if WaitForSingleObject(self.0 .0, 0) == 258 {
                self.terminate();
            }
        }
    }
}
fn error() -> String {
    std::io::Error::last_os_error().to_string()
}
fn wide(value: &OsStr) -> Vec<u16> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        value.encode_wide().collect()
    }
    #[cfg(not(windows))]
    {
        value.to_string_lossy().encode_utf16().collect()
    }
}
fn terminated(mut value: Vec<u16>) -> Result<Vec<u16>, String> {
    if value.contains(&0) {
        return Err("进程参数包含空字符".to_owned());
    }
    value.push(0);
    Ok(value)
}
fn pipe() -> Result<(Handle, Handle), String> {
    unsafe {
        let sa = Security {
            length: std::mem::size_of::<Security>() as u32,
            descriptor: null_mut(),
            inherit: 1,
        };
        let (mut read, mut write) = (0, 0);
        if CreatePipe(&mut read, &mut write, &sa, 0) == 0 {
            return Err(error());
        }
        let (read, write) = (Handle(read), Handle(write));
        if SetHandleInformation(read.0, 1, 0) == 0 {
            return Err(error());
        }
        Ok((read, write))
    }
}
fn drain(pipe: Handle) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let mut count = 0;
        let ok = unsafe {
            ReadFile(
                pipe.0,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                &mut count,
                null_mut(),
            )
        };
        if ok == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(109) {
                break;
            }
            return Err(error.to_string());
        }
        if count == 0 {
            break;
        }
        output.extend_from_slice(&buffer[..count as usize]);
    }
    Ok(output)
}
pub fn capture(
    command: &Command,
    cancelled: &AtomicBool,
    timeout: Duration,
) -> Result<String, String> {
    unsafe {
        if cancelled.load(Ordering::Relaxed) {
            return Err("操作已取消".to_owned());
        }
        // All callers use fixed absolute executable paths and inherit the app environment.
        if command.get_envs().next().is_some() {
            return Err("OCR 子进程不支持覆盖环境变量".to_owned());
        }
        let program = terminated(wide(command.get_program()))?;
        let mut line = Vec::new();
        for arg in std::iter::once(command.get_program()).chain(command.get_args()) {
            let units = wide(arg);
            if units.contains(&0) {
                return Err("进程参数包含空字符".to_owned());
            }
            if !line.is_empty() {
                line.push(32);
            }
            line.extend(pokemmo_hint::commandline::quote_argument(&units));
        }
        if line.len() >= 32767 {
            return Err("OCR 命令行过长".to_owned());
        }
        line.push(0);
        let directory = command
            .get_current_dir()
            .map(|dir| terminated(wide(dir.as_os_str())))
            .transpose()?;
        let (out_read, out_write) = pipe()?;
        let (err_read, err_write) = pipe()?;
        let sa = Security {
            length: std::mem::size_of::<Security>() as u32,
            descriptor: null_mut(),
            inherit: 1,
        };
        let input = CreateFileW([78u16, 85, 76, 0].as_ptr(), 0x80000000, 3, &sa, 3, 0, 0);
        if input == -1 {
            return Err(error());
        }
        let input = Handle(input);
        let mut size = 0;
        InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut size);
        if size == 0 {
            return Err(error());
        }
        let mut storage = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
        if InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), 1, 0, &mut size) == 0 {
            return Err(error());
        }
        let mut attributes = Attributes(storage);
        let mut handles = [input.0, out_write.0, err_write.0];
        if UpdateProcThreadAttribute(
            attributes.0.as_mut_ptr().cast(),
            0,
            0x20002,
            handles.as_mut_ptr().cast(),
            std::mem::size_of_val(&handles),
            null_mut(),
            null_mut(),
        ) == 0
        {
            return Err(error());
        }
        let startup = StartupEx {
            info: Startup {
                size: std::mem::size_of::<StartupEx>() as u32,
                reserved: null_mut(),
                desktop: null_mut(),
                title: null_mut(),
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                chars_x: 0,
                chars_y: 0,
                fill: 0,
                flags: 0x100 | 0x80 | 1, // USESTDHANDLES | FORCEOFFFEEDBACK | USESHOWWINDOW
                show: 0,
                reserved_size: 0,
                reserved_bytes: null_mut(),
                input: input.0,
                output: out_write.0,
                error: err_write.0,
            },
            attributes: attributes.0.as_mut_ptr().cast(),
        };
        let mut info = ProcessInfo::default();
        if CreateProcessW(
            program.as_ptr(),
            line.as_mut_ptr(),
            null(),
            null(),
            1,
            0x08000000 | 0x00080000,
            null(),
            directory.as_ref().map_or(null(), |d| d.as_ptr()),
            &startup,
            &mut info,
        ) == 0
        {
            return Err(format!("启动 OCR 失败：{}", error()));
        }
        let child = Child(Handle(info.process));
        drop(Handle(info.thread));
        drop(out_write);
        drop(err_write);
        drop(input);
        drop(attributes);
        let out = std::thread::spawn(move || drain(out_read));
        let err = std::thread::spawn(move || drain(err_read));
        let start = Instant::now();
        let result = loop {
            if cancelled.load(Ordering::Relaxed) || start.elapsed() > timeout {
                child.terminate();
                break Err("操作已取消或超时".to_owned());
            }
            match WaitForSingleObject(child.0 .0, 40) {
                0 => {
                    let mut code = 0;
                    if GetExitCodeProcess(child.0 .0, &mut code) == 0 {
                        break Err(error());
                    }
                    break Ok(code);
                }
                258 => {}
                _ => {
                    let message = error();
                    child.terminate();
                    break Err(message);
                }
            }
        };
        let output = out.join().map_err(|_| "OCR 输出读取失败")??;
        let error_output = err.join().map_err(|_| "OCR 错误读取失败")??;
        let code = result?;
        if code != 0 {
            return Err(format!(
                "OCR 失败（退出码 {code}）：{}",
                String::from_utf8_lossy(&error_output).trim()
            ));
        }
        String::from_utf8(output)
            .map(|s| s.trim().to_owned())
            .map_err(|_| "OCR 输出不是 UTF-8".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern "system" {
        fn GetStartupInfoW(info: *mut Startup);
    }

    #[test]
    #[ignore = "launched as an isolated child by hidden_child_keeps_output_without_startup_feedback"]
    fn child_startup_configuration() {
        let mut info: Startup = unsafe { std::mem::zeroed() };
        unsafe {
            GetStartupInfoW(&mut info);
        }
        assert_ne!(info.flags & 0x80, 0, "startup feedback must be off");
        assert_ne!(
            info.flags & 0x100,
            0,
            "redirected standard handles must survive launch"
        );
        assert_eq!(info.show, 0);
        println!("OCR_CHILD_OUTPUT 中文 No.573");
        eprintln!("OCR_CHILD_DIAGNOSTIC");
    }

    #[test]
    fn hidden_child_keeps_output_without_startup_feedback() {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "ocr_process::tests::child_startup_configuration",
            "--ignored",
            "--nocapture",
        ]);
        let output = capture(&command, &AtomicBool::new(false), Duration::from_secs(15)).unwrap();
        assert!(output.contains("OCR_CHILD_OUTPUT 中文 No.573"));
        assert!(!output.contains("OCR_CHILD_DIAGNOSTIC"));
    }

    #[test]
    #[ignore = "launched as an isolated child by cancellation_and_timeout_terminate_the_child"]
    fn sleeping_child() {
        std::thread::sleep(Duration::from_secs(30));
    }

    #[test]
    fn cancellation_and_timeout_terminate_the_child() {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "ocr_process::tests::sleeping_child",
            "--ignored",
            "--nocapture",
        ]);
        let stop = AtomicBool::new(false);
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(100));
                stop.store(true, Ordering::Relaxed);
            });
            let start = Instant::now();
            assert!(capture(&command, &stop, Duration::from_secs(10))
                .unwrap_err()
                .contains("取消"));
            assert!(start.elapsed() < Duration::from_secs(5));
        });
        assert!(capture(
            &command,
            &AtomicBool::new(false),
            Duration::from_millis(100)
        )
        .unwrap_err()
        .contains("超时"));
    }
}
