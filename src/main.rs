
#![no_std] // don't link the Rust standard library
#![no_main] // disbale all Rust-level entry points
mod vga_buffer;


use core::panic::PanicInfo;
use bootloader::{entry_point, BootInfo};

// パニック時に呼ばれる関数
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { // function that never returns: `-> !` (diverging function)
    loop {}
}

// static HELLO: &[u8] = b"Hello World!";

// bootloader 0.10~
entry_point!(kernel_main);
fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // turn the screen gray
//     if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
//         for byte in framebuffer.buffer_mut() {
//             *byte = 0x90;
//         }
//     }
    vga_buffer::print_something();
    loop {}
}

// bootloader < 0.9
// #[no_mangle] // don't mangle the name of this function
// pub extern "C" fn _start() -> ! {
//     // this function is the entry point, since the linker looks for a funtion
//     // named `_start` by default
//     
//     // cast integer `0xb8000` into raw pointer
//     let vga_buffer = 0xb8000 as *mut u8;
//     
//     // then, iterate over the bytes of the `HELLO`
//     for (i, &byte) in HELLO.iter().enumerate() {
//         unsafe {
//             // `offset` method to write the string byte and the corresponding color byte
//             *vga_buffer.offset(i as isize * 2) = byte;
//             *vga_buffer.offset(i as isize * 2 + 1) = 0xb; // light cyan
//         }
//     }
// 
// 
// 
//     loop {}
// }

