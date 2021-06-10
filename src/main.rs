
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// This function is called on panic.
/// パニック時に呼ばれる関数
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { // function that never returns: `-> !` (diverging function)
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop{}
}

