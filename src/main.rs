use std::env;
use window::Window;

use crate::eww::var;
fn main() {
    run_args();
    if let Ok(server) = server::try_connect() {
        server::try_update(server, volume)
    } else {
        let window = Window::new();
        server::start_with_window(window);
    }
}

fn run_args() {
    let mut args = env::args();
    if args.len() > 1 {
        match &*args.nth(1).unwrap() {
            "up" => wpctl::plus_two(),
            "down" => wpctl::minus_two(),
            "mute-toggle" => wpctl::mute_toggle(),
            _ => panic!("unexpected argument"),
        }
        eww::update(var::VOLUME_LEVEL, (wpctl::get_vol().level * 100.0) as u8);
    }
}

pub struct CachedVolume {
    pub level: f32,
    pub is_muted: bool,
}
impl CachedVolume {
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
    use std::process::Command;
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
    pub fn plus_two() {
        Command::new("wpctl")
            .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "0"])
            .output()
            .unwrap();
        Command::new("wpctl")
            .args(["set-volume", "@DEFAULT_AUDIO_SINK@", "0.02+", "-l", "1"])
            .output()
            .unwrap();
    }
    pub fn minus_two() {
        Command::new("wpctl")
            .args(["set-volume", "@DEFAULT_AUDIO_SINK@", "0.02-", "-l", "1"])
            .output()
            .unwrap();
    }
    pub fn mute_toggle() {
        Command::new("wpctl")
            .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
            .output()
            .unwrap();
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
        pub const MUTE: &'static str = "$HOME/.config/eww/resources/volume-mute.png";
        pub const LOW: &'static str = "$HOME/.config/eww/resources/volume-low.png";
        pub const MID: &'static str = "$HOME/.config/eww/resources/volume-mid.png";
        pub const HIGH: &'static str = "$HOME/.config/eww/resources/volume-high.png";
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
    pub fn try_update(
        mut stream: &UnixStream,
        volume: &CachedVolume,
    ) -> Result<(), Box<dyn Error>> {
        stream.write_all(&volume.to_bytes())?;
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
                let mut buffer = [0; 5];
                if let Ok(n) = stream.read(&mut buffer) {
                    volume = CachedVolume::from_bytes(&buffer);
                    window::update_icon(&volume);
                    eww::update(eww::var::VOLUME_LEVEL, volume.level);
                    window.reset();
                }
            }
            if window.age() > Duration::from_millis(1000) {
                window.kill();
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
