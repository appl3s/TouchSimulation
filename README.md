# TouchSimulation - Rust版本

本项目是将Go实现的Android触摸模拟功能移植到Rust语言，提供跨平台的触摸事件模拟能力。

## 主要特性

- **Type A/B触摸协议支持**：支持Android多点触摸的两种协议
- **线程安全的事件分发**：使用Arc+Mutex实现线程间数据共享
- **字节序正确性**：明确使用LittleEndian字节序确保与Android系统兼容
- **详细调试日志**：提供完整的事件跟踪和调试信息
- **内存安全**：利用Rust的所有权系统避免内存泄漏和数据竞争

## 构建和部署

### 构建
```bash
make all
```

### 推送到设备
```bash
make push
```

### 运行
```bash
adb shell /data/local/tmp/touch_simulation
```

## 项目结构

```
rust/
├── src/
│   ├── main.rs          # 主程序入口
│   ├── touch_input.rs   # 触摸输入管理（核心逻辑）
│   ├── uinput.rs        # uinput设备管理
│   ├── uinput_defs.rs   # uinput常量定义
│   └── utils.rs         # 工具函数
├── Cargo.toml           # Rust项目配置
├── Makefile            # 构建脚本
└── README.md           # 项目文档
```

## 使用说明

程序启动后会：
1. 创建uinput触摸设备
2. 模拟触摸事件（点击、滑动等）
3. 提供交互式命令行界面

支持的命令：
- 自动执行预设的滑动操作
- 手动输入坐标进行触摸模拟
- `exit` 退出程序

## 技术亮点

### 内存安全
- 使用Rust的所有权系统避免悬垂指针
- 编译时保证内存安全，无需运行时检查

### 线程安全
- Arc（原子引用计数）实现线程间数据共享
- Mutex（互斥锁）确保数据访问的线程安全性

### 错误处理
- 使用Result类型进行错误处理
- 详细的错误日志帮助调试

### 性能优化
- 零成本抽象，无运行时开销
- 手动内存管理，避免GC停顿

## 调试功能

程序提供详细的调试日志，包括：
- 触摸坐标转换过程
- 事件发送序列
- 联系人状态变化
- uinput设备创建过程

## 兼容性

- **目标平台**：Android aarch64
- **最低API级别**：21（Android 5.0）
- **依赖**：Linux uinput子系统

## 注意事项

1. 需要root权限才能访问`/dev/uinput`
2. 某些设备可能需要关闭SELinux
3. 确保系统支持多点触摸协议

## 许可证

与原始项目保持一致（需查看原始Go项目的许可证）