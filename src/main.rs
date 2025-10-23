use std::env;
use window::Window;

use crate::eww::var;
fn main() {
    run_args();
    window::update_icon();
    if server::try_signal().is_err() {
        let window = Window::new();
        server::start_with(window);
    }
}

pub struct CachedVolume {
    pub level: f32,
    pub is_muted: bool,
}
impl From<wpctl::WpctlVolume> for CachedVolume {
    fn from(vol: wpctl::WpctlVolume) -> Self {
        Self {
            level: vol.level,
            is_muted: vol.is_muted,
        }
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
        let volume = wpctl::get_vol();
        eww::update(var::VOLUME_LEVEL, (wpctl::get_vol().level * 100.0) as u8);
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
    use crate::{eww, wpctl};
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
    pub fn update_icon() {
        use crate::resource::icon;
        use eww::var;
        let volume = wpctl::get_vol();
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

mod server {
    use crate::CachedVolume;
    use crate::wpctl::{self, WpctlVolume};
    use std::fs;
    use std::io::prelude::*;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::time::Duration;
    pub fn try_signal() -> Result<(), &'static str> {
        if let Ok(mut stream) = UnixStream::connect("/tmp/ewwvolume.sock") {
            stream.write_all(b"reset").ok();
            Ok(())
        } else {
            Err("server not found")
        }
    }
    pub fn start_with(mut window: super::Window) {
        fs::remove_file("/tmp/ewwvolume.sock").ok();
        //set listener
        let mut volume: CachedVolume = wpctl::get_vol().into();
        let listener = UnixListener::bind("/tmp/ewwvolume.sock").unwrap();
        listener.set_nonblocking(true).unwrap();
        //listener loop
        loop {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0; 64];
                if stream.read(&mut buffer).is_ok() {
                    println!("received reset signal");
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
