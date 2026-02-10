mod uinput_defs;
mod uinput;
mod touch_input;
mod utils;
mod device_scanner;

use touch_input::{TouchSimulation, TypeMode};
use std::{
    io::{self, Write},
    thread,
    time::Duration,
};

const X: i32 = 746;
const Y: i32 = 1064;
const NX: i32 = 400;
const NY: i32 = 1408;

fn gen_move_points(sim: &mut TouchSimulation, start_x: i32, start_y: i32, end_x: i32, end_y: i32) {
    let min_point_count = 2;
    let max_move_distance = 10;

    let d_x = (end_x - start_x) as f32;
    let d_y = (end_y - start_y) as f32;

    let x_count = ((d_x.abs() / max_move_distance as f32) as i32).abs();
    let y_count = ((d_y.abs() / max_move_distance as f32) as i32).abs();
    let mut count = x_count.max(y_count);
    count = count.max(min_point_count);

    let act_delta_x = d_x / count as f32;
    let act_delta_y = d_y / count as f32;

    for i in 0..count {
        let x = (start_x as f32 + act_delta_x * i as f32) as i32;
        let y = (start_y as f32 + act_delta_y * i as f32) as i32;
        sim.send_touch_move(x, y);
    }
}

fn swipe(sim: &mut TouchSimulation, start_x: i32, start_y: i32, end_x: i32, end_y: i32) {
    sim.send_touch_move(start_x, start_y);
    gen_move_points(sim, start_x, start_y, end_x, end_y);
    sim.send_touch_move(end_x, end_y);
    sim.send_touch_up();
}

fn select_device(devices: &Vec<uinput::InputDevice>) -> Option<usize> {
    println!("Found {} input devices:", devices.len());
    for (i, device) in devices.iter().enumerate() {
        println!("{}: {} (path: {}, slots: {}, resolution: {}x{})", 
                 i, device.name, device.path, device.slots, 
                 device.touch_x_max - device.touch_x_min, 
                 device.touch_y_max - device.touch_y_min);
    }
    
    print!("Select device (0-{}), or press Enter to use default (0): ", devices.len() - 1);
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    
    let input = input.trim();
    if input.is_empty() {
        return Some(0);
    }
    
    match input.parse::<usize>() {
        Ok(index) if index < devices.len() => Some(index),
        _ => {
            println!("Invalid selection. Using default device 0.");
            Some(0)
        }
    }
}

fn main() {
    println!("Touch Simulation Rust Version - Starting...");
    
    // 扫描输入设备
    println!("Scanning for input devices...");
    let devices = match device_scanner::scan_input_devices() {
        Ok(devices) => devices,
        Err(e) => {
            eprintln!("Failed to scan input devices: {}", e);
            return;
        }
    };
    
    if devices.is_empty() {
        eprintln!("No input devices found!");
        return;
    }
    
    // 选择设备
    let selected_index = match select_device(&devices) {
        Some(index) => index,
        None => {
            println!("No device selected. Exiting.");
            return;
        }
    };
    
    let selected_device = &devices[selected_index];
    println!("Selected device: {} at {}", selected_device.name, selected_device.path);
    
    // Using Common Display Resolution, 2340x1080
    let mut sim = TouchSimulation::new_with_device(selected_device.clone());
    
    println!("Setting up touch input device...");
    if !sim.touch_input_setup(TypeMode::TypeB, 1440, 3216) {
        eprintln!("Failed to setup touch device!");
        return;
    }
    println!("Touch input device setup successful!");

    println!("Starting touch simulation in 3 seconds...");
    thread::sleep(Duration::from_secs(3));

    println!("Executing swipe 1: ({}, {}) -> ({}, {})", X, Y, X, NY);
    swipe(&mut sim, X, Y, X, NY);

    println!("Executing swipe 2: ({}, {}) -> ({}, {})", NX, Y, X, NY);
    thread::sleep(Duration::from_secs(3));
    swipe(&mut sim, NX, Y, X, NY);

    println!("Executing swipe 3: ({}, {}) -> ({}, {})", X, NY, X, Y);
    thread::sleep(Duration::from_secs(3));
    swipe(&mut sim, X, NY, X, Y);

    println!("Executing swipe 4: ({}, {}) -> ({}, {})", X, NY, NX, Y);
    thread::sleep(Duration::from_secs(3));
    swipe(&mut sim, X, NY, NX, Y);

    println!("All swipes completed. Enter 'exit' to quit.");
    loop {
        print!("Enter 'exit' to quit: ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        
        let input = input.trim().to_lowercase();
        if input == "exit" {
            println!("Stopping touch simulation...");
            sim.touch_input_stop();
            println!("Touch simulation stopped.");
            break;
        }
    }
}