#![feature(lang_items)]
#![feature(core_intrinsics)]
#![feature(const_fn)]
#![feature(asm)]
#![feature(optin_builtin_traits)]
#![feature(decl_macro)]
#![feature(repr_align)]
#![feature(attr_literals)]
#![feature(never_type)]
#![feature(ptr_internals)]

extern crate pi;
extern crate stack_vec;

pub mod lang_items;
pub mod mutex;
pub mod console;
pub mod shell;

use pi::uart::MiniUart;
use shell::shell;
use console::{kprint, kprintln, CONSOLE};
use pi::gpio::Gpio;

use std::fmt::Write;

#[no_mangle]
pub extern "C" fn kmain() {
    //let mut uart = MiniUart::new();
    //uart.set_read_timeout(100000);
    kprintln!("OS,OS,OS");
    let mut ready_led = Gpio::new(16).into_output();
    ready_led.set();
    shell("->");
    //loop {
    //    let temp = uart.read_byte();
    //    uart.write_byte(temp);
    //    uart.write_str("<-");
    //}
}
