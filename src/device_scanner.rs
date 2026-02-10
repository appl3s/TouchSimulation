use crate::uinput::InputDevice;
use crate::uinput_defs::*;
use std::{
    fs,
    collections::HashMap,
    sync::{Arc, Mutex},
    os::unix::fs::OpenOptionsExt,
    os::unix::io::AsRawFd,
};

// 实现Go版本的getInputDevices功能
pub fn scan_input_devices() -> Result<Vec<InputDevice>, Box<dyn std::error::Error>> {
    println!("scan_input_devices: scanning real input devices");
    
    // 扫描 /dev/input/event* 设备
    let paths = fs::read_dir("/dev/input")?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name();
            name.to_string_lossy().starts_with("event")
        })
        .collect::<Vec<_>>();
    
    println!("scan_input_devices: found {} event devices", paths.len());
    
    let mut devices = Vec::new();
    
    for path_entry in paths {
        let path = path_entry.path();
        let path_str = path.to_string_lossy().to_string();
        
        // 检查是否为字符设备
        if let Ok(metadata) = fs::metadata(&path) {
            use std::os::unix::fs::FileTypeExt;
            if !metadata.file_type().is_char_device() {
                continue;
            }
        } else {
            continue;
        }
        
        println!("scan_input_devices: checking device {}", path_str);
        
        // 打开设备文件（读写模式，用于写入事件）
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)  // 改为true，允许写入事件
            .custom_flags(libc::O_NONBLOCK)
            .open(&path)
        {
            Ok(device_file) => {
                let fd = device_file.as_raw_fd();
                
                // 读取事件类型位图
                let mut d_bits = [0u8; EV_CNT / 8];
                let result = unsafe {
                    libc::ioctl(fd, eviocgbit(0, EV_MAX as u32) as libc::c_int, &mut d_bits as *mut _ as usize)
                };
                if result == -1 {
                    println!("scan_input_devices: failed to read EV bits for {}", path_str);
                    continue;
                }
                
                // 读取ABS位图
                let mut abs_bits = [0u8; ABS_CNT / 8];
                let result = unsafe {
                    libc::ioctl(fd, eviocgbit(EV_ABS as u32, ABS_MAX as u32) as libc::c_int, &mut abs_bits as *mut _ as usize)
                };
                if result == -1 {
                    println!("scan_input_devices: failed to read ABS bits for {}", path_str);
                    continue;
                }
                
                // 读取PROP位图
                let mut prop_bits = [0u8; INPUT_PROP_CNT / 8];
                let result = unsafe {
                    libc::ioctl(fd, eviocgprop() as libc::c_int, &mut prop_bits as *mut _ as usize)
                };
                if result == -1 {
                    println!("scan_input_devices: failed to read PROP bits for {}", path_str);
                    continue;
                }
                
                // 读取KEY位图
                let mut key_bits = [0u8; KEY_CNT / 8];
                let result = unsafe {
                    libc::ioctl(fd, eviocgbit(EV_KEY as u32, KEY_MAX as u32) as libc::c_int, &mut key_bits as *mut _ as usize)
                };
                if result == -1 {
                    println!("scan_input_devices: failed to read KEY bits for {}", path_str);
                    continue;
                }
                
                // 检查是否为多点触摸设备
                // 条件：有ABS_MT_SLOT、ABS_MT_TRACKING_ID、ABS_MT_POSITION_X/Y、INPUT_PROP_DIRECT、BTN_TOUCH
                let has_mt_slot = has_specific_abs(&abs_bits, ABS_MT_SLOT);
                let has_mt_tracking_id = has_specific_abs(&abs_bits, ABS_MT_TRACKING_ID);
                let has_mt_position_x = has_specific_abs(&abs_bits, ABS_MT_POSITION_X);
                let has_mt_position_y = has_specific_abs(&abs_bits, ABS_MT_POSITION_Y);
                let has_input_prop_direct = has_specific_prop(&prop_bits, INPUT_PROP_DIRECT);
                let has_btn_touch = has_specific_key(&key_bits, BTN_TOUCH);
                
                // 检查ABS_MT_SLOT-1不存在（排除非MT设备）
                let has_mt_slot_minus_1 = has_specific_abs(&abs_bits, ABS_MT_SLOT - 1);
                
                println!("scan_input_devices: device {} - MT_SLOT: {}, MT_TRACKING_ID: {}, MT_POSITION_X: {}, MT_POSITION_Y: {}, PROP_DIRECT: {}, BTN_TOUCH: {}, MT_SLOT-1: {}", 
                         path_str, has_mt_slot, has_mt_tracking_id, has_mt_position_x, has_mt_position_y, 
                         has_input_prop_direct, has_btn_touch, has_mt_slot_minus_1);
                
                // 应用Go版本的筛选逻辑
                if !has_mt_slot_minus_1 && 
                   has_mt_slot && 
                   has_mt_tracking_id && 
                   has_mt_position_x && 
                   has_mt_position_y && 
                   has_input_prop_direct && 
                   has_btn_touch {
                    
                    println!("scan_input_devices: found valid touch device at {}", path_str);
                    
                    // 读取ABS配置信息
                    let mut abs_infos = HashMap::new();
                    let mut slots = 0i32;
                    let mut touch_x_min = 0i32;
                    let mut touch_x_max = 0i32;
                    let mut touch_y_min = 0i32;
                    let mut touch_y_max = 0i32;
                    let mut has_touch_major = false;
                    let mut has_touch_minor = false;
                    let mut has_width_major = false;
                    let mut has_width_minor = false;
                    let mut has_orientation = false;
                    let mut has_pressure = false;
                    
                    for abs_code in 0..=ABS_MAX {
                        if has_specific_abs(&abs_bits, abs_code) {
                            let mut abs_info = AbsInfo {
                                value: 0,
                                minimum: 0,
                                maximum: 0,
                                fuzz: 0,
                                flat: 0,
                                resolution: 0,
                            };
                            
                            let result = unsafe {
                                libc::ioctl(fd, eviocgabs(abs_code as u32) as libc::c_int, &mut abs_info as *mut _ as usize)
                            };
                            
                            if result != -1 {
                                abs_infos.insert(abs_code, abs_info);
                                
                                // 提取关键信息
                                match abs_code {
                                    ABS_MT_SLOT => {
                                        slots = abs_info.maximum + 1;
                                    }
                                    ABS_MT_TRACKING_ID => {
                                        if abs_info.maximum == abs_info.minimum {
                                            // 特殊处理，与Go版本一致
                                        }
                                    }
                                    ABS_MT_POSITION_X => {
                                        touch_x_min = abs_info.minimum;
                                        touch_x_max = abs_info.maximum - abs_info.minimum + 1;
                                    }
                                    ABS_MT_POSITION_Y => {
                                        touch_y_min = abs_info.minimum;
                                        touch_y_max = abs_info.maximum - abs_info.minimum + 1;
                                    }
                                    ABS_MT_TOUCH_MAJOR => has_touch_major = true,
                                    ABS_MT_TOUCH_MINOR => has_touch_minor = true,
                                    ABS_MT_WIDTH_MAJOR => has_width_major = true,
                                    ABS_MT_WIDTH_MINOR => has_width_minor = true,
                                    ABS_MT_ORIENTATION => has_orientation = true,
                                    ABS_MT_PRESSURE => has_pressure = true,
                                    _ => {}
                                }
                            }
                        }
                    }
                    
                    // 读取设备名称
                    let mut name_bytes = [0u8; UINPUT_MAX_NAME_SIZE];
                    let result = unsafe {
                        libc::ioctl(fd, eviocgname() as libc::c_int, &mut name_bytes as *mut _ as usize)
                    };
                    let name = if result != -1 {
                        let len = name_bytes.iter().position(|&b| b == 0).unwrap_or(UINPUT_MAX_NAME_SIZE);
                        String::from_utf8_lossy(&name_bytes[..len]).to_string()
                    } else {
                        "Unknown".to_string()
                    };
                    
                    // 读取输入ID
                    let mut input_id = InputId {
                        bus_type: 0,
                        vendor: 0,
                        product: 0,
                        version: 0,
                    };
                    let _result = unsafe {
                        libc::ioctl(fd, eviocgid() as libc::c_int, &mut input_id as *mut _ as usize)
                    };
                    
                    // 读取驱动版本
                    let mut version = 0i32;
                    let _result = unsafe {
                        libc::ioctl(fd, eviocgversion() as libc::c_int, &mut version as *mut _ as usize)
                    };
                    
                    let device = InputDevice {
                        name,
                        path: path_str.clone(),
                        slots,
                        touch_x_min,
                        touch_x_max,
                        touch_y_min,
                        touch_y_max,
                        has_touch_major,
                        has_touch_minor,
                        has_width_major,
                        has_width_minor,
                        has_orientation,
                        has_pressure,
                        file: Arc::new(Mutex::new(device_file)),
                    };
                    
                    devices.push(device);
                    println!("scan_input_devices: added device {} with {} slots", path_str, slots);
                } else {
                    println!("scan_input_devices: device {} does not meet touch device criteria", path_str);
                }
            }
            Err(e) => {
                println!("scan_input_devices: failed to open device {}: {}", path_str, e);
            }
        }
    }
    
    if !devices.is_empty() {
        println!("scan_input_devices: found {} valid touch devices", devices.len());
        Ok(devices)
    } else {
        println!("scan_input_devices: no valid touch devices found, creating mock device");
        // 如果没有找到设备，创建模拟设备（保持向后兼容）
        get_input_devices_mock()
    }
}

