
#![no_std] // don't link the Rust standard library
#![no_main] // disbale all Rust-level entry points
#![feature(custom_test_frameworks)]
#![test_runner(ferros::test_runner)]
#![reexport_test_harness_main = "test_main"]

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use ferros::{println};

// パニック時に呼ばれる関数
#[cfg(not(test))] // テスト時にはコンパイルしない. 
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { // function that never returns: `-> !` (diverging function)
    println!("{}", _info);
    ferros::hlt_loop(); 
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    ferros::test_panic_handler(info)
}

entry_point!(kernel_main);
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use ferros::memory::{self, BootInfoFrameAllocator};
    use x86_64::{structures::paging::Page, VirtAddr};

    println!("Hello World{}", "!");
    ferros::init(); // setup IDT

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut page_table = unsafe { memory::init(phys_mem_offset) };

    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };
    let page = Page::containing_address(VirtAddr::new(0)); // address `0` is unused. 
    memory::create_example_mapping(page, &mut page_table, &mut frame_allocator);

    let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
    // don't write to the start of the page because the top line of the VGA buffer is directly shifted off the screen by hte next `println`
    unsafe { page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e)} 

    // let addresses = [
    //     0xb8000, // VGA buffer page
    //     0x201008, // some code page
    //     0x0100_0020_1a10, // some stack page
    //     boot_info.physical_memory_offset, // virtual address mapped to physicall address 0
    // ];

    // for &address in &addresses {
    //     let virt = VirtAddr::new(address);
    //     let phys = page_table.translate_addr(virt);
    //     println!("{:?} -> {:?}", virt, phys);
    // }



    #[cfg(test)]
    test_main();

    println!("It did not crash!");
    ferros::hlt_loop(); 
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(0, 0);
}




