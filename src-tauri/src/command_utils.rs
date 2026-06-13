use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// 创建一个不显示控制台窗口的 Command（Windows 专用优化）
pub fn create_command(program: &str) -> Command {
    let mut command = Command::new(program);

    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW

    command
}
