//! Minimal ONNX Runtime C API v22 adapter. Native inference runs in a cancellable child.
use pokemmo_hint::engines::{decode_ctc, tensor_from_bmp};
use std::{
    ffi::{c_char, c_void, CStr, CString},
    path::Path,
    ptr::null_mut,
};
type Ptr = *mut c_void;
#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryExW(path: *const u16, file: isize, flags: u32) -> isize;
    fn GetProcAddress(module: isize, name: *const u8) -> *const c_void;
    fn FreeLibrary(module: isize) -> i32;
    fn SetErrorMode(mode: u32) -> u32;
}
#[repr(C)]
struct Base {
    get_api: unsafe extern "system" fn(u32) -> *const *const c_void,
    version: unsafe extern "system" fn() -> *const c_char,
}
macro_rules! api {
    ($a:expr,$i:expr,$t:ty) => {
        std::mem::transmute::<*const c_void, $t>(*($a).add($i))
    };
}
struct Object {
    ptr: Ptr,
    api: *const *const c_void,
    release: usize,
}
impl Drop for Object {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                api!(self.api, self.release, unsafe extern "system" fn(Ptr))(self.ptr);
            }
        }
    }
}
pub struct Session {
    objects: Vec<Object>,
    api: *const *const c_void,
    module: isize,
    input: CString,
    output: CString,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.objects.reverse();
        self.objects.clear();
        unsafe {
            FreeLibrary(self.module);
        }
    }
}
impl Session {
    unsafe fn status(&self, status: Ptr) -> Result<(), String> {
        if status.is_null() {
            return Ok(());
        }
        let message = CStr::from_ptr(api!(
            self.api,
            2,
            unsafe extern "system" fn(Ptr) -> *const c_char
        )(status))
        .to_string_lossy()
        .into_owned();
        api!(self.api, 93, unsafe extern "system" fn(Ptr))(status);
        Err(format!("ONNX Runtime：{message}"))
    }
    fn object(&self, ptr: Ptr, release: usize) -> Object {
        Object {
            ptr,
            release,
            api: self.api,
        }
    }
    pub fn load(dir: &Path) -> Result<Self, String> {
        unsafe {
            SetErrorMode(3); // isolated helper: report failures through stderr, no crash dialogs
            let dll = dir.join("onnxruntime.dll");
            let path: Vec<u16> = dll
                .to_string_lossy()
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let module = LoadLibraryExW(path.as_ptr(), 0, 0x1100);
            if module == 0 {
                return Err(format!(
                    "加载推理组件失败（可能缺少 VC++ x64 运行库）：{}",
                    std::io::Error::last_os_error()
                ));
            }
            let entry = GetProcAddress(module, c"OrtGetApiBase".as_ptr().cast());
            if entry.is_null() {
                FreeLibrary(module);
                return Err("无效 ONNX Runtime 组件".to_owned());
            }
            let base = std::mem::transmute::<
                *const c_void,
                unsafe extern "system" fn() -> *const Base,
            >(entry)();
            let api = ((*base).get_api)(22);
            if api.is_null() {
                FreeLibrary(module);
                return Err("推理组件不支持 API 22".to_owned());
            }
            let mut s = Self {
                objects: Vec::new(),
                api,
                module,
                input: CString::new("").unwrap(),
                output: CString::new("").unwrap(),
            };
            let mut env = null_mut();
            s.status(api!(
                api,
                3,
                unsafe extern "system" fn(i32, *const c_char, *mut Ptr) -> Ptr
            )(3, c"PokemonOCR".as_ptr(), &mut env))?;
            s.objects.push(s.object(env, 92));
            s.status(api!(api, 6, unsafe extern "system" fn(Ptr) -> Ptr)(env))?;
            let mut options = null_mut();
            s.status(api!(api, 10, unsafe extern "system" fn(*mut Ptr) -> Ptr)(
                &mut options,
            ))?;
            let options = s.object(options, 100);
            s.status(api!(api, 24, unsafe extern "system" fn(Ptr, i32) -> Ptr)(
                options.ptr,
                1,
            ))?;
            s.status(api!(api, 23, unsafe extern "system" fn(Ptr, i32) -> Ptr)(
                options.ptr,
                99,
            ))?;
            let model: Vec<u16> = dir
                .join("inference.onnx")
                .to_string_lossy()
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let mut session = null_mut();
            s.status(api!(
                api,
                7,
                unsafe extern "system" fn(Ptr, *const u16, Ptr, *mut Ptr) -> Ptr
            )(env, model.as_ptr(), options.ptr, &mut session))?;
            s.objects.push(s.object(session, 95));
            for index in [30, 31] {
                let mut count = 0;
                s.status(api!(
                    api,
                    index,
                    unsafe extern "system" fn(Ptr, *mut usize) -> Ptr
                )(session, &mut count))?;
                if count != 1 {
                    return Err("PP-OCR 模型输入输出数量不匹配".to_owned());
                }
            }
            let mut allocator = null_mut();
            s.status(api!(api, 78, unsafe extern "system" fn(*mut Ptr) -> Ptr)(
                &mut allocator,
            ))?;
            for index in [36, 37] {
                let mut name = null_mut();
                s.status(api!(
                    api,
                    index,
                    unsafe extern "system" fn(Ptr, usize, Ptr, *mut *mut c_char) -> Ptr
                )(session, 0, allocator, &mut name))?;
                let value = CStr::from_ptr(name).to_owned();
                s.status(api!(api, 76, unsafe extern "system" fn(Ptr, Ptr) -> Ptr)(
                    allocator,
                    name.cast(),
                ))?;
                if index == 36 {
                    s.input = value;
                } else {
                    s.output = value;
                }
            }
            Ok(s)
        }
    }
    pub fn recognize(&self, path: &Path) -> Result<String, String> {
        let (bytes, width) = tensor_from_bmp(&std::fs::read(path).map_err(|e| e.to_string())?)?;
        self.infer(bytes, width)
    }
    pub fn check(&self) -> Result<(), String> {
        match self.infer(vec![1.0; 3 * 48 * 320], 320) {
            Err(e) if e.contains("置信度不足") => Ok(()),
            result => result.map(|_| ()),
        }
    }
    fn infer(&self, mut pixels: Vec<f32>, width: usize) -> Result<String, String> {
        unsafe {
            let api = self.api;
            let mut mem = null_mut();
            self.status(api!(
                api,
                69,
                unsafe extern "system" fn(i32, i32, *mut Ptr) -> Ptr
            )(1, 0, &mut mem))?;
            let mem = self.object(mem, 94);
            let shape = [1i64, 3, 48, width as i64];
            let mut input = null_mut();
            self.status(api!(
                api,
                49,
                unsafe extern "system" fn(Ptr, Ptr, usize, *const i64, usize, i32, *mut Ptr) -> Ptr
            )(
                mem.ptr,
                pixels.as_mut_ptr().cast(),
                pixels.len() * 4,
                shape.as_ptr(),
                4,
                1,
                &mut input,
            ))?;
            let input = self.object(input, 96);
            let mut output = null_mut();
            let status = api!(
                api,
                9,
                unsafe extern "system" fn(
                    Ptr,
                    Ptr,
                    *const *const c_char,
                    *const Ptr,
                    usize,
                    *const *const c_char,
                    usize,
                    *mut Ptr,
                ) -> Ptr
            )(
                self.objects[1].ptr,
                null_mut(),
                &self.input.as_ptr(),
                &input.ptr,
                1,
                &self.output.as_ptr(),
                1,
                &mut output,
            );
            let output = self.object(output, 96);
            self.status(status)?;
            let mut info = null_mut();
            self.status(api!(
                api,
                65,
                unsafe extern "system" fn(Ptr, *mut Ptr) -> Ptr
            )(output.ptr, &mut info))?;
            let info = self.object(info, 99);
            let mut element = 0;
            self.status(api!(
                api,
                60,
                unsafe extern "system" fn(Ptr, *mut i32) -> Ptr
            )(info.ptr, &mut element))?;
            let mut rank = 0;
            self.status(api!(
                api,
                61,
                unsafe extern "system" fn(Ptr, *mut usize) -> Ptr
            )(info.ptr, &mut rank))?;
            if element != 1 || rank != 3 {
                return Err("PP-OCR 输出格式不匹配".to_owned());
            }
            let mut dims = [0i64; 3];
            self.status(api!(
                api,
                62,
                unsafe extern "system" fn(Ptr, *mut i64, usize) -> Ptr
            )(info.ptr, dims.as_mut_ptr(), 3))?;
            if dims[0] != 1 || dims[1] <= 0 || dims[2] <= 0 {
                return Err("PP-OCR 输出维度错误".to_owned());
            }
            let len = dims[1]
                .checked_mul(dims[2])
                .filter(|n| *n <= 10_000_000)
                .ok_or("PP-OCR 输出过大")? as usize;
            let mut data = null_mut();
            self.status(api!(
                api,
                51,
                unsafe extern "system" fn(Ptr, *mut Ptr) -> Ptr
            )(output.ptr, &mut data))?;
            let scores = std::slice::from_raw_parts(data.cast::<f32>(), len);
            // Blank-image checks may legitimately have low confidence.
            decode_ctc(scores, dims[1] as usize, dims[2] as usize)
        }
    }
}
