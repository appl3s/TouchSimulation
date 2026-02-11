use crate::uinput_defs::*;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
use std::os::unix::fs::OpenOptionsExt;

// InputDevice struct with actual functionality
#[derive(Debug)]
pub struct InputDevice {
    pub name: String,
    pub path: String,
    pub slots: i32,
    pub touch_x_min: i32,
    pub touch_x_max: i32,
    pub touch_y_min: i32,
    pub touch_y_max: i32,
    pub has_touch_major: bool,
    pub has_touch_minor: bool,
    pub has_width_major: bool,
    pub has_width_minor: bool,
    pub has_orientation: bool,
    pub has_pressure: bool,
    pub file: Arc<Mutex<File>>,
}

impl Clone for InputDevice {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            path: self.path.clone(),
            slots: self.slots,
            touch_x_min: self.touch_x_min,
            touch_x_max: self.touch_x_max,
            touch_y_min: self.touch_y_min,
            touch_y_max: self.touch_y_max,
            has_touch_major: self.has_touch_major,
            has_touch_minor: self.has_touch_minor,
            has_width_major: self.has_width_major,
            has_width_minor: self.has_width_minor,
            has_orientation: self.has_orientation,
            has_pressure: self.has_pressure,
            file: self.file.clone(), // 保留真实设备的文件
        }
    }
}

impl InputDevice {
    pub fn grab(&mut self) -> std::io::Result<()> {
        println!("InputDevice::grab: grabbing device {}", self.path);
        let file = self.file.lock().unwrap();
        unsafe {
            let fd = file.as_raw_fd();
            let result = libc::ioctl(fd, eviocgrab() as libc::c_int, 1i32 as libc::c_ulong);
            if result == -1 {
                return Err(std::io::Error::last_os_error());
            } else {
                println!("InputDevice::grab: ioctl result = {}", result);
            }
        }
        println!("InputDevice::grab: successfully grabbed device");
        Ok(())
    }

    pub fn release(&mut self) -> std::io::Result<()> {
        println!("InputDevice::release: releasing device {}", self.path);
        let file = self.file.lock().unwrap();
        unsafe {
            let fd = file.as_raw_fd();
            let result = libc::ioctl(fd, eviocgrab() as libc::c_int, 0i32 as libc::c_ulong);
            if result == -1 {
                return Err(std::io::Error::last_os_error());
            } else {
                println!("InputDevice::release: ioctl result = {}", result);
            }
        }
        println!("InputDevice::release: successfully released device");
        Ok(())
    }

    pub fn write_event(&mut self, event_type: u16, code: u16, value: i32) -> std::io::Result<()> {
        use crate::uinput_defs::InputEvent;
        use nix::sys::time::TimeVal;
        use std::time::{SystemTime, UNIX_EPOCH};
        
        // ✅ 修复：使用真实时间戳，而不是零时间戳
        // 内核需要真实的时间戳来正确处理事件
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        
        let secs = now.as_secs() as i64;
        let usecs = now.subsec_micros() as i64;
        
        let event = InputEvent {
            time: TimeVal::new(secs, usecs),
            event_type,
            code,
            value,
        };
        
        let mut file = self.file.lock().unwrap();
        
        // 使用与Go版本完全一致的序列化方式
        let mut buffer = Vec::with_capacity(std::mem::size_of::<InputEvent>());
        
        // TimeVal: 必须严格按照Go的syscall.Timeval格式
        // Go的Timeval: {sec: i64, usec: i64} = 16字节
        let time_val = event.time;
        buffer.extend_from_slice(&(time_val.tv_sec()).to_le_bytes());  // i64
        buffer.extend_from_slice(&(time_val.tv_usec()).to_le_bytes()); // i64
        
        // Type: u16 = 2字节
        buffer.extend_from_slice(&event.event_type.to_le_bytes());
        
        // Code: u16 = 2字节
        buffer.extend_from_slice(&event.code.to_le_bytes());
        
        // Value: i32 = 4字节
        buffer.extend_from_slice(&event.value.to_le_bytes());
        
        // 确保总大小为24字节（与Go的InputEvent大小一致）
        let current_size = buffer.len();
        let target_size = std::mem::size_of::<InputEvent>();
        if current_size < target_size {
            buffer.resize(target_size, 0);
        }
        
        println!("write_event: device path={}, writing event type={}, code={}, value={} (buffer size: {})",
                 self.path, event_type, code, value, buffer.len());
        
        // 立即刷新，确保事件被内核及时处理
        let result = file.write_all(&buffer).and_then(|_| file.flush());
        if result.is_ok() {
            println!("write_event: successfully wrote and flushed event");
        } else {
            println!("write_event: failed to write event: {:?}", result);
        }
        result
    }
}

