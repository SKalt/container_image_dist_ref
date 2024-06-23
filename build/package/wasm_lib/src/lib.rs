#![no_std]
use wee_alloc::WeeAlloc;

pub use container_image_dist_ref::*;

// FIXME: link external panic handler with well-known name from wasm runtime
#[cfg(target_family = "wasm")]
#[panic_handler]
fn handle_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub fn yeet(s: &str) -> u32 {
    let x = container_image_dist_ref::CanonicalImgRef::new(s).unwrap();
    x.name().to_str().len().try_into().unwrap()
}
