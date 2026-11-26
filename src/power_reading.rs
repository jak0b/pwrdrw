use std::{
    ffi::{CStr, c_char, c_void},
    io,
};

const VOLTAGE_KEY: u32 = u32::from_be_bytes(*b"VD0R");
const CURRENT_KEY: u32 = u32::from_be_bytes(*b"ID0R");
const SYSTEM_TOTAL_KEY: u32 = u32::from_be_bytes(*b"PSTR");
const BATTERY_PERCENTAGE_KEY: u32 = u32::from_be_bytes(*b"BUIC");
const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

#[derive(Clone, Copy)]
pub struct Readings {
    pub voltage: f32,
    pub current: f32,
    pub system_total: f32,
    pub on_ac_power: bool,
    pub battery_percentage: Option<f32>,
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
    value: [u8; 4],
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
    fn IOPSCopyPowerSourcesInfo() -> *const c_void;
    fn IOPSGetProvidingPowerSourceType(power_sources: *const c_void) -> *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: *const c_void);
    fn CFStringGetCString(
        value: *const c_void,
        buffer: *mut c_char,
        buffer_size: isize,
        encoding: u32,
    ) -> bool;
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
            on_ac_power: Self::on_ac_power()?,
            battery_percentage: self.battery_percentage(),
        })
    }

    fn battery_percentage(&self) -> Option<f32> {
        // BUIC is the SMC's direct battery charge percentage.
        self.read_key_u8(BATTERY_PERCENTAGE_KEY)
            .ok()
            .map(|percentage| f32::from(percentage.min(100)))
    }

    fn on_ac_power() -> Result<bool, io::Error> {
        // SAFETY: The Power Sources API returns a retained object that is
        // released below. Its power-source type is a borrowed CFString.
        unsafe {
            let power_sources = IOPSCopyPowerSourcesInfo();
            if power_sources.is_null() {
                return Err(io::Error::other(
                    "Unable to read macOS power source information",
                ));
            }

            let power_source_type = IOPSGetProvidingPowerSourceType(power_sources);
            let result = if power_source_type.is_null() {
                Err(io::Error::other(
                    "Unable to determine the macOS power source type",
                ))
            } else {
                let mut buffer = [0 as c_char; 32];
                if !CFStringGetCString(
                    power_source_type,
                    buffer.as_mut_ptr(),
                    buffer.len() as isize,
                    K_CF_STRING_ENCODING_UTF8,
                ) {
                    Err(io::Error::other(
                        "Unable to read the macOS power source type",
                    ))
                } else {
                    let power_source_type = CStr::from_ptr(buffer.as_ptr());
                    Ok(power_source_type.to_bytes() == b"AC Power")
                }
            };

            CFRelease(power_sources);
            result
        }
    }

    fn read_key(&self, key: u32) -> Result<f32, io::Error> {
        let value = self.read_key_bytes(key)?;
        Ok(f32::from_ne_bytes(value))
    }

    fn read_key_u8(&self, key: u32) -> Result<u8, io::Error> {
        let value = self.read_key_bytes(key)?;
        Ok(value[0])
    }

    fn read_key_bytes(&self, key: u32) -> Result<[u8; 4], io::Error> {
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
