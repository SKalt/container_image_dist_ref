#![no_std]

use container_image_dist_ref;

#[cfg(target_family = "wasm")]
#[panic_handler]
fn handle_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
// FIXME: link external panic handler with well-known name from wasm runtime

#[no_mangle]
pub fn yeet(s: &str) -> u32 {
    let x = container_image_dist_ref::CanonicalImgRef::new(s).unwrap();
    x.name().to_str().len().try_into().unwrap()
}
