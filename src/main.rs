
#![no_std] // don't link the Rust standard library
#![no_main] // disbale all Rust-level entry points
#![feature(custom_test_frameworks)]
#![test_runner(ferros::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use ferros::println;

// パニック時に呼ばれる関数
#[cfg(not(test))] // テスト時にはコンパイルしない. 
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { // function that never returns: `-> !` (diverging function)
    println!("{}", _info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    ferros::test_panic_handler(info)
}

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // this function is the entry point, since the linker looks for a funtion
    // named `_start` by default
    
    println!("Hello World{}", "!");
    
    // setup IDT
    ferros::init();

    // invoke a breakpoint exceptions
    x86_64::instructions::interrupts::int3();

    #[cfg(test)]
    test_main();

    println!("It did not crash!");

    loop {}
}


#[test_case]
fn trivial_assertion() {
    assert_eq!(0, 0);
}