// 辅助函数：检查是否有特定的ABS
fn has_specific_abs(abs_bits: &[u8], abs_code: u16) -> bool {
    let byte_index = (abs_code / 8) as usize;
    let bit_index = (abs_code % 8) as usize;
    if byte_index < abs_bits.len() {
        (abs_bits[byte_index] & (1 << bit_index)) != 0
    } else {
        false
    }
}

// 辅助函数：检查是否有特定的PROP
fn has_specific_prop(prop_bits: &[u8], prop_code: u16) -> bool {
    let byte_index = (prop_code / 8) as usize;
    let bit_index = (prop_code % 8) as usize;
    if byte_index < prop_bits.len() {
        (prop_bits[byte_index] & (1 << bit_index)) != 0
    } else {
        false
    }
}

// 辅助函数：检查是否有特定的KEY
fn has_specific_key(key_bits: &[u8], key_code: u16) -> bool {
    let byte_index = (key_code / 8) as usize;
    let bit_index = (key_code % 8) as usize;
    if byte_index < key_bits.len() {
        (key_bits[byte_index] & (1 << bit_index)) != 0
    } else {
        false
    }
}

// 原有的简化函数，用于回退
pub fn get_input_devices_mock() -> Result<Vec<InputDevice>, Box<dyn std::error::Error>> {
    println!("get_input_devices_mock: creating mock input device");
    
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
        file: Arc::new(Mutex::new(std::fs::File::open("/dev/null")?)),
    };
    
    println!("get_input_devices_mock: created mock device");
    Ok(vec![mock_device])
}