// Helper function for eviocgrab
fn eviocgrab() -> libc::c_ulong {
    const EV_IOC_MAGIC: u8 = b'E';
    const EV_IOC_NR_GRAB: u8 = 0x90;
    const IOC_WRITE: u32 = 0x1;
    const IOC_NR_BITS: u32 = 8;
    const IOC_TYPE_BITS: u32 = 8;
    const IOC_SIZE_BITS: u32 = 14;
    const IOC_NR_SHIFT: u32 = 0;
    const IOC_TYPE_SHIFT: u32 = IOC_NR_SHIFT + IOC_NR_BITS;
    const IOC_SIZE_SHIFT: u32 = IOC_TYPE_SHIFT + IOC_TYPE_BITS;
    const IOC_DIR_SHIFT: u32 = IOC_SIZE_SHIFT + IOC_SIZE_BITS;

    ((IOC_WRITE << IOC_DIR_SHIFT) | ((EV_IOC_MAGIC as u32) << IOC_TYPE_SHIFT) | ((EV_IOC_NR_GRAB as u32) << IOC_NR_SHIFT) | (4 << IOC_SIZE_SHIFT)) as libc::c_ulong
}

// Simplified function to get input devices - 不扫描，直接创建uinput设备
pub fn get_input_devices() -> Result<Vec<InputDevice>, Box<dyn std::error::Error>> {
    println!("get_input_devices: creating mock input device (no scanning)");
    
    // 创建一个模拟的触摸设备，就像Go实现中如果没有找到设备时的行为
    let mock_device = InputDevice {
        name: "MockTouchDevice".to_string(),
        path: "/dev/input/event0".to_string(),
        slots: 10,
        touch_x_min: 0,
        touch_x_max: 1080,
        touch_y_min: 0,
        touch_y_max: 2340,
        has_touch_major: true,
        has_touch_minor: true,
        has_width_major: true,
        has_width_minor: true,
        has_orientation: true,
        has_pressure: true,
        file: Arc::new(Mutex::new(File::open("/dev/null")?)),
    };
    
    println!("get_input_devices: created mock device");
    Ok(vec![mock_device])
}

