
#![no_std] // don't link the Rust standard library
#![no_main] // disbale all Rust-level entry points
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
mod vga_buffer;


use core::panic::PanicInfo;

// パニック時に呼ばれる関数
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { // function that never returns: `-> !` (diverging function)
    println!("{}", _info);
    loop {}
}


#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // this function is the entry point, since the linker looks for a funtion
    // named `_start` by default
    
    println!("Hello World{}", "!");

    #[cfg(test)]
    test_main();
    loop {}
}

#[test_case]
fn trivial_assertion() {
    print!("trivial asserttion... ");
    assert_eq!(1, 1);
    println!("[ok]");
}


#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}



