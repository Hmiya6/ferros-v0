
#![no_std] // don't link the Rust standard library
#![no_main] // disbale all Rust-level entry points

use core::panic::PanicInfo;

/// パニック時に呼ばれる関数
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { // function that never returns: `-> !` (diverging function)
    loop {}
}

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // this function is the entry point, since the linker looks for a funtion
    // named `_start` by default
    loop{}
}

