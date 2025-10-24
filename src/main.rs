use std::{env, process::Output};
use window::Window;

use crate::eww::var;
fn main() {
    let action = parse_args();
    if let Ok(server) = server::try_connect() {
        server::try_update(&server, action).unwrap();
        std::process::exit(0);
    } else {
        server::start_with_window(Window::new());
        action.run().unwrap();
    }
}

pub enum Action {
    Up,
    Down,
    MuteToggle,
}
impl Action {
    fn run(self) -> Result<Self, std::io::Error> {
        match self {
            Self::Up => {
                wpctl::vol_up()?;
                Ok(Self::Up)
            }
            Self::Down => {
                wpctl::vol_down()?;
                Ok(Self::Down)
            }

            Self::MuteToggle => {
                wpctl::mute_toggle()?;
                Ok(Self::MuteToggle)
            }
        }
    }
    fn to_bytes(&self) -> [u8; 1] {
        match self {
            Self::Up => [0b00000000],
            Self::Down => [0b11111111],
            Self::MuteToggle => [0b10101010],
        }
    }
    fn from_bytes(bytes: &[u8; 1]) -> Self {
        match bytes {
            [0] => Self::Up,
            [255] => Self::Down,
            _ => Self::MuteToggle,
        }
    }
}
fn parse_args() -> Action {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        panic!("no arguments provided");
    }
    match args[1].as_str() {
        "up" => Action::Up,
        "down" => Action::Down,
        "mute-toggle" => Action::MuteToggle,
        _ => panic!("unexpected argument: {}", args[1]),
    }
}
pub struct CachedVolume {
    pub level: f32,
    pub is_muted: bool,
}
impl CachedVolume {
    fn update_from(&mut self, action: Action) -> () {
        match action {
            Action::Up => {
                self.level += 2.0;
                self.is_muted = false;
            }
            Action::Down => self.level -= 2.0,
            Action::MuteToggle => self.toggle(),
        }
    }
    fn toggle(&mut self) {
        if self.is_muted {
            self.is_muted = false;
        } else {
            self.is_muted = true;
        }
    }
    fn to_bytes(&self) -> [u8; 5] {
        let mut bytes = [0u8; 5];
        bytes[0..4].copy_from_slice(&self.level.to_le_bytes());
        bytes[4] = self.is_muted as u8;
        bytes
    }
    fn from_bytes(bytes: &[u8; 5]) -> Self {
        Self {
            level: f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            is_muted: bytes[4] != 0,
        }
    }
}
impl From<wpctl::WpctlVolume> for CachedVolume {
    fn from(vol: wpctl::WpctlVolume) -> Self {
        Self {
            level: vol.level,
            is_muted: vol.is_muted,
        }
    }
}

mod wpctl {
    use std::{
        io::Error,
        process::{Command, Output},
    };

    use crate::Action;
    pub struct WpctlVolume {
        pub level: f32,
        pub is_muted: bool,
    }
    pub fn get_vol() -> WpctlVolume {
        let volume = String::from_utf8(
            Command::new("wpctl")
                .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        let mut level = 0.0;
        let mut is_muted = false;
        for slice in volume.split_whitespace() {
            match slice.parse::<f32>() {
                Ok(float) => level = float,
                Err(_) => {
                    if slice == "[MUTED]" {
                        is_muted = true
                    }
                }
            }
        }
        WpctlVolume { level, is_muted }
    }
    pub fn vol_up() -> Result<Output, std::io::Error> {
        let mut cmd1 = Command::new("wpctl");
        cmd1.args(["set-mute", "@DEFAULT_AUDIO_SINK@", "0"]);
        let mut cmd2 = Command::new("wpctl");
        cmd2.args(["set-volume", "@DEFAULT_AUDIO_SINK@", "0.02+", "-l", "1"]);
        cmd1.output()?;
        cmd2.output()
    }
    pub fn vol_down() -> Result<Output, std::io::Error> {
        let mut cmd1 = Command::new("wpctl");
        cmd1.args(["set-volume", "@DEFAULT_AUDIO_SINK@", "0.02-", "-l", "1"]);
        cmd1.output()
    }
    pub fn mute_toggle() -> Result<Output, std::io::Error> {
        let mut cmd1 = Command::new("wpctl");
        cmd1.args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]);
        cmd1.output()
    }
    pub fn match_command(action: Action) -> Result<Vec<Command>, &'static str> {
        match action {
            Action::Up => {
                let mut cmd1 = Command::new("wpctl");
                cmd1.args(["set-mute", "@DEFAULT_AUDIO_SINK@", "0"]);
                let mut cmd2 = Command::new("wpctl");
                cmd2.args(["set-volume", "@DEFAULT_AUDIO_SINK@", "0.02+", "-l", "1"]);
                Ok(vec![cmd1, cmd2])
            }
            Action::Down => {
                let mut cmd1 = Command::new("wpctl");
                cmd1.args(["set-volume", "@DEFAULT_AUDIO_SINK@", "0.02-", "-l", "1"]);
                Ok(vec![cmd1])
            }
            Action::MuteToggle => {
                let mut cmd1 = Command::new("wpctl");
                cmd1.args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]);
                Ok(vec![cmd1])
            }
            _ => Err("invalid arguments!"),
        }
    }
}
mod eww {
    use std::{fmt::Display, process::Command};
    pub mod var {
        pub const VOLUME_LEVEL: &'static str = "volume-level=";
        pub const ICON_PATH: &'static str = "volume-icon-path=";
        pub const COLOR: &'static str = "volcolor=";
    }
    pub fn update(var: &str, val: impl Display) {
        let arg = &format!("{var}{val}");
        Command::new("eww").args(["update", arg]).output().unwrap();
    }
    pub fn open_window(name: &str) {
        Command::new("eww").args(["open", name]).output().unwrap();
    }
    pub fn close_window(name: &str) {
        Command::new("eww").args(["close", name]).output().unwrap();
    }
}