// Function to create uinput device using proper Linux uinput interface - 参考Go实现
fn create_uinput_device(name: &str, is_type_b: bool) -> Result<File, Box<dyn std::error::Error>> {
    println!("create_uinput_device: creating {} uinput device (TypeB: {})", name, is_type_b);
    
    // Open uinput device - 参考Go实现使用O_WRONLY|O_NONBLOCK
    let mut device_file = OpenOptions::new()
        .read(false)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open("/dev/uinput")?;
    
    let fd = device_file.as_raw_fd();
    
    // Enable event types - 参考Go实现
    unsafe {
        // Enable EV_KEY - 参考Go实现
        let result = libc::ioctl(fd, UISETEVBIT() as libc::c_int, EV_KEY as libc::c_ulong);
        if result == -1 {
            return Err(Box::new(std::io::Error::last_os_error()));
        }
        
        // Enable BTN_TOUCH - 参考Go实现
        let result = libc::ioctl(fd, UISETKEYBIT() as libc::c_int, BTN_TOUCH as libc::c_ulong);
        if result == -1 {
            return Err(Box::new(std::io::Error::last_os_error()));
        }
        
        // Enable EV_ABS - 参考Go实现
        let result = libc::ioctl(fd, UISETEVBIT() as libc::c_int, EV_ABS as libc::c_ulong);
        if result == -1 {
            return Err(Box::new(std::io::Error::last_os_error()));
        }
        
        if is_type_b {
            // Type B设备配置 - 参考Go实现
            let result = libc::ioctl(fd, UISETABSBIT() as libc::c_int, ABS_MT_SLOT as libc::c_ulong);
            if result == -1 {
                return Err(Box::new(std::io::Error::last_os_error()));
            }
            let result = libc::ioctl(fd, UISETABSBIT() as libc::c_int, ABS_MT_POSITION_X as libc::c_ulong);
            if result == -1 {
                return Err(Box::new(std::io::Error::last_os_error()));
            }
            let result = libc::ioctl(fd, UISETABSBIT() as libc::c_int, ABS_MT_POSITION_Y as libc::c_ulong);
            if result == -1 {
                return Err(Box::new(std::io::Error::last_os_error()));
            }
            let result = libc::ioctl(fd, UISETABSBIT() as libc::c_int, ABS_MT_TRACKING_ID as libc::c_int);
            if result == -1 {
                return Err(Box::new(std::io::Error::last_os_error()));
            }
            let result = libc::ioctl(fd, UISETABSBIT() as libc::c_int, ABS_MT_TOUCH_MAJOR as libc::c_int);
            if result == -1 {
                return Err(Box::new(std::io::Error::last_os_error()));
            }
            let result = libc::ioctl(fd, UISETABSBIT() as libc::c_int, ABS_MT_TOUCH_MINOR as libc::c_int);
            if result == -1 {
                return Err(Box::new(std::io::Error::last_os_error()));
            }
            let result = libc::ioctl(fd, UISETABSBIT() as libc::c_int, ABS_MT_WIDTH_MAJOR as libc::c_int);
            if result == -1 {
                return Err(Box::new(std::io::Error::last_os_error()));
            }
            let result = libc::ioctl(fd, UISETABSBIT() as libc::c_int, ABS_MT_WIDTH_MINOR as libc::c_int);
            if result == -1 {
                return Err(Box::new(std::io::Error::last_os_error()));
            }
            let result = libc::ioctl(fd, UISETABSBIT() as libc::c_int, ABS_MT_ORIENTATION as libc::c_int);
            if result == -1 {
                return Err(Box::new(std::io::Error::last_os_error()));
            }
            let result = libc::ioctl(fd, UISETABSBIT() as libc::c_int, ABS_MT_PRESSURE as libc::c_int);
            if result == -1 {
                return Err(Box::new(std::io::Error::last_os_error()));
            }
        } else {
            // Type A设备配置 - 参考Go实现
            let result = libc::ioctl(fd, UISETABSBIT() as libc::c_int, ABS_MT_POSITION_X as libc::c_int);
            if result == -1 {
                return Err(Box::new(std::io::Error::last_os_error()));
            }
            let result = libc::ioctl(fd, UISETABSBIT() as libc::c_int, ABS_MT_POSITION_Y as libc::c_int);
            if result == -1 {
                return Err(Box::new(std::io::Error::last_os_error()));
            }
            let result = libc::ioctl(fd, UISETABSBIT() as libc::c_int, ABS_MT_TRACKING_ID as libc::c_int);
            if result == -1 {
                return Err(Box::new(std::io::Error::last_os_error()));
            }
        }
        
        // Enable INPUT_PROP_DIRECT - 参考Go实现
        let result = libc::ioctl(fd, UISETPROPBIT() as libc::c_int, INPUT_PROP_DIRECT as libc::c_int);
        if result == -1 {
            return Err(Box::new(std::io::Error::last_os_error()));
        }
        
        // 配置UinputUserDev - 参考Go实现
        println!("create_uinput_device: configuring UinputUserDev");
        
        // 创建ABS配置数组 - 参考Go实现
        let mut abs_mins = [0i32; ABS_CNT];
        let mut abs_maxs = [0i32; ABS_CNT];
        let abs_fuzz = [0i32; ABS_CNT];
        let abs_flat = [0i32; ABS_CNT];
        
        if is_type_b {
            // Type B的ABS配置 - 参考Go实现
            abs_mins[ABS_MT_SLOT as usize] = 0;
            abs_maxs[ABS_MT_SLOT as usize] = 9; // 10 slots
            abs_mins[ABS_MT_POSITION_X as usize] = 0;
            abs_maxs[ABS_MT_POSITION_X as usize] = 1080;
            abs_mins[ABS_MT_POSITION_Y as usize] = 0;
            abs_maxs[ABS_MT_POSITION_Y as usize] = 2340;
            abs_mins[ABS_MT_TRACKING_ID as usize] = 0;
            abs_maxs[ABS_MT_TRACKING_ID as usize] = 65535;
            abs_mins[ABS_MT_TOUCH_MAJOR as usize] = 0;
            abs_maxs[ABS_MT_TOUCH_MAJOR as usize] = 100;
            abs_mins[ABS_MT_TOUCH_MINOR as usize] = 0;
            abs_maxs[ABS_MT_TOUCH_MINOR as usize] = 100;
            abs_mins[ABS_MT_WIDTH_MAJOR as usize] = 0;
            abs_maxs[ABS_MT_WIDTH_MAJOR as usize] = 100;
            abs_mins[ABS_MT_WIDTH_MINOR as usize] = 0;
            abs_maxs[ABS_MT_WIDTH_MINOR as usize] = 100;
            abs_mins[ABS_MT_ORIENTATION as usize] = 0;
            abs_maxs[ABS_MT_ORIENTATION as usize] = 90;
            abs_mins[ABS_MT_PRESSURE as usize] = 0;
            abs_maxs[ABS_MT_PRESSURE as usize] = 255;
        } else {
            // Type A的ABS配置 - 参考Go实现
            abs_mins[ABS_MT_POSITION_X as usize] = 0;
            abs_maxs[ABS_MT_POSITION_X as usize] = 1080;
            abs_mins[ABS_MT_POSITION_Y as usize] = 0;
            abs_maxs[ABS_MT_POSITION_Y as usize] = 2340;
            abs_mins[ABS_MT_TRACKING_ID as usize] = 0;
            abs_maxs[ABS_MT_TRACKING_ID as usize] = 65535;
        }
        
        // 配置EV_SYN - 参考Go实现
        let result = libc::ioctl(fd, UISETEVBIT() as libc::c_int, EV_SYN as libc::c_int);
        if result == -1 {
            return Err(Box::new(std::io::Error::last_os_error()));
        }
        
        // 创建UinputUserDev结构体 - 参考Go实现
        let mut uidev = UinputUserDev {
            name: [0; UINPUT_MAX_NAME_SIZE],
            id: InputId {
                bus_type: 0x0018, // BUS_VIRTUAL - 参考Go实现
                vendor: 0x1234,
                product: 0x5678,
                version: 0x0100,
            },
            effects_max: 0, // 参考Go实现
            abs_max: abs_maxs,
            abs_min: abs_mins,
            abs_fuzz: abs_fuzz,
            abs_flat: abs_flat,
        };
        
        // 设置设备名称 - 参考Go实现
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len().min(UINPUT_MAX_NAME_SIZE - 1);
        uidev.name[..name_len].copy_from_slice(&name_bytes[..name_len]);
        
        // 写入UinputUserDev - 参考Go实现
        println!("create_uinput_device: writing UinputUserDev");
        let uidev_bytes = std::slice::from_raw_parts(&uidev as *const _ as *const u8, std::mem::size_of::<UinputUserDev>());
        device_file.write_all(uidev_bytes)?;
        
        // 创建输入设备 - 参考Go实现
        println!("create_uinput_device: creating input device");
        let result = libc::ioctl(fd, UIDEVCREATE() as libc::c_int);
        if result == -1 {
            return Err(Box::new(std::io::Error::last_os_error()));
        }
    }
    
    println!("create_uinput_device: successfully created uinput device");
    Ok(device_file)
}

