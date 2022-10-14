#[repr(u8)]
pub enum c_void {
  _x1,
}

extern "C" {
  pub fn close(fd: i32) -> i32;
}