mod resource {
    pub mod icon {
        pub const MUTE: &'static str = "/home/annie/.config/eww/resources/volume-mute.png";
        pub const LOW: &'static str = "/home/annie/.config/eww/resources/volume-low.png";
        pub const MID: &'static str = "/home/annie/.config/eww/resources/volume-mid.png";
        pub const HIGH: &'static str = "/home/annie/.config/eww/resources/volume-high.png";
    }
}

mod window {
    use crate::{CachedVolume, eww, wpctl};
    use std::{
        fmt::{Display, Formatter},
        time,
    };
    pub struct Window(time::Instant);
    impl Window {
        pub fn age(&self) -> time::Duration {
            self.0.elapsed()
        }
        pub fn reset(&mut self) {
            self.0 = time::Instant::now();
        }
        pub fn new() -> Self {
            eww::open_window("volume-float");
            Self(time::Instant::now())
        }
        pub fn kill(self) {
            eww::close_window("volume-float");
        }
    }
    pub fn update_icon(volume: &CachedVolume) {
        use crate::resource::icon;
        use eww::var;
        if volume.is_muted {
            eww::update(var::ICON_PATH, icon::MUTE);
        } else {
            match volume.level {
                level if level == 0.0 => eww::update(var::ICON_PATH, icon::MUTE),
                level if level < 0.33 => eww::update(var::ICON_PATH, icon::LOW),
                level if level < 0.66 => eww::update(var::ICON_PATH, icon::MID),
                _ => eww::update(var::ICON_PATH, icon::HIGH),
            }
        }
    }
}

mod server {
    use crate::*;
    use std::io::prelude::*;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::time::Duration;
    use std::{error::Error, fs};
    pub fn try_connect() -> Result<UnixStream, &'static str> {
        if let Ok(stream) = UnixStream::connect("/tmp/ewwvolume.sock") {
            return Ok(stream);
        } else {
            Err("server not found")
        }
    }
    pub fn try_update(mut stream: &UnixStream, action: Action) -> Result<(), Box<dyn Error>> {
        stream.write_all(&action.to_bytes())?;
        Ok(())
    }
    pub fn start_with_window(mut window: super::Window) {
        fs::remove_file("/tmp/ewwvolume.sock").ok();
        //set listener
        let mut volume: CachedVolume = wpctl::get_vol().into();
        let listener = UnixListener::bind("/tmp/ewwvolume.sock").unwrap();
        listener.set_nonblocking(true).unwrap();
        //listener loop
        loop {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0; 1];
                if let Ok(n) = stream.read(&mut buffer) {
                    let action = Action::from_bytes(&buffer);
                    volume.update_from(action.run().expect("wpctl call failed!"));
                    window::update_icon(&volume);
                    eww::update(eww::var::VOLUME_LEVEL, volume.level);
                    window.reset();
                }
            }
            if window.age() > Duration::from_millis(1000) {
                window.kill();
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
