use crate::uinput::{get_input_devices, new_type_a_dev_random, new_type_a_dev_same, new_type_b_dev_same, InputDevice};
use crate::uinput_defs::*;
use std::{
    thread,
    time::Duration,
    sync::{Arc, Mutex, mpsc},
    os::unix::io::AsRawFd,
    sync::atomic::{AtomicI32, Ordering},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TypeMode {
    TypeA,
    TypeARnd,
    TypeB,
}

const FAKE_CONTACT: usize = 9;
static TRACKING_ID_COUNTER: AtomicI32 = AtomicI32::new(100);

#[derive(Debug, Clone)]
pub struct TouchContactA {
    pub pos_x: i32,
    pub pos_y: i32,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct TouchContactB {
    pub touch_major: i32,
    pub touch_minor: i32,
    pub width_major: i32,
    pub width_minor: i32,
    pub orientation: i32,
    pub position_x: i32,
    pub position_y: i32,
    pub tracking_id: i32,
    pub pressure: i32,
    pub active: bool,
}

impl Default for TouchContactA {
    fn default() -> Self {
        Self {
            pos_x: -1,
            pos_y: -1,
            active: false,
        }
    }
}

impl Default for TouchContactB {
    fn default() -> Self {
        Self {
            touch_major: -1,
            touch_minor: -1,
            width_major: -1,
            width_minor: -1,
            orientation: -1,
            position_x: -1,
            position_y: -1,
            tracking_id: -1,
            pressure: -1,
            active: false,
        }
    }
}

#[derive(Debug)]
pub struct TouchSimulation {
    curr_mode: TypeMode,
    touch_send: bool,
    touch_start: bool,
    display_width: i32,
    display_height: i32,
    fake_touch_major: i32,
    fake_touch_minor: i32,
    fake_width_major: i32,
    fake_width_minor: i32,
    fake_orientation: i32,
    fake_pressure: i32,
    touch_device: Option<Arc<Mutex<InputDevice>>>,
    uinput_device: Option<Arc<Mutex<InputDevice>>>,
    sync_channel: Option<mpsc::Sender<bool>>,
    stop_channel: Option<mpsc::Sender<bool>>,
    touch_contacts_a: Vec<TouchContactA>,
    touch_contacts_b: Vec<TouchContactB>,
    touch_contacts_b_arc: Option<Arc<Mutex<Vec<TouchContactB>>>>, // 用于线程间共享
}

impl TouchSimulation {
    pub fn new() -> Self {
        Self {
            curr_mode: TypeMode::TypeB,
            touch_send: false,
            touch_start: false,
            display_width: 0,
            display_height: 0,
            fake_touch_major: -1,
            fake_touch_minor: -1,
            fake_width_major: -1,
            fake_width_minor: -1,
            fake_orientation: -1,
            fake_pressure: -1,
            touch_device: None,
            uinput_device: None,
            sync_channel: None,
            stop_channel: None,
            touch_contacts_a: Vec::new(),
            touch_contacts_b: Vec::new(),
            touch_contacts_b_arc: None,
        }
    }

    pub fn new_with_device(input_device: InputDevice) -> Self {
        let sim = Self {
            curr_mode: TypeMode::TypeB,
            touch_send: false,
            touch_start: false,
            display_width: 0,
            display_height: 0,
            fake_touch_major: -1,
            fake_touch_minor: -1,
            fake_width_major: -1,
            fake_width_minor: -1,
            fake_orientation: -1,
            fake_pressure: -1,
            touch_device: Some(Arc::new(Mutex::new(input_device))),
            uinput_device: None,
            sync_channel: None,
            stop_channel: None,
            touch_contacts_a: Vec::new(),
            touch_contacts_b: Vec::new(),
            touch_contacts_b_arc: None,
        };
        sim
    }

    pub fn touch_input_setup(&mut self, mode: TypeMode, width: i32, height: i32) -> bool {
        println!("touch_input_setup: mode={:?}, width={}, height={}", mode, width, height);
        
        // 如果已经有设备信息，直接使用它
        if let Some(device_arc) = self.touch_device.clone() {
            let device_guard = device_arc.lock().unwrap();
            let device_clone = device_guard.clone();
            drop(device_guard); // 释放锁，避免借用冲突
            
            let result = self.touch_input_start(mode, width, height, device_clone);
            println!("touch_input_start result: {}", result);
            result
        } else {
            // 如果没有设备信息，尝试扫描获取
            match get_input_devices() {
                Ok(devices) => {
                    println!("Found {} input devices", devices.len());
                    if !devices.is_empty() {
                        let result = self.touch_input_start(mode, width, height, devices[0].clone());
                        println!("touch_input_start result: {}", result);
                        result
                    } else {
                        println!("No input devices found");
                        false
                    }
                }
                Err(e) => {
                    println!("Error getting input devices: {}", e);
                    false
                }
            }
        }
    }

    pub fn touch_input_start(&mut self, mode: TypeMode, width: i32, height: i32, in_dev: InputDevice) -> bool {
        if !self.touch_start {
            self.curr_mode = mode;

            // 保存设备路径供后续使用（如果需要的话）
            let _device_path = in_dev.path.clone();
            
            // Init Things
            self.touch_device = Some(Arc::new(Mutex::new(in_dev)));
            self.display_width = width;
            self.display_height = height;

            let (sync_sender, sync_receiver) = mpsc::channel();
            let (stop_sender, stop_receiver) = mpsc::channel();
            self.sync_channel = Some(sync_sender);
            self.stop_channel = Some(stop_sender);

            // 独占设备（grab）
            {
                let mut device = self.touch_device.as_ref().unwrap().lock().unwrap();
                if let Err(e) = device.grab() {
                    println!("touch_input_start: failed to grab device {}: {}", device.path, e);
                    return false;
                }
                println!("touch_input_start: successfully grabbed device {}", device.path);
            }

            if mode == TypeMode::TypeA || mode == TypeMode::TypeARnd {
                // 配置真实设备
                {
                    let mut device = self.touch_device.as_ref().unwrap().lock().unwrap();
                    if let Err(e) = configure_real_device(&mut device, mode) {
                        println!("touch_input_start: failed to configure real device {}: {}", device.path, e);
                        return false;
                    }
                    println!("touch_input_start: successfully configured real device {} for TypeA mode", device.path);
                }

                // 直接使用真实设备写入事件
                let real_device = self.touch_device.as_ref().unwrap().lock().unwrap().clone();
                println!("touch_input_start: using real device {} for TypeA mode", real_device.path);
                self.uinput_device = Some(Arc::new(Mutex::new(real_device)));

                // Set Default Values in Touch Contacts Array
                let device = self.touch_device.as_ref().unwrap().lock().unwrap();
                self.touch_contacts_a = vec![TouchContactA::default(); device.slots as usize];

                // Start event dispatcher thread
                let uinput_clone = Arc::clone(self.uinput_device.as_ref().unwrap());
                let contacts_clone = self.touch_contacts_a.clone();
                thread::spawn(move || {
                    event_dispatcher_a(uinput_clone, contacts_clone, sync_receiver, stop_receiver);
                });
            } else {
                // 配置真实设备
                {
                    let mut device = self.touch_device.as_ref().unwrap().lock().unwrap();
                    if let Err(e) = configure_real_device(&mut device, mode) {
                        println!("touch_input_start: failed to configure real device {}: {}", device.path, e);
                        return false;
                    }
                    println!("touch_input_start: successfully configured real device {} for TypeB mode", device.path);
                }

                // 直接使用真实设备写入事件
                let real_device = self.touch_device.as_ref().unwrap().lock().unwrap().clone();
                println!("touch_input_start: using real device {} for TypeB mode", real_device.path);
                self.uinput_device = Some(Arc::new(Mutex::new(real_device)));

                let device = self.touch_device.as_ref().unwrap().lock().unwrap();
                
                if device.has_touch_major {
                    self.fake_touch_major = (device.touch_x_max * 14) / 100;
                }
                if device.has_touch_minor {
                    self.fake_touch_minor = (device.touch_x_max * 10) / 100;
                }
                if device.has_width_major {
                    self.fake_width_major = (device.touch_x_max * 14) / 100;
                }
                if device.has_width_minor {
                    self.fake_width_minor = (device.touch_x_max * 10) / 100;
                }
                if device.has_orientation {
                    self.fake_orientation = 50; // Default orientation
                }
                if device.has_pressure {
                    self.fake_pressure = (100 * 35) / 100;
                }

                // Set Default Values in Touch Contacts Array
                let contacts_vec = vec![TouchContactB::default(); device.slots as usize];
                
                // 使用Arc+Mutex共享同一份数据，而不是克隆副本
                let contacts_arc = Arc::new(Mutex::new(contacts_vec));
                
                // Start event dispatcher thread
                let uinput_clone = Arc::clone(self.uinput_device.as_ref().unwrap());
                let contacts_arc_clone = Arc::clone(&contacts_arc);
                let fake_values = (
                    self.fake_touch_major,
                    self.fake_touch_minor,
                    self.fake_width_major,
                    self.fake_width_minor,
                    self.fake_orientation,
                    self.fake_pressure,
                );
                thread::spawn(move || {
                    event_dispatcher_b(uinput_clone, contacts_arc_clone, fake_values, sync_receiver, stop_receiver);
                });
                
                // 保存Arc引用以便主线程使用
                self.touch_contacts_b_arc = Some(contacts_arc);
            }

            self.touch_start = true;
        }
        true
    }

    pub fn touch_input_stop(&mut self) {
        if self.touch_start {
            if let Some(stop_sender) = &self.stop_channel {
                let _ = stop_sender.send(true);
            }

            if let Some(uinput_device) = &self.uinput_device {
                let mut device = uinput_device.lock().unwrap();
                let _ = device.release();
            }

            // 释放原始设备的grab
            if let Some(touch_device) = &self.touch_device {
                let mut device = touch_device.lock().unwrap();
                if let Err(e) = device.release() {
                    println!("touch_input_stop: failed to release device {}: {}", device.path, e);
                } else {
                    println!("touch_input_stop: successfully released device {}", device.path);
                }
            }

            self.uinput_device = None;
            self.touch_device = None;
            self.sync_channel = None;
            self.stop_channel = None;
            self.touch_contacts_a.clear();
            self.touch_contacts_b.clear();
            self.touch_contacts_b_arc = None;
            self.touch_start = false;
        }
    }

    pub fn send_touch_move(&mut self, x: i32, y: i32) {
        if !self.touch_start {
            return;
        }

        if !self.touch_send {
            self.touch_send = true;
        }

        let device = self.touch_device.as_ref().unwrap().lock().unwrap();
        let x = (x * device.touch_x_max / self.display_width) + device.touch_x_min;
        let y = (y * device.touch_y_max / self.display_height) + device.touch_y_min;
        
        println!("send_touch_move: converted coordinates: x={}, y={}", x, y);

        if self.curr_mode == TypeMode::TypeA || self.curr_mode == TypeMode::TypeARnd {
            println!("send_touch_move: updating TypeA contact {}", FAKE_CONTACT);
            self.touch_contacts_a[FAKE_CONTACT].pos_x = x;
            self.touch_contacts_a[FAKE_CONTACT].pos_y = y;
            self.touch_contacts_a[FAKE_CONTACT].active = true;
        } else {
            println!("send_touch_move: updating TypeB contact {}", FAKE_CONTACT);
            
            // 使用Arc共享数据
            if let Some(contacts_arc) = &self.touch_contacts_b_arc {
                let mut contacts = contacts_arc.lock().unwrap();
                let contact = &mut contacts[FAKE_CONTACT];
                
                if device.has_touch_major {
                    contact.touch_major = self.fake_touch_major;
                    println!("send_touch_move: set touch_major = {}", self.fake_touch_major);
                }
                if device.has_touch_minor {
                    contact.touch_minor = self.fake_touch_minor;
                    println!("send_touch_move: set touch_minor = {}", self.fake_touch_minor);
                }
                if device.has_width_major {
                    contact.width_major = self.fake_width_major;
                    println!("send_touch_move: set width_major = {}", self.fake_width_major);
                }
                if device.has_width_minor {
                    contact.width_minor = self.fake_width_minor;
                    println!("send_touch_move: set width_minor = {}", self.fake_width_minor);
                }
                if device.has_orientation {
                    contact.orientation = self.fake_orientation;
                    println!("send_touch_move: set orientation = {}", self.fake_orientation);
                }
                if device.has_pressure {
                    contact.pressure = self.fake_pressure;
                    println!("send_touch_move: set pressure = {}", self.fake_pressure);
                }
                if contact.tracking_id < 0 {
                    contact.tracking_id = device.slots - 2;
                    println!("send_touch_move: set tracking_id = {}", contact.tracking_id);
                }

                contact.position_x = x;
                contact.position_y = y;
                contact.active = true;
                println!("send_touch_move: TypeB contact updated - active: true, pos_x: {}, pos_y: {}", x, y);
            } else {
                println!("send_touch_move: ERROR - touch_contacts_b_arc is None!");
            }
        }

        if let Some(sync_sender) = &self.sync_channel {
            let _ = sync_sender.send(true);
        }

        thread::sleep(Duration::from_millis(15));
    }

    pub fn send_touch_up(&mut self) {
        println!("send_touch_up: touch_start={}, touch_send={}", self.touch_start, self.touch_send);
        if !self.touch_start || !self.touch_send {
            println!("send_touch_up: early return - touch_start={}, touch_send={}", self.touch_start, self.touch_send);
            return;
        }

        self.touch_send = false;

        if self.curr_mode == TypeMode::TypeA || self.curr_mode == TypeMode::TypeARnd {
            println!("send_touch_up: TypeA mode, deactivating contact {}", FAKE_CONTACT);
            self.touch_contacts_a[FAKE_CONTACT].pos_x = -1;
            self.touch_contacts_a[FAKE_CONTACT].pos_y = -1;
            self.touch_contacts_a[FAKE_CONTACT].active = false;
        } else {
            println!("send_touch_up: TypeB mode, deactivating contact {}", FAKE_CONTACT);
            
            if let Some(contacts_arc) = &self.touch_contacts_b_arc {
                let mut contacts = contacts_arc.lock().unwrap();
                let contact = &mut contacts[FAKE_CONTACT];
                let device = self.touch_device.as_ref().unwrap().lock().unwrap();
                
                if device.has_touch_major {
                    contact.touch_major = -1;
                }
                if device.has_touch_minor {
                    contact.touch_minor = -1;
                }
                if device.has_width_major {
                    contact.width_major = -1;
                }
                if device.has_width_minor {
                    contact.width_minor = -1;
                }
                if device.has_orientation {
                    contact.orientation = 0;
                }
                if device.has_pressure {
                    contact.pressure = 0;
                }

                contact.tracking_id = -1;
                contact.position_x = -1;
                contact.position_y = -1;
                contact.active = false;
            } else {
                println!("send_touch_up: ERROR - touch_contacts_b_arc is None!");
            }
        }

        if let Some(sync_sender) = &self.sync_channel {
            println!("send_touch_up: sending sync signal");
            let _ = sync_sender.send(true);
        }

        thread::sleep(Duration::from_millis(15));
    }
}

// Event dispatcher for Type A
fn event_dispatcher_a(
    uinput_device: Arc<Mutex<InputDevice>>,
    touch_contacts: Vec<TouchContactA>,
    sync_receiver: mpsc::Receiver<bool>,
    stop_receiver: mpsc::Receiver<bool>,
) {
    println!("event_dispatcher_a: started");
    let mut is_btn_down = false;

    loop {
        // Check for stop signal
        if stop_receiver.try_recv().is_ok() {
            println!("event_dispatcher_a: received stop signal");
            break;
        }

        // Check for sync signal
        if sync_receiver.try_recv().is_ok() {
            println!("event_dispatcher_a: received sync signal");
            let mut active_slots = 0;
            let mut uinput = uinput_device.lock().unwrap();

            for (idx, contact) in touch_contacts.iter().enumerate() {
                if contact.active && contact.pos_x > 0 && contact.pos_y > 0 {
                    println!("event_dispatcher_a: processing active contact {}", idx);
                    let _ = uinput.write_event(EV_ABS, ABS_MT_POSITION_X, contact.pos_x);
                    let _ = uinput.write_event(EV_ABS, ABS_MT_POSITION_Y, contact.pos_y);
                    let _ = uinput.write_event(EV_ABS, ABS_MT_TRACKING_ID, idx as i32);
                    let _ = uinput.write_event(EV_SYN, SYN_MT_REPORT, 0);

                    active_slots += 1;
                }
            }

            if active_slots == 0 && is_btn_down {
                println!("event_dispatcher_a: button up");
                is_btn_down = false;
                let _ = uinput.write_event(EV_SYN, SYN_MT_REPORT, 0);
                let _ = uinput.write_event(EV_KEY, BTN_TOUCH, 0);
            } else if active_slots > 0 && !is_btn_down {
                println!("event_dispatcher_a: button down");
                is_btn_down = true;
                let _ = uinput.write_event(EV_KEY, BTN_TOUCH, 1);
            }

            if active_slots == 0 && is_btn_down {
                println!("event_dispatcher_a: button up");
                is_btn_down = false;
                println!("event_dispatcher_a: writing BTN_TOUCH: 0");
                let _ = uinput.write_event(EV_KEY, BTN_TOUCH, 0);
            } else if active_slots > 0 && !is_btn_down {
                println!("event_dispatcher_a: button down");
                is_btn_down = true;
                println!("event_dispatcher_a: writing BTN_TOUCH: 1");
                let _ = uinput.write_event(EV_KEY, BTN_TOUCH, 1);
            }

            println!("event_dispatcher_a: sending SYN_REPORT");
            let _ = uinput.write_event(EV_SYN, SYN_REPORT, 0);
        }

        thread::sleep(Duration::from_millis(1));
    }
    println!("event_dispatcher_a: stopped");
}

// Event dispatcher for Type B
fn event_dispatcher_b(
    uinput_device: Arc<Mutex<InputDevice>>,
    contacts_arc: Arc<Mutex<Vec<TouchContactB>>>,
    fake_values: (i32, i32, i32, i32, i32, i32),
    sync_receiver: mpsc::Receiver<bool>,
    stop_receiver: mpsc::Receiver<bool>,
) {
    println!("event_dispatcher_b: started");
    let mut is_btn_down = false;
    let mut tracking_id_state = vec![-1i32; 10]; // 记录每个slot的tracking_id状态
    let (_touch_major, _touch_minor, _width_major, _width_minor, _orientation, _pressure) = fake_values;

    loop {
        // Check for stop signal
        if stop_receiver.try_recv().is_ok() {
            println!("event_dispatcher_b: received stop signal");
            break;
        }

        // Check for sync signal - 使用阻塞接收而不是try_recv
        match sync_receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(_) => {
                println!("event_dispatcher_b: received sync signal");
                let mut active_slots = 0;
                let mut uinput = uinput_device.lock().unwrap();
                println!("event_dispatcher_b: writing to device path: {}", uinput.path);
                
                // 从Arc获取共享数据
                let mut contacts_guard = contacts_arc.lock().unwrap();
                let contacts = &mut *contacts_guard;
                
                println!("event_dispatcher_b: checking {} contacts", contacts.len());
                
                // 统计当前活跃接触点数量
                for contact in contacts.iter() {
                    if contact.active && contact.tracking_id >= 0 {
                        active_slots += 1;
                    }
                }
                
                // 处理所有接触点
                for (idx, contact) in contacts.iter_mut().enumerate() {
                    println!("event_dispatcher_b: contact {} - active: {}, tracking_id: {}, pos_x: {}, pos_y: {}",
                             idx, contact.active, contact.tracking_id, contact.position_x, contact.position_y);
                    
                    if contact.active && contact.tracking_id >= 0 {
                        println!("event_dispatcher_b: processing active contact {}", idx);

                        // 检查是否是新的触摸（tracking_id变化）
                        let old_id = tracking_id_state[idx];
                        let is_new_touch = contact.tracking_id >= 0 && contact.tracking_id != old_id;
                        
                        if is_new_touch {
                            // 新触摸开始：先发送TRACKING_ID
                            println!("event_dispatcher_b: writing ABS_MT_TRACKING_ID: {}", contact.tracking_id);
                            let _ = uinput.write_event(EV_ABS, ABS_MT_TRACKING_ID as u16, contact.tracking_id);
                            
                            // 更新tracking_id记录
                            tracking_id_state[idx] = contact.tracking_id;
                            
                            // 第一个手指按下时发送按钮事件
                            if !is_btn_down {
                                println!("event_dispatcher_b: button down");
                                is_btn_down = true;
                                println!("event_dispatcher_b: writing BTN_TOUCH: 1");
                                let _ = uinput.write_event(EV_KEY, BTN_TOUCH as u16, 1);
                            }
                        }

                        // 发送SLOT（必须在TRACKING_ID之后）
                        println!("event_dispatcher_b: writing ABS_MT_SLOT: {}", idx as i32);
                        let _ = uinput.write_event(EV_ABS, ABS_MT_SLOT as u16, idx as i32);

                        // 发送其他属性（按照真实设备顺序）
                        if contact.touch_major >= 0 {
                            println!("event_dispatcher_b: writing ABS_MT_TOUCH_MAJOR: {}", contact.touch_major);
                            let _ = uinput.write_event(EV_ABS, ABS_MT_TOUCH_MAJOR as u16, contact.touch_major);
                        }
                        if contact.touch_minor >= 0 {
                            println!("event_dispatcher_b: writing ABS_MT_TOUCH_MINOR: {}", contact.touch_minor);
                            let _ = uinput.write_event(EV_ABS, ABS_MT_TOUCH_MINOR as u16, contact.touch_minor);
                        }
                        if contact.pressure >= 0 {
                            println!("event_dispatcher_b: writing ABS_MT_PRESSURE: {}", contact.pressure);
                            let _ = uinput.write_event(EV_ABS, ABS_MT_PRESSURE as u16, contact.pressure);
                        }

                        // 最后发送坐标
                        if contact.position_x >= 0 {
                            println!("event_dispatcher_b: writing ABS_MT_POSITION_X: {}", contact.position_x);
                            let _ = uinput.write_event(EV_ABS, ABS_MT_POSITION_X as u16, contact.position_x);
                        }
                        if contact.position_y >= 0 {
                            println!("event_dispatcher_b: writing ABS_MT_POSITION_Y: {}", contact.position_y);
                            let _ = uinput.write_event(EV_ABS, ABS_MT_POSITION_Y as u16, contact.position_y);
                        }
                    } else if !contact.active && contact.tracking_id >= 0 {
                        println!("event_dispatcher_b: deactivating contact {}", idx);
                        
                        // 发送SLOT
                        println!("event_dispatcher_b: writing ABS_MT_SLOT: {}", idx as i32);
                        let _ = uinput.write_event(EV_ABS, ABS_MT_SLOT as u16, idx as i32);
                        
                        // 发送TRACKING_ID -1（释放）
                        println!("event_dispatcher_b: writing ABS_MT_TRACKING_ID: -1");
                        let _ = uinput.write_event(EV_ABS, ABS_MT_TRACKING_ID as u16, -1);
                        
                        // 重置tracking_id记录
                        tracking_id_state[idx] = -1;
                        
                        contact.tracking_id = -1;
                    }
                }
                
                // 检查是否需要释放按钮
                if active_slots == 0 && is_btn_down {
                    println!("event_dispatcher_b: button up");
                    is_btn_down = false;
                    println!("event_dispatcher_b: writing BTN_TOUCH: 0");
                    let _ = uinput.write_event(EV_KEY, BTN_TOUCH as u16, 0);
                }

                // 按钮事件现在在每个手指的tracking_id变化时处理
                // 这里不再需要额外的按钮逻辑

                println!("event_dispatcher_b: sending SYN_REPORT");
                let _ = uinput.write_event(EV_SYN, SYN_REPORT as u16, 0);
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // 超时，继续循环
                thread::sleep(Duration::from_millis(1));
                continue;
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                println!("event_dispatcher_b: sync channel disconnected");
                break;
            },
        }
    }
    println!("event_dispatcher_b: stopped");
}

// 配置真实触摸设备，类似于虚拟设备的设置
fn configure_real_device(device: &mut InputDevice, mode: TypeMode) -> Result<(), Box<dyn std::error::Error>> {
    println!("configure_real_device: configuring {} for mode {:?}", device.path, mode);
    
    let file = device.file.lock().unwrap();
    let fd = (*file).as_raw_fd();
    
    // 获取当前设备能力信息（用于调试）
    unsafe {
        // 读取当前支持的事件类型
        let mut ev_bits = [0u8; EV_CNT / 8];
        let result = libc::ioctl(fd, eviocgbit(0, EV_CNT as u32) as libc::c_int, &mut ev_bits as *mut _ as usize);
        if result == -1 {
            println!("configure_real_device: warning - failed to read EV bits for {}", device.path);
        }
        
        // 检查是否支持必要的事件类型
        let has_ev_abs = has_specific_bit(&ev_bits, EV_ABS);
        let has_ev_key = has_specific_bit(&ev_bits, EV_KEY);
        
        println!("configure_real_device: device {} - EV_ABS: {}, EV_KEY: {}", device.path, has_ev_abs, has_ev_key);
        
        if !has_ev_abs || !has_ev_key {
            return Err(format!("device {} does not support required event types", device.path).into());
        }
        
        // 读取ABS位图，确认支持的触摸事件
        let mut abs_bits = [0u8; ABS_CNT / 8];
        let result = libc::ioctl(fd, eviocgbit(EV_ABS as u32, ABS_CNT as u32) as libc::c_int, &mut abs_bits as *mut _ as usize);
        if result == -1 {
            println!("configure_real_device: warning - failed to read ABS bits for {}", device.path);
        } else {
            // 验证必要的ABS事件
            let required_abs_events = [
                ABS_MT_SLOT,
                ABS_MT_TRACKING_ID,
                ABS_MT_POSITION_X,
                ABS_MT_POSITION_Y,
            ];
            
            let mut missing_events = Vec::new();
            for &abs_code in &required_abs_events {
                if !has_specific_bit(&abs_bits, abs_code) {
                    missing_events.push(abs_code);
                }
            }
            
            if !missing_events.is_empty() {
                println!("configure_real_device: device {} missing ABS events: {:?}", device.path, missing_events);
            }
        }
        
        // 读取KEY位图，确认支持的按键事件
        let mut key_bits = [0u8; KEY_CNT / 8];
        let result = libc::ioctl(fd, eviocgbit(EV_KEY as u32, KEY_CNT as u32) as libc::c_int, &mut key_bits as *mut _ as usize);
        if result == -1 {
            println!("configure_real_device: warning - failed to read KEY bits for {}", device.path);
        } else {
            if !has_specific_bit(&key_bits, BTN_TOUCH) {
                println!("configure_real_device: device {} does not support BTN_TOUCH", device.path);
            }
        }
        
        // 尝试设置ABS参数（类似于虚拟设备的配置）
        // 注意：真实设备可能不允许修改这些参数，但我们可以尝试
        if mode == TypeMode::TypeB {
            // Type B 设备配置
            if device.has_touch_major {
                if let Err(e) = set_abs_params(fd, ABS_MT_TOUCH_MAJOR, 0, 100, 0, 0) {
                    println!("configure_real_device: warning - failed to set ABS_MT_TOUCH_MAJOR params for {}: {}", device.path, e);
                }
            }
            
            if device.has_touch_minor {
                if let Err(e) = set_abs_params(fd, ABS_MT_TOUCH_MINOR, 0, 100, 0, 0) {
                    println!("configure_real_device: warning - failed to set ABS_MT_TOUCH_MINOR params for {}: {}", device.path, e);
                }
            }
            
            if device.has_width_major {
                if let Err(e) = set_abs_params(fd, ABS_MT_WIDTH_MAJOR, 0, 100, 0, 0) {
                    println!("configure_real_device: warning - failed to set ABS_MT_WIDTH_MAJOR params for {}: {}", device.path, e);
                }
            }
            
            if device.has_width_minor {
                if let Err(e) = set_abs_params(fd, ABS_MT_WIDTH_MINOR, 0, 100, 0, 0) {
                    println!("configure_real_device: warning - failed to set ABS_MT_WIDTH_MINOR params for {}: {}", device.path, e);
                }
            }
            
            if device.has_orientation {
                if let Err(e) = set_abs_params(fd, ABS_MT_ORIENTATION, 0, 90, 0, 0) {
                    println!("configure_real_device: warning - failed to set ABS_MT_ORIENTATION params for {}: {}", device.path, e);
                }
            }
            
            if device.has_pressure {
                if let Err(e) = set_abs_params(fd, ABS_MT_PRESSURE, 0, 255, 0, 0) {
                    println!("configure_real_device: warning - failed to set ABS_MT_PRESSURE params for {}: {}", device.path, e);
                }
            }
        }
        
        // 设置设备属性（如果支持）
        if let Err(e) = set_device_property(fd, INPUT_PROP_DIRECT) {
            println!("configure_real_device: warning - failed to set INPUT_PROP_DIRECT for {}: {}", device.path, e);
        }
    }
    
    println!("configure_real_device: successfully configured {}", device.path);
    Ok(())
}

// 辅助函数：检查位图中特定位是否设置
fn has_specific_bit(bits: &[u8], bit_index: u16) -> bool {
    let byte_index = (bit_index / 8) as usize;
    let bit_offset = (bit_index % 8) as usize;
    if byte_index < bits.len() {
        (bits[byte_index] & (1 << bit_offset)) != 0
    } else {
        false
    }
}

// 辅助函数：设置ABS参数范围
fn set_abs_params(fd: libc::c_int, abs_code: u16, min: i32, max: i32, fuzz: i32, flat: i32) -> Result<(), Box<dyn std::error::Error>> {
    use crate::uinput_defs::AbsInfo;
    
    let abs_info = AbsInfo {
        value: 0,
        minimum: min,
        maximum: max,
        fuzz: fuzz,
        flat: flat,
        resolution: 0,
    };
    
    let result = unsafe {
        libc::ioctl(fd, eviocgabs(abs_code as u32) as libc::c_int, &abs_info as *const _ as usize)
    };
    
    if result == -1 {
        return Err(std::io::Error::last_os_error().into());
    }
    
    println!("set_abs_params: set ABS_{} params to min={}, max={}, fuzz={}, flat={}",
             abs_code, min, max, fuzz, flat);
    Ok(())
}

// 辅助函数：设置设备属性
fn set_device_property(fd: libc::c_int, prop: u16) -> Result<(), Box<dyn std::error::Error>> {
    let result = unsafe {
        libc::ioctl(fd, eviocgprop() as libc::c_int, &prop as *const _ as usize)
    };
    
    if result == -1 {
        return Err(std::io::Error::last_os_error().into());
    }
    
    println!("set_device_property: set property {}", prop);
    Ok(())
}