// Helper functions for uinput ioctls - 参考Go实现
fn UISETEVBIT() -> libc::c_ulong {
    const UI_IOC_MAGIC: u8 = b'U';
    const UI_IOC_NR_SET_EV_BIT: u8 = 100;
    const IOC_WRITE: u32 = 0x1;
    const IOC_NR_BITS: u32 = 8;
    const IOC_TYPE_BITS: u32 = 8;
    const IOC_SIZE_BITS: u32 = 14;
    const IOC_NR_SHIFT: u32 = 0;
    const IOC_TYPE_SHIFT: u32 = IOC_NR_SHIFT + IOC_NR_BITS;
    const IOC_SIZE_SHIFT: u32 = IOC_TYPE_SHIFT + IOC_TYPE_BITS;
    const IOC_DIR_SHIFT: u32 = IOC_SIZE_SHIFT + IOC_SIZE_BITS;

    ((IOC_WRITE << IOC_DIR_SHIFT) | ((UI_IOC_MAGIC as u32) << IOC_TYPE_SHIFT) | ((UI_IOC_NR_SET_EV_BIT as u32) << IOC_NR_SHIFT) | (4 << IOC_SIZE_SHIFT)) as libc::c_ulong
}

fn UISETKEYBIT() -> libc::c_ulong {
    const UI_IOC_MAGIC: u8 = b'U';
    const UI_IOC_NR_SET_KEY_BIT: u8 = 101;
    const IOC_WRITE: u32 = 0x1;
    const IOC_NR_BITS: u32 = 8;
    const IOC_TYPE_BITS: u32 = 8;
    const IOC_SIZE_BITS: u32 = 14;
    const IOC_NR_SHIFT: u32 = 0;
    const IOC_TYPE_SHIFT: u32 = IOC_NR_SHIFT + IOC_NR_BITS;
    const IOC_SIZE_SHIFT: u32 = IOC_TYPE_SHIFT + IOC_TYPE_BITS;
    const IOC_DIR_SHIFT: u32 = IOC_SIZE_SHIFT + IOC_SIZE_BITS;

    ((IOC_WRITE << IOC_DIR_SHIFT) | ((UI_IOC_MAGIC as u32) << IOC_TYPE_SHIFT) | ((UI_IOC_NR_SET_KEY_BIT as u32) << IOC_NR_SHIFT) | (4 << IOC_SIZE_SHIFT)) as libc::c_ulong
}

