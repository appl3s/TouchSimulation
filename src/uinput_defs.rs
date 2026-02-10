use nix::sys::time::TimeVal;

//---------------------------------EVCodes--------------------------------------//

// Ref: input-event-codes.h
pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_ABS: u16 = 0x03;
pub const EV_FF: u16 = 0x15;
pub const BTN_TOUCH: u16 = 0x14a;
pub const SYN_REPORT: u16 = 0;
pub const SYN_MT_REPORT: u16 = 2;
pub const SYN_DROPPED: u16 = 3;
pub const ABS_MT_SLOT: u16 = 0x2f;
pub const ABS_MT_TOUCH_MAJOR: u16 = 0x30;
pub const ABS_MT_TOUCH_MINOR: u16 = 0x31;
pub const ABS_MT_WIDTH_MAJOR: u16 = 0x32;
pub const ABS_MT_WIDTH_MINOR: u16 = 0x33;
pub const ABS_MT_ORIENTATION: u16 = 0x34;
pub const ABS_MT_POSITION_X: u16 = 0x35;
pub const ABS_MT_POSITION_Y: u16 = 0x36;
pub const ABS_MT_TOOL_TYPE: u16 = 0x37;
pub const ABS_MT_BLOB_ID: u16 = 0x38;
pub const ABS_MT_TRACKING_ID: u16 = 0x39;
pub const ABS_MT_PRESSURE: u16 = 0x3a;
pub const ABS_MT_DISTANCE: u16 = 0x3b;
pub const ABS_MT_TOOL_X: u16 = 0x3c;
pub const ABS_MT_TOOL_Y: u16 = 0x3d;

// 真实设备中看到的额外ABS事件代码
pub const ABS_PROFILE: u16 = 0x15;  // 在真实设备序列中看到
pub const ABS_UNKNOWN_22: u16 = 0x22;  // 未知代码，在真实设备序列中看到
pub const ABS_UNKNOWN_23: u16 = 0x23;  // 未知代码，在真实设备序列中看到
pub const EV_MAX: u16 = 0x1f;
pub const EV_CNT: usize = EV_MAX as usize + 1;
pub const ABS_MAX: u16 = 0x3f;
pub const ABS_CNT: usize = ABS_MAX as usize + 1;
pub const KEY_MAX: u16 = 0x2ff;
pub const KEY_CNT: usize = KEY_MAX as usize + 1;
pub const INPUT_PROP_DIRECT: u16 = 0x01;
pub const INPUT_PROP_MAX: u16 = 0x1f;
pub const INPUT_PROP_CNT: usize = INPUT_PROP_MAX as usize + 1;

//---------------------------------IOCTL--------------------------------------//

// Ref: ioctl.h
const IOC_NONE: u32 = 0x0;
const IOC_WRITE: u32 = 0x1;
const IOC_READ: u32 = 0x2;

const IOC_NR_BITS: u32 = 8;
const IOC_TYPE_BITS: u32 = 8;
const IOC_SIZE_BITS: u32 = 14;
const IOC_NR_SHIFT: u32 = 0;

const IOC_TYPE_SHIFT: u32 = IOC_NR_SHIFT + IOC_NR_BITS;
const IOC_SIZE_SHIFT: u32 = IOC_TYPE_SHIFT + IOC_TYPE_BITS;
const IOC_DIR_SHIFT: u32 = IOC_SIZE_SHIFT + IOC_SIZE_BITS;

fn _ioc(dir: u32, t: u32, nr: u32, size: u32) -> u32 {
    (dir << IOC_DIR_SHIFT) | (t << IOC_TYPE_SHIFT) |
    (nr << IOC_NR_SHIFT) | (size << IOC_SIZE_SHIFT)
}

fn _ior(t: u32, nr: u32, size: u32) -> u32 {
    _ioc(IOC_READ, t, nr, size)
}

fn _iow(t: u32, nr: u32, size: u32) -> u32 {
    _ioc(IOC_WRITE, t, nr, size)
}

// Ref: input.h
pub fn eviocgversion() -> u32 {
    _ioc(IOC_READ, b'E' as u32, 0x01, 4) // sizeof(int)
}

pub fn eviocgid() -> u32 {
    _ioc(IOC_READ, b'E' as u32, 0x02, 8) // sizeof(struct input_id)
}

pub fn eviocgname() -> u32 {
    _ioc(IOC_READ, b'E' as u32, 0x06, UINPUT_MAX_NAME_SIZE as u32)
}

pub fn eviocgprop() -> u32 {
    _ioc(IOC_READ, b'E' as u32, 0x09, INPUT_PROP_CNT as u32)
}

pub fn eviocgabs(abs: u32) -> u32 {
    _ior(b'E' as u32, 0x40 + abs, 24) // sizeof(struct input_absinfo)
}

pub fn eviocgkey() -> u32 {
    _ioc(IOC_READ, b'E' as u32, 0x18, KEY_MAX as u32)
}

pub fn eviocgbit(ev: u32, len: u32) -> u32 {
    _ioc(IOC_READ, b'E' as u32, 0x20 + ev, len)
}

pub fn eviocgrab() -> u32 {
    _iow(b'E' as u32, 0x90, 4) // sizeof(int)
}

//---------------------------------Input--------------------------------------//

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InputId {
    pub bus_type: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AbsInfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub time: TimeVal,
    pub event_type: u16,
    pub code: u16,
    pub value: i32,
}

//---------------------------------UInput--------------------------------------//

// Ref: uinput.h
pub const UINPUT_MAX_NAME_SIZE: usize = 80;

#[repr(C)]
pub struct UinputUserDev {
    pub name: [u8; UINPUT_MAX_NAME_SIZE],
    pub id: InputId,
    pub effects_max: u32,
    pub abs_max: [i32; ABS_CNT],
    pub abs_min: [i32; ABS_CNT],
    pub abs_fuzz: [i32; ABS_CNT],
    pub abs_flat: [i32; ABS_CNT],
}

// Ref: uinput.h
pub fn uisetevbit() -> u32 {
    _iow(b'U' as u32, 100, 4) // sizeof(int)
}

pub fn uisetkeybit() -> u32 {
    _iow(b'U' as u32, 101, 4) // sizeof(int)
}

pub fn uisetabsbit() -> u32 {
    _iow(b'U' as u32, 103, 4) // sizeof(int)
}

pub fn uisetpropbit() -> u32 {
    _iow(b'U' as u32, 110, 4) // sizeof(int)
}

pub fn uidevcreate() -> u32 {
    _ioc(IOC_NONE, b'U' as u32, 1, 0)
}

pub fn uidevdestroy() -> u32 {
    _ioc(IOC_NONE, b'U' as u32, 2, 0)
}