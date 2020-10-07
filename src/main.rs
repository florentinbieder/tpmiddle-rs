#[macro_use]
mod util;
mod input;
mod window;

use std::time::{Duration, Instant};

use anyhow::*;
use winapi::shared::minwindef::{DWORD, LPARAM, LRESULT, TRUE, UINT, WPARAM};
use winapi::shared::ntdef::NULL;
use winapi::shared::windef::HWND;
use winapi::um::processthreadsapi::{GetCurrentProcess, SetPriorityClass};
use winapi::um::winbase::HIGH_PRIORITY_CLASS;
use winapi::um::winuser::{
    DispatchMessageW, GetMessageW, TranslateMessage, HRAWINPUT, MOUSEEVENTF_HWHEEL,
    MOUSEEVENTF_WHEEL, MSG, WHEEL_DELTA, WM_INPUT, WM_NCCREATE,
};

use crate::input::{send_click, send_wheel, Event, Input, InputDevice, USAGE_PAGES};
use crate::window::{Devices, Window, WindowProc};

const MAX_MIDDLE_CLICK_DURATION: Duration = Duration::from_millis(50);

enum State {
    MiddleUp,
    MiddleDown { time: Instant },
    Scroll,
    XClicked,
}

struct TPMiddle {
    state: State,
}

impl TPMiddle {
    fn new() -> Result<Self> {
        Ok(TPMiddle {
            state: State::MiddleUp,
        })
    }
}

impl WindowProc for TPMiddle {
    fn proc(&mut self, u_msg: UINT, _w_param: WPARAM, l_param: LPARAM) -> LRESULT {
        if u_msg != WM_INPUT {
            return match u_msg {
                WM_NCCREATE => TRUE as LRESULT,
                _ => 0 as LRESULT,
            };
        }

        let input = if let Ok(input) = Input::from_raw_input(l_param as HRAWINPUT) {
            input
        } else {
            return 0;
        };

        match input.event {
            Event::ButtonDown => {
                self.state = State::MiddleDown {
                    time: Instant::now(),
                };
            }
            Event::ButtonUp => {
                if let State::MiddleDown { time } = self.state {
                    let now = Instant::now();
                    if now <= time + MAX_MIDDLE_CLICK_DURATION {
                        send_click(3);
                    }
                }
                self.state = State::MiddleUp;
            }
            Event::Vertical(dy) => {
                if let State::XClicked = self.state {
                    self.state = State::Scroll;
                }
                if input.device == InputDevice::USB {
                    send_wheel(MOUSEEVENTF_WHEEL, (dy as i32 * WHEEL_DELTA as i32) as DWORD);
                }
            }
            Event::Horizontal(dx) => {
                if let State::MiddleDown { .. } = self.state {
                    self.state = State::XClicked;
                    let button = if dx < 0 { 4 } else { 5 };
                    send_click(button);
                } else if let State::XClicked = self.state {
                    self.state = State::Scroll;
                    send_wheel(
                        MOUSEEVENTF_HWHEEL,
                        (dx as i32 * WHEEL_DELTA as i32) as DWORD,
                    );
                }
            }
        }

        0
    }
}

fn try_main() -> Result<WPARAM> {
    c_try!(SetPriorityClass(GetCurrentProcess(), HIGH_PRIORITY_CLASS));

    let app = TPMiddle::new()?;
    let window = Window::new(app)?;
    let _devices = Devices::new(&window, &USAGE_PAGES)?;
    let exit_code = unsafe {
        let mut message: MSG = Default::default();
        loop {
            let status = c_try_ne!(-1, GetMessageW(&mut message, NULL as HWND, 0, 0));
            if status == 0 {
                break message.wParam;
            }

            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    };

    Ok(exit_code)
}

fn main() -> Result<()> {
    let code = try_main()?;
    std::process::exit(code as i32);
}