fn UISETABSBIT() -> libc::c_ulong {
    const UI_IOC_MAGIC: u8 = b'U';
    const UI_IOC_NR_SET_ABS_BIT: u8 = 103;
    const IOC_WRITE: u32 = 0x1;
    const IOC_NR_BITS: u32 = 8;
    const IOC_TYPE_BITS: u32 = 8;
    const IOC_SIZE_BITS: u32 = 14;
    const IOC_NR_SHIFT: u32 = 0;
    const IOC_TYPE_SHIFT: u32 = IOC_NR_SHIFT + IOC_NR_BITS;
    const IOC_SIZE_SHIFT: u32 = IOC_TYPE_SHIFT + IOC_TYPE_BITS;
    const IOC_DIR_SHIFT: u32 = IOC_SIZE_SHIFT + IOC_SIZE_BITS;

    ((IOC_WRITE << IOC_DIR_SHIFT) | ((UI_IOC_MAGIC as u32) << IOC_TYPE_SHIFT) | ((UI_IOC_NR_SET_ABS_BIT as u32) << IOC_NR_SHIFT) | (4 << IOC_SIZE_SHIFT)) as libc::c_ulong
}

fn UISETPROPBIT() -> libc::c_ulong {
    const UI_IOC_MAGIC: u8 = b'U';
    const UI_IOC_NR_SET_PROP_BIT: u8 = 110;
    const IOC_WRITE: u32 = 0x1;
    const IOC_NR_BITS: u32 = 8;
    const IOC_TYPE_BITS: u32 = 8;
    const IOC_SIZE_BITS: u32 = 14;
    const IOC_NR_SHIFT: u32 = 0;
    const IOC_TYPE_SHIFT: u32 = IOC_NR_SHIFT + IOC_NR_BITS;
    const IOC_SIZE_SHIFT: u32 = IOC_TYPE_SHIFT + IOC_TYPE_BITS;
    const IOC_DIR_SHIFT: u32 = IOC_SIZE_SHIFT + IOC_SIZE_BITS;

    ((IOC_WRITE << IOC_DIR_SHIFT) | ((UI_IOC_MAGIC as u32) << IOC_TYPE_SHIFT) | ((UI_IOC_NR_SET_PROP_BIT as u32) << IOC_NR_SHIFT) | (4 << IOC_SIZE_SHIFT)) as libc::c_ulong
}

