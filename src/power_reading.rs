use std::{ffi::c_void, io};

const VOLTAGE_KEY: u32 = u32::from_be_bytes(*b"VD0R");
const CURRENT_KEY: u32 = u32::from_be_bytes(*b"ID0R");
const SYSTEM_TOTAL_KEY: u32 = u32::from_be_bytes(*b"PSTR");

#[derive(Clone, Copy)]
pub struct Readings {
    pub voltage: f32,
    pub current: f32,
    pub system_total: f32,
}

pub struct SmcConnection {
    connection: u32,
}

#[repr(C)]
struct SmcKeyData {
    key: u32,
    ignored: [u8; 24],
    size: u32,
    ignored2: [u8; 10],
    command: i8,
    ignored3: [u8; 5],
    value: f32,
    ignored4: [u8; 28],
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const i8) -> *mut c_void;
    fn IOServiceGetMatchingServices(
        master_port: u32,
        matching: *mut c_void,
        iterator: *mut u32,
    ) -> i32;
    fn IOIteratorNext(iterator: u32) -> u32;
    fn IOObjectRelease(object: u32) -> i32;
    fn IOServiceOpen(service: u32, owning_task: u32, options: u32, connection: *mut u32) -> i32;
    fn IOServiceClose(connection: u32) -> i32;
    fn IOConnectCallStructMethod(
        connection: u32,
        selector: u32,
        input: *const c_void,
        input_size: usize,
        output: *mut c_void,
        output_size: *mut usize,
    ) -> i32;
}

#[link(name = "System")]
unsafe extern "C" {
    static mach_task_self_: u32;
}

impl SmcConnection {
    pub fn open() -> Result<Self, io::Error> {
        let service_name = b"AppleSMC\0";
        let mut iterator = 0;
        let mut connection = 0;

        // SAFETY: These calls use the IOKit API with valid pointers and release
        // each returned IOKit object on every path.
        unsafe {
            let matching = IOServiceMatching(service_name.as_ptr().cast());
            if matching.is_null() || IOServiceGetMatchingServices(0, matching, &mut iterator) != 0 {
                return Err(io::Error::other("Unable to find AppleSMC"));
            }

            let service = IOIteratorNext(iterator);
            IOObjectRelease(iterator);
            if service == 0 {
                return Err(io::Error::other("AppleSMC service is unavailable"));
            }

            let result = IOServiceOpen(service, mach_task_self_, 0, &mut connection);
            IOObjectRelease(service);
            if result != 0 {
                return Err(io::Error::other("Unable to open AppleSMC"));
            }
        }

        Ok(Self { connection })
    }

    pub fn read_power(&self) -> Result<Readings, io::Error> {
        Ok(Readings {
            voltage: self.read_key(VOLTAGE_KEY)?,
            current: self.read_key(CURRENT_KEY)?,
            system_total: self.read_key(SYSTEM_TOTAL_KEY)?,
        })
    }

    fn read_key(&self, key: u32) -> Result<f32, io::Error> {
        let mut request: SmcKeyData = unsafe { std::mem::zeroed() };
        let mut response: SmcKeyData = unsafe { std::mem::zeroed() };
        let mut response_size = std::mem::size_of::<SmcKeyData>();
        request.key = key;
        request.size = 4;
        request.command = 5;

        // SAFETY: The request and response match the SMC structure expected by
        // selector 2. Both buffers remain valid for the duration of the call.
        let result = unsafe {
            IOConnectCallStructMethod(
                self.connection,
                2,
                (&request as *const SmcKeyData).cast(),
                std::mem::size_of::<SmcKeyData>(),
                (&mut response as *mut SmcKeyData).cast(),
                &mut response_size,
            )
        };
        if result != 0 {
            return Err(io::Error::other(format!(
                "Unable to read AppleSMC key {key:#010x}"
            )));
        }

        Ok(response.value)
    }
}

impl Drop for SmcConnection {
    fn drop(&mut self) {
        // SAFETY: The connection was successfully opened by this instance.
        unsafe {
            IOServiceClose(self.connection);
        }
    }
}