fn UIDEVCREATE() -> libc::c_ulong {
    const UI_IOC_MAGIC: u8 = b'U';
    const UI_IOC_NR_CREATE_DEV: u8 = 1;
    const IOC_NONE: u32 = 0x0;
    const IOC_NR_BITS: u32 = 8;
    const IOC_TYPE_BITS: u32 = 8;
    const IOC_SIZE_BITS: u32 = 14;
    const IOC_NR_SHIFT: u32 = 0;
    const IOC_TYPE_SHIFT: u32 = IOC_NR_SHIFT + IOC_NR_BITS;
    const IOC_SIZE_SHIFT: u32 = IOC_TYPE_SHIFT + IOC_TYPE_BITS;
    const IOC_DIR_SHIFT: u32 = IOC_SIZE_SHIFT + IOC_SIZE_BITS;

    ((IOC_NONE << IOC_DIR_SHIFT) | ((UI_IOC_MAGIC as u32) << IOC_TYPE_SHIFT) | ((UI_IOC_NR_CREATE_DEV as u32) << IOC_NR_SHIFT) | (0 << IOC_SIZE_SHIFT)) as libc::c_ulong
}

// Simplified function to create Type-B device - 直接创建，不扫描
pub fn new_type_b_dev_same(_input_dev: &InputDevice) -> Result<InputDevice, Box<dyn std::error::Error>> {
    println!("new_type_b_dev_same: creating Type B device");
    let uinput_file = create_uinput_device("TouchSimulation_B", true)?;
    
    Ok(InputDevice {
        name: "TouchSimulation_B".to_string(),
        path: "/dev/uinput".to_string(),
        slots: 10,
        touch_x_min: 0,
        touch_x_max: 1080,
        touch_y_min: 0,
        touch_y_max: 2340,
        has_touch_major: true,
        has_touch_minor: true,
        has_width_major: true,
        has_width_minor: true,
        has_orientation: true,
        has_pressure: true,
        file: Arc::new(Mutex::new(uinput_file)),
    })
}

// Simplified function to create Type-A device with same properties
pub fn new_type_a_dev_same(_input_dev: &InputDevice) -> Result<InputDevice, Box<dyn std::error::Error>> {
    println!("new_type_a_dev_same: creating Type A device");
    let uinput_file = create_uinput_device("TouchSimulation_A", false)?;
    
    Ok(InputDevice {
        name: "TouchSimulation_A".to_string(),
        path: "/dev/uinput".to_string(),
        slots: 10,
        touch_x_min: 0,
        touch_x_max: 1080,
        touch_y_min: 0,
        touch_y_max: 2340,
        has_touch_major: false,
        has_touch_minor: false,
        has_width_major: false,
        has_width_minor: false,
        has_orientation: false,
        has_pressure: false,
        file: Arc::new(Mutex::new(uinput_file)),
    })
}

// Simplified function to create Type-A device with random properties
pub fn new_type_a_dev_random(_input_dev: &InputDevice) -> Result<InputDevice, Box<dyn std::error::Error>> {
    println!("new_type_a_dev_random: creating Type A device with random properties");
    // 使用随机名称
    use crate::utils::rand_string_bytes;
    let random_name = rand_string_bytes(7);
    let uinput_file = create_uinput_device(&random_name, false)?;
    
    Ok(InputDevice {
        name: random_name,
        path: "/dev/uinput".to_string(),
        slots: 10,
        touch_x_min: 0,
        touch_x_max: 1080,
        touch_y_min: 0,
        touch_y_max: 2340,
        has_touch_major: false,
        has_touch_minor: false,
        has_width_major: false,
        has_width_minor: false,
        has_orientation: false,
        has_pressure: false,
        file: Arc::new(Mutex::new(uinput_file)),
    })
}